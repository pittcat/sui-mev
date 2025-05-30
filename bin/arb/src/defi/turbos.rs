// 该文件 `turbos.rs` 实现了与 Turbos Finance 协议（一个Sui区块链上的去中心化交易所DEX）交互的逻辑。
// Turbos Finance 也是一个采用 CLMM (集中流动性做市商) 模型的DEX，允许更高效的流动性利用。
//
// 文件概览:
// 1. 定义了与 Turbos 协议相关的常量，主要是其版本化对象ID (`VERSIONED`)。
//    Turbos 使用一个版本化对象 (`Versioned`) 来管理其协议的状态或升级。
// 2. `ObjectArgs` 结构体: 用于缓存 `Versioned` 和 `Clock` 对象的 `ObjectArg`。
// 3. `Turbos` 结构体: 代表一个 Turbos CLMM 池的实例，实现了 `Dex` trait。
// 4. `new()` 方法: 初始化 `Turbos` 实例，从链上获取池的详细信息，如流动性、池是否解锁等。
// 5. `build_swap_tx()` / `build_swap_args()`: 构建在 Turbos 池中执行交换的交易参数和PTB。
//    Turbos的交换函数也区分 `swap_a2b` 和 `swap_b2a` 方向。
// 6. 实现了 `Dex` trait 的方法。值得注意的是，这里的 `extend_trade_tx` 方法同样使用了 `CETUS_AGGREGATOR` 的包ID，
//    这表明与 Turbos 池的交互可能是通过 Cetus 的聚合器合约进行的。
//
// Sui/DeFi概念:
// - CLMM (Concentrated Liquidity Market Maker): 集中流动性做市商，与Cetus, FlowX, Kriya CLMM类似。
// - Versioned Object (版本化对象): Turbos使用一个全局的 `Versioned` 对象，可能用于管理协议版本、全局开关或关键参数。
// - `unlocked`: Turbos池对象中的一个布尔字段，指示该池当前是否已解锁并允许交易。

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

use super::{TradeCtx, CETUS_AGGREGATOR}; // 从父模块(defi)引入 `TradeCtx` 和 `CETUS_AGGREGATOR`常量
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 `Dex` trait

// Turbos Finance 版本化对象ID (Versioned)
// 这个对象包含了协议版本等全局信息，在调用Turbos合约时通常需要传入。
const VERSIONED: &str = "0xf1cf0e81048df168ebeb1b8030fad24b3e0b53ae827c25053fff0779c1445b6f";

/// `ObjectArgs` 结构体
///
/// 缓存Turbos交互所需的关键对象的 `ObjectArg` 形式。
#[derive(Clone)]
pub struct ObjectArgs {
    versioned: ObjectArg, // 版本化对象的ObjectArg
    clock: ObjectArg,     // Sui时钟对象的ObjectArg
}

// 用于缓存 `ObjectArgs` 的静态 `OnceCell`
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数
///
/// 获取并缓存 `ObjectArgs` (包含versioned, clock)。
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let versioned_id = ObjectID::from_hex_literal(VERSIONED).unwrap();
            // 通过模拟器获取对象信息
            let versioned_obj = simulator.get_object(&versioned_id).await.unwrap();
            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();

            ObjectArgs {
                versioned: shared_obj_arg(&versioned_obj, false), // Versioned对象通常是不可变的
                clock: shared_obj_arg(&clock_obj, false),       // Clock是不可变的
            }
        })
        .await
        .clone()
}

/// `Turbos` 结构体
///
/// 代表一个Turbos Finance的CLMM交易池。
#[derive(Clone)]
pub struct Turbos {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息
    pool_arg: ObjectArg,     // 池对象本身的 `ObjectArg`
    liquidity: u128,         // 池的流动性
    coin_in_type: String,    // 当前交易方向的输入代币类型
    coin_out_type: String,   // 当前交易方向的输出代币类型
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是[CoinA, CoinB, FeeTier])
                               // Turbos的Pool对象通常有三个泛型参数: CoinA, CoinB, 和 Fee (手续费等级)。
    // 共享的对象参数
    versioned: ObjectArg,
    clock: ObjectArg,
}

impl Turbos {
    /// `new` 构造函数
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `Turbos` DEX实例。
    ///
    /// 参数:
    /// - `simulator`: 共享的模拟器实例。
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。
    /// - `coin_in_type`: 输入代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `Turbos` 实例，否则返回错误。
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 确保池协议是Turbos
        ensure!(pool_info.protocol == Protocol::Turbos, "提供的不是Turbos协议的池");

        // 获取并解析池对象的Move结构体内容
        let pool_obj = simulator
            .get_object(&pool_info.pool) // pool_info.pool 是池的ObjectID
            .await
            .ok_or_else(|| eyre!("Turbos池对象未找到: {}", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("Turbos池对象的布局(layout)未找到")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 检查池是否已解锁 (unlocked 字段)
        // Turbos的池对象可能有一个 `unlocked` 字段，表示池是否可交易。
        let unlocked = extract_bool_from_move_struct(&parsed_pool_struct, "unlocked")?;
        ensure!(unlocked, "Turbos池已锁定 (locked)，无法交易");

        // 提取流动性 (liquidity 字段)
        let liquidity = extract_u128_from_move_struct(&parsed_pool_struct, "liquidity")?;

        // 根据输入代币推断输出代币 (假设是双币池)
        let coin_out_type = if pool_info.token0_type() == coin_in_type {
            pool_info.token1_type().to_string()
        } else {
            pool_info.token0_type().to_string()
        };

        // 获取池本身的泛型类型参数。对于Turbos，这通常是 `[CoinTypeA, CoinTypeB, FeeType]`。
        // FeeType 是一个代表手续费等级的类型。
        let type_params = parsed_pool_struct.type_.type_params.clone();
        ensure!(type_params.len() == 3, "Turbos池的泛型参数应为三种类型 (CoinA, CoinB, Fee)");


        // 将池对象转换为 `ObjectArg` (在交易中通常是可变的)
        let pool_arg = shared_obj_arg(&pool_obj, true);
        // 获取共享的协议对象参数 (versioned, clock)
        let ObjectArgs { versioned, clock } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(),
            liquidity,
            coin_in_type: coin_in_type.to_string(),
            coin_out_type,
            type_params, // 通常是 [TokenTypeA, TokenTypeB, FeeType]
            pool_arg,
            versioned,
            clock,
        })
    }

    /// `swap_tx` (私有辅助函数，应重命名以避免与Dex trait中的同名函数混淆，改为 `build_pt_swap_tx`)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在Turbos池中执行一次常规交换。
    #[allow(dead_code)] // 允许存在未使用的代码
    async fn build_pt_swap_tx( // 重命名
        &self,
        sender: SuiAddress,
        recipient: SuiAddress,
        coin_in_ref: ObjectRef,
        amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default();

        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        // `None` 表示 `amount_in` 对于 `extend_trade_tx` 是可选的或不直接使用u64值
        // (Turbos的swap函数可能直接使用传入Coin对象的全部余额)。
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数)
    ///
    /// 构建调用Turbos常规交换方法 (如聚合器中的 `turbos::swap_a2b`) 所需的参数列表。
    /// 聚合器中的函数签名可能类似于:
    /// `public fun swap_a2b<CoinA, CoinB, Fee>(pool: &mut Pool<CoinA, CoinB, Fee>, coin_a: Coin<CoinA>, clock: &Clock, versioned: &Versioned, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数包括: pool, 输入的coin对象, clock对象, versioned对象。
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;
        let versioned_arg = ctx.obj(self.versioned).map_err(|e| eyre!(e))?;

        // 返回参数列表，顺序必须与聚合器中 turbos 模块的 swap_a2b/swap_b2a 函数签名一致。
        Ok(vec![pool_arg, coin_in_arg, clock_arg, versioned_arg])
    }
}

/// 为 `Turbos` 结构体实现 `Dex` trait。
#[async_trait::async_trait]
impl Dex for Turbos {
    /// `extend_trade_tx`
    ///
    /// 将Turbos的交换操作（通过Cetus聚合器）添加到现有的PTB中。
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
        _amount_in: Option<u64>, // Turbos的swap函数通常消耗整个传入的Coin对象
    ) -> Result<Argument> {
        // 根据 `is_a2b()` 的结果选择调用聚合器中的 `swap_a2b` 还是 `swap_b2a` 函数。
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        // 这表明这里的Turbos交易是通过Cetus的聚合器合约来执行的。
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("turbos").map_err(|e| eyre!(e))?; // 聚合器中与Turbos交互的模块
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        // 泛型类型参数，对于Turbos是 `[CoinTypeA, CoinTypeB, FeeType]`。
        // `self.type_params` 在 `Turbos::new` 中被设置为池的这三种类型。
        // 需要确保这里的顺序与聚合器中 `swap_a2b` / `swap_b2a` 的泛型参数顺序匹配。
        // 如果 `is_a2b()` 为true (输入CoinA, 输出CoinB), 泛型参数应为 [CoinA, CoinB, Fee]。
        // 如果 `self.type_params` 是 `[CoinA, CoinB, Fee]`，则对于 `swap_a2b` 是正确的。
        // 对于 `swap_b2a` (输入CoinB, 输出CoinA)，泛型参数应为 `[CoinB, CoinA, Fee]`。
        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() { // 如果是 B to A (即 coin_in is CoinB)
            // 交换 CoinA 和 CoinB 的位置，FeeType 位置不变 (假设FeeType总是在最后)。
            type_arguments.swap(0, 1);
        }

        // 构建调用参数
        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;

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
        Protocol::Turbos // 协议类型为Turbos
    }

    fn liquidity(&self) -> u128 {
        self.liquidity
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 池的ObjectID
    }

    /// `flip` 方法
    ///
    /// 翻转交易方向 (输入币和输出币互换)。
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        // `type_params` ([CoinA, CoinB, Fee]) 中的CoinA和CoinB也需要交换位置。
        // FeeType通常保持在最后。
        if self.type_params.len() == 3 { // 确保有三个泛型参数
            self.type_params.swap(0, 1);
        }
    }

    /// `is_a2b` 方法
    ///
    /// 判断当前 `coin_in_type` 是否是池中定义的 "第一个" 代币 (token0)。
    /// 聚合器中的 `swap_a2b` 通常指 token0 -> token1。
    fn is_a2b(&self) -> bool {
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试)
    ///
    /// 构建一个完整的、独立的常规交换交易。
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await;

        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;

        // 调用内部的 build_pt_swap_tx (已重命名)
        let pt = self
            .build_pt_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in)
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

    use super::*; // 导入外部模块 (turbos.rs)
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL}, // 测试配置
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器
    };

    /// `test_turbos_swap_tx` 测试函数
    ///
    /// 测试通过Turbos (经由Cetus聚合器) 进行常规交换的流程。
    #[tokio::test]
    async fn test_turbos_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        // 定义测试参数
        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap(); // 从配置获取
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI"; // 输入SUI
        // DEEP是Cetus上的一个代币，这里可能只是作为示例，实际Turbos上交易对可能不同
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

        // 从找到的DEX中筛选出Turbos协议的池，并选择流动性最大的那个。
        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::Turbos) // 过滤Turbos池
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 按流动性排序
            .last() // 取流动性最大的
            .expect("测试中未找到Turbos的池");

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
