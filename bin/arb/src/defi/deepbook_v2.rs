// 该文件 `deepbook_v2.rs` 实现了与 DeepBook V2 协议交互的逻辑。
// DeepBook 是 Sui 原生的中央限价订单簿 (CLOB) 去中心化交易所。
// 与AMM（自动做市商）不同，CLOB允许用户提交限价单和市价单，类似于传统交易所的交易方式。
//
// **文件概览 (File Overview)**:
// 这个 `deepbook_v2.rs` 文件是专门用来和Sui区块链上的DeepBook V2这个官方“股票交易所”（订单簿式DEX）“对话”的代码。
// DeepBook V2和我们前面看到的那些AMM（如Cetus, Kriya, Turbos, FlowX, Aftermath）不一样。
// AMM是靠流动性池和数学公式自动定价的，而DeepBook V2更像传统的证券交易所：
// -   买家可以下一个“限价买单”（比如：我最多愿意花1.05美元买1个SUI币）。
// -   卖家可以下一个“限价卖单”（比如：我最少要卖1.06美元才肯卖1个SUI币）。
// -   这些买卖订单会集中在一个“订单簿”（Order Book）上，系统会自动把匹配的买单和卖单撮合起来成交。
// -   用户也可以下“市价单”，就是“不管现在什么价，立刻帮我买/卖一定数量的币”，系统会去吃掉订单簿上最优价格的单子。
// (This `deepbook_v2.rs` file contains code specifically for "communicating" with Sui's official "stock exchange" (order book style DEX) called DeepBook V2.
//  DeepBook V2 is different from the AMMs we've seen earlier (like Cetus, Kriya, Turbos, FlowX, Aftermath).
//  AMMs rely on liquidity pools and mathematical formulas for automatic pricing, whereas DeepBook V2 is more like a traditional stock exchange:
//  - Buyers can place "limit buy orders" (e.g., "I am willing to pay at most $1.05 for 1 SUI coin").
//  - Sellers can place "limit sell orders" (e.g., "I will only sell 1 SUI coin if I get at least $1.06").
//  - These buy and sell orders are collected in an "Order Book", and the system automatically matches compatible buy and sell orders for execution.
//  - Users can also place "market orders", which means "buy/sell a certain amount of coins for me immediately at whatever the current best price is"; the system will fill this by consuming the best-priced orders on the order book.)
//
// **主要内容 (Main Contents)**:
// 1.  **常量定义 (Constant Definitions)**:
//     -   `ACCOUNT_CAP`: 这是一个关键的“账户能力”（Account Capability）对象的ID。在DeepBook V2里，用户（包括机器人）如果想下单、取消订单等操作，通常需要先创建一个与自己Sui账户关联的`AccountCap`对象。这个对象就像一个“交易许可”，证明你有权在这个订单簿上进行操作。这里的常量可能指向一个预先为机器人创建好的`AccountCap`对象。
//
// 2.  **`ObjectArgs` 结构体与 `OBJ_CACHE`**:
//     -   `ObjectArgs`: 用来打包缓存Sui系统时钟对象和上面提到的`ACCOUNT_CAP`对象的引用信息。
//     -   `OBJ_CACHE`: 一个一次性初始化并全局共享的缓存。
//
// 3.  **`DeepbookV2` 结构体**:
//     -   代表DeepBook V2协议里的一个具体的“交易池”（在这里，一个“池”实际上就是一个特定代币对的订单簿，比如SUI/USDC的订单簿）。
//     -   它也实现了项目内部定义的 `Dex` 通用接口。
//
// 4.  **`new()` 构造函数**:
//     -   异步方法，根据从`dex_indexer`获取的池信息（代表一个订单簿）和指定的输入代币类型来初始化一个 `DeepbookV2` 实例。
//     -   它会去链上读取这个订单簿池对象的详细数据。DeepBook的池对象泛型参数直接定义了交易对的两种代币类型。
//
// 5.  **交易构建逻辑 (Transaction Building Logic)**:
//     -   `build_pt_swap_tx()` (原 `swap_tx`，已重命名) / `build_swap_args()`：内部辅助函数，用来准备在DeepBook V2上进行交换（通常是提交一个市价单去“吃掉”订单簿上已有的限价单）时需要发送给Sui区块链的指令和参数。
//     -   DeepBook的交换函数（或其在聚合器中的封装）也区分 `swap_a2b` 和 `swap_b2a` 方向，这通常对应于是提交买入基础代币的市价单还是卖出基础代币的市价单。
//
// 6.  **`Dex` trait 实现**:
//     -   `DeepbookV2` 结构体实现了 `Dex` 接口要求的方法。
//     -   一个非常关键的细节是，这里的常规交换 (`extend_trade_tx`) **也使用了 `CETUS_AGGREGATOR` 的包ID**。
//         这意味着，与DeepBook V2池子进行市价单交换，实际的链上调用也可能是通过Cetus协议提供的一个“聚合器”智能合约来路由的。
//         这个聚合器能够将市价单智能地发送到包括DeepBook V2在内的多个流动性场所，以寻求最佳成交价。
//         (A very key detail is that regular swaps (`extend_trade_tx`) here **also use the `CETUS_AGGREGATOR` package ID**.
//          This implies that for market order swaps with DeepBook V2 pools, the actual on-chain calls might also be routed through an "Aggregator" smart contract provided by the Cetus protocol.
//          This aggregator can intelligently send market orders to multiple liquidity venues, including DeepBook V2, to seek the best execution price.)
//     -   `liquidity()` 方法返回0。这是因为订单簿的流动性不是一个单一的数字（像AMM池那样），而是分布在不同价位上的买卖订单的总和（即“订单簿深度”）。从`dex_indexer`获取的`Pool`信息中的流动性字段可能不直接适用于订单簿，或者这里的实现没有去主动查询订单簿深度来估算一个等效值。
//
// **Sui区块链和DeFi相关的概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **中央限价订单簿 (CLOB / Central Limit Order Book)**:
//     一种经典的交易所交易机制。它维护着一个包含所有未成交的买入限价单（bids）和卖出限价单（asks）的列表。
//     -   **限价单 (Limit Order)**: 用户指定一个价格和数量（例如，“我想以不高于$1.05的价格买入100个SUI”）。这个订单会进入订单簿，等待对手方匹配。
//     -   **市价单 (Market Order)**: 用户只指定数量，不指定价格（例如，“我想立刻买入100个SUI”）。系统会立即以当前订单簿上最优的可用价格（或一系列价格）来成交这个订单。
//     -   **撮合 (Matching)**: 当一个新的买单价格高于或等于订单簿上最低的卖单价格，或者一个新的卖单价格低于或等于订单簿上最高的买单价格时，交易就会发生。
//     DeepBook是Sui官方提供的、完全上链的CLOB实现。
//
// -   **AccountCap (账户能力 / Account Capability)**:
//     在DeepBook V2中，用户与订单簿进行交互（如下单、撤单、提取资金等）之前，通常需要先为自己的Sui账户创建一个“账户能力”（`AccountCap`）对象。
//     这个`AccountCap`对象与用户的Sui地址绑定，并作为一种授权凭证，证明该用户有权管理其在DeepBook上的订单和资金。
//     它通常是通过调用DeepBook合约的 `create_account` 函数来创建的，并且是一个归用户所有的私有对象。
//     在后续的交易操作中（比如 `place_market_order`），需要将这个 `AccountCap` 对象作为参数传入，以验证操作的合法性。
//     机器人进行交易时，也需要一个预先创建好的、属于机器人操作地址的 `AccountCap` 对象。

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::sync::Arc; // 原子引用计数 (Atomic Reference Counting)

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` 引入Pool和Protocol类型 (Import Pool and Protocol types from `dex_indexer`)
use eyre::{ensure, eyre, OptionExt, Result}; // 错误处理库 (Error handling library)
use move_core_types::annotated_value::MoveStruct; // Move核心类型 (Move core types)
use simulator::Simulator; // 交易模拟器接口 (Transaction simulator interface)
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型 (Sui basic types)
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建类型 (Sui transaction building types)
    Identifier, TypeTag, SUI_CLOCK_OBJECT_ID, // Sui标识符, 类型标签, 时钟对象ID (Sui Identifier, TypeTag, Clock Object ID)
};
use tokio::sync::OnceCell; // Tokio异步单次初始化单元 (Tokio asynchronous single initialization cell)
use utils::{coin, new_test_sui_client, object::shared_obj_arg}; // 自定义工具库 (Custom utility library)

use super::{TradeCtx, CETUS_AGGREGATOR}; // 从父模块(defi)引入 `TradeCtx` 和 `CETUS_AGGREGATOR`常量
                                         // (Import `TradeCtx` and `CETUS_AGGREGATOR` constant from parent module (defi))
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 `Dex` trait (Import config and `Dex` trait from current crate)

// DeepBook V2 交互时可能需要的 `AccountCap` 对象ID。
// (ObjectID of the `AccountCap` object possibly required for DeepBook V2 interaction.)
// 这个ID通常是用户首次与DeepBook交互（创建账户）时生成的。
// (This ID is usually generated when a user first interacts with DeepBook (creates an account).)
// 对于一个通用的套利机器人，它需要有自己的 `AccountCap` 来进行交易。
// (For a general arbitrage bot, it needs its own `AccountCap` to trade.)
// 这里的常量可能是一个预先创建好的、机器人专用的 `AccountCap` ID。
// (The constant here might be a pre-created `AccountCap` ID dedicated to the bot.)
const ACCOUNT_CAP: &str = "0xc1928315ba33482366465426bdb179c7000f557838ae5d945e96263373f24b32";

/// `ObjectArgs` 结构体 (对象参数结构体 / Object Arguments Struct)
///
/// 缓存DeepBook V2交互所需的关键对象的 `ObjectArg` 形式。
/// (Caches the `ObjectArg` form of key objects required for DeepBook V2 interaction.)
#[derive(Clone)]
pub struct ObjectArgs {
    clock: ObjectArg,       // Sui时钟对象的ObjectArg (Sui clock object's ObjectArg)
    account_cap: ObjectArg, // 用户AccountCap对象的ObjectArg (User's AccountCap object's ObjectArg)
}

// 用于缓存 `ObjectArgs` 的静态 `OnceCell` (Static `OnceCell` for caching `ObjectArgs`)
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数 (获取对象参数函数 / Get Object Arguments Function)
///
/// 获取并缓存 `ObjectArgs` (包含clock, account_cap)。
/// (Fetches and caches `ObjectArgs` (containing clock, account_cap).)
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let account_cap_id = ObjectID::from_hex_literal(ACCOUNT_CAP).unwrap();
            // 获取 AccountCap 对象。注意：AccountCap 通常是用户的私有对象，不是共享对象。
            // (Get AccountCap object. Note: AccountCap is usually a user's private object, not a shared object.)
            // 其 ObjectArg 类型应为 ImmOrOwnedObject。
            // (Its ObjectArg type should be ImmOrOwnedObject.)
            let account_cap_obj = simulator.get_object(&account_cap_id).await.unwrap();

            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();
            ObjectArgs {
                clock: shared_obj_arg(&clock_obj, false), // Clock是共享只读对象 (Clock is a shared read-only object)
                // `account_cap_obj.compute_object_reference()` 获取该对象的引用 (ID, version, digest)
                // (`account_cap_obj.compute_object_reference()` gets the object's reference (ID, version, digest))
                // `ObjectArg::ImmOrOwnedObject` 用于将私有对象作为参数传递给Move调用。
                // (`ObjectArg::ImmOrOwnedObject` is used to pass private objects as arguments to Move calls.)
                account_cap: ObjectArg::ImmOrOwnedObject(account_cap_obj.compute_object_reference()),
            }
        })
        .await
        .clone()
}

/// `DeepbookV2` 结构体 (DeepbookV2 Struct)
///
/// 代表一个DeepBook V2的交易对（订单簿）。
/// (Represents a trading pair (order book) of DeepBook V2.)
#[derive(Clone)]
pub struct DeepbookV2 {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息 (代表一个交易对的订单簿)
                             // (Original pool information from `dex_indexer` (representing an order book for a trading pair))
    pool_arg: ObjectArg,     // 订单簿池对象的 `ObjectArg` (Order book pool object's `ObjectArg`)
    coin_in_type: String,    // 当前交易方向的输入代币类型 (Base Coin)
                             // (Input coin type for the current trading direction (Base Coin))
    coin_out_type: String,   // 当前交易方向的输出代币类型 (Quote Coin)
                             // (Output coin type for the current trading direction (Quote Coin))
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是 [BaseCoinType, QuoteCoinType])
                              // (Generic type parameters needed when calling the contract (usually [BaseCoinType, QuoteCoinType]))
    // 共享或必需的对象参数 (Shared or required object parameters)
    clock: ObjectArg,
    account_cap: ObjectArg,
}

impl DeepbookV2 {
    /// `new` 构造函数 (new constructor)
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `DeepbookV2` DEX实例。
    /// (Creates a `DeepbookV2` DEX instance based on `Pool` information provided by `dex_indexer` and the input coin type.)
    ///
    /// 参数 (Parameters):
    /// - `simulator`: 共享的模拟器实例。(Shared simulator instance.)
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)，代表一个DeepBook的交易对。
    ///                (Pool information from `dex_indexer` (`&Pool`), representing a DeepBook trading pair.)
    /// - `coin_in_type`: 输入代币的类型字符串。(Type string of the input coin.)
    ///
    /// 返回 (Returns):
    /// - `Result<Self>`: 成功则返回 `DeepbookV2` 实例，否则返回错误。(Returns a `DeepbookV2` instance if successful, otherwise an error.)
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        ensure!(pool_info.protocol == Protocol::DeepbookV2, "提供的不是DeepbookV2协议的池 (Provided pool is not of DeepbookV2 protocol)");

        let pool_obj = simulator.get_object(&pool_info.pool).await
            .ok_or_else(|| eyre!("DeepbookV2池对象 {} 未找到 (DeepbookV2 pool object {} not found)", pool_info.pool))?;

        let parsed_pool_struct = { // 解析池对象的Move结构 (Parse the Move struct of the pool object)
            let layout = simulator.get_object_layout(&pool_info.pool)
                .ok_or_eyre(format!("DeepbookV2池 {} 布局未找到 (Layout for DeepbookV2 pool {} not found)", pool_info.pool))?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre(format!("对象 {} 非Move对象 (Object {} is not a Move object)", pool_info.pool))?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!("反序列化DeepbookV2池 {} 失败: {} (Failed to deserialize DeepbookV2 pool {}: {})", pool_info.pool, e))?
        };

        // DeepBook的池对象泛型参数直接定义了交易对的两种代币类型 [BaseCoin, QuoteCoin]。
        // (DeepBook's pool object generic parameters directly define the two coin types of the trading pair [BaseCoin, QuoteCoin].)
        let type_params = parsed_pool_struct.type_.type_params.clone();
        ensure!(type_params.len() == 2, "DeepBookV2池泛型参数应为两种代币 (DeepBookV2 pool should have two generic type parameters for coins)");

        let coin_out_type = if let Some(0) = pool_info.token_index(coin_in_type) { // 如果输入是token0 (BaseCoin)
            pool_info.token1_type() // 则输出是token1 (QuoteCoin)
        } else { // 否则输入是token1 (QuoteCoin)
            pool_info.token0_type() // 则输出是token0 (BaseCoin)
        };

        let pool_arg = shared_obj_arg(&pool_obj, true); // 池对象在交易中是可变的 (Pool object is mutable in transactions)
        let ObjectArgs { clock, account_cap } = get_object_args(simulator).await; // 获取共享参数 (Get shared arguments)

        Ok(Self {
            pool: pool_info.clone(), pool_arg,
            coin_in_type: coin_in_type.to_string(), coin_out_type,
            type_params, // [BaseCoinType, QuoteCoinType]
            clock, account_cap,
        })
    }

    /// `build_pt_swap_tx` (原 `swap_tx`，已重命名以避免与 `Dex` trait 中的同名函数混淆 / Original `swap_tx`, renamed to avoid conflict with `Dex` trait's method)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在DeepBookV2池中执行一次市价单交换。
    /// (Builds a complete Sui Programmable Transaction (PTB) for executing a market order swap in a DeepBookV2 pool.)
    async fn build_pt_swap_tx(
        &self, sender: SuiAddress, recipient: SuiAddress,
        coin_in_ref: ObjectRef, amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default();
        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?; // None for amount_in
        ctx.transfer_arg(recipient, coin_out_arg);
        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数，构建合约调用参数 / Private helper, builds contract call arguments)
    ///
    /// 构建调用DeepBook V2交换方法 (如聚合器中的 `deepbook::swap_a2b`) 所需的参数列表。
    /// (Builds the argument list for calling DeepBook V2 swap methods (e.g., `deepbook::swap_a2b` in an aggregator).)
    /// 聚合器函数签名示例: `fun swap_a2b<CoinA, CoinB>(pool: &mut Pool<CoinA, CoinB>, coin_a: Coin<CoinA>, account_cap: &AccountCap, clock: &Clock, ctx: &mut TxContext): Coin<CoinB>`
    /// (Example aggregator function signature: ...)
    async fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let account_cap_arg = ctx.obj(self.account_cap).map_err(|e| eyre!(e))?; // AccountCap是私有对象，ctx.obj应能处理
                                                                              // (AccountCap is a private object, ctx.obj should handle it)
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;
        Ok(vec![pool_arg, coin_in_arg, account_cap_arg, clock_arg]) // 参数顺序 (Argument order)
    }
}

/// 为 `DeepbookV2` 结构体实现 `Dex` trait。(Implement `Dex` trait for `DeepbookV2` struct.)
#[async_trait::async_trait]
impl Dex for DeepbookV2 {
    /// `extend_trade_tx` (将DeepBook V2交换操作添加到PTB / Add DeepBook V2 swap op to PTB)
    ///
    /// 通过Cetus聚合器执行DeepBook V2的市价单交换。
    /// (Executes a DeepBook V2 market order swap via the Cetus aggregator.)
    async fn extend_trade_tx(
        &self, ctx: &mut TradeCtx, _sender: SuiAddress,
        coin_in_arg: Argument, _amount_in: Option<u64>, // DeepBook市价单消耗整个传入Coin (DeepBook market order consumes the entire passed Coin)
    ) -> Result<Argument> {
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        // (**IMPORTANT**: Package ID uses `CETUS_AGGREGATOR`.)
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("deepbook").map_err(|e| eyre!(e))?; // 聚合器中与DeepBook交互的模块 (Module in aggregator for DeepBook interaction)
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        let mut type_arguments = self.type_params.clone(); // [BaseCoin, QuoteCoin]
        if !self.is_a2b() { // 如果是 B->A (即 coin_in is QuoteCoin) (If B->A (i.e., coin_in is QuoteCoin))
            type_arguments.swap(0, 1); // 交换为 [QuoteCoin, BaseCoin] (Swap to [QuoteCoin, BaseCoin])
        }

        let call_arguments = self.build_swap_args(ctx, coin_in_arg).await?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        Ok(Argument::Result(ctx.last_command_idx())) // 返回输出代币 (Return the output coin)
    }

    // --- Dex trait 的其他 getter 方法 ---
    // (Other getter methods for Dex trait)
    fn coin_in_type(&self) -> String { self.coin_in_type.clone() }
    fn coin_out_type(&self) -> String { self.coin_out_type.clone() }
    fn protocol(&self) -> Protocol { Protocol::DeepbookV2 } // 协议类型为DeepbookV2 (Protocol type is DeepbookV2)

    /// `liquidity` 方法 (流动性 / Liquidity method)
    /// 对于订单簿，流动性不是单一数值，这里返回0。实际流动性需查订单簿深度。
    /// (For an order book, liquidity is not a single value; returns 0 here. Actual liquidity requires checking order book depth.)
    fn liquidity(&self) -> u128 { 0 }

    fn object_id(&self) -> ObjectID { self.pool.pool } // 订单簿池的ObjectID (Order book pool's ObjectID)

    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        // type_params 在 extend_trade_tx 中根据 is_a2b() 动态调整，这里无需修改。
        // (type_params are dynamically adjusted in extend_trade_tx based on is_a2b(), no need to modify here.)
    }

    fn is_a2b(&self) -> bool { // 判断 coin_in_type 是否是池的 BaseCoin (token0)
                              // (Check if coin_in_type is the pool's BaseCoin (token0))
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试 / Mainly for testing)
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await;
        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;
        let pt = self.build_pt_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in).await?; // 调用重命名后的内部函数 (Call renamed internal function)
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
        config::tests::TEST_HTTP_URL, // 注意：TEST_ATTACKER 在此未使用 (Note: TEST_ATTACKER is unused here)
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher},
    };

    /// `test_deepbookv2_swap_tx` 测试函数 (test_deepbookv2_swap_tx test function)
    #[tokio::test]
    async fn test_deepbookv2_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);
        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        // DeepBook测试需要一个已经创建了AccountCap的地址
        // (DeepBook test requires an address that has already created an AccountCap)
        let owner = SuiAddress::from_str("0xc0f620f28826593835606e174e6e9912c342101920519a1e376957691178e345").unwrap();
        let recipient = SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI";
        let token_out_type = "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN"; // Wormhole USDC
        let amount_in = 10000; // 0.00001 SUI

        let simulator_pool_for_searcher = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new().unwrap().block_on(async { Box::new(DBSimulator::new_test(true).await) as Box<dyn Simulator> })
        }));

        let searcher = IndexerDexSearcher::new(TEST_HTTP_URL, simulator_pool_for_searcher).await.unwrap();
        let dexes = searcher.find_dexes(token_in_type, Some(token_out_type.into())).await.unwrap();
        info!("🧀 找到的DEX数量 (Number of DEXs found): {}", dexes.len());

        let dex_to_test = dexes.into_iter()
            .filter(|dex| dex.protocol() == Protocol::DeepbookV2)
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 流动性对于DeepBook可能不直接适用 (Liquidity might not be directly applicable for DeepBook)
            .last()
            .expect("测试中未找到DeepbookV2的池 (DeepbookV2 pool not found in test)");

        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 构建的交易数据 (Constructed transaction data): {:?}", tx_data);

        let response = http_simulator.simulate(tx_data, Default::default()).await.unwrap();
        info!("🧀 模拟结果 (Simulation result): {:?}", response);

        assert!(response.is_ok(), "交易模拟应成功 (Transaction simulation should succeed)");
    }
}

[end of bin/arb/src/defi/deepbook_v2.rs]
