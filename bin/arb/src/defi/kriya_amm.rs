// 该文件 `kriya_amm.rs` 实现了与 KriyaDEX 协议的传统 AMM (自动做市商) 池交互的逻辑。
// KriyaDEX 是 Sui 生态系统中的一个去中心化交易所，它同时提供传统的 AMM 池和 CLMM (集中流动性做市商) 池。
// 这个文件专门处理其 AMM 池部分。
//
// 文件概览:
// 1. `KriyaAmm` 结构体: 代表一个 Kriya AMM 池的实例，实现了 `Dex` trait。
// 2. `new()` 方法: 用于根据链上数据初始化 `KriyaAmm` 实例。它会从池对象中提取流动性等信息。
// 3. `build_swap_tx()` / `build_swap_args()`: 构建在 Kriya AMM 池中执行交换的交易参数和PTB。
//    Kriya AMM 的交换函数也区分 `swap_a2b` 和 `swap_b2a` 方向。
// 4. 实现了 `Dex` trait 的方法。值得注意的是，这里的 `extend_trade_tx` 方法也使用了 `CETUS_AGGREGATOR` 的包ID，
//    这表明与 Kriya AMM 池的交互可能是通过 Cetus 的聚合器合约进行的，该聚合器合约能路由交易到包括 Kriya 在内的多个DEX。
//
// Sui/DeFi概念:
// - AMM (Automated Market Maker): 自动做市商。一种类型的去中心化交易所，它不依赖传统的订单簿，
//   而是使用流动性池和数学公式（例如 XYK=K 常数乘积公式）来确定资产价格和执行交易。
// - Liquidity Pool (流动性池): AMM 的核心。用户（流动性提供者）将代币对存入池中以提供流动性，
//   交易者则与这些池子进行代币交换。流动性提供者会获得LP代币作为其份额凭证，并赚取交易手续费。
// - `is_swap_enabled`: Kriya AMM 池对象中的一个布尔字段，指示该池当前是否允许交换操作。

// 引入标准库及第三方库
use std::sync::Arc; // 原子引用计数

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` 引入Pool和Protocol类型
use eyre::{ensure, eyre, OptionExt, Result}; // 错误处理库
use move_core_types::annotated_value::MoveStruct; // Move核心类型
use simulator::Simulator; // 交易模拟器接口
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建类型
    Identifier, TypeTag, // Sui标识符和类型标签
};
use utils::{coin, new_test_sui_client, object::*}; // 自定义工具库

use super::{TradeCtx, CETUS_AGGREGATOR}; // 从父模块(defi)引入 `TradeCtx` 和 `CETUS_AGGREGATOR`常量
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 `Dex` trait

/// `KriyaAmm` 结构体
///
/// 代表一个KriyaDEX的传统AMM交易池。
#[derive(Clone)]
pub struct KriyaAmm {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息
    pool_arg: ObjectArg,     // 池对象本身的 `ObjectArg`
    liquidity: u128,         // 池的流动性 (通常是LP代币的总供应量)
    coin_in_type: String,    // 当前交易方向的输入代币类型
    coin_out_type: String,   // 当前交易方向的输出代币类型
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是[CoinA, CoinB])
}

impl KriyaAmm {
    /// `new` 构造函数
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `KriyaAmm` DEX实例。
    ///
    /// 参数:
    /// - `simulator`: 共享的模拟器实例。
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。
    /// - `coin_in_type`: 输入代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `KriyaAmm` 实例，否则返回错误。
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 确保池协议是KriyaAmm
        ensure!(pool_info.protocol == Protocol::KriyaAmm, "提供的不是Kriya AMM协议的池");

        // 获取并解析池对象的Move结构体内容
        let pool_obj = simulator
            .get_object(&pool_info.pool) // pool_info.pool 是池的ObjectID
            .await
            .ok_or_else(|| eyre!("Kriya AMM池对象未找到: {}", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("Kriya AMM池对象的布局(layout)未找到")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 检查池是否启用了交换功能 (is_swap_enabled 字段)
        let is_swap_enabled = extract_bool_from_move_struct(&parsed_pool_struct, "is_swap_enabled")?;
        ensure!(is_swap_enabled, "Kriya AMM池的交换功能未启用");

        // 提取流动性 (通常从 lsp_supply.value 字段获取，代表LP代币的总供应量)
        let liquidity = {
            let lsp_supply_struct = extract_struct_from_move_struct(&parsed_pool_struct, "lsp_supply")?;
            extract_u64_from_move_struct(&lsp_supply_struct, "value")? as u128
        };

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

        Ok(Self {
            pool: pool_info.clone(),
            pool_arg,
            liquidity,
            coin_in_type: coin_in_type.to_string(),
            coin_out_type,
            type_params, // 通常是 [TokenTypeA, TokenTypeB]
        })
    }

    /// `build_swap_tx` (私有辅助函数)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在Kriya AMM池中执行一次交换。
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
        // 将Kriya AMM交换操作添加到PTB
        // `None` 表示 `amount_in` 参数对于 `extend_trade_tx` 是可选的或不直接使用u64值
        // (Kriya AMM的swap函数通常直接使用传入Coin对象的全部余额作为输入数量)。
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;
        // 将输出代币转移给接收者
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish()) // 完成并返回PTB
    }

    /// `build_swap_args` (私有辅助函数)
    ///
    /// 构建调用Kriya AMM交换方法 (如 `swap_a2b` 或 `swap_b2a`，在Cetus聚合器中封装) 所需的参数列表。
    /// 聚合器中 `kriya_amm::swap_a2b` 的签名可能类似于:
    /// `public fun swap_a2b<CoinA, CoinB>(pool: &mut Pool<CoinA, CoinB>, coin_a: Coin<CoinA>, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数通常是可变的池对象和输入的Coin对象。
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;

        // 返回参数列表: [pool_arg, coin_in_arg]
        Ok(vec![pool_arg, coin_in_arg])
    }
}

/// 为 `KriyaAmm` 结构体实现 `Dex` trait。
#[async_trait::async_trait]
impl Dex for KriyaAmm {
    /// `extend_trade_tx`
    ///
    /// 将Kriya AMM的交换操作（通过Cetus聚合器）添加到现有的PTB中。
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
        _amount_in: Option<u64>, // Kriya AMM的swap函数通常消耗整个传入的Coin对象
    ) -> Result<Argument> {
        // 根据 `is_a2b()` 的结果选择调用聚合器中的 `swap_a2b` 还是 `swap_b2a` 函数。
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        // 这表明这里的Kriya AMM交易是通过Cetus的聚合器合约来执行的。
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("kriya_amm").map_err(|e| eyre!(e))?; // 聚合器中与Kriya AMM交互的模块
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        // 泛型类型参数，通常是 `[CoinTypeA, CoinTypeB]`。
        // `self.type_params` 在 `KriyaAmm::new` 中被设置为池的两种代币类型。
        // 需要确保这里的顺序与聚合器中 `swap_a2b` / `swap_b2a` 的泛型参数顺序匹配。
        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() { // 如果是 B to A (即 coin_in is token1)
            type_arguments.swap(0, 1); // 交换泛型参数顺序，变为 [CoinB, CoinA]
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
        Protocol::KriyaAmm // 协议类型为KriyaAmm
    }

    fn liquidity(&self) -> u128 {
        self.liquidity // 返回池的流动性 (LP代币供应量)
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 返回池的ObjectID (从原始Pool信息中获取)
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
    /// 判断当前 `coin_in_type` 是否是池中定义的 "第一个" 代币 (token0)。
    /// 聚合器中的 `swap_a2b` 通常指 token0 -> token1。
    fn is_a2b(&self) -> bool {
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试)
    ///
    /// 构建一个完整的、独立的交换交易。
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await; // 创建测试Sui客户端

        // 获取输入代币对象
        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;

        // 构建包含交换操作的PTB (调用内部的 build_swap_tx)
        let pt = self
            .build_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in)
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

    use super::*; // 导入外部模块 (kriya_amm.rs)
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL}, // 测试配置
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器
    };

    /// `test_kriya_amm_swap_tx` 测试函数
    ///
    /// 测试通过Kriya AMM (经由Cetus聚合器) 进行交换的流程。
    #[tokio::test]
    async fn test_kriya_amm_swap_tx() {
        // 初始化日志
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        // 创建一个HTTP模拟器 (用于实际模拟交易)
        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        // 定义测试参数
        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap(); // 从配置获取
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI"; // 输入SUI
        // Wormhole USDC
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

        // 从找到的DEX中筛选出KriyaAmm协议的池，并选择流动性最大的那个。
        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::KriyaAmm) // 过滤KriyaAmm池
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 按流动性排序
            .last() // 取流动性最大的
            .expect("测试中未找到KriyaAmm的池");

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
