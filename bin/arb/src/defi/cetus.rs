// 该文件 `cetus.rs` 实现了与 Cetus 协议（一个Sui区块链上的去中心化交易所DEX和流动性协议）交互的逻辑。
// Cetus 以其集中流动性做市商 (CLMM) 模型和提供闪电贷 (Flashloan) 功能而闻名。
//
// 文件概览:
// 1. 定义了与 Cetus 协议相关的常量，如合约包ID、全局配置对象ID、合作伙伴对象ID等。
// 2. `ObjectArgs` 结构体: 用于缓存这些常用 Cetus 对象的 `ObjectArg`，通过 `OnceCell` 实现单次初始化。
// 3. `Cetus` 结构体: 代表一个 Cetus 交易池的实例，包含了与该池交互所需的所有信息和方法。
//    它实现了 `Dex` trait。
// 4. `new()` 方法: 用于根据链上数据初始化 `Cetus` 实例。
// 5. 交换相关方法:
//    - `build_swap_tx()` / `build_swap_args()`: 构建常规的精确输入交换的交易参数和PTB。
//    - Cetus的交换函数也区分 `swap_a2b` 和 `swap_b2a`。
// 6. 闪电贷相关方法:
//    - `build_flashloan_args()`: 构建发起闪电贷的参数。
//    - `build_repay_args()`: 构建偿还闪电贷的参数。
//    - `extend_flashloan_tx()`: 将发起闪电贷的操作添加到PTB。
//    - `extend_repay_tx()`: 将偿还闪电贷的操作添加到PTB。
//    - `support_flashloan()`: 表明Cetus支持闪电贷。
// 7. 实现了 `Dex` trait 的其他方法，如 `extend_trade_tx`, `coin_in_type`, `coin_out_type` 等。
//
// Sui/DeFi概念:
// - Concentrated Liquidity Market Maker (CLMM): 集中流动性做市商。与传统的XYK AMM不同，CLMM允许流动性提供者 (LP)
//   将其资金集中在特定的价格范围内，从而提高资本效率。Cetus是Sui上CLMM的代表。
// - Flashloan (闪电贷): 一种无抵押贷款，但要求在同一笔原子交易 (transaction block) 内归还本金和手续费。
//   如果未能及时归还，整个交易将回滚。闪电贷常用于套利、清算、抵押品互换等DeFi操作。
// - Sui Clock Object: `0x6`，Sui系统中的一个共享对象，提供当前时间戳等信息，常被合约用于时间相关的逻辑。

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
use utils::{coin, new_test_sui_client, object::*}; // 自定义工具库

use super::{trade::FlashResult, TradeCtx}; // 从父模块(defi)引入 `FlashResult` (闪电贷结果), `TradeCtx`
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 `Dex` trait

// --- Cetus协议相关的常量定义 ---
// Cetus核心合约包ID
const CETUS_DEX: &str = "0xeffc8ae61f439bb34c9b905ff8f29ec56873dcedf81c7123ff2f1f67c45ec302";
// Cetus全局配置对象ID
const CONFIG: &str = "0xdaa46292632c3c4d8f31f23ea0f9b36a28ff3677e9684980e4438403a67a3d8f";
// Cetus合作伙伴对象ID (可能用于记录推荐关系或分配特定费用)
const PARTNER: &str = "0x639b5e433da31739e800cd085f356e64cae222966d0f1b11bd9dc76b322ff58b";

/// `ObjectArgs` 结构体
///
/// 缓存Cetus交互所需的关键对象的 `ObjectArg` 形式。
#[derive(Clone)]
pub struct ObjectArgs {
    config: ObjectArg,  // 全局配置对象的ObjectArg
    partner: ObjectArg, // 合作伙伴对象的ObjectArg
    clock: ObjectArg,   // Sui时钟对象的ObjectArg
}

// 用于缓存 `ObjectArgs` 的静态 `OnceCell`
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数
///
/// 获取并缓存 `ObjectArgs` (包含config, partner, clock)。
/// 如果缓存未初始化，则从链上获取这些对象并转换为 `ObjectArg`。
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let config_id = ObjectID::from_hex_literal(CONFIG).unwrap();
            let partner_id = ObjectID::from_hex_literal(PARTNER).unwrap();

            // 通过模拟器获取对象信息
            let config_obj = simulator.get_object(&config_id).await.unwrap();
            let partner_obj = simulator.get_object(&partner_id).await.unwrap();
            // SUI_CLOCK_OBJECT_ID 是一个已知的系统对象ID (0x6)
            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();

            ObjectArgs {
                config: shared_obj_arg(&config_obj, false), // config通常是不可变的共享对象
                partner: shared_obj_arg(&partner_obj, true),  // partner对象在交易中可能是可变的
                clock: shared_obj_arg(&clock_obj, false),   // clock是不可变的共享对象
            }
        })
        .await
        .clone()
}

/// `Cetus` 结构体
///
/// 代表一个Cetus协议的交易池。
#[derive(Clone)]
pub struct Cetus {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息
    pool_arg: ObjectArg,     // 池对象本身的 `ObjectArg`
    liquidity: u128,         // 池的流动性
    coin_in_type: String,    // 当前交易方向的输入代币类型
    coin_out_type: String,   // 当前交易方向的输出代币类型
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是CoinA, CoinB)
    // 共享的对象参数
    config: ObjectArg,
    partner: ObjectArg,
    clock: ObjectArg,
}

impl Cetus {
    /// `new` 构造函数
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `Cetus` DEX实例。
    ///
    /// 参数:
    /// - `simulator`: 共享的模拟器实例。
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。
    /// - `coin_in_type`: 输入代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `Cetus` 实例，否则返回错误。
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 确保池协议是Cetus
        ensure!(pool_info.protocol == Protocol::Cetus, "提供的不是Cetus协议的池");

        // 获取并解析池对象的Move结构体内容
        let pool_obj = simulator
            .get_object(&pool_info.pool)
            .await
            .ok_or_else(|| eyre!("Cetus池对象未找到: {}", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("Cetus池对象的布局(layout)未找到")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 检查池是否暂停 (is_pause 字段)
        let is_pause = extract_bool_from_move_struct(&parsed_pool_struct, "is_pause")?;
        ensure!(!is_pause, "Cetus池已暂停，无法交易");

        // 提取流动性 (liquidity 字段)
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
        // 获取共享的协议对象参数 (config, partner, clock)
        let ObjectArgs { config, partner, clock } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(),
            liquidity,
            coin_in_type: coin_in_type.to_string(),
            coin_out_type,
            type_params, // 通常是 [TokenTypeA, TokenTypeB]
            pool_arg,
            config,
            partner,
            clock,
        })
    }

    /// `build_swap_tx` (私有辅助函数)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在Cetus池中执行一次常规交换。
    #[allow(dead_code)] // 允许存在未使用的代码
    async fn build_swap_tx(
        &self,
        sender: SuiAddress,
        recipient: SuiAddress,
        coin_in_ref: ObjectRef,
        amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default(); // 创建交易上下文

        // 如果需要，分割输入代币
        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        // 将Cetus交换操作添加到PTB
        // `None` 表示 `amount_in` 参数对于 `extend_trade_tx` 是可选的或不直接使用u64值
        // (Cetus的swap函数直接使用传入Coin对象的全部余额作为输入数量)
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;
        // 将输出代币转移给接收者
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish()) // 完成并返回PTB
    }

    /// `build_swap_args` (私有辅助函数)
    ///
    /// 构建调用Cetus常规交换方法 (如 `swap_a2b`) 所需的参数列表。
    /// 合约方法签名示例 (来自注释):
    /// `fun swap_a2b<CoinA, CoinB>(config: &GlobalConfig, pool: &mut Pool<CoinA, CoinB>, partner: &mut Partner, coin_a: Coin<CoinA>, clock: &Clock, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数包括: config, pool, partner, 输入的coin对象, clock。
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let config_arg = ctx.obj(self.config).map_err(|e| eyre!(e))?;
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let partner_arg = ctx.obj(self.partner).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        // 返回参数列表，顺序必须与合约方法一致
        Ok(vec![config_arg, pool_arg, partner_arg, coin_in_arg, clock_arg])
    }

    /// `build_flashloan_args` (私有辅助函数)
    ///
    /// 构建调用Cetus发起闪电贷方法 (如 `flash_swap_a2b`) 所需的参数列表。
    /// 合约方法签名示例 (来自注释):
    /// `public fun flash_swap_a2b<CoinA, CoinB>(config: &GlobalConfig, pool: &mut Pool<CoinA, CoinB>, partner: &mut Partner, amount: u64, by_amount_in: bool, clock: &Clock, ctx: &mut TxContext): (Coin<CoinB>, FlashSwapReceipt<CoinA, CoinB>, u64)`
    /// 参数包括: config, pool, partner, 借贷数量 (amount), 是否按输入数量计算 (by_amount_in), clock。
    fn build_flashloan_args(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<Vec<Argument>> {
        let config_arg = ctx.obj(self.config).map_err(|e| eyre!(e))?;
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let partner_arg = ctx.obj(self.partner).map_err(|e| eyre!(e))?;

        let amount_arg = ctx.pure(amount_in).map_err(|e| eyre!(e))?; // 借贷数量
        // `by_amount_in = true` 表示 `amount_in` 是指希望借入的CoinA的数量 (如果是a2b)
        // 或者指希望用这么多CoinA去交换得到的CoinB的数量 (取决于合约具体实现)。
        // 通常对于闪电贷，`amount` 是指你想要借出的代币数量。
        // 如果是 `flash_swap_a2b`，`amount` 是 CoinA 的数量。
        let by_amount_in_arg = ctx.pure(true).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        Ok(vec![config_arg, pool_arg, partner_arg, amount_arg, by_amount_in_arg, clock_arg])
    }

    /// `build_repay_args` (私有辅助函数)
    ///
    /// 构建调用Cetus偿还闪电贷方法 (如 `repay_flash_swap_a2b`) 所需的参数列表。
    /// 合约方法签名示例 (来自注释):
    /// `public fun repay_flash_swap_a2b<CoinA, CoinB>(config: &GlobalConfig, pool: &mut Pool<CoinA, CoinB>, partner: &mut Partner, coin_a: Coin<CoinA>, receipt: FlashSwapReceipt<CoinA, CoinB>, ctx: &mut TxContext): Coin<CoinA>;`
    /// 参数包括: config, pool, partner, 用于偿还的coin对象, 闪电贷回执 (receipt)。
    fn build_repay_args(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, receipt_arg: Argument) -> Result<Vec<Argument>> {
        let config_arg = ctx.obj(self.config).map_err(|e| eyre!(e))?;
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let partner_arg = ctx.obj(self.partner).map_err(|e| eyre!(e))?;

        Ok(vec![config_arg, pool_arg, partner_arg, coin_to_repay_arg, receipt_arg])
    }
}

/// 为 `Cetus` 结构体实现 `Dex` trait。
#[async_trait::async_trait]
impl Dex for Cetus {
    /// `support_flashloan` 方法
    ///
    /// 指明该DEX是否支持闪电贷。Cetus是支持的。
    fn support_flashloan(&self) -> bool {
        true
    }

    /// `extend_flashloan_tx`
    ///
    /// 将发起Cetus闪电贷的操作添加到现有的PTB中。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文。
    /// - `amount_in`: 希望借入的代币数量。
    ///
    /// 返回:
    /// - `Result<FlashResult>`: 包含借出的代币 (`coin_out`) 和闪电贷回执 (`receipt`) 的 `Argument`。
    ///   `coin_out` 是指如果借的是A，那么通过 `flash_swap_a2b` 得到的是B。
    async fn extend_flashloan_tx(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<FlashResult> {
        // 根据交易方向选择 `flash_swap_a2b` 或 `flash_swap_b2a`
        let function_name_str = if self.is_a2b() {
            "flash_swap_a2b" // 如果当前输入是A (token0), 输出是B (token1), 则借A还B (或借A换B再用B还A)
                             // flash_swap_a2b: 借入CoinA，得到CoinB和回执 (用于之后偿还CoinA)
        } else {
            "flash_swap_b2a" // 如果当前输入是B (token1), 输出是A (token0), 则借B还A
        };

        let package_id = ObjectID::from_hex_literal(CETUS_DEX)?;
        let module_name = Identifier::new("cetus").map_err(|e| eyre!(e))?; // Cetus核心模块
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;
        
        // 泛型类型参数，与常规swap类似，需要根据 is_a2b 调整顺序
        let mut type_arguments = self.type_params.clone(); // [CoinA, CoinB] or [CoinB, CoinA]
        if !self.is_a2b() { // 如果是 B to A (即 coin_in is token1, coin_out is token0)
            type_arguments.swap(0, 1); // 确保泛型参数是 [CoinIn, CoinOut]
        }
        
        let call_arguments = self.build_flashloan_args(ctx, amount_in)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        // `flash_swap` 函数通常返回一个元组 `(Coin<Out>, FlashSwapReceipt, fee_amount_or_other_u64)`
        // 我们需要从这个元组中提取出 `Coin<Out>` 和 `FlashSwapReceipt` 作为后续操作的输入。
        let last_idx = ctx.last_command_idx(); // 获取刚添加的move_call命令的索引

        Ok(FlashResult {
            // `Argument::NestedResult(command_index, field_index)` 用于引用一个命令返回的元组中的特定元素。
            coin_out: Argument::NestedResult(last_idx, 0), // 元组的第一个元素是借出的代币 (Coin<Out>)
            receipt: Argument::NestedResult(last_idx, 1),  // 元组的第二个元素是闪电贷回执
            pool: None, // Cetus的flash_swap不直接返回pool对象作为结果的一部分给PTB，所以是None
        })
    }

    /// `extend_repay_tx`
    ///
    /// 将偿还Cetus闪电贷的操作添加到现有的PTB中。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文。
    /// - `coin_to_repay_arg`: 用于偿还的代币的 `Argument` (必须是借入的代币类型，并包含本金+手续费)。
    /// - `flash_res`: 从 `extend_flashloan_tx` 返回的 `FlashResult`，主要使用其中的 `receipt`。
    ///
    /// 返回:
    /// - `Result<Argument>`: 可能代表找零的代币 (如果偿还的多余了)，或一个空结果。
    ///   Cetus的 `repay_flash_swap` 通常返回多余的支付金额 (如果有的话)。
    async fn extend_repay_tx(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, flash_res: FlashResult) -> Result<Argument> {
        // 根据交易方向选择 `repay_flash_swap_a2b` 或 `repay_flash_swap_b2a`
        let function_name_str = if self.is_a2b() {
            "repay_flash_swap_a2b" // 如果之前是flash_swap_a2b (借A得B), 现在要用A来偿还
        } else {
            "repay_flash_swap_b2a" // 如果之前是flash_swap_b2a (借B得A), 现在要用B来偿还
        };

        let package_id = ObjectID::from_hex_literal(CETUS_DEX)?;
        let module_name = Identifier::new("cetus").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;
        
        // 泛型类型参数，与flash_swap时一致
        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() {
            type_arguments.swap(0, 1);
        }

        let call_arguments = self.build_repay_args(ctx, coin_to_repay_arg, flash_res.receipt)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx)) // repay函数通常返回一个Coin作为找零 (如果支付的超过了应还金额)
    }

    /// `extend_trade_tx` (常规交换)
    ///
    /// 将Cetus的常规交换操作添加到现有的PTB中。
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress, // 未使用
        coin_in_arg: Argument,
        _amount_in: Option<u64>, // Cetus的swap函数直接使用传入Coin对象的全部余额
    ) -> Result<Argument> {
        // 根据交易方向选择 `swap_a2b` 或 `swap_b2a`
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        let package_id = ObjectID::from_hex_literal(CETUS_DEX)?;
        let module_name = Identifier::new("cetus").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;
        
        // 泛型类型参数，与flash_swap时类似，需要根据 is_a2b 调整顺序
        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() {
            type_arguments.swap(0, 1); // 确保泛型参数是 [CoinInType, CoinOutType]
        }
        
        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx)) // swap函数返回输出的Coin对象
    }

    // --- Dex trait 的其他 getter 和 setter 方法 ---
    fn coin_in_type(&self) -> String {
        self.coin_in_type.clone()
    }

    fn coin_out_type(&self) -> String {
        self.coin_out_type.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::Cetus // 协议类型为Cetus
    }

    fn liquidity(&self) -> u128 {
        self.liquidity
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 池的ObjectID (从原始Pool信息中获取)
    }

    /// `flip` 方法
    ///
    /// 翻转交易方向。
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        // 注意：`type_params` 在 `extend_trade_tx` 和 `extend_flashloan_tx` 中会根据 `is_a2b` 动态调整，
        // 所以这里不需要修改 `type_params` 的顺序。
    }

    /// `is_a2b` 方法
    ///
    /// 判断当前 `coin_in_type` 是否是池中定义的 "第一个" 代币 (token0)。
    /// Cetus的 `swap_a2b` / `flash_swap_a2b` 通常指 token0 -> token1。
    fn is_a2b(&self) -> bool {
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试)
    ///
    /// 构建一个完整的、独立的常规交换交易。
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await;

        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;

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
    use std::{str::FromStr, time::Instant};

    use itertools::Itertools; // 用于迭代器操作
    use object_pool::ObjectPool; // 对象池
    use simulator::{DBSimulator, SimulateCtx, Simulator}; // 模拟器
    use sui_sdk::SuiClientBuilder; // Sui客户端构建器
    use tracing::info; // 日志

    use super::*; // 导入外部模块 (cetus.rs)
    use crate::{
        common::get_latest_epoch, // 获取最新纪元信息的函数
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL}, // 测试配置
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器
    };

    /// `test_cetus_swap_tx` 测试函数
    ///
    /// 测试通过Cetus进行常规交换的流程。
    // 可以通过以下命令单独运行此测试:
    // cargo test --package arb --bin arb --all-features -- defi::cetus::tests::test_cetus_swap_tx --exact --show-output
    #[tokio::test]
    async fn test_cetus_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        // 定义测试参数
        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap();
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI"; // 输入SUI
        // DEEP是Cetus上的一个代币示例
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

        // 从找到的DEX中筛选出Cetus协议的池，并选择流动性最大的那个。
        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::Cetus) // 过滤Cetus池
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 按流动性排序
            .last() // 取流动性最大的
            .expect("测试中未找到Cetus的池");

        // 使用选定的DEX实例构建交换交易数据
        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 构建的交易数据: {:?}", tx_data);

        // --- 使用一个连接到真实慢速数据库的DBSimulator进行模拟 ---
        // 这部分用于更接近真实链上状态的模拟。
        let start_time = Instant::now();
        // 注意：这里的路径是硬编码的，需要根据实际环境修改。
        let db_sim = DBSimulator::new_slow(
            "/home/ubuntu/sui-nick/db/live/store", // 数据库路径
            "/home/ubuntu/sui-nick/fullnode.yaml", // 全节点配置文件路径
            None,
            None,
        )
        .await;
        info!("DBSimulator::new_slow 初始化耗时: {:?}", start_time.elapsed());

        // 获取最新的纪元信息用于模拟上下文
        let sui_client = SuiClientBuilder::default().build(TEST_HTTP_URL).await.unwrap();
        let epoch = get_latest_epoch(&sui_client).await.unwrap();
        let sim_ctx = SimulateCtx::new(epoch, vec![]); // 创建模拟上下文

        // 执行交易模拟
        let sim_start_time = Instant::now();
        let db_res = db_sim.simulate(tx_data, sim_ctx).await.unwrap();
        info!("🧀 数据库模拟耗时 {:?}, 结果: {:?}", sim_start_time.elapsed(), db_res);

        // 断言交易模拟成功
        assert!(db_res.is_ok(), "数据库模拟应成功");
    }
}
