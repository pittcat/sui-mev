// 该文件 `turbos.rs` 实现了与 Turbos Finance 协议（一个Sui区块链上的去中心化交易所DEX）交互的逻辑。
// Turbos Finance 也是一个采用 CLMM (集中流动性做市商) 模型的DEX，允许更高效的流动性利用。
//
// **文件概览 (File Overview)**:
// 这个 `turbos.rs` 文件是专门用来和Sui区块链上的Turbos Finance这个DeFi协议“对话”的代码。
// Turbos Finance也是一个“去中心化交易所”（DEX），并且和Cetus、Kriya CLMM、FlowX CLMM一样，都使用了CLMM（集中流动性做市商）这种允许你把钱更精确地放到特定价格范围的技术。
// (This `turbos.rs` file contains code specifically for interacting with the Turbos Finance DeFi protocol on the Sui blockchain.
//  Turbos Finance is also a Decentralized Exchange (DEX). Like Cetus, Kriya CLMM, and FlowX CLMM, it uses the CLMM (Concentrated Liquidity Market Maker) model,
//  which allows you to place your funds more precisely within specific price ranges.)
//
// **主要内容 (Main Contents)**:
// 1.  **常量定义 (Constant Definitions)**:
//     -   `VERSIONED`: Turbos协议可能用到的一个“版本化对象”的ID。这个对象用于管理合约升级或版本控制，与Kriya CLMM和FlowX CLMM中的类似概念相似。
//         (The ID of a "versioned object" that the Turbos protocol might use. This object is used for managing contract upgrades or version control, similar to concepts in Kriya CLMM and FlowX CLMM.)
//
// 2.  **`ObjectArgs` 结构体与 `OBJ_CACHE`**:
//     -   `ObjectArgs` 用来打包缓存上面提到的 `VERSIONED` 对象和Sui系统时钟对象的引用信息。
//     -   `OBJ_CACHE` 是一个一次性初始化并全局共享的缓存。
//
// 3.  **`Turbos` 结构体**:
//     -   代表Turbos协议里的一个具体的CLMM交易池实例。
//     -   包含了与该池交互所需的信息，如原始池信息、池对象的引用、流动性、代币类型、以及调用合约所需的类型参数（对于Turbos，这通常是三种：两种代币类型和一个手续费等级类型）。
//     -   它也实现了项目内部定义的 `Dex` 通用接口。
//
// 4.  **`new()` 构造函数**:
//     -   异步方法，根据从`dex_indexer`获取的池信息和指定的输入代币类型来初始化一个 `Turbos` 实例。
//     -   它会去链上读取池子对象的详细数据，检查池子是否“已解锁”（`unlocked`字段，表示是否可交易），并提取流动性等信息。
//
// 5.  **常规交换相关方法 (Regular Swap Methods)**:
//     -   `build_pt_swap_tx()` (原 `swap_tx`，已重命名以区分 `Dex` trait中的同名方法) / `build_swap_args()`: 构建普通代币交换所需的交易参数和PTB指令。
//     -   一个关键点是，这里的常规交换 (`extend_trade_tx`) **也使用了 `CETUS_AGGREGATOR` 的包ID**。
//         这意味着，与Turbos池子进行常规交换，实际的链上调用也可能是通过Cetus协议提供的一个“聚合器”（Aggregator）智能合约来路由的。
//         这个聚合器能够与包括Turbos在内的多个不同DEX协议的池子进行交互。
//         (A key point is that regular swaps (`extend_trade_tx`) here **also use the `CETUS_AGGREGATOR` package ID**.
//          This implies that for regular swaps with Turbos pools, the actual on-chain calls might also be routed through an "Aggregator" smart contract provided by the Cetus protocol.
//          This aggregator can interact with pools from multiple different DEX protocols, including Turbos.)
//
// 6.  **`Dex` trait 实现**:
//     -   `Turbos` 结构体实现了 `Dex` 接口要求的其他方法，如 `coin_in_type()`, `coin_out_type()`, `protocol()`, `liquidity()`, `object_id()`, `flip()`, `is_a2b()`。
//     -   值得注意的是，当前 `Turbos` 实现的 `support_flashloan()` 方法返回 `false`，并且没有实现闪电贷相关的 `extend_flashloan_tx` 和 `extend_repay_tx` 方法（它们会直接返回错误）。
//         这表明，在这个代码库的当前版本中，Turbos池的闪电贷功能可能不被支持或未被集成。
//         (It's noteworthy that the `support_flashloan()` method in the current `Turbos` implementation returns `false`, and the flashloan-related methods `extend_flashloan_tx` and `extend_repay_tx` are not implemented (they would directly return errors).
//          This indicates that in the current version of this codebase, the flash loan functionality of Turbos pools might not be supported or integrated.)
//
// **Sui区块链和DeFi相关的概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **CLMM (Concentrated Liquidity Market Maker / 集中流动性做市商)**:
//     与Cetus, Kriya CLMM, FlowX CLMM文件中的解释相同。Turbos也采用这种模型。
//     (Same explanation as in the Cetus, Kriya CLMM, and FlowX CLMM files. Turbos also adopts this model.)
//
// -   **Versioned Object (版本化对象 / Versioned Object)**:
//     与Kriya CLMM和FlowX CLMM文件中的解释类似。Turbos也使用一个全局的 `Versioned` 对象来管理协议版本或全局参数。
//     (Similar explanation to the Kriya CLMM and FlowX CLMM files. Turbos also uses a global `Versioned` object to manage protocol versions or global parameters.)
//
// -   **`unlocked` (解锁状态)**:
//     Turbos的池对象中可能有一个名为 `unlocked` 的布尔（true/false）字段。
//     这个字段用来指示该特定的交易池当前是否处于“解锁”状态，即是否允许用户进行交易。
//     如果一个池子是“锁定”的（`unlocked` 为 `false`），那么尝试与它进行交换等操作通常会失败。
//     这可以作为协议管理者临时暂停某个池子交易的一种机制。
//     (Turbos pool objects might have a boolean (true/false) field named `unlocked`.
//      This field indicates whether that specific trading pool is currently in an "unlocked" state, i.e., whether users are allowed to trade with it.
//      If a pool is "locked" (`unlocked` is `false`), attempts to perform operations like swaps with it will usually fail.
//      This can serve as a mechanism for protocol administrators to temporarily suspend trading in a particular pool.)

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
use utils::{coin, new_test_sui_client, object::*}; // 自定义工具库 (Custom utility library)

use super::{TradeCtx, CETUS_AGGREGATOR}; // 从父模块(defi)引入 `TradeCtx` 和 `CETUS_AGGREGATOR`常量
                                         // (Import `TradeCtx` and `CETUS_AGGREGATOR` constant from parent module (defi))
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 `Dex` trait (Import config and `Dex` trait from current crate)

// Turbos Finance 版本化对象ID (Versioned)
// (Turbos Finance Versioned Object ID)
// 这个对象包含了协议版本等全局信息，在调用Turbos合约时通常需要传入。
// (This object contains global information like protocol version, and is usually required when calling Turbos contracts.)
const VERSIONED: &str = "0xf1cf0e81048df168ebeb1b8030fad24b3e0b53ae827c25053fff0779c1445b6f";

/// `ObjectArgs` 结构体 (对象参数结构体 / Object Arguments Struct)
///
/// 缓存Turbos交互所需的关键对象的 `ObjectArg` 形式。
/// (Caches the `ObjectArg` form of key objects required for Turbos interaction.)
#[derive(Clone)]
pub struct ObjectArgs {
    versioned: ObjectArg, // 版本化对象的ObjectArg (Versioned object's ObjectArg)
    clock: ObjectArg,     // Sui时钟对象的ObjectArg (Sui clock object's ObjectArg)
}

// 用于缓存 `ObjectArgs` 的静态 `OnceCell` (Static `OnceCell` for caching `ObjectArgs`)
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数 (获取对象参数函数 / Get Object Arguments Function)
///
/// 获取并缓存 `ObjectArgs` (包含versioned, clock)。
/// (Fetches and caches `ObjectArgs` (containing versioned, clock).)
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let versioned_id = ObjectID::from_hex_literal(VERSIONED).unwrap();
            // 通过模拟器获取对象信息 (Fetch object information via simulator)
            let versioned_obj = simulator.get_object(&versioned_id).await.unwrap();
            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();

            ObjectArgs {
                versioned: shared_obj_arg(&versioned_obj, false), // Versioned对象通常是不可变的 (Versioned object is usually immutable)
                clock: shared_obj_arg(&clock_obj, false),       // Clock是不可变的 (Clock is immutable)
            }
        })
        .await
        .clone()
}

/// `Turbos` 结构体 (Turbos Struct)
///
/// 代表一个Turbos Finance的CLMM交易池。
/// (Represents a CLMM trading pool of Turbos Finance.)
#[derive(Clone)]
pub struct Turbos {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息 (Original pool information from `dex_indexer`)
    pool_arg: ObjectArg,     // 池对象本身的 `ObjectArg` (The pool object's own `ObjectArg`)
    liquidity: u128,         // 池的流动性 (Pool's liquidity)
    coin_in_type: String,    // 当前交易方向的输入代币类型 (Input coin type for the current trading direction)
    coin_out_type: String,   // 当前交易方向的输出代币类型 (Output coin type for the current trading direction)
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是[CoinA, CoinB, FeeTier])
                              // (Generic type parameters needed when calling the contract (usually [CoinA, CoinB, FeeTier]))
                              // Turbos的Pool对象通常有三个泛型参数: CoinA, CoinB, 和 Fee (手续费等级)。
                              // (Turbos Pool objects usually have three generic parameters: CoinA, CoinB, and Fee (fee tier).)
    // 共享的对象参数 (Shared object parameters)
    versioned: ObjectArg,
    clock: ObjectArg,
}

impl Turbos {
    /// `new` 构造函数 (new constructor)
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `Turbos` DEX实例。
    /// (Creates a `Turbos` DEX instance based on `Pool` information provided by `dex_indexer` and the input coin type.)
    ///
    /// 参数 (Parameters):
    /// - `simulator`: 共享的模拟器实例。(Shared simulator instance.)
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。(Pool information from `dex_indexer` (`&Pool`).)
    /// - `coin_in_type`: 输入代币的类型字符串。(Type string of the input coin.)
    ///
    /// 返回 (Returns):
    /// - `Result<Self>`: 成功则返回 `Turbos` 实例，否则返回错误。(Returns a `Turbos` instance if successful, otherwise an error.)
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        ensure!(pool_info.protocol == Protocol::Turbos, "提供的不是Turbos协议的池 (Provided pool is not of Turbos protocol)");

        let pool_obj = simulator
            .get_object(&pool_info.pool)
            .await
            .ok_or_else(|| eyre!("Turbos池对象未找到: {} (Turbos pool object not found: {})", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("Turbos池对象的布局(layout)未找到 (Layout for Turbos pool object not found)")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象 (Object is not a Move object)")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 检查池是否已解锁 (unlocked 字段)
        // (Check if the pool is unlocked (unlocked field))
        // Turbos的池对象可能有一个 `unlocked` 字段，表示池是否可交易。
        // (Turbos pool objects might have an `unlocked` field, indicating if the pool is tradable.)
        let unlocked = extract_bool_from_move_struct(&parsed_pool_struct, "unlocked")?;
        ensure!(unlocked, "Turbos池已锁定 (locked)，无法交易 (Turbos pool is locked, cannot trade)");

        let liquidity = extract_u128_from_move_struct(&parsed_pool_struct, "liquidity")?;

        let coin_out_type = if pool_info.token0_type() == coin_in_type {
            pool_info.token1_type().to_string()
        } else {
            pool_info.token0_type().to_string()
        };

        // 获取池本身的泛型类型参数。对于Turbos，这通常是 `[CoinTypeA, CoinTypeB, FeeType]`。
        // (Get the generic type parameters of the pool itself. For Turbos, this is usually `[CoinTypeA, CoinTypeB, FeeType]`.)
        // FeeType 是一个代表手续费等级的类型。
        // (FeeType is a type representing the fee tier.)
        let type_params = parsed_pool_struct.type_.type_params.clone();
        ensure!(type_params.len() == 3, "Turbos池的泛型参数应为三种类型 (CoinA, CoinB, Fee) (Turbos pool generic parameters should be three types (CoinA, CoinB, Fee))");

        let pool_arg = shared_obj_arg(&pool_obj, true);
        let ObjectArgs { versioned, clock } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(), liquidity,
            coin_in_type: coin_in_type.to_string(), coin_out_type,
            type_params, // 通常是 [TokenTypeA, TokenTypeB, FeeType] (Usually [TokenTypeA, TokenTypeB, FeeType])
            pool_arg, versioned, clock,
        })
    }

    /// `build_pt_swap_tx` (原 `swap_tx`，已重命名以避免与Dex trait中的同名函数混淆 / Original `swap_tx`, renamed to avoid conflict with Dex trait's method)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在Turbos池中执行一次常规交换。
    /// (Builds a complete Sui Programmable Transaction (PTB) for executing a regular swap in a Turbos pool.)
    #[allow(dead_code)]
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

    /// `build_swap_args` (私有辅助函数 / Private helper function)
    ///
    /// 构建调用Turbos常规交换方法 (如聚合器中的 `turbos::swap_a2b`) 所需的参数列表。
    /// (Builds the argument list required for calling Turbos regular swap methods (e.g., `turbos::swap_a2b` in an aggregator).)
    /// 聚合器中的函数签名可能类似于 (The function signature in an aggregator might be similar to):
    /// `public fun swap_a2b<CoinA, CoinB, Fee>(pool: &mut Pool<CoinA, CoinB, Fee>, coin_a: Coin<CoinA>, clock: &Clock, versioned: &Versioned, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数包括: pool, 输入的coin对象, clock对象, versioned对象。
    /// (Arguments include: pool, input coin object, clock object, versioned object.)
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;
        let versioned_arg = ctx.obj(self.versioned).map_err(|e| eyre!(e))?;

        // 返回参数列表，顺序必须与聚合器中 turbos 模块的 swap_a2b/swap_b2a 函数签名一致。
        // (Return the argument list; order must strictly match the swap_a2b/swap_b2a function signature in the aggregator's turbos module.)
        Ok(vec![pool_arg, coin_in_arg, clock_arg, versioned_arg])
    }
}

/// 为 `Turbos` 结构体实现 `Dex` trait。(Implement `Dex` trait for `Turbos` struct.)
#[async_trait::async_trait]
impl Dex for Turbos {
    /// `extend_trade_tx` (将Turbos交换操作添加到PTB / Add Turbos swap op to PTB)
    ///
    /// 将Turbos的交换操作（通过Cetus聚合器）添加到现有的PTB中。
    /// (Adds Turbos's swap operation (via Cetus aggregator) to the existing PTB.)
    async fn extend_trade_tx(
        &self, ctx: &mut TradeCtx, _sender: SuiAddress,
        coin_in_arg: Argument, _amount_in: Option<u64>,
    ) -> Result<Argument> {
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        // (**IMPORTANT**: Package ID uses `CETUS_AGGREGATOR`.)
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("turbos").map_err(|e| eyre!(e))?; // 聚合器中与Turbos交互的模块 (Module in aggregator for interacting with Turbos)
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        let mut type_arguments = self.type_params.clone(); // [CoinA, CoinB, FeeType]
        if !self.is_a2b() { // 如果是 B to A (即 coin_in is CoinB) (If B to A (i.e., coin_in is CoinB))
            type_arguments.swap(0, 1); // 交换 CoinA 和 CoinB 的位置，FeeType 位置不变 (Swap CoinA and CoinB, FeeType position remains unchanged)
        }

        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        Ok(Argument::Result(ctx.last_command_idx())) // 返回输出代币 (Return the output coin)
    }

    // --- Dex trait 的其他 getter 和 setter 方法 ---
    // (Other getter and setter methods for Dex trait)
    fn coin_in_type(&self) -> String { self.coin_in_type.clone() }
    fn coin_out_type(&self) -> String { self.coin_out_type.clone() }
    fn protocol(&self) -> Protocol { Protocol::Turbos } // 协议类型为Turbos (Protocol type is Turbos)
    fn liquidity(&self) -> u128 { self.liquidity }
    fn object_id(&self) -> ObjectID { self.pool.pool } // 池的ObjectID (Pool's ObjectID)

    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        if self.type_params.len() == 3 { // 确保有三个泛型参数 (Ensure there are three generic parameters)
            self.type_params.swap(0, 1); // 交换CoinA和CoinB，FeeType保持在最后 (Swap CoinA and CoinB, FeeType remains at the end)
        }
    }

    fn is_a2b(&self) -> bool { // 判断当前 coin_in_type 是否是池的 token0 (Check if current coin_in_type is pool's token0)
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
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL},
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher},
    };

    /// `test_turbos_swap_tx` 测试函数 (test_turbos_swap_tx test function)
    #[tokio::test]
    async fn test_turbos_swap_tx() {
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
            .filter(|dex| dex.protocol() == Protocol::Turbos)
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity()))
            .last()
            .expect("测试中未找到Turbos的池 (Turbos pool not found in test)");

        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 构建的交易数据 (Constructed transaction data): {:?}", tx_data);

        let response = http_simulator.simulate(tx_data, Default::default()).await.unwrap();
        info!("🧀 模拟结果 (Simulation result): {:?}", response);

        assert!(response.is_ok(), "交易模拟应成功 (Transaction simulation should succeed)");
    }
}

[end of bin/arb/src/defi/turbos.rs]
