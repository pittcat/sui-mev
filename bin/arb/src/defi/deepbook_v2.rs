// 该文件 `deepbook_v2.rs` 实现了与 DeepBook V2 协议交互的逻辑。
// DeepBook 是 Sui 原生的中央限价订单簿 (CLOB) 去中心化交易所。
// 与AMM（自动做市商）不同，CLOB允许用户提交限价单和市价单，类似于传统交易所的交易方式。
//
// 文件概览:
// 1. 定义了与 DeepBook V2 相关的常量，主要是 `ACCOUNT_CAP` 对象ID。
//    `AccountCap` (账户能力) 对象是与DeepBook交互（如下单、取消订单）时必需的，代表用户的交易账户权限。
// 2. `ObjectArgs` 结构体: 用于缓存 `Clock` 和 `AccountCap` 对象的 `ObjectArg`。
// 3. `DeepbookV2` 结构体: 代表一个DeepBook V2的交易池 (订单簿)。
//    它实现了 `Dex` trait。
// 4. `new()` 方法: 用于根据链上数据初始化 `DeepbookV2` 实例。
// 5. `swap_tx()` / `build_swap_args()`: 构建在DeepBook V2上执行交换（通常是市价单吃单）的交易参数和PTB。
//    DeepBook的交换函数也区分 `swap_a2b` 和 `swap_b2a`。
// 6. 实现了 `Dex` trait 的方法。值得注意的是，这里的 `extend_trade_tx` 方法也使用了 `CETUS_AGGREGATOR` 的包ID，
//    这表明与DeepBook的交互可能是通过Cetus的聚合器合约进行的，该聚合器能够将市价单路由到DeepBook。
//
// Sui/DeFi概念:
// - Central Limit Order Book (CLOB): 中央限价订单簿。一种交易机制，买卖双方提交带有价格和数量的订单，
//   系统将匹配的买单和卖单进行撮合。DeepBook是Sui官方支持的CLOB实现。
// - AccountCap (Account Capability): 在DeepBook中，用户需要一个 `AccountCap` 对象来与其账户关联的订单进行交互。
//   这个对象由 `create_account` 函数创建，并作为后续交易操作（如 `place_market_order`）的凭证。
// - Liquidity (流动性): 对于订单簿交易所，流动性通常指订单簿的深度，即在不同价位有多少买单和卖单。
//   这里的 `liquidity()` 方法返回0，可能是因为从`dex_indexer`获取的`Pool`信息中的流动性字段不直接适用于订单簿，
//   或者这里的实现没有去主动查询订单簿深度。

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
use utils::{coin, new_test_sui_client, object::shared_obj_arg}; // 自定义工具库

use super::{TradeCtx, CETUS_AGGREGATOR}; // 从父模块(defi)引入 `TradeCtx` 和 `CETUS_AGGREGATOR`常量
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 `Dex` trait

// DeepBook V2 交互时可能需要的 `AccountCap` 对象ID。
// 这个ID通常是用户首次与DeepBook交互（创建账户）时生成的。
// 对于一个通用的套利机器人，它需要有自己的 `AccountCap` 来进行交易。
// 这里的常量可能是一个预先创建好的、机器人专用的 `AccountCap` ID。
const ACCOUNT_CAP: &str = "0xc1928315ba33482366465426bdb179c7000f557838ae5d945e96263373f24b32";

/// `ObjectArgs` 结构体
///
/// 缓存DeepBook V2交互所需的关键对象的 `ObjectArg` 形式。
#[derive(Clone)]
pub struct ObjectArgs {
    clock: ObjectArg,       // Sui时钟对象的ObjectArg
    account_cap: ObjectArg, // 用户AccountCap对象的ObjectArg
}

// 用于缓存 `ObjectArgs` 的静态 `OnceCell`
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数
///
/// 获取并缓存 `ObjectArgs` (包含clock, account_cap)。
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let account_cap_id = ObjectID::from_hex_literal(ACCOUNT_CAP).unwrap();
            // 获取 AccountCap 对象。注意：AccountCap 通常是用户的私有对象，不是共享对象。
            // 其 ObjectArg 类型应为 ImmOrOwnedObject。
            let account_cap_obj = simulator.get_object(&account_cap_id).await.unwrap();

            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();
            ObjectArgs {
                clock: shared_obj_arg(&clock_obj, false), // Clock是共享只读对象
                // `account_cap_obj.compute_object_reference()` 获取该对象的引用 (ID, version, digest)
                // `ObjectArg::ImmOrOwnedObject` 用于将私有对象作为参数传递给Move调用。
                account_cap: ObjectArg::ImmOrOwnedObject(account_cap_obj.compute_object_reference()),
            }
        })
        .await
        .clone()
}

/// `DeepbookV2` 结构体
///
/// 代表一个DeepBook V2的交易对（订单簿）。
#[derive(Clone)]
pub struct DeepbookV2 {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息 (代表一个交易对的订单簿)
    pool_arg: ObjectArg,     // 订单簿池对象的 `ObjectArg`
    coin_in_type: String,    // 当前交易方向的输入代币类型 (Base Coin)
    coin_out_type: String,   // 当前交易方向的输出代币类型 (Quote Coin)
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是 [BaseCoinType, QuoteCoinType])
    // 共享或必需的对象参数
    clock: ObjectArg,
    account_cap: ObjectArg,
}

impl DeepbookV2 {
    /// `new` 构造函数
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `DeepbookV2` DEX实例。
    ///
    /// 参数:
    /// - `simulator`: 共享的模拟器实例。
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)，代表一个DeepBook的交易对。
    /// - `coin_in_type`: 输入代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `DeepbookV2` 实例，否则返回错误。
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 确保池协议是DeepbookV2
        ensure!(pool_info.protocol == Protocol::DeepbookV2, "提供的不是DeepbookV2协议的池");

        // 获取并解析池对象的Move结构体内容 (DeepBook的Pool对象)
        let pool_obj = simulator
            .get_object(&pool_info.pool) // pool_info.pool 是订单簿池的ObjectID
            .await
            .ok_or_else(|| eyre!("DeepbookV2池对象未找到: {}", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("DeepbookV2池对象的布局(layout)未找到")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // DeepBook的池对象通常直接在其泛型参数中定义交易对的两种代币类型。
        // 例如 `Pool<BaseCoin, QuoteCoin>`。
        // `parsed_pool_struct.type_.type_params` 应该包含这两种代币的TypeTag。
        let type_params = parsed_pool_struct.type_.type_params.clone();
        ensure!(type_params.len() == 2, "DeepBookV2池的泛型参数应为两种代币类型");

        // 根据输入代币推断输出代币。
        // `pool_info.token_index(coin_in_type)` 返回输入代币在池代币对中的索引 (0 或 1)。
        // DeepBook通常有BaseCoin (token0) 和 QuoteCoin (token1)。
        let coin_out_type = if let Some(0) = pool_info.token_index(coin_in_type) {
            pool_info.token1_type() // 如果输入是token0 (BaseCoin), 输出是token1 (QuoteCoin)
        } else {
            pool_info.token0_type() // 如果输入是token1 (QuoteCoin), 输出是token0 (BaseCoin)
        };

        // 将池对象转换为 `ObjectArg` (在交易中通常是可变的，因为订单簿会更新)
        let pool_arg = shared_obj_arg(&pool_obj, true);
        // 获取共享的协议对象参数 (clock, account_cap)
        let ObjectArgs { clock, account_cap } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(),
            pool_arg,
            coin_in_type: coin_in_type.to_string(),
            coin_out_type,
            type_params, // 通常是 [BaseCoinType, QuoteCoinType]
            clock,
            account_cap,
        })
    }

    /// `swap_tx` (私有辅助函数，与`Dex` trait中的`swap_tx`名称冲突，但签名不同，这里改为 `build_pt_swap_tx`)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在DeepBookV2池中执行一次市价单交换。
    ///
    /// 参数:
    /// - `sender`: 交易发送者地址。
    /// - `recipient`: 接收输出代币的地址。
    /// - `coin_in_ref`: 输入代币对象的引用。
    /// - `amount_in`: 输入代币的数量。
    ///
    /// 返回:
    /// - `Result<ProgrammableTransaction>`: 构建好的PTB。
    async fn build_pt_swap_tx( // 重命名以避免与 Dex trait 中的 swap_tx 混淆
        &self,
        sender: SuiAddress,
        recipient: SuiAddress,
        coin_in_ref: ObjectRef,
        amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default(); // 创建交易上下文

        // 如果需要，分割输入代币
        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        // 将DeepBook交换操作添加到PTB
        // `None` 表示 `amount_in` 参数对于 `extend_trade_tx` 是可选的或不直接使用u64值
        // (DeepBook的市价单函数通常消耗整个传入的Coin对象作为输入)。
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;
        // 将输出代币转移给接收者
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish()) // 完成并返回PTB
    }

    /// `build_swap_args` (私有辅助函数)
    ///
    /// 构建调用DeepBook V2交换方法 (如 `swap_a2b` 或 `swap_b2a`，在聚合器中可能是 `place_market_order`的封装) 所需的参数列表。
    /// 合约方法签名示例 (来自DeepBook V2的 `router` 模块的 `place_market_order`):
    /// `public fun place_market_order<BaseAsset, QuoteAsset>(
    ///     pool: &mut Pool<BaseAsset, QuoteAsset>,
    ///     account_cap: &AccountCapability,
    ///     client_order_id: u64, // 客户端生成的订单ID，用于追踪
    ///     is_bid: bool,         // true表示买单 (用QuoteAsset买BaseAsset), false表示卖单 (卖BaseAsset换QuoteAsset)
    ///     quantity: Coin<TY>,   // 支付的代币对象
    ///     base_coin_minimum_out: u64,  // 对于卖单，期望最少收到的BaseAsset数量
    ///     quote_coin_minimum_out: u64, // 对于买单，期望最少收到的QuoteAsset数量
    ///     clock: &Clock,
    ///     ctx: &mut TxContext
    /// ): Coin<TYR>`
    ///
    /// 注意：这里的 `build_swap_args` 是为Cetus聚合器中的 `deepbook::swap_a2b` 或 `deepbook::swap_b2a` 准备参数。
    /// 这些聚合器函数签名可能更简单，例如 (来自注释):
    /// `public fun swap_a2b<CoinA, CoinB>(pool: &mut Pool<CoinA, CoinB>, coin_a: Coin<CoinA>, account_cap: &AccountCap, clock: &Clock, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数包括: pool, 输入的coin对象, account_cap, clock。
    async fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        // AccountCap 通常是 ImmOrOwnedObject，ctx.obj() 应该能正确处理
        let account_cap_arg = ctx.obj(self.account_cap).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        // 返回参数列表，顺序必须与聚合器中 deepbook 模块的 swap_a2b/swap_b2a 函数签名一致。
        Ok(vec![pool_arg, coin_in_arg, account_cap_arg, clock_arg])
    }
}

/// 为 `DeepbookV2` 结构体实现 `Dex` trait。
#[async_trait::async_trait]
impl Dex for DeepbookV2 {
    /// `extend_trade_tx`
    ///
    /// 将DeepBook V2的交换操作（通过Cetus聚合器）添加到现有的PTB中。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文。
    /// - `_sender`: 发送者地址 (未使用)。
    /// - `coin_in_arg`: 输入代币的 `Argument`。
    /// - `_amount_in`: 输入金额 (未使用，因为聚合器的swap函数直接使用输入Coin对象的全部余额)。
    ///
    /// 返回:
    /// - `Result<Argument>`: 代表输出代币的 `Argument`。
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress,
        coin_in_arg: Argument,
        _amount_in: Option<u64>, // DeepBook市价单通常消耗整个传入的Coin对象
    ) -> Result<Argument> {
        // 根据 `is_a2b()` 的结果选择调用聚合器中的 `swap_a2b` 还是 `swap_b2a` 函数。
        // `is_a2b()` 判断当前 `coin_in_type` 是否是池中的 "BaseCoin" (通常是交易对的第一个代币)。
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        // 这表明这里的DeepBook V2交易是通过Cetus的聚合器合约来执行的。
        // Cetus聚合器在其内部会调用实际的DeepBook V2合约逻辑 (如 place_market_order)。
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("deepbook").map_err(|e| eyre!(e))?; // 聚合器中与DeepBook交互的模块
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        // 泛型类型参数，通常是 `[BaseCoinType, QuoteCoinType]`。
        // `self.type_params` 在 `DeepbookV2::new` 中被设置为池的两种代币类型。
        // 需要确保这里的顺序与聚合器中 `swap_a2b` / `swap_b2a` 的泛型参数顺序匹配。
        // 如果 `is_a2b()` 为true (输入Base, 输出Quote), 泛型参数应为 [Base, Quote]。
        // 如果 `self.type_params` 是 `[BaseCoinType, QuoteCoinType]`，则对于 `swap_a2b` 是正确的。
        // 对于 `swap_b2a` (输入Quote, 输出Base)，泛型参数应为 [Quote, Base]。
        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() { // 如果是 B to A (即 coin_in is QuoteCoin)
            type_arguments.swap(0, 1); // 交换泛型参数顺序，变为 [QuoteCoin, BaseCoin]
        }

        // 构建调用参数
        let call_arguments = self.build_swap_args(ctx, coin_in_arg).await?;

        // 添加Move调用命令到PTB
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        // 返回代表输出代币的Argument (通常是最后一个命令的结果)
        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx))
    }

    // --- Dex trait 的其他 getter 方法 ---
    fn coin_in_type(&self) -> String {
        self.coin_in_type.clone()
    }

    fn coin_out_type(&self) -> String {
        self.coin_out_type.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::DeepbookV2 // 协议类型为DeepbookV2
    }

    /// `liquidity` 方法
    ///
    /// 对于订单簿交易所，流动性的概念与AMM池不同。
    /// AMM池的流动性通常是池中代币的总价值或LP代币数量。
    /// 订单簿的流动性是分散在不同价位的买卖订单的总和。
    /// 从 `dex_indexer::types::Pool` 获取的 `liquidity` 字段可能不适用于DeepBook。
    /// 这里简单返回0，表示需要更复杂的方法来衡量DeepBook的实际可交易流动性 (例如查询订单簿深度)。
    fn liquidity(&self) -> u128 {
        0 // DeepBook的流动性不能简单用一个u128表示，具体取决于订单簿深度
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 返回订单簿池的ObjectID (从原始Pool信息中获取)
    }

    /// `flip` 方法
    ///
    /// 翻转交易方向 (输入币和输出币互换)。
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        // `type_params` 在 `extend_trade_tx` 中会根据 `is_a2b` 动态调整，所以这里不需要修改 `type_params`。
    }

    /// `is_a2b` 方法
    ///
    /// 判断当前 `coin_in_type` 是否是池中定义的 "第一个" 代币 (BaseCoin)。
    /// 聚合器中的 `swap_a2b` 通常指 BaseCoin -> QuoteCoin。
    fn is_a2b(&self) -> bool {
        // `self.pool` 是 `dex_indexer::types::Pool` 类型。
        // `token_index` 方法返回该代币在池代币对中的索引 (0 或 1)。
        // 假设 token0 是 BaseCoin，token1 是 QuoteCoin。
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试)
    ///
    /// 构建一个完整的、独立的交换交易。
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await; // 创建测试Sui客户端

        // 获取输入代币对象
        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;

        // 构建包含交换操作的PTB (调用内部的 build_pt_swap_tx)
        let pt = self
            .build_pt_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in) // 使用重命名后的函数
            .await?;

        // 获取Gas币并构建最终的TransactionData
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

    use super::*; // 导入外部模块 (deepbook_v2.rs)
    use crate::{
        config::tests::TEST_HTTP_URL, // 测试配置 (注意：TEST_ATTACKER在deepbook测试中未使用owner变量)
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器
    };

    /// `test_deepbookv2_swap_tx` 测试函数
    ///
    /// 测试通过DeepBookV2 (经由Cetus聚合器) 进行交换的流程。
    #[tokio::test]
    async fn test_deepbookv2_swap_tx() {
        // 初始化日志
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        // 创建一个HTTP模拟器 (用于实际模拟交易)
        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        // 定义测试参数
        // 注意：TEST_ATTACKER常量在此测试中未直接使用，而是硬编码了一个owner地址。
        // 确保这个owner地址在测试环境中有足够的SUI和AccountCap。
        let owner = SuiAddress::from_str("0xc0f620f28826593835606e174e6e9912c342101920519a1e376957691178e345").unwrap();
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI"; // 输入SUI
        // Wormhole USDC (来自以太坊的USDC)
        let token_out_type = "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN";
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

        // 从找到的DEX中筛选出DeepbookV2协议的池，并按流动性排序（尽管Deepbook流动性返回0），取最后一个。
        // 对于订单簿，流动性排序可能意义不大，除非`dex_indexer`为DeepBook提供了某种流动性估算值。
        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::DeepbookV2) // 过滤DeepbookV2池
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 按（可能是0的）流动性排序
            .last() // 取最后一个
            .expect("测试中未找到DeepbookV2的池");

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
