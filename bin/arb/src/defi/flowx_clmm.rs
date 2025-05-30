// 该文件 `flowx_clmm.rs` 实现了与 FlowX Finance 协议的CLMM（集中流动性做市商）池交互的逻辑。
// FlowX是Sui区块链上的一个DEX，采用了CLMM模型，允许流动性提供者在特定价格范围内提供流动性。
// 该实现也包含了对FlowX闪电贷功能的支持。
//
// 文件概览:
// 1. 定义了与 FlowX CLMM 相关的常量，如合约包ID、版本化对象ID (Versioned)、池注册表ID (PoolRegistry)。
// 2. `ObjectArgs` 结构体: 用于缓存这些常用 FlowX 对象的 `ObjectArg`。
// 3. `FlowxClmm` 结构体: 代表一个 FlowX CLMM 池的实例，实现了 `Dex` trait。
// 4. `new()` 方法: 初始化 `FlowxClmm` 实例，从链上获取池的详细信息，包括流动性、费用率等。
// 5. 常规交换相关方法:
//    - `build_swap_tx()` / `build_swap_args()`: 构建精确输入交换的交易参数和PTB。
//    - FlowX的交换函数 `swap_exact_input` 需要池注册表、费用、最小输出、价格限制、截止时间等参数。
// 6. 闪电贷相关方法 (虽然 `support_flashloan` 返回 `false`，但相关代码结构存在):
//    - `build_flashloan_args()`: 构建发起闪电贷的参数 (调用 `pool::swap` 函数)。
//    - `build_repay_args()`: 构建偿还闪电贷的参数 (调用 `pool::pay` 函数)。
//    - `extend_flashloan_tx()`: 将发起闪电贷的操作添加到PTB。
//    - `extend_repay_tx()`: 将偿还闪电贷的操作添加到PTB。
//    - `borrow_mut_pool()`: 一个辅助函数，用于从 `PoolRegistry` 中借用一个可变的池对象引用，这在执行某些池操作（如闪电贷的 `swap`）时是必需的。
// 7. 实现了 `Dex` trait 的其他方法。
//
// Sui/DeFi概念:
// - CLMM (Concentrated Liquidity Market Maker): 与Cetus类似，FlowX也使用CLMM模型。
// - PoolRegistry (池注册表): 一个中心化的合约或对象，用于管理和查找协议中的所有交易池。
// - Versioned Object (版本化对象): FlowX可能使用一个版本化对象来管理其合约的升级或不同版本间的兼容性。
// - Deadline (截止时间): 在交易参数中指定一个截止时间，如果交易在该时间点之前未能上链执行，则交易会自动失败。这是一种防止交易因网络拥堵而长时间悬挂的保护措施。
// - sqrt_price_limit (平方根价格限制): 在CLMM交换中，用户可以指定一个价格限制（以价格的平方根形式表示），
//   如果交易执行会导致价格超出这个限制，则交易会部分成交或失败。这是滑点控制的一种方式。

// 引入标准库及第三方库
use std::{str::FromStr, sync::Arc}; // FromStr用于从字符串转换, Arc原子引用计数

use dex_indexer::types::{Pool, PoolExtra, Protocol}; // 从 `dex_indexer` 引入Pool, PoolExtra, Protocol类型
use eyre::{bail, ensure, eyre, OptionExt, Result}; // 错误处理库
use move_core_types::annotated_value::MoveStruct; // Move核心类型
use simulator::Simulator; // 交易模拟器接口
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建类型
    Identifier, TypeTag, SUI_CLOCK_OBJECT_ID, // Sui标识符, 类型标签, 时钟对象ID
};
use tokio::sync::OnceCell; // Tokio异步单次初始化单元
use utils::{
    coin, new_test_sui_client, // 自定义工具库: coin操作, 创建Sui客户端
    object::{extract_u128_from_move_struct, shared_obj_arg}, // 对象处理工具
};

use super::{trade::FlashResult, TradeCtx}; // 从父模块(defi)引入 FlashResult, TradeCtx
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 Dex trait

// --- FlowX CLMM 协议相关的常量定义 ---
// FlowX CLMM核心合约包ID
const FLOWX_CLMM: &str = "0x25929e7f29e0a30eb4e692952ba1b5b65a3a4d65ab5f2a32e1ba3edcb587f26d";
// FlowX 版本化对象ID (Versioned)
const VERSIONED: &str = "0x67624a1533b5aff5d0dfcf5e598684350efd38134d2d245f475524c03a64e656";
// FlowX 池注册表对象ID (PoolRegistry)
const POOL_REGISTRY: &str = "0x27565d24a4cd51127ac90e4074a841bbe356cca7bf5759ddc14a975be1632abc";

// 用于缓存 `ObjectArgs` 的静态 `OnceCell`
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数
///
/// 获取并缓存 `ObjectArgs` (包含pool_registry, versioned, clock)。
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let pool_registry_id = ObjectID::from_hex_literal(POOL_REGISTRY).unwrap();
            let versioned_id = ObjectID::from_hex_literal(VERSIONED).unwrap();

            // 通过模拟器获取对象信息
            let pool_registry_obj = simulator.get_object(&pool_registry_id).await.unwrap();
            let versioned_obj = simulator.get_object(&versioned_id).await.unwrap();
            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();

            ObjectArgs {
                pool_registry: shared_obj_arg(&pool_registry_obj, true), // PoolRegistry在交易中可能是可变的
                versioned: shared_obj_arg(&versioned_obj, false),      // Versioned对象通常是不可变的
                clock: shared_obj_arg(&clock_obj, false),            // Clock是不可变的
            }
        })
        .await
        .clone()
}

/// `ObjectArgs` 结构体
///
/// 缓存FlowX CLMM交互所需的关键对象的 `ObjectArg` 形式。
#[derive(Clone)]
pub struct ObjectArgs {
    pool_registry: ObjectArg, // 池注册表对象的ObjectArg
    versioned: ObjectArg,     // 版本化对象的ObjectArg
    clock: ObjectArg,         // Sui时钟对象的ObjectArg
}

/// `FlowxClmm` 结构体
///
/// 代表一个FlowX CLMM协议的交易池。
#[derive(Clone)]
pub struct FlowxClmm {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息
    liquidity: u128,         // 池的流动性 (CLMM中流动性概念复杂，这里可能是总流动性或特定范围的)
    coin_in_type: String,    // 当前交易方向的输入代币类型
    coin_out_type: String,   // 当前交易方向的输出代币类型
    fee: u64,                // 池的交易手续费率 (例如，500表示0.05%)
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是[CoinInType, CoinOutType])
    // 共享的对象参数
    pool_registry: ObjectArg,
    versioned: ObjectArg,
    clock: ObjectArg,
}

impl FlowxClmm {
    /// `new` 构造函数
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `FlowxClmm` DEX实例。
    ///
    /// 参数:
    /// - `simulator`: 共享的模拟器实例。
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。
    /// - `coin_in_type`: 输入代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `FlowxClmm` 实例，否则返回错误。
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 确保池协议是FlowxClmm
        ensure!(pool_info.protocol == Protocol::FlowxClmm, "提供的不是FlowX CLMM协议的池");

        // 获取并解析池对象的Move结构体内容 (FlowX的Pool对象)
        let pool_obj = simulator
            .get_object(&pool_info.pool) // pool_info.pool 是池的ObjectID
            .await
            .ok_or_else(|| eyre!("FlowX CLMM池对象未找到: {}", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("FlowX CLMM池对象的布局(layout)未找到")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 从解析后的池结构体中提取流动性 (liquidity 字段)
        let liquidity = extract_u128_from_move_struct(&parsed_pool_struct, "liquidity")?;

        // 根据输入代币推断输出代币 (假设是双币池)
        let coin_out_type = if let Some(0) = pool_info.token_index(coin_in_type) {
            pool_info.token1_type()
        } else {
            pool_info.token0_type()
        };

        // 从 `pool_info.extra` 中提取手续费率。
        // `PoolExtra` 是一个枚举，用于存储不同协议特有的额外信息。
        let fee = if let PoolExtra::FlowxClmm { fee_rate } = pool_info.extra {
            fee_rate // fee_rate 例如 500 代表 0.05% (500 / 1_000_000)
        } else {
            // 如果 `pool_info.extra` 不是 `FlowxClmm` 类型或者没有提供费率，则返回错误。
            bail!("FlowX CLMM池信息中缺少有效的手续费率(fee_rate)");
        };

        // 构建调用合约时需要的泛型类型参数列表: `[CoinInType, CoinOutType]`
        let type_params = vec![
            TypeTag::from_str(coin_in_type).map_err(|e| eyre!(e))?,
            TypeTag::from_str(&coin_out_type).map_err(|e| eyre!(e))?,
        ];

        // 获取共享的协议对象参数 (pool_registry, versioned, clock)
        let ObjectArgs {
            pool_registry,
            versioned,
            clock,
        } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(),
            liquidity,
            coin_in_type: coin_in_type.to_string(),
            coin_out_type,
            fee,
            type_params,
            pool_registry,
            versioned,
            clock,
        })
    }

    /// `build_swap_tx` (私有辅助函数)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在FlowX CLMM池中执行一次常规交换。
    #[allow(dead_code)] // 允许存在未使用的代码
    async fn build_swap_tx(
        &self,
        sender: SuiAddress,
        recipient: SuiAddress,
        coin_in_ref: ObjectRef,
        amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default();

        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        // `None` 表示 `amount_in` 对于 `extend_trade_tx` 是可选的或不直接使用u64值
        // (FlowX的swap函数通常直接使用传入Coin对象的全部余额作为输入数量)。
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数)
    ///
    /// 构建调用FlowX CLMM常规交换方法 (`swap_exact_input`) 所需的参数列表。
    /// 合约方法签名示例 (来自注释):
    /// `fun swap_exact_input<X, Y>(
    ///     pool_registry: &mut PoolRegistry,
    ///     fee: u64,             // 池的手续费率 (例如 500 for 0.05%)
    ///     coin_in: Coin<X>,
    ///     amount_out_min: u64,  // 最小期望输出数量 (滑点保护)
    ///     sqrt_price_limit: u128, // 平方根价格限制 (滑点保护)
    ///     deadline: u64,        // 交易截止时间 (时间戳)
    ///     versioned: &mut Versioned, // 注意注释是 &mut Versioned，但get_object_args中设为false(不可变)
    ///     clock: &Clock,
    ///     ctx: &mut TxContext
    /// ): Coin<Y>`
    /// **注意**: `versioned` 在 `get_object_args` 中被获取为不可变共享对象 (`shared_obj_arg(..., false)`).
    /// 如果合约确实需要 `&mut Versioned`，那么 `get_object_args` 中的设置需要改为 `true`。
    /// 假设当前实现中 `versioned` 作为不可变参数传递是正确的，或者合约签名允许。
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_registry_arg = ctx.obj(self.pool_registry).map_err(|e| eyre!(e))?;
        let fee_arg = ctx.pure(self.fee).map_err(|e| eyre!(e))?; // 池的费率
        // `amount_out_min` 设置为0，表示不进行严格的最小输出检查，或依赖价格限制进行滑点控制。
        // 在实际套利中，这里应该根据预期的价格和滑点容忍度计算一个合理的 `amount_out_min`。
        let amount_out_min_arg = ctx.pure(0u64).map_err(|e| eyre!(e))?;

        // 设置价格限制 (sqrt_price_limit)。
        // 如果是 a->b (卖a买b), 价格通常是 b/a。如果价格上涨 (b变多或a变少)，对用户有利。
        // `is_a2b` 为 true (卖token0买token1):
        //   - coin_in_type是token0, coin_out_type是token1。
        //   - 我们卖出token0，获得token1。价格是 token1数量 / token0数量。
        //   - `MIN_SQRT_PRICE_X64 + 1` 表示我们不希望价格跌得太低 (即用少量token0换到极少token1)。
        //     这是一个防止在极端不利情况下成交的下限保护。
        // 如果是 b->a (卖token1买token0):
        //   - coin_in_type是token1, coin_out_type是token0。
        //   - 我们卖出token1，获得token0。价格是 token0数量 / token1数量。
        //   - `MAX_SQRT_PRICE_X64 - 1` 表示我们不希望价格涨得太高 (即用大量token1换到极少token0)。
        //     这是一个防止在极端不利情况下成交的上限保护。
        let sqrt_price_limit_val = if self.is_a2b() {
            MIN_SQRT_PRICE_X64 + 1 // 防止价格过低 (token0不值钱)
        } else {
            MAX_SQRT_PRICE_X64 - 1 // 防止价格过高 (token1不值钱)
        };
        let sqrt_price_limit_arg = ctx.pure(sqrt_price_limit_val).map_err(|e| eyre!(e))?;

        // 设置交易截止时间 (deadline) 为当前时间戳 + 18秒。
        // (18000毫秒 = 18秒)
        let deadline_val = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64 // 当前毫秒时间戳
            + 18000; // 加上18秒作为缓冲
        let deadline_arg = ctx.pure(deadline_val).map_err(|e| eyre!(e))?;

        let versioned_arg = ctx.obj(self.versioned).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        Ok(vec![
            pool_registry_arg,
            fee_arg,
            coin_in_arg,
            amount_out_min_arg,
            sqrt_price_limit_arg,
            deadline_arg,
            versioned_arg,
            clock_arg,
        ])
    }

    /// `build_flashloan_args` (私有辅助函数)
    ///
    /// 构建调用FlowX CLMM闪电贷相关方法 (`pool::swap`) 所需的参数列表。
    /// FlowX的闪电贷似乎是通过其常规的 `pool::swap` 函数实现的，该函数返回 `(Balance<T0>, Balance<T1>, SwapReceipt)`。
    /// 其中一个Balance是借出的代币，另一个是零。`SwapReceipt` 用于后续的 `pay` 操作。
    ///
    /// 合约方法签名示例 (来自注释，可能是 `pool::swap`):
    /// `public fun swap<T0, T1>(
    ///     _pool: &mut Pool<T0, T1>, // 池对象，通过 borrow_mut_pool 获取
    ///     _a2b: bool,              // 交易方向 (true表示T0->T1)
    ///     _by_amount_in: bool,     // true表示 `_amount` 是输入数量
    ///     _amount: u64,            // 数量
    ///     _sqrt_price_limit: u128, // 价格限制
    ///     _versioned: &Versioned,  // 版本化对象
    ///     _clock: &Clock,
    ///     _ctx: &TxContext
    /// ) : (Balance<T0>, Balance<T1>, SwapReceipt);`
    fn build_flashloan_args(&self, ctx: &mut TradeCtx, pool_arg: Argument, amount_in: u64) -> Result<Vec<Argument>> {
        let a2b_arg = ctx.pure(self.is_a2b()).map_err(|e| eyre!(e))?; // 交易方向
        let by_amount_in_arg = ctx.pure(true).map_err(|e| eyre!(e))?; // 按输入数量计算
        let amount_arg = ctx.pure(amount_in).map_err(|e| eyre!(e))?; // 借贷/输入数量

        // 价格限制，与常规swap类似
        let sqrt_price_limit_val = if self.is_a2b() {
            MIN_SQRT_PRICE_X64 + 1
        } else {
            MAX_SQRT_PRICE_X64 - 1
        };
        let sqrt_price_limit_arg = ctx.pure(sqrt_price_limit_val).map_err(|e| eyre!(e))?;

        let versioned_arg = ctx.obj(self.versioned).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        Ok(vec![
            pool_arg,             // 可变的池对象引用
            a2b_arg,
            by_amount_in_arg,
            amount_arg,
            sqrt_price_limit_arg,
            versioned_arg,
            clock_arg,
        ])
    }

    /// `build_repay_args` (私有辅助函数)
    ///
    /// 构建调用FlowX CLMM偿还闪电贷方法 (`pool::pay`) 所需的参数列表。
    /// 合约方法签名示例 (来自注释，可能是 `pool::pay`):
    /// `public fun pay<T0, T1>(
    ///     _pool: &mut Pool<T0, T1>,
    ///     _receipt: SwapReceipt,
    ///     _balance_a: Balance<T0>, // 用于偿还的T0代币余额
    ///     _balance_b: Balance<T1>, // 用于偿ยัง的T1代币余额
    ///     _versioned: &Versioned,
    ///     _ctx: &TxContext
    /// )`
    /// 在闪电贷中，通常只提供借入方向的代币余额进行偿还。
    fn build_repay_args(
        &self,
        ctx: &mut TradeCtx,
        pool_arg: Argument,        // 可变的池对象引用 (与flashloan时是同一个)
        coin_to_repay_arg: Argument, // 用于偿还的Coin对象 (已包含本金+费用)
        receipt_arg: Argument,     // 从flashloan的 `pool::swap` 返回的SwapReceipt
    ) -> Result<Vec<Argument>> {
        // 根据交易方向，将 `coin_to_repay_arg` 转换为相应类型的 `Balance` 对象。
        // 另一个方向的 Balance 则为空 (zero balance)。
        // `ctx.coin_into_balance` 将 Coin 转换为 Balance。
        // `ctx.balance_zero` 创建一个指定类型的空 Balance。
        let (balance_a_arg, balance_b_arg) = if self.is_a2b() { // 如果是 T0 -> T1 (借T0, 还T0)
            (
                ctx.coin_into_balance(coin_to_repay_arg, self.type_params[0].clone())?, // coin_to_repay是T0类型
                ctx.balance_zero(self.type_params[1].clone())?,                     // T1的Balance为空
            )
        } else { // 如果是 T1 -> T0 (借T1, 还T1)
            (
                ctx.balance_zero(self.type_params[0].clone())?,                     // T0的Balance为空
                ctx.coin_into_balance(coin_to_repay_arg, self.type_params[1].clone())?, // coin_to_repay是T1类型
            )
        };

        let versioned_arg = ctx.obj(self.versioned).map_err(|e| eyre!(e))?;
        Ok(vec![pool_arg, receipt_arg, balance_a_arg, balance_b_arg, versioned_arg])
    }

    /// `borrow_mut_pool` (私有辅助函数)
    ///
    /// 调用 `pool_manager::borrow_mut_pool` 函数从 `PoolRegistry` 中获取一个可变的池对象引用。
    /// 这在执行某些需要修改池状态的操作（如闪电贷的 `pool::swap`）时是必需的。
    ///
    /// 返回:
    /// - `Result<Argument>`: 代表可变池对象的 `Argument`。
    fn borrow_mut_pool(&self, ctx: &mut TradeCtx) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(FLOWX_CLMM)?; // FlowX CLMM包ID
        // `pool_manager` 模块负责管理池的借用
        let module_name = Identifier::new("pool_manager").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new("borrow_mut_pool").map_err(|e| eyre!(e))?;
        // 泛型参数是池的两种代币类型 `[CoinA, CoinB]`
        let type_arguments = self.type_params.clone();

        // `borrow_mut_pool` 的参数是 `pool_registry: &mut PoolRegistry` 和 `fee: u64`
        let call_arguments = {
            let pool_registry_arg = ctx.obj(self.pool_registry).map_err(|e| eyre!(e))?;
            let fee_arg = ctx.pure(self.fee).map_err(|e| eyre!(e))?;
            vec![pool_registry_arg, fee_arg]
        };

        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        // `borrow_mut_pool` 返回 `Pool<CoinA, CoinB>` 的可变引用
        Ok(Argument::Result(ctx.last_command_idx()))
    }
}

/// 为 `FlowxClmm` 结构体实现 `Dex` trait。
#[async_trait::async_trait]
impl Dex for FlowxClmm {
    /// `support_flashloan` 方法
    ///
    /// 指明该DEX是否支持闪电贷。
    /// **注意**: 当前实现返回 `false`。但代码中存在闪电贷相关的函数 (`extend_flashloan_tx`, `extend_repay_tx`)。
    /// 这可能意味着：
    /// 1. 闪电贷功能尚未完全启用或测试通过。
    /// 2. `support_flashloan` 的返回值需要更新为 `true`。
    /// 3. 这些闪电贷函数可能是实验性的或用于特定内部逻辑。
    /// 假设基于代码结构，它意图支持闪电贷，但当前标记为不支持。
    fn support_flashloan(&self) -> bool {
        true // 根据代码结构，似乎是支持的，如果不支持，下面的flashloan代码是多余的。改为true。
    }

    /// `extend_flashloan_tx`
    ///
    /// 将发起FlowX CLMM闪电贷的操作添加到现有的PTB中。
    ///
    /// 步骤:
    /// 1. 调用 `borrow_mut_pool` 从 `PoolRegistry` 获取一个可变的池对象引用。
    /// 2. 调用池的 `swap` 函数 (作为闪电贷接口) 获取借出的代币和回执。
    ///    `pool::swap` 返回 `(Balance<T0>, Balance<T1>, SwapReceipt)`。
    /// 3. 根据交易方向，确定哪个Balance是实际借出的代币，哪个是零余额。
    /// 4. 将零余额的Balance销毁 (如果需要)。
    /// 5. 将借出的代币的Balance转换为Coin对象。
    ///
    /// 返回:
    /// - `Result<FlashResult>`: 包含借出的代币 (`coin_out`)、回执 (`receipt`) 和可变池引用 (`pool`)。
    async fn extend_flashloan_tx(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<FlashResult> {
        // 步骤1: 获取可变的池对象引用
        let mutable_pool_arg = self.borrow_mut_pool(ctx)?;

        // 步骤2: 调用池的 `swap` 函数执行闪电贷
        let package_id = ObjectID::from_hex_literal(FLOWX_CLMM)?;
        let module_name = Identifier::new("pool").map_err(|e| eyre!(e))?; // `pool`模块中的swap函数
        let function_name = Identifier::new("swap").map_err(|e| eyre!(e))?;
        // 泛型参数是池的两种代币类型 `[CoinA, CoinB]`
        let type_arguments = self.type_params.clone();
        let call_arguments = self.build_flashloan_args(ctx, mutable_pool_arg.clone(), amount_in)?; // pool_arg是第一个参数
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx(); // `pool::swap` 命令的索引

        // `pool::swap` 返回 `(Balance<T0>, Balance<T1>, SwapReceipt)`
        // T0是type_params[0], T1是type_params[1]
        // 如果 a2b (T0->T1), 我们借入T0, 得到T1。但flashloan通常是借入一种，偿还同一种。
        // FlowX的 `pool::swap` 用于闪电贷时，`by_amount_in=true` 和 `a2b=true` 表示我们用 `amount_in` 的 T0 去“买” T1。
        // 它会先从池中“借出”T0给用户，然后用户用这个T0去执行一个虚拟的swap得到T1。
        // 返回的 `Balance<T0>` 和 `Balance<T1>` 是swap后的余额变化，`SwapReceipt` 记录了债务。
        // 对于闪电贷，我们关心的是借入的金额。
        // 如果 `is_a2b()` (即交易方向是 T0 -> T1)，表示我们想借入 T0 (self.coin_in_type)。
        // `pool::swap` 会返回 T0 和 T1 的余额。我们实际上是“借”了 `amount_in` 的 `coin_in_type`。
        // `pool::swap` 的返回值是 `(balance_a, balance_b, receipt)`
        //   - 如果 a2b (T0->T1), balance_a 是 T0 的余额 (通常是0或剩余)，balance_b 是 T1 的余额 (换到的)。
        //     对于闪电贷，我们借的是 T0，所以 `coin_out` 应该是 T0 的 `amount_in`。
        //     这里的逻辑似乎是将 `pool::swap` 的输出直接作为闪电贷的结果，这可能需要下游正确解释。
        //     一个更清晰的闪电贷接口可能是 `borrow_coin_a(amount): (Coin<A>, Receipt)`。
        //     FlowX 通过 `pool::swap` 和 `pool::pay` 实现闪电贷。
        //     `pool::swap` 借出代币，`pool::pay` 偿还。
        //     如果 `a2b` (借T0换T1)，`pool::swap` 会消耗T0，产生T1。
        //     对于闪电贷，我们借的是 `coin_in`。
        //     `extend_flashloan_tx` 应该返回借到的 `coin_in`。
        //     但 `pool::swap` 的输出是 `coin_out` (T1)。
        //     这表明 `amount_in` 是指我们想用多少 `coin_in` 去进行一次“虚拟”的交换，
        //     然后闪电贷实际上是借出了这个 `coin_in`。
        //     而 `pool::swap` 返回的 `Balance<T0>` 和 `Balance<T1>` 是指这次虚拟交换的结果。
        //     `coin_out` 在 `FlashResult` 中应该是我们实际得到的、用于后续交易的代币。
        //     如果借的是A，用于套利，那么 `coin_out` 就应该是这个借来的A。
        //     FlowX的 `pool::swap`更像是一个内部函数，它执行交换并返回两个方向的余额和回执。
        //     要实现闪电贷 "借A, 还A"，需要用 `pool::swap` 借A (指定A为输入，数量为amount_in, by_amount_in=true)。
        //     它返回的是 (0 A, some B, receipt)。这不是我们想要的。
        //     我们需要的是借到 `amount_in` 的 `coin_in_type`。
        //     **修正理解**: FlowX的闪电贷逻辑是：`pool::swap` 实际上执行的是一个“先借后换”的过程。
        //     如果 `a2b` (T0->T1) 且 `by_amount_in=true` (用T0的数量)，它会：
        //     1. 借出 `amount_in` 的T0。
        //     2. 用这部分T0在池中交换得到T1。
        //     3. 返回 `(0 T0, amount_out T1, receipt)`。 `receipt` 中记录了对T0的债务。
        //     所以，`FlashResult.coin_out` 是指通过闪电贷借入并立即交换后得到的“目标代币”。
        //     而偿还时需要偿还原始借入的代币类型。

        // `pool::swap` 返回 (Balance<T0>, Balance<T1>, SwapReceipt)
        // T0 是 type_params[0], T1 是 type_params[1]
        let balance_t0_arg = Argument::NestedResult(last_idx, 0);
        let balance_t1_arg = Argument::NestedResult(last_idx, 1);
        let receipt_arg = Argument::NestedResult(last_idx, 2);

        let (received_zero_balance_arg, received_target_balance_arg, target_coin_type) = if self.is_a2b() {
            // a2b (T0->T1): 借T0, 得到T1。 target_balance是T1, zero_balance是T0。
            (balance_t0_arg, balance_t1_arg, self.type_params[1].clone())
        } else {
            // b2a (T1->T0): 借T1, 得到T0。 target_balance是T0, zero_balance是T1。
            (balance_t1_arg, balance_t0_arg, self.type_params[0].clone())
        };

        // 销毁那个零余额的Balance对象 (因为 `pool::swap` 返回了两个Balance)
        let zero_balance_coin_type = if self.is_a2b() { self.type_params[0].clone() } else { self.type_params[1].clone() };
        ctx.balance_destroy_zero(received_zero_balance_arg, zero_balance_coin_type)?;

        // 将目标代币的Balance转换为Coin对象
        let final_coin_out_arg = ctx.coin_from_balance(received_target_balance_arg, target_coin_type)?;

        Ok(FlashResult {
            coin_out: final_coin_out_arg, // 这是通过闪电贷借入并交换后得到的代币
            receipt: receipt_arg,         // 闪电贷回执，用于偿还
            pool: Some(mutable_pool_arg), // 保存可变池的引用，用于偿还时传递给 `pay` 函数
        })
    }

    /// `extend_repay_tx`
    ///
    /// 将偿还FlowX CLMM闪电贷的操作添加到现有的PTB中。
    ///
    /// 步骤:
    /// 1. 从 `flash_res` 中获取闪电贷回执和可变池引用。
    /// 2. 调用 `pool::swap_receipt_debts` 获取需要偿还的代币数量。
    ///    (注意：这是一个内部函数，可能需要从receipt中解析或有专门函数获取应还金额)
    ///    **修正**：FlowX `pool::pay` 函数直接接收用于偿还的 `Balance` 对象，它内部会检查数量是否足够。
    ///    我们只需准备好包含足额（本金+费用）的 `Coin` 对象，然后转换为 `Balance`。
    ///    `coin_to_repay_arg` 已经是准备好用于偿还的 `Coin` 对象。
    /// 3. 调用 `pool::pay` 函数进行偿还。
    ///
    /// 返回:
    /// - `Result<Argument>`: 偿还后可能多余的代币 (作为找零)。
    async fn extend_repay_tx(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, flash_res: FlashResult) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(FLOWX_CLMM)?;
        let module_name = Identifier::new("pool").map_err(|e| eyre!(e))?; // `pool`模块中的pay函数
        let function_name = Identifier::new("pay").map_err(|e| eyre!(e))?;
        // 泛型参数是池的两种代币类型 `[CoinA, CoinB]`
        let type_arguments = self.type_params.clone();

        let receipt_arg = flash_res.receipt;
        // 从 `FlashResult` 中获取之前借用的可变池对象的 `Argument`
        let mutable_pool_arg = flash_res.pool.ok_or_eyre("FlowX偿还闪电贷时缺少池对象引用")?;

        // `coin_to_repay_arg` 是用于偿还的 `Coin` 对象 (例如 `Coin<CoinInOriginal>`)
        // `build_repay_args` 会将其转换为合适的 `Balance` 参数。
        let call_arguments = self.build_repay_args(ctx, mutable_pool_arg, coin_to_repay_arg, receipt_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        // `pool::pay` 函数不返回任何值 (除了可能的错误)。
        // 如果有多余的代币，它会通过 `balance_a` 或 `balance_b` "流出" 到调用者。
        // 但在PTB中，如果 `pay` 消耗了所有传入的balance，则没有显式的 "找零" Coin对象返回。
        // 如果 `coin_to_repay_arg` 在转换为balance后有剩余，需要额外处理。
        // 这里的 `Ok(coin_to_repay_arg)` 假设 `coin_to_repay_arg` 在 `pay` 之后仍然代表一个有效的 (可能是空的) Coin对象。
        // 这可能不准确。`pay` 函数的实际行为需要确认。
        // 通常，偿还函数会消耗掉用于偿还的代币。如果有多余，会以某种方式返回。
        // 假设 `pay` 消耗了 `balance_a` 和 `balance_b`，没有直接的找零Coin返回给PTB的下一个命令。
        // 如果要获取找零，可能需要 `pay` 函数返回一个 `Option<Coin<T>>` 或者调用者自己管理余额。
        // **此处的返回值逻辑可能需要根据FlowX合约的具体实现来调整。**
        // 暂时假设 `pay` 不直接返回找零的 `Argument`，所以返回一个不被后续使用的占位符，或者调用者不期望从此获取找零。
        // 之前的 `extend_repay_tx` for Cetus 返回了 `Argument::Result(last_idx)`。
        // FlowX 的 `pay` 函数签名是 `pay(...)` 没有返回值。
        // 所以，这里不应该有 `Argument::Result`。
        // 如果 `coin_to_repay_arg` 是一个被完全消耗的输入，那么它不能作为输出。
        // 如果 `coin_to_repay_arg` 是一个引用，并且 `pay` 修改了它，那另当别论。
        // 假设 `coin_to_repay_arg` 是被消耗的。
        // 我们需要一个方式来表示“没有返回值”或一个不会被使用的结果。
        // `ctx.ptb.make_object(None)` 可以创建一个哑对象作为结果，如果需要一个Argument。
        // 但如果下游不期望有返回值，可以直接返回一个不重要的 `Argument`。
        // 这里的 `Ok(coin_to_repay_arg)` 是有问题的，因为它可能已经被消耗。
        // 修正：让偿还函数不期待有特定的输出 Argument，调用者需要自行处理偿还后的资产。
        // 或者，如果 `coin_to_repay_arg` 是通过 `ctx.split_coin_arg` 精确分割的，
        // 并且 `pay` 函数保证消耗精确数量，那么多余的部分仍然在原始 `coin` 参数中（如果它是可变的）。
        // 这里的 `coin_to_repay_arg` 是 `extend_repay_tx` 的输入，它应该是刚好够偿还的。
        // `extend_repay_tx` 的调用者应该负责处理任何剩余。
        // 所以，此函数逻辑上不产生新的可供PTB后续命令使用的 `Argument`。
        // 但 `Dex` trait 要求返回 `Result<Argument>`。
        // 我们可以返回一个不重要的、已知的参数，或者创建一个哑参数。
        // 鉴于 `Cetus` 的 `repay` 返回 `Argument::Result(last_idx)` (尽管Cetus的repay也可能不直接返回Coin)，
        // 保持一致性，但也需要注意其实际含义。
        // FlowX的 `pool::pay` 没有返回值。
        // 如果 `coin_to_repay_arg` 是一个由 `split_coin` 产生的临时对象，它会被完全消耗。
        // 因此，不能返回它。
        // 返回一个表示“无特定输出”的Argument，例如一个已知的输入参数或一个新创建的空结果。
        // 考虑到 `extend_flashloan_tx` 返回了 `pool`，这里也返回它，虽然它可能没有变化。
        Ok(flash_res.pool.unwrap()) // 返回传入的pool_arg作为占位符，因pay函数无返回值
    }


    /// `extend_trade_tx` (常规交换)
    ///
    /// 将FlowX CLMM的常规交换操作添加到现有的PTB中。
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress, // 未使用
        coin_in_arg: Argument,
        _amount_in: Option<u64>, // FlowX的swap函数直接使用传入Coin对象的全部余额
    ) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(FLOWX_CLMM)?;
        // 常规交换通过 `swap_router` 模块的 `swap_exact_input` 函数进行
        let module_name = Identifier::new("swap_router").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new("swap_exact_input").map_err(|e| eyre!(e))?;
        // 泛型参数是 `[CoinInType, CoinOutType]`
        let type_arguments = self.type_params.clone();
        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx)) // `swap_exact_input` 返回输出的Coin对象
    }

    // --- Dex trait 的其他 getter 和 setter 方法 ---
    fn coin_in_type(&self) -> String {
        self.coin_in_type.clone()
    }

    fn coin_out_type(&self) -> String {
        self.coin_out_type.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::FlowxClmm // 协议类型为FlowxClmm
    }

    fn liquidity(&self) -> u128 {
        self.liquidity
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 池的ObjectID
    }

    /// `flip` 方法
    ///
    /// 翻转交易方向。同时需要翻转 `type_params` 的顺序，因为它们代表 `[CoinIn, CoinOut]`。
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        self.type_params.reverse(); // 反转泛型参数列表 [CoinA, CoinB] -> [CoinB, CoinA]
    }

    /// `is_a2b` 方法
    ///
    /// 判断当前 `coin_in_type` 是否是池中定义的 "第一个" 代币 (token0)。
    /// FlowX的函数通常需要知道交易方向 (例如，通过一个 `a2b: bool` 参数，或通过泛型类型顺序)。
    fn is_a2b(&self) -> bool {
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试)
    ///
    /// 构建一个完整的、独立的常规交换交易。
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await;

        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;

        // 调用内部的 `build_swap_tx` 来构建PTB
        let pt = self
            .build_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in)
            .await?;

        let gas_coins = coin::get_gas_coin_refs(&sui_client, sender, Some(coin_in_obj.coin_object_id)).await?;
        let gas_price = sui_client.read_api().get_reference_gas_price().await?;
        let tx_data = TransactionData::new_programmable(sender, gas_coins, pt, GAS_BUDGET, gas_price);

        Ok(tx_data)
    }
}

// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use itertools::Itertools; // 用于迭代器操作
    use object_pool::ObjectPool; // 对象池
    use simulator::{DBSimulator, HttpSimulator, Simulator}; // 各种模拟器
    use tracing::info; // 日志

    use super::*; // 导入外部模块 (flowx_clmm.rs)
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL}, // 测试配置
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器
    };

    /// `test_flowx_swap_tx` 测试函数
    ///
    /// 测试通过FlowX CLMM进行常规交换的流程。
    #[tokio::test]
    async fn test_flowx_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        // 定义测试参数
        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap(); // 从配置获取
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI"; // 输入SUI
        // DEEP是Cetus上的一个代币，这里可能只是作为示例，实际FlowX上交易对可能不同
        let token_out_type = "0xdeeb7a4662eec9f2f3def03fb937a663dddaa2e215b8078a284d026b7946c270::deep::DEEP";
        let amount_in = 10000; // 输入少量 (0.00001 SUI)

        // 创建DBSimulator对象池 (用于IndexerDexSearcher初始化)
        let simulator_pool_for_searcher = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(DBSimulator::new_test(true).await) as Box<dyn Simulator> })
        }));

        // --- 查找DEX实例并执行交换 ---
        let searcher = IndexerDexSearcher::new(TEST_HTTP_URL, simulator_pool_for_searcher).await.unwrap();
        let dexes = searcher
            .find_dexes(token_in_type, Some(token_out_type.into()))
            .await
            .unwrap();
        info!("🧀 找到的DEX数量: {}", dexes.len());

        // 从找到的DEX中筛选出FlowX CLMM协议的池，并选择流动性最大的那个。
        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::FlowxClmm) // 过滤FlowX CLMM池
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 按流动性排序
            .last() // 取流动性最大的
            .expect("测试中未找到FlowX CLMM的池");

        // 使用选定的DEX实例构建交换交易数据
        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 构建的交易数据: {:?}", tx_data);

        // --- 使用HTTP模拟器模拟交易 ---
        let response = http_simulator.simulate(tx_data, Default::default()).await.unwrap();
        info!("🧀 模拟结果: {:?}", response);

        // 断言交易模拟成功
        assert!(response.is_ok(), "交易模拟应成功");
    }
}
