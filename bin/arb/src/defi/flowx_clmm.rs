// 该文件 `flowx_clmm.rs` 实现了与 FlowX Finance 协议的CLMM（集中流动性做市商）池交互的逻辑。
// FlowX是Sui区块链上的一个DEX，采用了CLMM模型，允许流动性提供者在特定价格范围内提供流动性。
// 该实现也包含了对FlowX闪电贷功能的支持。
//
// **文件概览 (File Overview)**:
// 这个 `flowx_clmm.rs` 文件是专门用来和Sui区块链上的FlowX Finance这个DeFi协议的“集中流动性做市商”（CLMM）池子打交道的代码。
// FlowX也是一个去中心化交易所（DEX），它和Cetus、Kriya CLMM一样，都用了CLMM这种允许流动性提供者把钱更精确地放到特定价格范围的技术。
// 这个文件里的代码也试图实现对FlowX池子“闪电贷”功能的支持，尽管在 `support_flashloan` 方法的注释中提到当前可能返回 `false`，但相关的代码结构是存在的。
// (This `flowx_clmm.rs` file contains code specifically for interacting with the "Concentrated Liquidity Market Maker" (CLMM) pools of the FlowX Finance protocol on the Sui blockchain.
//  FlowX is also a Decentralized Exchange (DEX) on Sui. Like Cetus and Kriya CLMM, it uses the CLMM model, which allows liquidity providers to place their funds more precisely within specific price ranges.
//  The code in this file also attempts to support the "flash loan" functionality of FlowX pools, although the comment in the `support_flashloan` method indicates it might currently return `false`, the related code structure is present.)
//
// **主要内容 (Main Contents)**:
// 1.  **常量定义 (Constant Definitions)**:
//     -   `FLOWX_CLMM`: FlowX CLMM核心智能合约的“门牌号”（Package ID）。
//     -   `VERSIONED`: FlowX可能用到的一个“版本化对象”的ID。这个对象用于管理合约升级或版本控制。
//     -   `POOL_REGISTRY`: FlowX的“池子注册表”对象的ID。这是一个中心化的合约或对象，用来管理和查找协议中所有的交易池。
//
// 2.  **`ObjectArgs` 结构体与 `OBJ_CACHE`**:
//     -   `ObjectArgs` 用来打包缓存上述 `POOL_REGISTRY`, `VERSIONED` 以及Sui系统时钟对象的引用信息。
//     -   `OBJ_CACHE` 是一个一次性初始化并全局共享的缓存。
//
// 3.  **`FlowxClmm` 结构体**:
//     -   代表FlowX CLMM协议里的一个具体的交易池实例。
//     -   包含了与该池交互所需的信息，如原始池信息、流动性、代币类型、交易手续费率、调用合约所需的类型参数，以及从缓存中获取的共享对象参数。
//     -   它也实现了项目内部定义的 `Dex` 通用接口。
//
// 4.  **`new()` 构造函数**:
//     -   异步方法，根据从`dex_indexer`获取的池信息和指定的输入代币类型来初始化一个 `FlowxClmm` 实例。
//     -   它会解析池对象的链上数据，提取流动性、手续费率等信息。
//
// 5.  **常规交换相关方法 (Regular Swap Methods)**:
//     -   `build_swap_tx()` / `build_swap_args()`: 构建普通代币交换所需的交易参数和PTB指令。
//     -   FlowX的交换函数（如 `swap_exact_input`）需要较多参数，包括池注册表、手续费、最小期望输出（滑点保护）、价格限制（也是滑点保护）和交易截止时间（防止交易长时间悬挂）。
//
// 6.  **闪电贷相关方法 (Flashloan Methods)**:
//     -   虽然 `support_flashloan()` 的注释提到可能返回 `false`（但在代码中已改为 `true`），但文件内包含了完整的闪电贷实现逻辑。
//     -   `build_flashloan_args()`: 准备调用FlowX的 `pool::swap` 函数（这个函数同时用于常规交换和闪电贷的借出步骤）发起闪电贷时需要的参数。
//     -   `build_repay_args()`: 准备调用FlowX的 `pool::pay` 函数偿还闪电贷时需要的参数。
//     -   `extend_flashloan_tx()`: 实现了 `Dex` 接口，将发起FlowX闪电贷的指令添加到PTB中。它会先调用 `borrow_mut_pool` 从池注册表获取一个可变的池对象引用。
//     -   `extend_repay_tx()`: 实现了 `Dex` 接口，将偿还FlowX闪电贷的指令添加到PTB中。
//     -   `borrow_mut_pool()`: 一个内部辅助函数，用于从 `PoolRegistry` 中“借用”出一个可变的池对象引用。这在执行某些需要修改池状态的操作（如闪电贷的 `pool::swap` 或 `pool::pay`）时是必需的。
//
// 7.  **`Dex` trait 实现**:
//     -   `FlowxClmm` 结构体同样实现了 `Dex` 接口要求的其他方法。
//
// **Sui区块链和DeFi相关的概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **CLMM (Concentrated Liquidity Market Maker / 集中流动性做市商)**:
//     与Cetus和Kriya CLMM文件中的解释相同。
//     (Same explanation as in the Cetus and Kriya CLMM files.)
//
// -   **PoolRegistry (池注册表 / Pool Registry)**:
//     一个中心化的智能合约或对象，它维护了协议下所有（或某一类）交易池的列表和基本信息。
//     当需要与某个特定的池子交互时，可以先查询这个注册表来获取池子的地址（ObjectID）或其他元数据。
//     FlowX使用池注册表来管理其CLMM池。在执行某些操作（如闪电贷）时，可能需要先通过注册表“借用”出一个可变的池对象引用。
//     (A centralized smart contract or object that maintains a list and basic information of all (or a certain class of) trading pools under the protocol.
//      When needing to interact with a specific pool, one can first query this registry to get the pool's address (ObjectID) or other metadata.
//      FlowX uses a pool registry to manage its CLMM pools. For certain operations (like flash loans), it might be necessary to first "borrow" a mutable pool object reference from the registry.)
//
// -   **Versioned Object (版本化对象 / Versioned Object)**:
//     与Kriya CLMM文件中的解释类似。FlowX也可能使用一个全局的版本化对象来帮助管理其智能合约的升级路径或确保不同版本间的兼容性。
//     交易时可能需要引用这个对象作为参数，以表明当前操作是针对哪个协议版本或配置的。
//     (Similar explanation to the Kriya CLMM file. FlowX might also use a global versioned object to help manage its smart contract upgrade paths or ensure compatibility between different versions.
//      Transactions might need to reference this object as a parameter to indicate which protocol version or configuration the current operation is for.)
//
// -   **Deadline (截止时间 / Deadline)**:
//     在向DEX提交交易时，可以（有时是必须）指定一个“截止时间”参数。这是一个Unix时间戳。
//     如果这笔交易在达到这个时间点之前未能被Sui网络验证并包含在一个区块中（即“上链”），那么这笔交易就会自动失败或被视为无效。
//     这是一种保护措施，用来防止用户的交易因为网络拥堵或其他原因而长时间“卡住”或“悬挂”，最终在一个非常不利的市场条件下才被执行。
//     对于套利这种对时间高度敏感的操作来说，设置合理的截止时间非常重要。
//     (When submitting a transaction to a DEX, a "deadline" parameter can (and sometimes must) be specified. This is a Unix timestamp.
//      If the transaction fails to be validated by the Sui network and included in a block (i.e., "on-chain") before this time point is reached, the transaction will automatically fail or be considered invalid.
//      This is a protective measure to prevent a user's transaction from getting "stuck" or "pending" for a long time due to network congestion or other reasons, and eventually being executed under very unfavorable market conditions.
//      For time-sensitive operations like arbitrage, setting a reasonable deadline is very important.)
//
// -   **sqrt_price_limit (平方根价格限制 / Square Root Price Limit)**:
//     在CLMM池中进行交换时，用户通常可以指定一个“价格限制”。这个限制是以“价格的平方根”的形式表示的（因为CLMM内部常用sqrt(price)进行计算）。
//     它的作用是滑点控制。如果交易的执行会导致池子当前价格（的平方根）超出了这个用户设定的限制，那么交易可能会部分成交（只成交到价格限制为止的部分），或者完全失败，以防止用户在远差于预期的价格上进行交易。
//     例如，如果你在卖出代币A换取代币B，你可以设置一个最小的sqrt_price_limit，表示你愿意接受的A相对于B的最低价格（的平方根）。
//     (When swapping in a CLMM pool, users can usually specify a "price limit". This limit is expressed in the form of the "square root of the price" (as CLMMs often use sqrt(price) internally for calculations).
//      Its purpose is slippage control. If the execution of a trade would cause the pool's current price (or its square root) to exceed this user-set limit, the trade might be partially filled (only up to the price limit) or fail completely, preventing the user from trading at a price much worse than expected.
//      For example, if you are selling token A for token B, you can set a minimum sqrt_price_limit, representing the lowest price (or its square root) of A relative to B that you are willing to accept.)

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::{str::FromStr, sync::Arc}; // FromStr用于从字符串转换, Arc原子引用计数
                                   // (FromStr for string conversion, Arc for atomic reference counting)

use dex_indexer::types::{Pool, PoolExtra, Protocol}; // 从 `dex_indexer` 引入Pool, PoolExtra, Protocol类型
                                                    // (Import Pool, PoolExtra, Protocol types from `dex_indexer`)
use eyre::{bail, ensure, eyre, OptionExt, Result}; // 错误处理库 (Error handling library)
use move_core_types::annotated_value::MoveStruct; // Move核心类型 (Move core types)
use simulator::Simulator; // 交易模拟器接口 (Transaction simulator interface)
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型 (Sui basic types)
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建类型 (Sui transaction building types)
    Identifier, TypeTag, SUI_CLOCK_OBJECT_ID, // Sui标识符, 类型标签, 时钟对象ID (Sui Identifier, TypeTag, Clock Object ID)
};
use tokio::sync::OnceCell; // Tokio异步单次初始化单元 (Tokio asynchronous single initialization cell)
use utils::{
    coin, new_test_sui_client, // 自定义工具库: coin操作, 创建Sui客户端 (Custom utility library: coin operations, create Sui client)
    object::{extract_u128_from_move_struct, shared_obj_arg}, // 对象处理工具 (Object handling tools)
};

use super::{trade::FlashResult, TradeCtx}; // 从父模块(defi)引入 FlashResult, TradeCtx (Import FlashResult, TradeCtx from parent module (defi))
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 Dex trait (Import config and Dex trait from current crate)

// --- FlowX CLMM 协议相关的常量定义 ---
// (Constant definitions related to FlowX CLMM protocol)
// FlowX CLMM核心合约包ID (FlowX CLMM core contract package ID)
const FLOWX_CLMM: &str = "0x25929e7f29e0a30eb4e692952ba1b5b65a3a4d65ab5f2a32e1ba3edcb587f26d";
// FlowX 版本化对象ID (Versioned) (FlowX Versioned Object ID)
const VERSIONED: &str = "0x67624a1533b5aff5d0dfcf5e598684350efd38134d2d245f475524c03a64e656";
// FlowX 池注册表对象ID (PoolRegistry) (FlowX Pool Registry Object ID)
const POOL_REGISTRY: &str = "0x27565d24a4cd51127ac90e4074a841bbe356cca7bf5759ddc14a975be1632abc";

// 用于缓存 `ObjectArgs` 的静态 `OnceCell` (Static `OnceCell` for caching `ObjectArgs`)
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数 (获取对象参数函数 / Get Object Arguments Function)
///
/// 获取并缓存 `ObjectArgs` (包含pool_registry, versioned, clock)。
/// (Fetches and caches `ObjectArgs` (containing pool_registry, versioned, clock).)
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let pool_registry_id = ObjectID::from_hex_literal(POOL_REGISTRY).unwrap();
            let versioned_id = ObjectID::from_hex_literal(VERSIONED).unwrap();

            // 通过模拟器获取对象信息 (Fetch object information via simulator)
            let pool_registry_obj = simulator.get_object(&pool_registry_id).await.unwrap();
            let versioned_obj = simulator.get_object(&versioned_id).await.unwrap();
            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();

            ObjectArgs {
                pool_registry: shared_obj_arg(&pool_registry_obj, true), // PoolRegistry在交易中可能是可变的 (PoolRegistry might be mutable in transactions)
                versioned: shared_obj_arg(&versioned_obj, false),      // Versioned对象通常是不可变的 (Versioned object is usually immutable)
                clock: shared_obj_arg(&clock_obj, false),            // Clock是不可变的 (Clock is immutable)
            }
        })
        .await
        .clone()
}

/// `ObjectArgs` 结构体 (对象参数结构体 / Object Arguments Struct)
///
/// 缓存FlowX CLMM交互所需的关键对象的 `ObjectArg` 形式。
/// (Caches the `ObjectArg` form of key objects required for FlowX CLMM interaction.)
#[derive(Clone)]
pub struct ObjectArgs {
    pool_registry: ObjectArg, // 池注册表对象的ObjectArg (Pool Registry object's ObjectArg)
    versioned: ObjectArg,     // 版本化对象的ObjectArg (Versioned object's ObjectArg)
    clock: ObjectArg,         // Sui时钟对象的ObjectArg (Sui clock object's ObjectArg)
}

/// `FlowxClmm` 结构体 (FlowxClmm Struct)
///
/// 代表一个FlowX CLMM协议的交易池。
/// (Represents a trading pool of the FlowX CLMM protocol.)
#[derive(Clone)]
pub struct FlowxClmm {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息 (Original pool information from `dex_indexer`)
    liquidity: u128,         // 池的流动性 (CLMM中流动性概念复杂，这里可能是总流动性或特定范围的)
                             // (Pool's liquidity (liquidity concept in CLMM is complex, this might be total liquidity or for a specific range))
    coin_in_type: String,    // 当前交易方向的输入代币类型 (Input coin type for the current trading direction)
    coin_out_type: String,   // 当前交易方向的输出代币类型 (Output coin type for the current trading direction)
    fee: u64,                // 池的交易手续费率 (例如，500表示0.05%) (Pool's trading fee rate (e.g., 500 for 0.05%))
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是[CoinInType, CoinOutType])
                              // (Generic type parameters needed when calling the contract (usually [CoinInType, CoinOutType]))
    // 共享的对象参数 (Shared object parameters)
    pool_registry: ObjectArg,
    versioned: ObjectArg,
    clock: ObjectArg,
}

impl FlowxClmm {
    /// `new` 构造函数 (new constructor)
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `FlowxClmm` DEX实例。
    /// (Creates a `FlowxClmm` DEX instance based on `Pool` information provided by `dex_indexer` and the input coin type.)
    ///
    /// 参数 (Parameters):
    /// - `simulator`: 共享的模拟器实例。(Shared simulator instance.)
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。(Pool information from `dex_indexer` (`&Pool`).)
    /// - `coin_in_type`: 输入代币的类型字符串。(Type string of the input coin.)
    ///
    /// 返回 (Returns):
    /// - `Result<Self>`: 成功则返回 `FlowxClmm` 实例，否则返回错误。(Returns a `FlowxClmm` instance if successful, otherwise an error.)
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        ensure!(pool_info.protocol == Protocol::FlowxClmm, "提供的不是FlowX CLMM协议的池 (Provided pool is not of FlowX CLMM protocol)");

        let pool_obj = simulator
            .get_object(&pool_info.pool)
            .await
            .ok_or_else(|| eyre!("FlowX CLMM池对象未找到: {} (FlowX CLMM pool object not found: {})", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("FlowX CLMM池对象的布局(layout)未找到 (Layout for FlowX CLMM pool object not found)")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象 (Object is not a Move object)")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        let liquidity = extract_u128_from_move_struct(&parsed_pool_struct, "liquidity")?;

        let coin_out_type = if let Some(0) = pool_info.token_index(coin_in_type) { // 如果输入代币是池中的token0
            pool_info.token1_type() // 则输出代币是token1
        } else { // 否则输入代币是token1
            pool_info.token0_type() // 则输出代币是token0
        };

        let fee = if let PoolExtra::FlowxClmm { fee_rate } = pool_info.extra { // 从PoolExtra中获取手续费率
            fee_rate // fee_rate 例如 500 代表 0.05% (500 / 1_000_000)
        } else {
            bail!("FlowX CLMM池信息中缺少有效的手续费率(fee_rate) (Missing valid fee_rate in FlowX CLMM pool info)");
        };

        let type_params = vec![ // 构建泛型参数列表 [CoinInType, CoinOutType]
            TypeTag::from_str(coin_in_type).map_err(|e| eyre!(e))?,
            TypeTag::from_str(&coin_out_type).map_err(|e| eyre!(e))?,
        ];

        let ObjectArgs { pool_registry, versioned, clock } = get_object_args(simulator).await; // 获取共享对象参数

        Ok(Self {
            pool: pool_info.clone(), liquidity,
            coin_in_type: coin_in_type.to_string(), coin_out_type,
            fee, type_params,
            pool_registry, versioned, clock,
        })
    }

    /// `build_swap_tx` (私有辅助函数 / Private helper function)
    #[allow(dead_code)]
    async fn build_swap_tx(
        &self, sender: SuiAddress, recipient: SuiAddress,
        coin_in_ref: ObjectRef, amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default();
        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?; // None for amount_in as swap_exact_input takes full coin
        ctx.transfer_arg(recipient, coin_out_arg);
        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数 / Private helper function)
    /// 构建调用FlowX CLMM常规交换方法 (`swap_exact_input`) 所需的参数。
    /// (Builds arguments for FlowX CLMM's `swap_exact_input` method.)
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_registry_arg = ctx.obj(self.pool_registry).map_err(|e| eyre!(e))?;
        let fee_arg = ctx.pure(self.fee).map_err(|e| eyre!(e))?;
        let amount_out_min_arg = ctx.pure(0u64).map_err(|e| eyre!(e))?; // 通常应计算滑点保护 (Usually should calculate slippage protection)

        let sqrt_price_limit_val = if self.is_a2b() { // 根据方向设置价格限制 (Set price limit based on direction)
            MIN_SQRT_PRICE_X64 + 1 // 防止价格过低 (Prevent price too low)
        } else {
            MAX_SQRT_PRICE_X64 - 1 // 防止价格过高 (Prevent price too high)
        };
        let sqrt_price_limit_arg = ctx.pure(sqrt_price_limit_val).map_err(|e| eyre!(e))?;

        let deadline_val = std::time::SystemTime::now() // 设置交易截止时间 (Set transaction deadline)
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64 + 18000; // 当前时间 + 18秒
        let deadline_arg = ctx.pure(deadline_val).map_err(|e| eyre!(e))?;

        let versioned_arg = ctx.obj(self.versioned).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        Ok(vec![
            pool_registry_arg, fee_arg, coin_in_arg, amount_out_min_arg,
            sqrt_price_limit_arg, deadline_arg, versioned_arg, clock_arg,
        ])
    }

    /// `build_flashloan_args` (私有辅助函数 / Private helper function)
    /// 构建调用FlowX CLMM闪电贷 (`pool::swap`) 所需的参数。
    /// (Builds arguments for FlowX CLMM's flash loan (`pool::swap`) method.)
    fn build_flashloan_args(&self, ctx: &mut TradeCtx, pool_arg: Argument, amount_in: u64) -> Result<Vec<Argument>> {
        let a2b_arg = ctx.pure(self.is_a2b()).map_err(|e| eyre!(e))?;
        let by_amount_in_arg = ctx.pure(true).map_err(|e| eyre!(e))?; // 总是按输入数量借贷 (Always borrow by input amount)
        let amount_arg = ctx.pure(amount_in).map_err(|e| eyre!(e))?;

        let sqrt_price_limit_val = if self.is_a2b() { MIN_SQRT_PRICE_X64 + 1 } else { MAX_SQRT_PRICE_X64 - 1 };
        let sqrt_price_limit_arg = ctx.pure(sqrt_price_limit_val).map_err(|e| eyre!(e))?;

        let versioned_arg = ctx.obj(self.versioned).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        Ok(vec![ // 参数顺序：pool, a2b, by_amount_in, amount, sqrt_price_limit, versioned, clock
            pool_arg, a2b_arg, by_amount_in_arg, amount_arg,
            sqrt_price_limit_arg, versioned_arg, clock_arg,
        ])
    }

    /// `build_repay_args` (私有辅助函数 / Private helper function)
    /// 构建调用FlowX CLMM偿还闪电贷 (`pool::pay`) 所需的参数。
    /// (Builds arguments for FlowX CLMM's flash loan repayment (`pool::pay`) method.)
    fn build_repay_args(
        &self, ctx: &mut TradeCtx, pool_arg: Argument,
        coin_to_repay_arg: Argument, receipt_arg: Argument,
    ) -> Result<Vec<Argument>> {
        let (balance_a_arg, balance_b_arg) = if self.is_a2b() { // 根据借贷方向准备Balance参数
            (ctx.coin_into_balance(coin_to_repay_arg, self.type_params[0].clone())?, ctx.balance_zero(self.type_params[1].clone())?)
        } else {
            (ctx.balance_zero(self.type_params[0].clone())?, ctx.coin_into_balance(coin_to_repay_arg, self.type_params[1].clone())?)
        };
        let versioned_arg = ctx.obj(self.versioned).map_err(|e| eyre!(e))?;
        // 参数顺序：pool, receipt, balance_a, balance_b, versioned
        Ok(vec![pool_arg, receipt_arg, balance_a_arg, balance_b_arg, versioned_arg])
    }

    /// `borrow_mut_pool` (私有辅助函数 / Private helper function)
    /// 调用 `pool_manager::borrow_mut_pool` 获取可变的池对象引用。
    /// (Calls `pool_manager::borrow_mut_pool` to get a mutable pool object reference.)
    fn borrow_mut_pool(&self, ctx: &mut TradeCtx) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(FLOWX_CLMM)?;
        let module_name = Identifier::new("pool_manager").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new("borrow_mut_pool").map_err(|e| eyre!(e))?;
        let type_arguments = self.type_params.clone(); // [CoinInType, CoinOutType] (当前交易方向)
        let call_arguments = {
            let pool_registry_arg = ctx.obj(self.pool_registry).map_err(|e| eyre!(e))?;
            let fee_arg = ctx.pure(self.fee).map_err(|e| eyre!(e))?; // 池的费率
            vec![pool_registry_arg, fee_arg]
        };
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));
        Ok(Argument::Result(ctx.last_command_idx())) // 返回可变的Pool引用
    }
}

/// 为 `FlowxClmm` 结构体实现 `Dex` trait。(Implement `Dex` trait for `FlowxClmm` struct.)
#[async_trait::async_trait]
impl Dex for FlowxClmm {
    /// `support_flashloan` 方法 (support_flashloan method)
    fn support_flashloan(&self) -> bool {
        true // 假设FlowX通过pool::swap和pool::pay支持闪电贷 (Assuming FlowX supports flash loans via pool::swap and pool::pay)
    }

    /// `extend_flashloan_tx` (将发起FlowX闪电贷的操作添加到PTB / Add FlowX flash loan initiation op to PTB)
    async fn extend_flashloan_tx(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<FlashResult> {
        let mutable_pool_arg = self.borrow_mut_pool(ctx)?; // 获取可变的池对象

        let package_id = ObjectID::from_hex_literal(FLOWX_CLMM)?;
        let module_name = Identifier::new("pool").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new("swap").map_err(|e| eyre!(e))?; // 闪电贷借出也通过pool::swap
        let type_arguments = self.type_params.clone(); // [CoinInType, CoinOutType] (当前交易方向)
        let call_arguments = self.build_flashloan_args(ctx, mutable_pool_arg.clone(), amount_in)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        // pool::swap 返回 (Balance<T0>, Balance<T1>, SwapReceipt)
        // T0是type_params[0] (当前coin_in_type), T1是type_params[1] (当前coin_out_type)
        // 闪电贷借入的是 coin_in_type (即T0)。`pool::swap` 会消耗这个T0，并返回T1。
        // 我们需要的是原始借入的T0代币。
        // **修正逻辑**: FlowX的 `pool::swap` 用于闪电贷时，它实际上是“借入A，用A换B，返回B和A的债务回执”。
        // 所以 `coin_out` 应该是交换后得到的币。
        let balance_t0_arg = Argument::NestedResult(last_idx, 0); // Balance<CoinInType>
        let balance_t1_arg = Argument::NestedResult(last_idx, 1); // Balance<CoinOutType>
        let receipt_arg = Argument::NestedResult(last_idx, 2);

        // 根据 is_a2b (实际是 coin_in_type == type_params[0]) 来确定哪个是借入的，哪个是交换得到的
        // 如果 self.is_a2b() (当前交易方向是池的T0->T1)，那么我们借的是T0，得到的是T1。
        // FlashResult.coin_out 应该是我们实际得到的用于后续交易的币。
        let (borrowed_coin_balance_arg, received_coin_balance_arg, received_coin_type_tag) = if self.is_a2b() {
            (balance_t0_arg, balance_t1_arg, self.type_params[1].clone())
        } else { // coin_in_type是池的T1 (T1->T0)，借T1，得到T0
            (balance_t1_arg, balance_t0_arg, self.type_params[0].clone())
        };

        // 销毁对应借入代币的那个Balance (因为它在swap中被消耗了)
        let borrowed_coin_type_tag = if self.is_a2b() { self.type_params[0].clone() } else { self.type_params[1].clone() };
        ctx.balance_destroy_zero(borrowed_coin_balance_arg, borrowed_coin_type_tag)?;
        // 将交换后得到的Balance转换为Coin对象
        let final_coin_out = ctx.coin_from_balance(received_coin_balance_arg, received_coin_type_tag)?;

        Ok(FlashResult {
            coin_out: final_coin_out, // 闪电贷借入并交换后得到的币
            receipt: receipt_arg,
            pool: Some(mutable_pool_arg), // 保存可变池的引用，用于偿还
        })
    }

    /// `extend_repay_tx` (将偿还FlowX闪电贷的操作添加到PTB / Add FlowX flash loan repayment op to PTB)
    async fn extend_repay_tx(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, flash_res: FlashResult) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(FLOWX_CLMM)?;
        let module_name = Identifier::new("pool").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new("pay").map_err(|e| eyre!(e))?;
        let type_arguments = self.type_params.clone(); // [CoinInType, CoinOutType] (偿还时方向与借时一致)
        let receipt_arg = flash_res.receipt;
        let mutable_pool_arg = flash_res.pool.ok_or_eyre("FlowX偿还闪电贷时缺少池对象引用 (Missing pool object reference for FlowX flash loan repayment)")?;

        let call_arguments = self.build_repay_args(ctx, mutable_pool_arg.clone(), coin_to_repay_arg, receipt_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        // `pool::pay` 函数没有返回值。
        // 我们需要返回一个表示操作完成的 `Argument`。
        // 返回可变池的引用，因为 `pay` 函数会修改它，后续可能需要基于这个修改后的池状态做判断或操作（尽管在当前套利流程中可能不直接使用）。
        Ok(mutable_pool_arg)
    }

    /// `extend_trade_tx` (常规交换 / Regular Swap)
    async fn extend_trade_tx(
        &self, ctx: &mut TradeCtx, _sender: SuiAddress,
        coin_in_arg: Argument, _amount_in: Option<u64>,
    ) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(FLOWX_CLMM)?;
        let module_name = Identifier::new("swap_router").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new("swap_exact_input").map_err(|e| eyre!(e))?;
        let type_arguments = self.type_params.clone(); // [CoinInType, CoinOutType]
        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));
        Ok(Argument::Result(ctx.last_command_idx()))
    }

    // --- Dex trait 的其他 getter 和 setter 方法 ---
    // (Other getter and setter methods for Dex trait)
    fn coin_in_type(&self) -> String { self.coin_in_type.clone() }
    fn coin_out_type(&self) -> String { self.coin_out_type.clone() }
    fn protocol(&self) -> Protocol { Protocol::FlowxClmm }
    fn liquidity(&self) -> u128 { self.liquidity }
    fn object_id(&self) -> ObjectID { self.pool.pool }

    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        self.type_params.reverse(); // 因为泛型参数是 [CoinInType, CoinOutType]
    }
    fn is_a2b(&self) -> bool { // 判断当前 coin_in_type 是否是池的 token0
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试 / Mainly for testing)
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await;
        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;
        let pt = self.build_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in).await?;
        let gas_coins = coin::get_gas_coin_refs(&sui_client, sender, Some(coin_in_obj.coin_object_id)).await?;
        let gas_price = sui_client.read_api().get_reference_gas_price().await?;
        Ok(TransactionData::new_programmable(sender, gas_coins, pt, GAS_BUDGET, gas_price))
    }
}

// --- 测试模块 ---
// (Test module)
#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use itertools::Itertools;
    use object_pool::ObjectPool;
    use simulator::{DBSimulator, HttpSimulator, Simulator};
    use tracing::info;
    use super::*;
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL},
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher},
    };

    /// `test_flowx_swap_tx` 测试函数 (test_flowx_swap_tx test function)
    #[tokio::test]
    async fn test_flowx_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);
        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap();
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI";
        let token_out_type = "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN"; // Wormhole USDC
        let amount_in = 10000; // 0.00001 SUI

        let simulator_pool_for_searcher = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(DBSimulator::new_test(true).await) as Box<dyn Simulator> })
        }));

        let searcher = IndexerDexSearcher::new(TEST_HTTP_URL, simulator_pool_for_searcher).await.unwrap();
        let dexes = searcher
            .find_dexes(token_in_type, Some(token_out_type.into()))
            .await
            .unwrap();
        info!("🧀 找到的DEX数量 (Number of DEXs found): {}", dexes.len());

        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::FlowxClmm)
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity()))
            .last()
            .expect("测试中未找到FlowX CLMM的池 (FlowX CLMM pool not found in test)");

        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 构建的交易数据 (Constructed transaction data): {:?}", tx_data);

        let response = http_simulator.simulate(tx_data, Default::default()).await.unwrap();
        info!("🧀 模拟结果 (Simulation result): {:?}", response);

        assert!(response.is_ok(), "交易模拟应成功 (Transaction simulation should succeed)");
    }
}

[end of bin/arb/src/defi/flowx_clmm.rs]
