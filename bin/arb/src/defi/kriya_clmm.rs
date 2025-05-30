// 该文件 `kriya_clmm.rs` 实现了与 KriyaDEX 协议的 CLMM (集中流动性做市商) 池交互的逻辑。
// KriyaDEX 是 Sui 上的一个DEX，同时提供传统AMM池和CLMM池。此文件专注于CLMM部分。
// CLMM允许流动性提供者将资金集中在特定的价格区间内，以提高资本效率。
// 此实现也包含了对Kriya CLMM闪电贷功能的支持。
//
// 文件概览:
// 1. 定义了与 Kriya CLMM 相关的常量，如合约包ID (`KRIYA_CLMM`) 和版本对象ID (`VERSION`)。
// 2. `ObjectArgs` 结构体: 用于缓存 `Version` 和 `Clock` 对象的 `ObjectArg`。
// 3. `KriyaClmm` 结构体: 代表一个 Kriya CLMM 池的实例，实现了 `Dex` trait。
// 4. `new()` 方法: 初始化 `KriyaClmm` 实例，从链上获取池的详细信息。
// 5. 常规交换相关方法:
//    - `build_swap_tx()` / `build_swap_args()`: 构建精确输入交换的交易参数和PTB。
//    - 常规交换 (`extend_trade_tx`) 似乎也是通过 `CETUS_AGGREGATOR` 进行路由的。
// 6. 闪电贷相关方法:
//    - `build_flashloan_args()`: 构建发起闪电贷 (调用 `trade::flash_swap`) 的参数。
//    - `build_repay_args()`: 构建偿还闪电贷 (调用 `trade::repay_flash_swap`) 的参数。
//    - `extend_flashloan_tx()`: 将发起闪电贷的操作添加到PTB。
//    - `extend_repay_tx()`: 将偿还闪电贷的操作添加到PTB。
//    - `support_flashloan()`: 返回 `true`，表明支持闪电贷。
// 7. 实现了 `Dex` trait 的其他方法。
//
// Sui/DeFi概念:
// - CLMM (Concentrated Liquidity Market Maker): 集中流动性做市商，与Cetus和FlowX类似。
// - Version Object (版本对象): KriyaDEX可能使用一个全局的版本对象来管理其合约升级和版本控制。
// - Flashloan (闪电贷): 与其他支持闪电贷的协议类似，允许在单笔交易内无抵押借贷并归还。

// 引入标准库及第三方库
use std::sync::Arc; // 原子引用计数

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` 引入Pool和Protocol类型
use eyre::{ensure, eyre, OptionExt, Result}; // 错误处理库
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

use super::{trade::FlashResult, TradeCtx, CETUS_AGGREGATOR}; // 从父模块(defi)引入 FlashResult, TradeCtx, CETUS_AGGREGATOR
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 Dex trait

// --- Kriya CLMM 协议相关的常量定义 ---
// Kriya CLMM核心合约包ID
const KRIYA_CLMM: &str = "0xbd8d4489782042c6fafad4de4bc6a5e0b84a43c6c00647ffd7062d1e2bb7549e";
// Kriya CLMM 版本对象ID (Version)
const VERSION: &str = "0xf5145a7ac345ca8736cf8c76047d00d6d378f30e81be6f6eb557184d9de93c78";

/// `ObjectArgs` 结构体
///
/// 缓存Kriya CLMM交互所需的关键对象的 `ObjectArg` 形式。
#[derive(Clone)]
pub struct ObjectArgs {
    version: ObjectArg, // 版本对象的ObjectArg
    clock: ObjectArg,   // Sui时钟对象的ObjectArg
}

// 用于缓存 `ObjectArgs` 的静态 `OnceCell`
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数
///
/// 获取并缓存 `ObjectArgs` (包含version, clock)。
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let version_id = ObjectID::from_hex_literal(VERSION).unwrap();
            // 通过模拟器获取对象信息
            let version_obj = simulator.get_object(&version_id).await.unwrap();
            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();

            ObjectArgs {
                version: shared_obj_arg(&version_obj, false), // Version对象通常是不可变的
                clock: shared_obj_arg(&clock_obj, false),   // Clock是不可变的
            }
        })
        .await
        .clone()
}

/// `KriyaClmm` 结构体
///
/// 代表一个KriyaDEX的CLMM交易池。
#[derive(Clone)]
pub struct KriyaClmm {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息
    pool_arg: ObjectArg,     // 池对象本身的 `ObjectArg`
    liquidity: u128,         // 池的流动性
    coin_in_type: String,    // 当前交易方向的输入代币类型
    coin_out_type: String,   // 当前交易方向的输出代币类型
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是[CoinA, CoinB])
    // 共享的对象参数
    version: ObjectArg,
    clock: ObjectArg,
}

impl KriyaClmm {
    /// `new` 构造函数
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `KriyaClmm` DEX实例。
    ///
    /// 参数:
    /// - `simulator`: 共享的模拟器实例。
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。
    /// - `coin_in_type`: 输入代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `KriyaClmm` 实例，否则返回错误。
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 确保池协议是KriyaClmm
        ensure!(pool_info.protocol == Protocol::KriyaClmm, "提供的不是Kriya CLMM协议的池");

        // 获取并解析池对象的Move结构体内容
        let pool_obj = simulator
            .get_object(&pool_info.pool) // pool_info.pool 是池的ObjectID
            .await
            .ok_or_else(|| eyre!("Kriya CLMM池对象未找到: {}", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("Kriya CLMM池对象的布局(layout)未找到")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 从解析后的池结构体中提取流动性 (liquidity 字段)
        let liquidity = extract_u128_from_move_struct(&parsed_pool_struct, "liquidity")?;

        // 根据输入代币推断输出代币 (假设是双币池)
        let coin_out_type = if pool_info.token0_type() == coin_in_type {
            pool_info.token1_type().to_string()
        } else {
            pool_info.token0_type().to_string()
        };

        // 获取池本身的泛型类型参数，这通常是池中包含的两种代币的类型。
        // 例如 `Pool<CoinTypeA, CoinTypeB>` 中的 `CoinTypeA, CoinTypeB`。
        let type_params = parsed_pool_struct.type_.type_params.clone();

        // 将池对象转换为 `ObjectArg` (在交易中通常是可变的)
        let pool_arg = shared_obj_arg(&pool_obj, true);
        // 获取共享的协议对象参数 (version, clock)
        let ObjectArgs { version, clock } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(),
            liquidity,
            coin_in_type: coin_in_type.to_string(),
            coin_out_type,
            type_params, // 通常是 [TokenTypeA, TokenTypeB]
            pool_arg,
            version,
            clock,
        })
    }

    /// `build_swap_tx` (私有辅助函数)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在Kriya CLMM池中执行一次常规交换。
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
        // (Kriya CLMM的swap函数可能直接使用传入Coin对象的全部余额)。
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数)
    ///
    /// 构建调用Kriya CLMM常规交换方法 (如聚合器中的 `kriya_clmm::swap_a2b`) 所需的参数列表。
    /// 聚合器中的函数签名可能类似于:
    /// `fun swap_a2b<CoinA, CoinB>(pool: &mut Pool<CoinA, CoinB>, coin_a: Coin<CoinA>, version: &Version, clock: &Clock, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数包括: pool, 输入的coin对象, version对象, clock对象。
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let version_arg = ctx.obj(self.version).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        // 返回参数列表，顺序必须与聚合器中 kriya_clmm 模块的 swap_a2b/swap_b2a 函数签名一致。
        Ok(vec![pool_arg, coin_in_arg, version_arg, clock_arg])
    }

    /// `build_flashloan_args` (私有辅助函数)
    ///
    /// 构建调用Kriya CLMM发起闪电贷方法 (`trade::flash_swap`) 所需的参数列表。
    /// 合约方法签名示例 (来自注释):
    /// `public fun flash_swap<T0, T1>(
    ///     _pool: &mut Pool<T0, T1>,
    ///     _a2b: bool,              // 交易方向 (true表示T0->T1, 即借T0换T1)
    ///     _by_amount_in: bool,     // true表示 `_amount` 是输入数量 (要借的数量)
    ///     _amount: u64,            // 数量
    ///     _sqrt_price_limit: u128, // 价格限制
    ///     _clock: &Clock,
    ///     _version: &Version,
    ///     _ctx: &TxContext
    /// ) : (Balance<T0>, Balance<T1>, FlashSwapReceipt)`
    fn build_flashloan_args(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?; // 可变的池对象引用
        let a2b_arg = ctx.pure(self.is_a2b()).map_err(|e| eyre!(e))?; // 交易方向
        let by_amount_in_arg = ctx.pure(true).map_err(|e| eyre!(e))?; // 按输入数量计算
        let amount_arg = ctx.pure(amount_in).map_err(|e| eyre!(e))?; // 借贷/输入数量

        // 设置价格限制 (sqrt_price_limit)。
        // 对于闪电贷，如果只是单纯借款而不关心虚拟交换的价格，可以设置一个较宽松的限制。
        // Kriya CLMM的 `flash_swap` 似乎也执行一个虚拟的swap来计算费用或确定债务。
        // `MIN_SQRT_PRICE_X64` (不是加1) for a2b, `MAX_SQRT_PRICE_X64` for b2a.
        // 这表示允许价格达到最极端的情况，因为主要目的是借款。
        let sqrt_price_limit_val = if self.is_a2b() {
            MIN_SQRT_PRICE_X64 // 借 T0 (a), 换 T1 (b)。价格是 b/a。允许价格到最小。
        } else {
            MAX_SQRT_PRICE_X64 // 借 T1 (b), 换 T0 (a)。价格是 a/b。允许价格到最大。
        };
        let sqrt_price_limit_arg = ctx.pure(sqrt_price_limit_val).map_err(|e| eyre!(e))?;

        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;
        let version_arg = ctx.obj(self.version).map_err(|e| eyre!(e))?;

        Ok(vec![
            pool_arg,
            a2b_arg,
            by_amount_in_arg,
            amount_arg,
            sqrt_price_limit_arg,
            clock_arg,
            version_arg,
        ])
    }

    /// `build_repay_args` (私有辅助函数)
    ///
    /// 构建调用Kriya CLMM偿还闪电贷方法 (`trade::repay_flash_swap`) 所需的参数列表。
    /// 合约方法签名示例 (来自注释):
    /// `public fun repay_flash_swap<T0, T1>(
    ///     _pool: &mut Pool<T0, T1>,
    ///     _receipt: FlashSwapReceipt,
    ///     _balance_a: Balance<T0>, // 用于偿还的T0代币余额
    ///     _balance_b: Balance<T1>, // 用于偿还的T1代币余额
    ///     _version: &Version,
    ///     _ctx: &TxContext
    /// )`
    /// 在闪电贷中，通常只提供借入方向的代币余额进行偿还。
    fn build_repay_args(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, receipt_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?; // 可变的池对象引用

        // 根据交易方向，将 `coin_to_repay_arg` 转换为相应类型的 `Balance` 对象。
        // 另一个方向的 Balance 则为空 (zero balance)。
        // T0是type_params[0], T1是type_params[1]
        let (balance_a_arg, balance_b_arg) = if self.is_a2b() {
            // 如果是 a2b (借T0/CoinA, 得到T1/CoinB), 那么偿还的是T0/CoinA。
            // `coin_to_repay_arg` 应该是 `Coin<T0>`。
            (
                ctx.coin_into_balance(coin_to_repay_arg, self.type_params[0].clone())?, // coin_to_repay是T0类型
                ctx.balance_zero(self.type_params[1].clone())?,                     // T1的Balance为空
            )
        } else {
            // 如果是 b2a (借T1/CoinB, 得到T0/CoinA), 那么偿还的是T1/CoinB。
            // `coin_to_repay_arg` 应该是 `Coin<T1>`。
            (
                ctx.balance_zero(self.type_params[0].clone())?,                     // T0的Balance为空
                ctx.coin_into_balance(coin_to_repay_arg, self.type_params[1].clone())?, // coin_to_repay是T1类型
            )
        };

        let version_arg = ctx.obj(self.version).map_err(|e| eyre!(e))?;
        Ok(vec![pool_arg, receipt_arg, balance_a_arg, balance_b_arg, version_arg])
    }
}

/// 为 `KriyaClmm` 结构体实现 `Dex` trait。
#[async_trait::async_trait]
impl Dex for KriyaClmm {
    /// `support_flashloan` 方法
    ///
    /// 指明该DEX是否支持闪电贷。Kriya CLMM是支持的。
    fn support_flashloan(&self) -> bool {
        true
    }

    /// `extend_flashloan_tx`
    ///
    /// 将发起Kriya CLMM闪电贷的操作添加到现有的PTB中。
    /// Kriya CLMM的闪电贷通过其 `trade::flash_swap` 函数实现。
    ///
    /// 返回:
    /// - `Result<FlashResult>`: 包含借出的代币 (`coin_out`) 和闪电贷回执 (`receipt`)。
    ///   `coin_out` 是指通过闪电贷借入并立即进行虚拟交换后得到的“目标代币”。
    async fn extend_flashloan_tx(&self, ctx: &mut TradeCtx, amount_to_borrow: u64) -> Result<FlashResult> {
        let package_id = ObjectID::from_hex_literal(KRIYA_CLMM)?; // Kriya CLMM包ID
        let module_name = Identifier::new("trade").map_err(|e| eyre!(e))?; // `trade`模块
        let function_name = Identifier::new("flash_swap").map_err(|e| eyre!(e))?;
        // 泛型参数是池的两种代币类型 `[CoinA, CoinB]`
        let type_arguments = self.type_params.clone();
        let call_arguments = self.build_flashloan_args(ctx, amount_to_borrow)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx(); // `flash_swap` 命令的索引

        // `flash_swap` 返回 `(Balance<T0>, Balance<T1>, FlashSwapReceipt)`
        // T0是type_params[0], T1是type_params[1]
        // 根据 `is_a2b` 判断哪个Balance是0 (对应借入的币种的初始余额，在swap后变0或剩余手续费)
        // 哪个Balance是实际交换得到的币种。
        let balance_t0_arg = Argument::NestedResult(last_idx, 0);
        let balance_t1_arg = Argument::NestedResult(last_idx, 1);
        let receipt_arg = Argument::NestedResult(last_idx, 2); // 闪电贷回执

        // 如果 is_a2b() (借T0换T1):
        //   - `balance_t0_arg` 是 T0 的剩余/债务余额 (通常为0，或手续费部分)
        //   - `balance_t1_arg` 是交换后得到的 T1 余额 (这是我们用于后续操作的 `coin_out`)
        //   - `coin_in_type_for_flash_result` 是 T0, `coin_out_type_for_flash_result` 是 T1
        let (zero_balance_arg, target_balance_arg, _original_borrow_coin_type, target_coin_type_tag) = if self.is_a2b() {
            (balance_t0_arg, balance_t1_arg, self.type_params[0].clone(), self.type_params[1].clone())
        } else {
            // b2a (借T1换T0)
            (balance_t1_arg, balance_t0_arg, self.type_params[1].clone(), self.type_params[0].clone())
        };

        // 销毁那个零余额的Balance对象 (对应原始借入代币在swap后的剩余，通常是0)
        let zero_balance_coin_type_tag = if self.is_a2b() { self.type_params[0].clone() } else { self.type_params[1].clone() };
        ctx.balance_destroy_zero(zero_balance_arg, zero_balance_coin_type_tag)?;

        // 将目标代币的Balance转换为Coin对象
        let final_coin_out_arg = ctx.coin_from_balance(target_balance_arg, target_coin_type_tag)?;

        Ok(FlashResult {
            coin_out: final_coin_out_arg, // 这是通过闪电贷借入并交换后得到的“目标代币”
            receipt: receipt_arg,         // 闪电贷回执，用于偿还原始借入的代币
            pool: None,                   // Kriya的flash_swap不直接返回pool对象作为PTB结果
        })
    }

    /// `extend_repay_tx`
    ///
    /// 将偿还Kriya CLMM闪电贷的操作添加到现有的PTB中。
    /// Kriya CLMM的闪电贷偿还通过其 `trade::repay_flash_swap` 函数实现。
    ///
    /// 步骤:
    /// 1. 从 `flash_res` 中获取闪电贷回执。
    /// 2. 调用 `trade::swap_receipt_debts` (或类似函数) 获取确切需要偿还的代币类型和数量。
    ///    (注意：Kriya的`repay_flash_swap`直接接收偿还的Balance，它内部会检查数量。)
    ///    我们需要确保传入的 `coin_to_repay_arg` 包含本金+费用。
    /// 3. 调用 `trade::repay_flash_swap` 函数进行偿还。
    ///
    /// 返回:
    /// - `Result<Argument>`: 偿还后多余的代币 (如果有的话)。Kriya的`repay_flash_swap`不返回任何值。
    ///   所以这里返回传入的 `coin_to_repay_arg` (可能已被部分或完全消耗)。
    async fn extend_repay_tx(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, flash_res: FlashResult) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(KRIYA_CLMM)?; // Kriya CLMM包ID
        let module_name = Identifier::new("trade").map_err(|e| eyre!(e))?; // `trade`模块
        let receipt_arg = flash_res.receipt;

        // 为了偿还，我们需要提供 `coin_to_repay_arg` (例如 Coin<OriginalCoinIn>)。
        // `build_repay_args` 会将其转换为合适的 Balance 参数。
        // Kriya的 `repay_flash_swap` 需要知道确切的应还金额。
        // `swap_receipt_debts` 函数可以从回执中读取债务。
        let repay_amount_arg = {
            let debts_fn_name = Identifier::new("swap_receipt_debts").map_err(|e| eyre!(e))?;
            // `swap_receipt_debts` 的泛型参数是 `FlashSwapReceipt` 的泛型，即 `[CoinA, CoinB]`
            // 它不依赖于当前的 `is_a2b` 方向，而是回执本身记录了借贷方向。
            // `self.type_params` 是 `[PoolCoin0, PoolCoin1]`。
            // `FlashSwapReceipt<CoinA, CoinB>` 中的 CoinA, CoinB 是指借贷发生时的 a 和 b。
            // 假设 `self.type_params` 顺序与回执中的顺序一致。
            let debts_type_args = self.type_params.clone();
            let debts_args = vec![receipt_arg.clone()]; // `receipt` 需要被克隆或之后重新指定
            ctx.command(Command::move_call(
                package_id,
                module_name.clone(), // trade模块
                debts_fn_name,
                debts_type_args,
                debts_args,
            ));

            let last_debts_idx = ctx.last_command_idx();
            // `swap_receipt_debts` 返回 `(u64, u64)` 分别是 coin_a_debt 和 coin_b_debt
            // 我们需要偿还的是原始借入的那个币种的债务。
            if self.is_a2b() { // 如果是借 CoinA (type_params[0])
                Argument::NestedResult(last_debts_idx, 0) // coin_a_debt
            } else { // 如果是借 CoinB (type_params[1])
                Argument::NestedResult(last_debts_idx, 1) // coin_b_debt
            }
        };

        // 从 `coin_to_repay_arg` (这是我们拥有的、用于偿还的币的总量) 中分割出确切的 `repay_amount_arg`。
        // `repay_coin_exact_arg` 是精确数量的偿还用币。
        let repay_coin_exact_arg = ctx.split_coin_arg(coin_to_repay_arg.clone(), repay_amount_arg);

        // 调用 `repay_flash_swap` 函数
        let repay_fn_name = Identifier::new("repay_flash_swap").map_err(|e| eyre!(e))?;
        let repay_type_args = self.type_params.clone(); // [PoolCoin0, PoolCoin1]
        // `build_repay_args` 需要 `repay_coin_exact_arg` 和 `receipt_arg`
        let repay_call_args = self.build_repay_args(ctx, repay_coin_exact_arg, receipt_arg)?;
        ctx.command(Command::move_call(package_id, module_name, repay_fn_name, repay_type_args, repay_call_args));

        // `repay_flash_swap` 函数没有返回值。
        // `coin_to_repay_arg` 是调用者传入的，可能在分割后还有剩余。
        // 这里返回原始传入的 `coin_to_repay_arg`，调用者需要知道它可能已经被部分消耗。
        Ok(coin_to_repay_arg)
    }

    /// `extend_trade_tx` (常规交换)
    ///
    /// 将Kriya CLMM的常规交换操作（通过Cetus聚合器）添加到现有的PTB中。
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress, // 未使用
        coin_in_arg: Argument,
        _amount_in: Option<u64>, // Kriya CLMM的swap函数(通过聚合器)通常消耗整个传入的Coin对象
    ) -> Result<Argument> {
        // 根据交易方向选择聚合器中的 `swap_a2b` 或 `swap_b2a` 函数。
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("kriya_clmm").map_err(|e| eyre!(e))?; // 聚合器中与Kriya CLMM交互的模块
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        // 泛型类型参数，通常是 `[CoinTypeA, CoinTypeB]`。
        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() { // 如果是 B to A (即 coin_in is token1)
            type_arguments.swap(0, 1); // 交换泛型参数顺序，变为 [CoinB, CoinA]
        }

        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx)) // 聚合器的swap函数返回输出的Coin对象
    }

    // --- Dex trait 的其他 getter 和 setter 方法 ---
    fn coin_in_type(&self) -> String {
        self.coin_in_type.clone()
    }

    fn coin_out_type(&self) -> String {
        self.coin_out_type.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::KriyaClmm // 协议类型为KriyaClmm
    }

    fn liquidity(&self) -> u128 {
        self.liquidity
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 池的ObjectID
    }

    /// `flip` 方法
    ///
    /// 翻转交易方向。
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        // `type_params` 在 `extend_trade_tx` 和 `extend_flashloan_tx` 中会根据 `is_a2b` 动态调整，
        // 所以这里不需要修改 `type_params` 的原始顺序（即PoolCoin0, PoolCoin1的顺序）。
    }

    /// `is_a2b` 方法
    ///
    /// 判断当前 `coin_in_type` 是否是池中定义的 "第一个" 代币 (token0)。
    /// Kriya的函数（或通过聚合器调用的函数）通常需要知道交易方向。
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

    use super::*; // 导入外部模块 (kriya_clmm.rs)
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL}, // 测试配置
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器
    };

    /// `test_kriya_clmm_swap_tx` 测试函数
    ///
    /// 测试通过Kriya CLMM (可能经由Cetus聚合器) 进行常规交换的流程。
    #[tokio::test]
    async fn test_kriya_clmm_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        // 定义测试参数
        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap(); // 从配置获取
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI"; // 输入SUI
        // DEEP是Cetus上的一个代币，这里可能只是作为示例，实际Kriya CLMM上交易对可能不同
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

        // 从找到的DEX中筛选出KriyaClmm协议的池，并选择流动性最大的那个。
        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::KriyaClmm) // 过滤KriyaClmm池
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 按流动性排序
            .last() // 取流动性最大的
            .expect("测试中未找到KriyaClmm的池");

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
