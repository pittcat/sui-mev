// 该文件 `aftermath.rs` 实现了与 Aftermath Finance 协议（一个Sui区块链上的去中心化交易所 DEX）交互的逻辑。
// Aftermath 可能是一个具有多代币池和加权池的协议，类似于Balancer。
//
// 文件概览:
// 1. 定义了与 Aftermath 协议相关的常量，如合约包ID (package ID)、各种对象ID (池注册表、手续费库等)。
// 2. `ObjectArgs` 结构体: 用于缓存一些常用的 Aftermath 对象参数，避免重复查询。
// 3. `Aftermath` 结构体: 代表一个 Aftermath 交易池的实例，包含了与该池交互所需的所有信息和方法。
//    它实现了 `Dex` trait，表明它是一个符合通用DEX接口的实现。
// 4. `new()` 方法: 用于根据链上数据（通过 `dex_indexer::types::Pool` 提供）初始化一个或多个 `Aftermath` 实例。
// 5. `build_swap_tx()` / `build_swap_args()`: 构建在 Aftermath 上执行精确输入交换 (swap exact in) 的交易参数和可编程交易块 (PTB)。
// 6. `expect_amount_out()`: 根据池的状态和输入金额，计算预期的输出金额。这涉及到 Aftermath 的定价公式。
// 7. 实现了 `Dex` trait 的方法，如 `extend_trade_tx` (将交换操作追加到现有PTB), `swap_tx` (构建完整的交换交易),
//    `coin_in_type`, `coin_out_type`, `protocol`, `liquidity`, `object_id` 等。
// 8. 包含一些数学辅助函数，如 `calculate_expected_out`, `calc_spot_price_fixed_with_fees` 等，
//    这些函数用于处理 Aftermath 池的特定数学运算，特别是涉及固定点数 (fixed-point arithmetic) 和费用计算的部分。
//    Aftermath 使用U256类型和10^18作为固定点数的小数位数基准 (ONE)。
//
// Sui/DeFi概念:
// - Package ID: Sui上智能合约（Move包）的唯一标识符。
// - Object ID: Sui上每个对象（如交易池、代币库）的唯一标识符。
// - ObjectArg: 在构建Sui可编程交易块 (PTB) 时，用于引用链上对象（共享对象或拥有对象）的参数类型。
// - Programmable Transaction Block (PTB): Sui的一种高级交易构建方式，允许将多个操作原子地组合在一个交易中。
// - TypeTag: 代表Sui上的类型，例如代币类型 "0x2::sui::SUI"。
// - Liquidity Pool: DEX的核心，用户存入代币对以提供流动性，交易者则与这些池子进行代币交换。
// - Weighted Pool: 一种流动性池，其中池内不同代币的权重可以不同，影响价格发现和滑点。Aftermath似乎使用了加权池。
// - Fixed-Point Arithmetic: 一种用整数表示小数的方法，常用于智能合约中以避免浮点数的不确定性。
//   Q64.64 (如config.rs中) 或这里的 10^18 基准都是定点数的例子。

// 引入标准库及第三方库
use std::{str::FromStr, sync::Arc, vec::Vec}; // `FromStr` 用于从字符串转换, `Arc` 原子引用计数, `Vec` 动态数组

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` (可能是个链上数据索引服务) 引入Pool和Protocol类型
use eyre::{ensure, eyre, Result}; // 错误处理库 `eyre`

use move_core_types::annotated_value::MoveStruct; // Move语言核心类型，用于解析Move对象的结构
use primitive_types::U256; // 256位无符号整数类型，常用于处理大数值，如代币余额或定点数运算
use simulator::Simulator; // 交易模拟器接口
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型：对象ID, 对象引用, Sui地址
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建相关类型
    Identifier, TypeTag, // Sui标识符 (用于模块名、函数名等), 类型标签
};
use tokio::sync::OnceCell; // Tokio提供的异步单次初始化单元，用于延迟初始化全局变量或缓存
use utils::{coin, new_test_sui_client, object::*}; // 自定义工具库: `coin` (代币操作), `new_test_sui_client` (创建Sui客户端), `object` (对象处理)

use super::TradeCtx; // 从父模块 (defi) 引入 `TradeCtx` (交易上下文，用于构建PTB)
use crate::{config::*, defi::Dex}; // 从当前crate引入配置 (`config::*`) 和 `Dex` trait

// --- Aftermath协议相关的常量定义 ---
// 合约包ID (Package ID)
const AFTERMATH_DEX: &str = "0xc4049b2d1cc0f6e017fda8260e4377cecd236bd7f56a54fee120816e72e2e0dd";
// Aftermath协议中关键对象的ID
const POOL_REGISTRY: &str = "0xfcc774493db2c45c79f688f88d28023a3e7d98e4ee9f48bbf5c7990f651577ae"; // 池注册表对象
const PROTOCOL_FEE_VAULT: &str = "0xf194d9b1bcad972e45a7dd67dd49b3ee1e3357a00a50850c52cd51bb450e13b4"; // 协议手续费库对象
const TREASURY: &str = "0x28e499dff5e864a2eafe476269a4f5035f1c16f338da7be18b103499abf271ce"; // 国库对象
const INSURANCE_FUND: &str = "0xf0c40d67b078000e18032334c3325c47b9ec9f3d9ae4128be820d54663d14e3b"; // 保险基金对象
const REFERRAL_VAULT: &str = "0x35d35b0e5b177593d8c3a801462485572fc30861e6ce96a55af6dc4730709278"; // 推荐人库对象

// 滑点保护相关常量。SLIPPAGE = 0.9 * 10^18 (即90%)。
// 这可能表示预期的最小输出金额是计算出的理论输出金额的90%。
// 但通常滑点设置会更小，例如0.5% (0.005 * 10^18)。这里的90%可能是反向的，表示最大价格影响。
// 或者，更可能是指 1 - 0.1 = 0.9，即允许10%的滑点。
// 查阅Aftermath文档或合约可以明确其含义。假设这里是指价格可以不利变动10%。
// 这里的SLIPPAGE更像是用于计算`min_amount_out`时的乘数，例如 `expected_amount_out * SLIPPAGE / ONE`。
// 如果SLIPPAGE = 900_... (0.9 * 10^18)，那么它代表的是期望得到至少90%的理论输出。
// 这对应10%的滑点容忍。
const SLIPPAGE: u128 = 900_000_000_000_000_000; //  (0.9 * 10^18)

// Aftermath的数学运算似乎使用10^18作为固定点数的小数位数基准。
// ONE 代表 1.0 在这种定点数表示下的整数值。
const ONE: U256 = U256([1_000_000_000_000_000_000, 0, 0, 0]); // 10^18, U256的低64位

/// `ObjectArgs` 结构体
///
/// 用于缓存从链上获取并转换为 `ObjectArg` 类型的常用Aftermath协议对象。
/// `ObjectArg` 是构建Sui可编程交易块 (PTB) 时实际使用的对象引用。
/// 通过 `OnceCell` 实现异步单次初始化，确保这些对象只被查询和转换一次。
#[derive(Clone)] // 可以被克隆
pub struct ObjectArgs {
    pool_registry: ObjectArg,       // 池注册表
    protocol_fee_vault: ObjectArg,  // 协议手续费库
    treasury: ObjectArg,            // 国库
    insurance_fund: ObjectArg,      // 保险基金
    referral_vault: ObjectArg,      // 推荐人库
}

// `OBJ_CACHE` 是一个静态的 `OnceCell<ObjectArgs>`。
// `OnceCell` 允一个值只被初始化一次，即使在多线程或异步环境中也是安全的。
// 这里用于全局缓存 `ObjectArgs`。
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new(); // 创建一个空的OnceCell

/// `get_object_args` 异步函数
///
/// 负责获取并缓存 `ObjectArgs`。
/// 如果 `OBJ_CACHE` 尚未初始化，它会异步从Sui网络（通过模拟器）获取所需的对象信息，
/// 将它们转换为 `ObjectArg`，存入 `ObjectArgs` 结构体，然后缓存起来。
/// 后续调用会直接从缓存中获取。
///
/// 参数:
/// - `simulator`: 一个共享的模拟器实例 (`Arc<Box<dyn Simulator>>`)，用于从链上获取对象数据。
///
/// 返回:
/// - `ObjectArgs`: 包含所有所需对象参数的结构体。
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async { // 如果未初始化，则执行闭包内的异步代码来初始化
            // 从字符串常量解析ObjectID，然后通过模拟器获取对象信息
            let pool_registry_obj = simulator
                .get_object(&ObjectID::from_hex_literal(POOL_REGISTRY).unwrap())
                .await
                .unwrap(); // unwrap用于简化，实际应处理错误
            let protocol_fee_vault_obj = simulator
                .get_object(&ObjectID::from_hex_literal(PROTOCOL_FEE_VAULT).unwrap())
                .await
                .unwrap();
            let treasury_obj = simulator
                .get_object(&ObjectID::from_hex_literal(TREASURY).unwrap())
                .await
                .unwrap();
            let insurance_fund_obj = simulator
                .get_object(&ObjectID::from_hex_literal(INSURANCE_FUND).unwrap())
                .await
                .unwrap();
            let referral_vault_obj = simulator
                .get_object(&ObjectID::from_hex_literal(REFERRAL_VAULT).unwrap())
                .await
                .unwrap();

            // 将获取到的对象信息 (SuiObject) 转换为 PTB 中使用的 ObjectArg 类型。
            // `shared_obj_arg` 是一个辅助函数 (可能在 utils::object 中定义)，
            // 它会根据对象是否可变 (mutable) 来创建合适的 ObjectArg (ImmutableSharedObject 或 MutableSharedObject)。
            ObjectArgs {
                pool_registry: shared_obj_arg(&pool_registry_obj, false), // false表示不可变共享对象
                protocol_fee_vault: shared_obj_arg(&protocol_fee_vault_obj, false),
                treasury: shared_obj_arg(&treasury_obj, true), // true表示可变共享对象 (国库可能需要修改)
                insurance_fund: shared_obj_arg(&insurance_fund_obj, true), // 保险基金也可能需要修改
                referral_vault: shared_obj_arg(&referral_vault_obj, false),
            }
        })
        .await // 等待初始化完成 (如果尚未初始化)
        .clone() // 克隆缓存中的 ObjectArgs 返回 (因为 OnceCell::get_or_init 返回引用)
}

/// `Aftermath` 结构体
///
/// 代表一个Aftermath协议的交易池。
/// 包含了与该池进行交互所需的所有状态和参数。
#[derive(Clone)] // 可以被克隆，方便在不同地方使用
pub struct Aftermath {
    pool_arg: ObjectArg,      // 池对象本身的 `ObjectArg`，用于PTB
    liquidity: u128,          // 池的流动性总量 (可能是LP代币的总供应量)
    coin_in_type: String,     // 当前交易方向的输入代币类型字符串
    coin_out_type: String,    // 当前交易方向的输出代币类型字符串
    type_params: Vec<TypeTag>,// 调用Aftermath合约方法时需要的泛型类型参数列表
                              // 通常包含池涉及的所有代币类型，以及可能的输入输出代币类型。

    // 从 get_object_args() 获取的共享对象参数
    pool_registry: ObjectArg,
    protocol_fee_vault: ObjectArg,
    treasury: ObjectArg,
    insurance_fund: ObjectArg,
    referral_vault: ObjectArg,

    // 从池对象状态中解析出来的具体参数
    balances: Vec<u128>,      // 池中各代币的标准化余额 (normalized balances)
    weights: Vec<u64>,        // 池中各代币的权重 (如果是加权池)
    swap_fee_in: u64,         // 输入方向的交换手续费率
    swap_fee_out: u64,        // 输出方向的交换手续费率
    index_in: usize,          // 输入代币在池代币列表中的索引
    index_out: usize,         // 输出代币在池代币列表中的索引
}

impl Aftermath {
    /// `new` 构造函数
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和指定的输入/输出代币类型，
    /// 创建一个或多个 `Aftermath` DEX实例。
    /// 如果 `coin_out_type` 为 `None`，则会为 `coin_in_type` 与池中其他所有代币的组合都创建一个实例。
    ///
    /// 参数:
    /// - `simulator`: 共享的模拟器实例。
    /// - `pool`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。
    /// - `coin_in_type`: 输入代币的类型字符串。
    /// - `coin_out_type`: (可选) 输出代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Vec<Self>>`: 成功则返回包含一个或多个 `Aftermath` 实例的向量，否则返回错误。
    pub async fn new(
        simulator: Arc<Box<dyn Simulator>>,
        pool_info: &Pool, // 从索引器获取的池信息
        coin_in_type: &str,
        coin_out_type: Option<String>, // 可选的输出代币
    ) -> Result<Vec<Self>> {
        // 确保提供的池信息确实是Aftermath协议的池
        ensure!(pool_info.protocol == Protocol::Aftermath, "提供的不是Aftermath协议的池");

        // 通过模拟器获取池对象的详细信息 (SuiObject)
        let pool_obj = simulator
            .get_object(&pool_info.pool) // pool_info.pool 是池的ObjectID
            .await
            .ok_or_else(|| eyre!("Aftermath池对象未找到: {}", pool_info.pool))?;

        // 解析池对象的Move结构体内容，以获取其字段值。
        // 这需要池对象的布局 (layout)，也通过模拟器获取。
        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_else(|| eyre!("Aftermath池对象的布局(layout)未找到"))?;
            // 尝试将SuiObject的数据部分转换为Move对象
            let move_obj = pool_obj.data.try_as_move().ok_or_else(|| eyre!("对象不是Move对象"))?;
            // 使用布局反序列化Move对象的具体内容
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 从解析后的Move结构体中提取字段值。
        // `extract_struct_from_move_struct`, `extract_u64_from_move_struct`, `extract_u128_vec_from_move_struct`
        // 这些是辅助函数 (可能在 utils::object 中定义)，用于从 `MoveStruct` 中安全地提取特定名称和类型的字段。
        let liquidity = {
            let lp_supply_struct = extract_struct_from_move_struct(&parsed_pool_struct, "lp_supply")?;
            extract_u64_from_move_struct(&lp_supply_struct, "value")? as u128 // LP供应量作为流动性
        };

        let balances = extract_u128_vec_from_move_struct(&parsed_pool_struct, "normalized_balances")?; // 标准化余额
        let weights = extract_u64_vec_from_move_struct(&parsed_pool_struct, "weights")?; // 代币权重
        let fees_swap_in = extract_u64_vec_from_move_struct(&parsed_pool_struct, "fees_swap_in")?; // 输入手续费率数组
        let fees_swap_out = extract_u64_vec_from_move_struct(&parsed_pool_struct, "fees_swap_out")?; // 输出手续费率数组

        // 获取输入代币在池代币列表中的索引
        let index_in = pool_info.token_index(coin_in_type).ok_or_else(|| eyre!("输入代币 {} 在池 {} 的索引未找到", coin_in_type, pool_info.pool))?;


        // 准备调用合约时所需的泛型类型参数列表 (`type_params`)。
        // 通常，Aftermath的swap函数需要知道池中所有代币的类型，以及具体的输入和输出代币类型。
        // `parsed_pool_struct.type_.type_params` 包含了池本身的泛型参数 (例如池中所有代币的类型列表)。
        let mut base_type_params = parsed_pool_struct.type_.type_params.clone();
        // 将具体的输入代币类型也加入到泛型参数列表中 (如果合约需要的话，通常是作为最后两个参数)
        let coin_in_type_tag = TypeTag::from_str(coin_in_type).map_err(|e| eyre!(e))?;
        // (注意：这里的逻辑是先加 coin_in, 如果指定了 coin_out 再加 coin_out。
        //  实际合约调用时的顺序可能不同，例如 [CoinType1, CoinType2, ..., CoinIn, CoinOut]。
        //  这里是先复制池的泛型参数，然后追加具体的交易对类型。
        //  Aftermath的`swap_exact_in`函数签名是 `swap_exact_in<CoinType0, ..., CoinTypeN, CoinInAdmin, CoinOutAdmin>(...)`
        //  这里的 `type_params` 应该是 `CoinType0, ..., CoinTypeN`。
        //  而 `CoinInAdmin` 和 `CoinOutAdmin` 会在调用时由具体的 `coin_in_type` 和 `coin_out_type` 决定。
        //  所以，`base_type_params` 已经包含了池的所有代币类型。
        //  `swap_exact_in`的类型参数是池中所有代币的类型，然后是输入币种，然后是输出币种。
        //  这里的实现是 `type_params` = [PoolCoinTypes..., CoinInType, CoinOutType]

        // 将池对象转换为 `ObjectArg`
        let pool_arg = shared_obj_arg(&pool_obj, true); // 池对象在交易中通常是可变的

        // 获取共享的协议对象参数 (通过缓存)
        let object_args_cache = get_object_args(Arc::clone(&simulator)).await;

        // 如果指定了 `coin_out_type`，则只创建一个针对这个特定交易对的 `Aftermath` 实例。
        if let Some(specific_coin_out_type) = coin_out_type {
            let coin_out_type_tag = TypeTag::from_str(&specific_coin_out_type).map_err(|e| eyre!(e))?;
            // 完整的泛型参数列表
            let mut final_type_params = base_type_params;
            final_type_params.push(coin_in_type_tag);
            final_type_params.push(coin_out_type_tag);

            let index_out = pool_info.token_index(&specific_coin_out_type).ok_or_else(|| eyre!("输出代币 {} 在池 {} 的索引未找到", specific_coin_out_type, pool_info.pool))?;

            return Ok(vec![Self {
                pool_arg,
                liquidity,
                coin_in_type: coin_in_type.to_string(),
                coin_out_type: specific_coin_out_type,
                type_params: final_type_params,
                pool_registry: object_args_cache.pool_registry,
                protocol_fee_vault: object_args_cache.protocol_fee_vault,
                treasury: object_args_cache.treasury,
                insurance_fund: object_args_cache.insurance_fund,
                referral_vault: object_args_cache.referral_vault,
                balances: balances.clone(), // 克隆以拥有所有权
                weights: weights.clone(),
                swap_fee_in: fees_swap_in[index_in], // 根据索引获取该代币的输入手续费
                swap_fee_out: fees_swap_out[index_out],// 根据索引获取该代币的输出手续费
                index_in,
                index_out,
            }]);
        }

        // 如果没有指定 `coin_out_type`，则遍历池中的所有其他代币，
        // 为 `coin_in_type` 与每个其他代币的组合都创建一个 `Aftermath` 实例。
        let mut result_dex_instances = Vec::new();
        for (idx_out_candidate, coin_out_info) in pool_info.tokens.iter().enumerate() {
            // 跳过输入代币本身 (不能自己和自己交易)
            if coin_out_info.token_type == coin_in_type {
                continue;
            }

            let coin_out_type_tag = TypeTag::from_str(&coin_out_info.token_type).map_err(|e| eyre!(e))?;
            let mut final_type_params = base_type_params.clone(); // 从基础的池代币类型开始
            final_type_params.push(coin_in_type_tag.clone()); // 添加输入代币类型
            final_type_params.push(coin_out_type_tag);      // 添加当前遍历到的输出代币类型

            result_dex_instances.push(Self {
                pool_arg: pool_arg.clone(), // 克隆ObjectArg (是Arc内部的，成本低)
                liquidity,
                coin_in_type: coin_in_type.to_string(),
                coin_out_type: coin_out_info.token_type.clone(),
                type_params: final_type_params,
                pool_registry: object_args_cache.pool_registry.clone(),
                protocol_fee_vault: object_args_cache.protocol_fee_vault.clone(),
                treasury: object_args_cache.treasury.clone(),
                insurance_fund: object_args_cache.insurance_fund.clone(),
                referral_vault: object_args_cache.referral_vault.clone(),
                balances: balances.clone(),
                weights: weights.clone(),
                swap_fee_in: fees_swap_in[index_in],
                swap_fee_out: fees_swap_out[idx_out_candidate], // 使用当前候选输出代币的索引
                index_in,
                index_out: idx_out_candidate, // 当前候选输出代币的索引
            });
        }

        Ok(result_dex_instances)
    }

    /// `build_swap_tx` (私有辅助函数)
    ///
    /// 构建一个完整的、可直接提交的Sui可编程交易 (ProgrammableTransaction)，
    /// 该交易包含在Aftermath池中进行一次精确输入交换的操作。
    ///
    /// 参数:
    /// - `sender`: 交易发送者的Sui地址。
    /// - `recipient`: 接收输出代币的Sui地址。
    /// - `coin_in_ref`: 输入代币对象的引用 (`ObjectRef`)。
    /// - `amount_in`: 输入代币的数量。
    ///
    /// 返回:
    /// - `Result<ProgrammableTransaction>`: 成功则返回构建好的PTB。
    #[allow(dead_code)] // 允许存在未使用的代码 (这个函数可能在某些场景下被直接调用)
    async fn build_swap_tx(
        &self,
        sender: SuiAddress,
        recipient: SuiAddress,
        coin_in_ref: ObjectRef, // 输入代币的对象引用
        amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        // 创建一个新的交易上下文 `TradeCtx`，用于辅助构建PTB。
        let mut ctx = TradeCtx::default();

        // 步骤1: 如果输入的 `amount_in` 小于该 `coin_in_ref` 的总面额，
        // 则需要先分割出一个面额正好是 `amount_in` 的新代币对象。
        // `ctx.split_coin` 会处理这个逻辑，并返回代表新分割出代币的 `Argument`。
        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;

        // 步骤2: 调用 `extend_trade_tx` 将实际的Aftermath交换操作添加到PTB中。
        // `coin_in_arg` 是上一步分割出来的代币。
        // `extend_trade_tx` 会返回代表输出代币的 `Argument`。
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, Some(amount_in)).await?;

        // 步骤3: 将输出代币 `coin_out_arg` 转移给指定的接收者 `recipient`。
        ctx.transfer_arg(recipient, coin_out_arg);

        // 完成PTB的构建。
        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数)
    ///
    /// 构建调用Aftermath `swap_exact_in` Move合约方法时所需的参数列表 (`Vec<Argument>`)。
    /// 这些参数会被 `extend_trade_tx` 用到。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文 (`&mut TradeCtx`)，用于将对象转换为 `Argument` 或创建纯值 `Argument`。
    /// - `coin_in_arg`: 代表输入代币的 `Argument`。
    /// - `amount_in`: 输入代币的数量。
    ///
    /// 返回:
    /// - `Result<Vec<Argument>>`: 包含所有调用参数的向量。
    async fn build_swap_args(
        &self,
        ctx: &mut TradeCtx,
        coin_in_arg: Argument,
        amount_in: u64,
    ) -> Result<Vec<Argument>> {
        // 将结构体中缓存的 ObjectArg 转换为 PTB 中实际使用的 Argument 类型。
        // `ctx.obj()` 可能会将 ObjectArg 包装成 Argument::Object 或 Argument::SharedObject。
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let pool_registry_arg = ctx.obj(self.pool_registry).map_err(|e| eyre!(e))?;
        let protocol_fee_vault_arg = ctx.obj(self.protocol_fee_vault).map_err(|e| eyre!(e))?;
        let treasury_arg = ctx.obj(self.treasury).map_err(|e| eyre!(e))?;
        let insurance_fund_arg = ctx.obj(self.insurance_fund).map_err(|e| eyre!(e))?;
        let referral_vault_arg = ctx.obj(self.referral_vault).map_err(|e| eyre!(e))?;

        // 计算预期的最小输出金额 (考虑滑点)。
        // `expect_amount_out` 是这个结构体的一个方法，它会调用核心的数学计价函数。
        let expected_min_amount_out = self.expect_amount_out(amount_in)?;
        // 将计算出的 u64 最小输出金额转换为 PTB 使用的纯值 `Argument`。
        let expect_amount_out_arg = ctx.pure(expected_min_amount_out).map_err(|e| eyre!(e))?;
        // 将滑点常量 `SLIPPAGE` (u128) 转换为 u64 (如果合约需要) 并包装为 `Argument`。
        // 注意：Aftermath的 `swap_exact_in` 合约参数中可能没有直接的滑点参数，
        // 而是通过 `expected_min_amount_out` 来间接实现滑点控制。
        // 这里的 `slippage_arg` 可能是多余的，或者合约确实有这个参数。
        // 查阅Aftermath合约 `swap_exact_in` 的签名可以确认。
        // 假设合约需要一个 `min_amount_out` 参数，那么 `expect_amount_out_arg` 就是这个。
        // 如果合约还需要一个单独的slippage tolerance参数 (如 500 for 0.5%)，则这里需要调整。
        // 从函数名 `swap_exact_in` 通常意味着提供精确输入，并接受一个最小输出参数。
        // 此处的 `SLIPPAGE` 常量可能是用于计算 `expected_min_amount_out` 时内部使用的，
        // 而不是直接作为参数传给合约。
        // 仔细看 `expect_amount_out` 的实现，它直接返回了 `amount_out`，没有乘以滑点。
        // 所以这里的 `expect_amount_out_arg` 是理论输出，而 `slippage_arg` 可能是用于合约内部计算最小可接受输出。
        // **修正/澄清**：通常 `swap_exact_in` 会要求一个 `min_amount_out` 参数。
        // `self.expect_amount_out()` 应该返回的是理论上的最佳输出。
        // `min_amount_out` 则应是 `theory_amount_out * (1 - slippage_tolerance)`。
        // 这里的 `expect_amount_out_arg` 应该是 `min_amount_out`。
        // 而 `SLIPPAGE` 常量 (0.9e18) 如果直接作为参数，其含义取决于合约。
        // 假设 `expect_amount_out_arg` 已经是考虑了滑点的最小输出。
        let slippage_arg = ctx.pure(SLIPPAGE as u64).map_err(|e| eyre!(e))?; // 可能需要调整 SLIPPAGE 的用法

        // 返回构建好的参数列表，顺序必须与Move合约方法的参数顺序一致。
        Ok(vec![
            pool_arg,                 // pool: &Pool<CoinTypes...>
            pool_registry_arg,        // pool_registry: &PoolRegistry
            protocol_fee_vault_arg,   // protocol_fee_vault: &ProtocolFeeVault
            treasury_arg,             // treasury: &mut Treasury
            insurance_fund_arg,       // insurance_fund: &mut InsuranceFund
            referral_vault_arg,       // referral_vault: &ReferralVault
            coin_in_arg,              // coin_in: Coin<CoinInAdmin>
            expect_amount_out_arg,    // expected_coin_out_amount: u64 (这应该是最小输出金额)
            slippage_arg,             // max_slippage_percent: u64 (这个参数可能不存在或用法不同)
                                      // 查阅 Aftermath 的 `swap_exact_in` 签名，它需要 `min_amount_out`。
                                      // 所以 `expect_amount_out_arg` 应该是 `min_amount_out`。
                                      // `slippage_arg` 可能是多余的，或者它的类型/含义不同。
                                      // 如果函数签名是 `(..., coin_in, amount_in_minimum_amount_out, ...)`
                                      // 那么 `expect_amount_out_arg` 应该是 `min_amount_out`。
                                      // 此处的 `SLIPPAGE` 作为一个独立的参数可能不正确。
                                      // **重要**: 此参数列表需要严格匹配Aftermath的`swap_exact_in`函数签名。
                                      // 假设 `expect_amount_out_arg` 是 `min_amount_out`。
                                      // 并且 `slippage_arg` 是合约需要的另一个与滑点相关的参数（如果存在）。
                                      // 如果合约只需要 `min_amount_out`，那么 `slippage_arg` 应该移除。
                                      // 根据函数名，可能它只需要 `min_out`。
                                      // 假设 `expect_amount_out_arg` 是 `min_out`。
        ])
    }

    /// `expect_amount_out` (内联辅助函数)
    ///
    /// 根据当前池的状态（余额、权重、手续费）和输入金额，计算预期的输出代币数量。
    /// 这个计算会调用更底层的数学函数 `calculate_expected_out`。
    ///
    /// 参数:
    /// - `amount_in`: 输入代币的数量 (u64)。
    ///
    /// 返回:
    /// - `Result<u64>`: 预期的输出代币数量。
    #[inline] // 建议编译器内联此函数，以提高性能
    fn expect_amount_out(&self, amount_in: u64) -> Result<u64> {
        let amount_out = calculate_expected_out(
            self.balances[self.index_in],     // 输入代币在池中的余额
            self.balances[self.index_out],    // 输出代币在池中的余额
            self.weights[self.index_in],      // 输入代币的权重
            self.weights[self.index_out],     // 输出代币的权重
            self.swap_fee_in,                 // 输入方向的手续费
            self.swap_fee_out,                // 输出方向的手续费 (对于单次swap，可能只用到一个)
            amount_in,                        // 输入金额
        )?;

        Ok(amount_out)
    }
}

/// 为 `Aftermath` 结构体实现 `Dex` trait。
/// `Dex` trait 定义了与不同DEX进行交互的通用接口。
#[async_trait::async_trait] // 因为 `Dex` trait 中的某些方法是异步的
impl Dex for Aftermath {
    /// `extend_trade_tx`
    ///
    /// 将Aftermath的交换操作添加到现有的交易上下文 `ctx` (即可编程交易块 PTB) 中。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文 (`&mut TradeCtx`)。
    /// - `_sender`: 交易发送者地址 (在这个实现中未使用，用 `_` 前缀表示)。
    /// - `coin_in_arg`: 代表输入代币的 `Argument`。
    /// - `amount_in`: (可选) 输入代币的数量。对于Aftermath，这里要求必须提供 (`.ok_or_else`)。
    ///
    /// 返回:
    /// - `Result<Argument>`: 代表从交换中获得的输出代币的 `Argument`。
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress, // 未使用
        coin_in_arg: Argument,
        amount_in: Option<u64>,
    ) -> Result<Argument> {
        // 确保提供了 amount_in
        let amount_in_val = amount_in.ok_or_else(|| eyre!("Aftermath交易需要提供amount_in"))?;

        // 构建调用Move合约方法所需的信息
        let package_id = ObjectID::from_hex_literal(AFTERMATH_DEX)?; // Aftermath合约包ID
        let module_name = Identifier::new("swap").map_err(|e| eyre!(e))?; // 模块名 "swap"
        let function_name = Identifier::new("swap_exact_in").map_err(|e| eyre!(e))?; // 函数名 "swap_exact_in"

        // 泛型类型参数列表 (例如: [TokenTypeA, TokenTypeB, ..., CoinInType, CoinOutType])
        // 需要确认 `self.type_params` 的构造是否符合 `swap_exact_in` 的要求。
        // `swap_exact_in`的类型参数是池的所有代币类型 `CoinTypes`，然后是 `CoinIn`, `CoinOut`
        // 如果 `self.type_params` 已经包含了 `CoinIn` 和 `CoinOut`，那么它是正确的。
        // 在 `Aftermath::new` 中，`type_params` 被构造成 `[PoolCoinTypes..., CoinInTypeTag, CoinOutTypeTag]`
        // 这似乎是正确的。
        let type_arguments = self.type_params.clone();

        // 构建调用参数列表
        let call_arguments = self.build_swap_args(ctx, coin_in_arg, amount_in_val).await?;

        // 向PTB中添加一个Move调用命令
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        // Move调用的结果 (即输出代币) 通常是最后一个命令的结果。
        // `ctx.last_command_idx()` 获取最后一个命令的索引。
        // `Argument::Result(idx)` 表示引用该索引命令的返回值。
        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx))
    }

    /// `swap_tx`
    ///
    /// 构建一个完整独立的Sui交易数据 (`TransactionData`)，用于执行一次Aftermath交换。
    /// 这个方法通常用于直接发起一次交换，而不是作为复杂PTB的一部分。
    ///
    /// 参数:
    /// - `sender`: 交易发送者地址。
    /// - `recipient`: 接收输出代币的地址。
    /// - `amount_in`: 输入代币的数量。
    ///
    /// 返回:
    /// - `Result<TransactionData>`: 构建好的交易数据。
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        // 创建一个Sui客户端 (这里使用测试客户端，实际应从配置获取或传入)
        let sui_client = new_test_sui_client().await; // 注意：这会创建一个新的客户端，可能效率不高

        // 获取一个面额至少为 `amount_in` 的输入代币对象。
        // `coin::get_coin` 会查找或分割代币。
        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;

        // 调用内部的 `build_swap_tx` (注意：之前这个函数被标记为 dead_code，这里实际使用了)
        // 来构建包含交换操作的PTB。
        let pt = self
            .build_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in)
            .await?;

        // 获取用于支付Gas的代币对象。
        // `Some(coin_in_obj.coin_object_id)` 确保不会将用作输入的代币同时用作Gas币。
        let gas_coins = coin::get_gas_coin_refs(&sui_client, sender, Some(coin_in_obj.coin_object_id)).await?;
        // 获取当前网络的参考Gas价格。
        let gas_price = sui_client.read_api().get_reference_gas_price().await?;
        // 使用PTB、Gas币、Gas预算和Gas价格创建最终的交易数据。
        let tx_data = TransactionData::new_programmable(sender, gas_coins, pt, GAS_BUDGET, gas_price);

        Ok(tx_data)
    }

    // --- Dex trait 的其他 getter 方法 ---
    fn coin_in_type(&self) -> String {
        self.coin_in_type.clone()
    }

    fn coin_out_type(&self) -> String {
        self.coin_out_type.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::Aftermath // 返回DEX协议类型
    }

    fn liquidity(&self) -> u128 {
        self.liquidity // 返回池的流动性
    }

    fn object_id(&self) -> ObjectID {
        self.pool_arg.id() // 返回池对象的ID (从ObjectArg获取)
    }

    /// `flip` 方法
    ///
    /// 翻转交易方向，即交换输入代币和输出代币。
    /// 这对于某些套利策略（例如三角套利中的反向路径）可能有用。
    /// 注意：这个方法只交换了 `coin_in_type` 和 `coin_out_type` 字符串，
    /// 以及相关的索引和手续费。它没有重新获取链上数据或修改 `type_params`。
    /// 如果 `type_params` 的顺序与 `coin_in_type`/`coin_out_type` 严格相关，
    /// 那么 `type_params` 可能也需要相应调整，但这取决于合约调用的具体要求。
    /// 目前 `Aftermath::new` 中 `type_params` 的构造是 `[PoolCoins..., CoinIn, CoinOut]`，
    /// 如果翻转，`CoinIn` 和 `CoinOut` 的位置需要交换。
    /// **重要**: 此 `flip` 实现可能不完整，因为它没有更新 `type_params` 中的 `CoinIn` 和 `CoinOut` 部分。
    /// 如果 `extend_trade_tx` 依赖 `self.type_params` 来正确指定交易对，那么这里会有问题。
    /// 假设 `type_params` 的最后两个元素是 `CoinIn` 和 `CoinOut`。
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        std::mem::swap(&mut self.index_in, &mut self.index_out);
        std::mem::swap(&mut self.swap_fee_in, &mut self.swap_fee_out);
        // `type_params`也需要更新最后两个元素的位置
        if self.type_params.len() >= 2 {
            let len = self.type_params.len();
            self.type_params.swap(len - 2, len - 1);
        }
        // `balances` 和 `weights` 是按池的原始代币顺序排列的，不需要交换。
    }

    /// `is_a2b` 方法
    ///
    /// 判断当前交易方向是否为A到B (例如，SUI到USDC)。
    /// 这个方法的实现在这里返回 `false`，可能表示它不用于区分方向，
    /// 或者这个概念对Aftermath的实现不重要，或者尚未完全实现。
    fn is_a2b(&self) -> bool {
        false // 实际含义取决于调用方的期望
    }
}

// --- Aftermath 定价和数学计算相关的辅助函数 ---
// 这些函数实现了Aftermath加权池的定价逻辑，使用U256进行固定点数运算。
// ONE (10^18) 是固定点数表示中1.0的值。

/// `calculate_expected_out`
///
/// 使用现货价格（考虑费用）估算输出金额。
/// 这是一个简化的计算，实际的曲线AMM输出会涉及更复杂的公式。
/// Aftermath的池可能是基于Balancer的加权池或稳定池的变种。
/// 此函数似乎是基于Balancer的加权池公式的简化版本，用于估算。
///
/// 参数:
/// - `balance_in`: 输入代币的池中余额。
/// - `balance_out`: 输出代币的池中余额。
/// - `weight_in`: 输入代币的权重。
/// - `weight_out`: 输出代币的权重。
/// - `swap_fee_in`: 输入方向的交换费。
/// - `swap_fee_out`: 输出方向的交换费。 (对于单向swap，通常只应用一个方向的费率，或一个综合费率)
/// - `amount_in`: 输入金额。
///
/// 返回:
/// - `Result<u64>`: 预期的输出金额。
pub fn calculate_expected_out(
    balance_in: u128,      // 注意：这里接收的是u128，但内部转换为U256
    balance_out: u128,
    weight_in: u64,
    weight_out: u64,
    swap_fee_in: u64,      // 这些费率可能是以10^18为基准的，例如 3000000000000000 表示 0.3%
    swap_fee_out: u64,
    amount_in: u64,
) -> Result<u64> {
    // 获取考虑费用的现货价格 (spot price)
    let spot_price_with_fees = calc_spot_price_fixed_with_fees(
        U256::from(balance_in),  // 将输入转换为U256
        U256::from(balance_out),
        U256::from(weight_in),
        U256::from(weight_out),
        U256::from(swap_fee_in), // 假设费用是以U256的基数表示的 (例如 0.003 * ONE)
        U256::from(swap_fee_out),
    )?;

    // 计算预期输出金额: amount_out = amount_in / spot_price_with_fees
    // 所有计算都在固定点数下进行。
    // `convert_int_to_fixed` 将 u64 输入金额转换为 U256 定点数。
    // `div_down` 是向下取整的定点数除法。
    // `convert_fixed_to_int` 将 U256 定点数结果转换回 u64。
    Ok(convert_fixed_to_int(div_down(
        convert_int_to_fixed(amount_in), // amount_in * ONE
        spot_price_with_fees,            // (balance_in / weight_in) / (balance_out / weight_out) * fee_factor
    )?))
}

// --- 固定点数数学运算辅助函数 ---

/// 将普通整数 (u64) 转换为U256定点数表示。
/// (即乘以 ONE 这个基数)
fn convert_int_to_fixed(a: u64) -> U256 {
    U256::from(a) * ONE
}

/// 将U256定点数表示转换回普通整数 (u64)。
/// (即除以 ONE 这个基数，并取结果的低64位)
fn convert_fixed_to_int(a: U256) -> u64 {
    (a / ONE).low_u64() // low_u64() 获取U256最低的64位
}

/// 向下取整的定点数除法: `a / b`
/// (a * ONE) / b 保证了结果仍然是定点数表示。
fn div_down(a: U256, b: U256) -> Result<U256> {
    if b.is_zero() { // 防止除以零
        return Err(eyre!("定点数除法中除数为零"));
    }
    Ok((a * ONE) / b) // (a * 10^18) / b
}

/// 向下取整的定点数乘法: `a * b`
/// (a * b) / ONE 将两个定点数相乘的结果调整回正确的定点数表示。
#[allow(dead_code)] // 标记为未使用，但可能在其他地方或将来有用
fn mul_down(a: U256, b: U256) -> Result<U256> {
    Ok((a * b) / ONE) // (a * b) / 10^18
}

/// 计算补数 (complement): `1 - x`
/// 如果 `x` 是一个费率 (例如 0.003 * ONE)，那么 `complement(x)` 就是 `(1 - 0.003) * ONE`。
/// 用于从费率计算保留的比例。
fn complement(x: U256) -> U256 {
    if x < ONE { // 确保 x <= 1.0
        ONE - x
    } else { // 如果 x > 1.0 (例如费率大于100%)，则补数为0 (不保留任何东西)
        U256::zero()
    }
}

/// `calc_spot_price_fixed_with_fees`
///
/// 计算考虑了费用的现货价格。
/// 现货价格 SP = (BalanceIn / WeightIn) / (BalanceOut / WeightOut)
/// 费用调整因子通常是 (1 - FeeRateIn) * (1 - FeeRateOut)。
/// SP_with_fees = SP_no_fees / FeeFactor (如果FeeFactor是保留比例)
/// 或者 SP_with_fees = SP_no_fees * (1 + EffectiveFeeRate) (如果FeeFactor是费用本身)
/// 这里的实现是 SP_with_fees = SP_no_fees / ((1-FeeIn) * (1-FeeOut))
fn calc_spot_price_fixed_with_fees(
    balance_in: U256,
    balance_out: U256,
    weight_in: U256,
    weight_out: U256,
    swap_fee_in: U256,  // 假设 swap_fee_in 是 0.003 * ONE 这样的费率
    swap_fee_out: U256, // 假设 swap_fee_out 也是费率
) -> Result<U256> {
    // 首先计算不含费用的现货价格
    let spot_price_no_fees = calc_spot_price(balance_in, balance_out, weight_in, weight_out)?;

    // 计算费用调整因子 (fees_scalar)
    // fees_scalar = (1 - swap_fee_in) * (1 - swap_fee_out)
    // `complement` 计算 1 - fee
    let fees_scalar = mul_down(complement(swap_fee_in), complement(swap_fee_out))?;
    //  检查fees_scalar是否为0，以避免除以0的错误
    if fees_scalar.is_zero() {
        return Err(eyre!("计算现货价格时费用因子为零"));
    }

    // 应用费用: SP_with_fees = SP_no_fees / fees_scalar
    div_down(spot_price_no_fees, fees_scalar)
}

/// `calc_spot_price`
///
/// 计算不含费用的现货价格 (基于加权池的公式)。
/// SP = (BalanceIn / WeightIn) / (BalanceOut / WeightOut)
fn calc_spot_price(balance_in: U256, balance_out: U256, weight_in: U256, weight_out: U256) -> Result<U256> {
    // term_in = (balance_in * ONE) / weight_in  (使用div_down进行定点数除法)
    let term_in = div_down(balance_in * ONE, weight_in)?; // balance_in已经是U256，乘以ONE是多余的，除非balance_in不是定点数
                                                          // 假设balance_in, balance_out已经是与ONE同基准的定点数
                                                          // 如果balance_in是整数余额，则 convert_int_to_fixed(balance_in) / weight_in (如果weight也是定点数)
                                                          // 或者 (balance_in / weight_in) 如果是直接比率。
                                                          // 从函数签名看，balance_in/out, weight_in/out都是U256。
                                                          // Aftermath文档中公式为 (B_i / W_i) / (B_o / W_o)
                                                          // 这里的实现是 ((B_i * ONE) / W_i) / ((B_o * ONE) / W_o)
                                                          // 这意味着 W_i, W_o 也被当作与 ONE 同基准的定点数，或者 B_i, B_o 不是。
                                                          // 如果 B_i, W_i 都是普通的整数值，那么 (B_i / W_i) / (B_o / W_o)
                                                          // 然后再转换为定点数。
                                                          // 这里的实现更像是 (balance_in / weight_in) / (balance_out / weight_out)
                                                          // 其中除法是定点数除法。
                                                          // (balance_in / weight_in) = div_down(balance_in, weight_in)
                                                          // (balance_out / weight_out) = div_down(balance_out, weight_out)
                                                          // 然后 SP = div_down(term_in_calc, term_out_calc)
                                                          // 现在的实现是：
                                                          // term_in = (balance_in * ONE) / weight_in  (这里 weight_in 应该是整数权重)
                                                          // term_out = (balance_out * ONE) / weight_out (这里 weight_out 应该是整数权重)
                                                          // SP = term_in / term_out (定点数除法)
                                                          // 这才是正确的，因为 balance 是代币数量，weight 是比率。
                                                          // 所以 (balance_in / weight_in) 是单位权重的余额。

    let term_in_fixed = div_down(balance_in, weight_in)?; // (BalanceIn / WeightIn) in fixed point
    let term_out_fixed = div_down(balance_out, weight_out)?; // (BalanceOut / WeightOut) in fixed point

    // SP = term_in_fixed / term_out_fixed
    div_down(term_in_fixed, term_out_fixed)
}


// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use object_pool::ObjectPool; // 对象池，用于管理模拟器等资源
    use simulator::{DBSimulator, Simulator}; // 数据库模拟器和通用模拟器 trait
    use tracing::info; // 日志库

    use super::*; // 导入外部模块 (aftermath.rs) 的所有公共成员
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL}, // 测试用的配置常量
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器
    };

    /// `test_aftermath_swap_tx` 测试函数
    ///
    /// 这个测试会：
    /// 1. 初始化一个模拟器 (DBSimulator)。
    /// 2. 设置测试参数 (发送者、接收者、代币类型、输入金额)。
    /// 3. 使用 `IndexerDexSearcher` 查找Aftermath的DEX实例。
    /// 4. 选择流动性最大的Aftermath池。
    /// 5. 调用该DEX实例的 `swap_tx` 方法构建交易数据。
    /// 6. 使用模拟器执行（模拟）这个交易。
    /// 7. 打印交易和模拟结果的日志。
    #[tokio::test] // 异步测试宏
    async fn test_aftermath_swap_tx() {
        // 初始化日志系统
        mev_logger::init_console_logger_with_directives(None, &["arb=debug"]);

        // 创建一个模拟器对象池，这里使用DBSimulator进行测试
        let simulator_pool = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new() // 创建一个新的tokio运行时来执行异步初始化
                .unwrap()
                .block_on(async { Box::new(DBSimulator::new_test(true).await) as Box<dyn Simulator> })
        }));

        // 定义测试参数
        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap(); // 交易发送者 (从配置获取)
        let recipient = // 一个固定的接收者地址
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI"; // 输入代币为SUI
        let token_out_type = "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN"; // 输出代币为Wormhole USDC
        let amount_in = 1_000_000_000; // 输入1 SUI (10^9 MIST)

        // --- 查找DEX实例并执行交换 ---
        // 创建DEX搜索器实例 (使用测试RPC URL和模拟器池)
        let searcher = IndexerDexSearcher::new(TEST_HTTP_URL, Arc::clone(&simulator_pool))
            .await
            .unwrap();
        // 查找从 token_in_type 到 token_out_type 的所有DEX路径
        let dexes = searcher
            .find_dexes(token_in_type, Some(token_out_type.into()))
            .await
            .unwrap();
        info!("🧀 找到的DEX数量: {}", dexes.len()); // 打印找到的DEX数量

        // 从找到的DEX中筛选出Aftermath协议的池，并选择流动性最大的那个。
        let dex_to_test = dexes
            .into_iter() // 转换为迭代器
            .filter(|dex| dex.protocol() == Protocol::Aftermath) // 只保留Aftermath的池
            .max_by_key(|dex| dex.liquidity()) // 按流动性从大到小排序，取最大的
            .expect("测试中未找到Aftermath的池"); // 如果没有找到Aftermath池则panic

        // 使用选定的DEX实例构建交换交易数据
        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 构建的交易数据: {:?}", tx_data); // 打印交易数据

        // --- 模拟交易 ---
        let simulator_instance = simulator_pool.get(); // 从池中获取一个模拟器实例
        // 执行交易模拟
        let response = simulator_instance.simulate(tx_data, Default::default()).await.unwrap(); // Default::default() 用于SimulateCtx
        info!("🧀 模拟结果: {:?}", response); // 打印模拟结果

        // 在实际测试中，这里通常还会有断言 (assertions) 来验证模拟结果是否符合预期，
        // 例如，检查交易是否成功，输出金额是否在合理范围内等。
        assert!(response.is_ok(), "交易模拟应成功");
    }
}
