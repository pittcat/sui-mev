// 该文件 `blue_move.rs` 实现了与 BlueMove 协议交互的逻辑。
// BlueMove 主要是一个NFT市场，但也可能提供或集成了一些去中心化交易所 (DEX) 的功能，
// 或者这里的代码是通过某个聚合器 (如Cetus Aggregator) 与BlueMove的池子进行交互。
//
// 文件概览:
// 1. 定义了与 BlueMove 相关的常量，主要是其 `DEX_INFO` 对象ID。
//    这个 `Dex_Info` 对象可能包含了BlueMove DEX功能所需的状态或配置。
// 2. `ObjectArgs` 结构体: 用于缓存 `Dex_Info` 对象的 `ObjectArg`，通过 `OnceCell` 实现单次初始化。
// 3. `BlueMove` 结构体: 代表一个BlueMove上的交易池（或通过聚合器访问的池）。
//    它实现了 `Dex` trait，表明它遵循通用的DEX接口。
// 4. `new()` 方法: 用于根据链上数据初始化 `BlueMove` 实例。
// 5. `build_swap_tx()` / `build_swap_args()`: 构建在BlueMove上执行交换的交易参数和PTB。
//    注意其 `swap_a2b` 和 `swap_b2a` 函数签名，这表明它区分了交易方向。
// 6. 实现了 `Dex` trait 的方法，如 `extend_trade_tx`, `coin_in_type`, `coin_out_type` 等。
//    `extend_trade_tx` 方法中使用了 `CETUS_AGGREGATOR` 的包ID，这强烈暗示了
//    这里的BlueMove交互可能是通过Cetus的聚合器合约进行的，该聚合器合约能路由交易到包括BlueMove在内的多个DEX。
//
// Sui/DeFi概念:
// - NFT Marketplace: 非同质化代币（NFT）的交易平台。BlueMove是Sui上知名的NFT市场。
// - DEX Aggregator: DEX聚合器是一种服务或智能合约，它会从多个DEX中查找最佳的交易价格和路径，
//   然后将用户的交易分割或路由到这些DEX以获得最优执行结果。Cetus提供了聚合器功能。
// - `Dex_Info` Object: BlueMove合约中可能存在一个中心化的对象，存储了其DEX功能的状态、配置或路由信息。

// 引入标准库及第三方库
use std::sync::Arc; // 原子引用计数，用于安全共享数据

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` 引入Pool和Protocol类型
use eyre::{ensure, eyre, OptionExt, Result}; // 错误处理库 `eyre`
use move_core_types::annotated_value::MoveStruct; // Move核心类型，用于解析Move对象结构
use simulator::Simulator; // 交易模拟器接口
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建类型
    Identifier, TypeTag, // Sui标识符和类型标签
};
use tokio::sync::OnceCell; // Tokio异步单次初始化单元
use utils::{coin, new_test_sui_client, object::*}; // 自定义工具库

use super::{TradeCtx, CETUS_AGGREGATOR}; // 从父模块(defi)引入 `TradeCtx` 和 `CETUS_AGGREGATOR`常量
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 `Dex` trait

// BlueMove 的 `Dex_Info` 对象ID。这个对象可能由BlueMove或其聚合器（如Cetus）管理，
// 存储了BlueMove相关池或路由逻辑的信息。
const DEX_INFO: &str = "0x3f2d9f724f4a1ce5e71676448dc452be9a6243dac9c5b975a588c8c867066e92";

// 用于缓存 `ObjectArgs` 的静态 `OnceCell`
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数
///
/// 获取并缓存 `ObjectArgs` (这里只包含 `dex_info`)。
/// 如果缓存未初始化，则从链上获取 `DEX_INFO` 对象并转换为 `ObjectArg`。
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let id = ObjectID::from_hex_literal(DEX_INFO).unwrap(); // 解析DEX_INFO的ObjectID
            let dex_info_obj = simulator.get_object(&id).await.unwrap(); // 获取对象

            ObjectArgs {
                // 将获取的 dex_info_obj 转换为 ObjectArg。
                // `true` 表示这个对象在交易中可能是可变的。
                dex_info: shared_obj_arg(&dex_info_obj, true),
            }
        })
        .await
        .clone()
}

/// `ObjectArgs` 结构体
///
/// 缓存BlueMove交互所需的关键对象的 `ObjectArg` 形式。
#[derive(Clone)]
pub struct ObjectArgs {
    dex_info: ObjectArg, // BlueMove的Dex_Info对象的ObjectArg
}

/// `BlueMove` 结构体
///
/// 代表一个BlueMove的交易池（或通过聚合器访问的池）。
#[derive(Clone)]
pub struct BlueMove {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息
    liquidity: u128,         // 池的流动性 (可能是LP代币供应量)
    coin_in_type: String,    // 当前交易方向的输入代币类型
    coin_out_type: String,   // 当前交易方向的输出代币类型
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是CoinA, CoinB)
    dex_info: ObjectArg,     // Dex_Info对象的ObjectArg
}

impl BlueMove {
    /// `new` 构造函数
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `BlueMove` DEX实例。
    /// 输出代币类型会根据输入代币类型自动推断（假设是双币池）。
    ///
    /// 参数:
    /// - `simulator`: 共享的模拟器实例。
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。
    /// - `coin_in_type`: 输入代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `BlueMove` 实例，否则返回错误。
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 确保池协议是BlueMove
        ensure!(pool_info.protocol == Protocol::BlueMove, "提供的不是BlueMove协议的池");

        // 获取并解析池对象的Move结构体内容
        let parsed_pool_struct = {
            let pool_obj = simulator
                .get_object(&pool_info.pool) // pool_info.pool 是池的ObjectID
                .await
                .ok_or_else(|| eyre!("BlueMove池对象未找到: {}", pool_info.pool))?;

            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("BlueMove池对象的布局(layout)未找到")?; // 使用 Option::ok_or_eyre

            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 检查池是否被冻结 (is_freeze 字段)
        let is_freeze = extract_bool_from_move_struct(&parsed_pool_struct, "is_freeze")?;
        ensure!(!is_freeze, "BlueMove池已被冻结，无法交易");

        // 提取流动性 (lsp_supply.value)
        let liquidity = {
            let lsp_supply_struct = extract_struct_from_move_struct(&parsed_pool_struct, "lsp_supply")?;
            extract_u64_from_move_struct(&lsp_supply_struct, "value")? as u128
        };

        // 根据输入代币推断输出代币。
        // BlueMove的池（或通过Cetus聚合器访问的池）通常是双币池。
        // `pool_info.token_index(coin_in_type)` 返回输入代币在池代币对中的索引 (0 或 1)。
        // 如果输入代币是 token0，则输出代币是 token1，反之亦然。
        let coin_out_type = if let Some(0) = pool_info.token_index(coin_in_type) {
            pool_info.token1_type() // 如果输入是token0, 输出是token1
        } else {
            pool_info.token0_type() // 如果输入是token1, 输出是token0
        };

        // 获取池本身的泛型类型参数，这通常是池中包含的两种代币的类型。
        // 例如 `[CoinTypeA, CoinTypeB]`
        // 这些将作为调用swap函数时的类型参数。
        let type_params = parsed_pool_struct.type_.type_params.clone();

        // 获取共享的 `Dex_Info` ObjectArg
        let ObjectArgs { dex_info } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(), // 克隆一份原始池信息
            liquidity,
            coin_in_type: coin_in_type.to_string(),
            coin_out_type,
            type_params, // 通常是 [TokenTypeA, TokenTypeB]
            dex_info,
        })
    }

    /// `build_swap_tx` (私有辅助函数)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在BlueMove池中执行一次交换。
    ///
    /// 参数:
    /// - `sender`: 交易发送者地址。
    /// - `recipient`: 接收输出代币的地址。
    /// - `coin_in_ref`: 输入代币对象的引用。
    /// - `amount_in`: 输入代币的数量。
    ///
    /// 返回:
    /// - `Result<ProgrammableTransaction>`: 构建好的PTB。
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
        // 将BlueMove交换操作添加到PTB
        // 注意：这里的 `_amount_in` (None) 传递给 `extend_trade_tx`，
        // 这表明BlueMove的swap函数（或其聚合器接口）可能直接从 `coin_in_arg` 的面额推断输入数量，
        // 而不需要一个单独的 `amount_in` u64参数。
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;
        // 将输出代币转移给接收者
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish()) // 完成并返回PTB
    }

    /// `build_swap_args` (私有辅助函数)
    ///
    /// 构建调用BlueMove交换方法 (如 `swap_a2b` 或 `swap_b2a`) 所需的参数列表。
    /// 合约方法签名示例 (来自注释):
    /// `public fun swap_a2b<CoinA, CoinB>(dex_info: &mut Dex_Info, coin_a: Coin<CoinA>, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数通常是 `dex_info` 和输入的代币对象。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文。
    /// - `coin_in_arg`: 代表输入代币的 `Argument`。
    ///
    /// 返回:
    /// - `Result<Vec<Argument>>`: 参数列表。
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        // 获取 Dex_Info 对象的 Argument
        let dex_info_arg = ctx.obj(self.dex_info).map_err(|e| eyre!(e))?;

        // 返回参数列表: [dex_info_arg, coin_in_arg]
        Ok(vec![dex_info_arg, coin_in_arg])
    }
}

/// 为 `BlueMove` 结构体实现 `Dex` trait。
#[async_trait::async_trait]
impl Dex for BlueMove {
    /// `extend_trade_tx`
    ///
    /// 将BlueMove的交换操作添加到现有的PTB中。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文。
    /// - `_sender`: 发送者地址 (未使用)。
    /// - `coin_in_arg`: 输入代币的 `Argument`。
    /// - `_amount_in`: 输入金额 (未使用，因为BlueMove的swap函数直接使用输入Coin对象的全部余额)。
    ///
    /// 返回:
    /// - `Result<Argument>`: 代表输出代币的 `Argument`。
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress,
        coin_in_arg: Argument,
        _amount_in: Option<u64>, // BlueMove的swap函数通常消耗整个传入的Coin对象
    ) -> Result<Argument> {
        // 根据 `is_a2b()` 的结果选择调用 `swap_a2b` 还是 `swap_b2a` 函数。
        // `is_a2b()` 判断当前 `coin_in_type` 是否是池中的 "token0" (通常是交易对的第一个代币)。
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // --- 构建Move调用命令 ---
        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        // 这意味着这里的BlueMove交易实际上是通过Cetus的聚合器合约来执行的。
        // Cetus聚合器在其内部会调用实际的BlueMove合约逻辑。
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("bluemove").map_err(|e| eyre!(e))?; // 聚合器中与BlueMove交互的模块
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        // 泛型类型参数，通常是 `[CoinTypeA, CoinTypeB]`，其中A是输入，B是输出。
        // `self.type_params` 在 `BlueMove::new` 中被设置为池的两种代币类型。
        // 需要确保这里的顺序与 `swap_a2b` / `swap_b2a` 的泛型参数顺序匹配。
        // 如果 `is_a2b()` 为true, CoinA是token0, CoinB是token1。
        // 如果 `self.type_params` 是 `[token0_type, token1_type]`，则对于 `swap_a2b` 是正确的。
        // 对于 `swap_b2a`，泛型参数应该是 `[token1_type, token0_type]`。
        // 所以，这里的 `type_arguments` 可能需要根据 `is_a2b()` 的结果调整顺序。
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
        Protocol::BlueMove // 协议类型为BlueMove
    }

    fn liquidity(&self) -> u128 {
        self.liquidity // 返回池的流动性
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 返回池的ObjectID (从原始Pool信息中获取)
    }

    /// `flip` 方法
    ///
    /// 翻转交易方向 (输入币和输出币互换)。
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        // 注意：`type_params` 在 `extend_trade_tx` 中会根据 `is_a2b` 动态调整，所以这里不需要修改 `type_params`。
    }

    /// `is_a2b` 方法
    ///
    /// 判断当前 `coin_in_type` 是否是池中定义的 "第一个" 代币 (token0)。
    /// BlueMove (或其聚合器接口) 的 `swap_a2b` 通常指 token0 -> token1，
    /// `swap_b2a` 指 token1 -> token0。
    fn is_a2b(&self) -> bool {
        // `self.pool` 是 `dex_indexer::types::Pool` 类型。
        // `token_index` 方法返回该代币在池代币对中的索引 (0 或 1)。
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

    use itertools::Itertools; // 用于迭代器操作，如 `sorted_by`
    use object_pool::ObjectPool; // 对象池
    use simulator::{DBSimulator, HttpSimulator, Simulator}; // 各种模拟器
    use tracing::info; // 日志

    use super::*; // 导入外部模块 (blue_move.rs)
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL}, // 测试配置
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器
    };

    /// `test_flowx_swap_tx` 测试函数 (函数名可能是笔误，应为 test_bluemove_swap_tx)
    ///
    /// 测试通过BlueMove (经由Cetus聚合器) 进行交换的流程。
    #[tokio::test]
    async fn test_bluemove_swap_tx() { // 修正函数名
        // 初始化日志
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        // 创建一个HTTP模拟器 (用于实际模拟交易)
        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        // 定义测试参数
        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap();
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI"; // 输入SUI
        // 一个示例输出代币 (SUICAT)
        let token_out_type = "0x0bffc4f0333fb1256431156395a93fc252432152b0ff732197e8459a365e5a9f::suicat::SUICAT";
        let amount_in = 10000; // 输入少量 (0.00001 SUI)

        // 创建DBSimulator对象池 (用于IndexerDexSearcher初始化，可能用于获取对象布局等)
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

        // 从找到的DEX中筛选出BlueMove协议的池，并按流动性排序，选择流动性最大的那个。
        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::BlueMove) // 过滤BlueMove池
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 按流动性排序
            .last() // 取流动性最大的 (因为是升序排序后取最后一个)
            .expect("测试中未找到BlueMove的池"); // 如果没有找到则panic

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
