// 该文件 `mod.rs` 是 `defi` 模块的根文件，充当该模块的组织和入口点。
// `defi` 模块封装了与去中心化金融 (DeFi) 协议交互的所有逻辑，
// 特别是与各种去中心化交易所 (DEX) 的交互，以及交易路径的发现和评估。
//
// 文件概览:
// 1. 声明子模块: 列出了所有具体的DEX协议实现 (如aftermath, cetus, turbos等) 和辅助模块 (如trade, indexer_searcher)。
//    每个子模块通常在同级目录下有一个对应的 `.rs` 文件。
// 2. 定义核心 Traits (接口):
//    - `DexSearcher`: 定义了查找DEX实例的接口。
//    - `Dex`: 定义了与单个DEX池交互的通用接口，包括获取信息、执行交易、闪电贷等。
// 3. `Defi` 结构体: 封装了 `DexSearcher` 和 `Trader` (用于执行交易和模拟)，提供了更高层次的API来查找交易路径和构建交易。
// 4. 路径发现逻辑:
//    - `find_sell_paths()`: 查找从给定输入代币开始，最终换回SUI代币的潜在交易路径。
//    - `find_buy_paths()`: 查找从SUI代币开始，购买到给定输出代币的潜在交易路径 (通过翻转sell_paths实现)。
//    - 使用深度优先搜索 (DFS) 算法在DEX网络中探索可能的交易路径。
// 5. 路径评估和交易构建:
//    - `find_best_path_exact_in()`: 在一组给定的路径中，为指定的输入金额找到能产生最佳输出结果的路径。
//    - `build_final_tx_data()`: 构建最终的可提交Sui交易数据 (TransactionData)，可能包含闪电贷操作。
// 6. 常量定义: 如 `MAX_HOP_COUNT` (路径中的最大跳数/DEX数量), `MAX_POOL_COUNT` (从索引器获取的池数量限制),
//    `MIN_LIQUIDITY` (池的最小流动性门槛), `CETUS_AGGREGATOR` (Cetus聚合器的包ID)。
//
// Sui/DeFi概念:
// - Trait (接口): Rust中用于定义共享行为的方式，类似于其他语言中的接口或抽象类。
// - `Box<dyn Dex>`: 一个特征对象，允许在运行时使用不同类型的DEX实现，只要它们都实现了 `Dex` trait。
// - Path (交易路径): 一系列连续的DEX交换，例如 TokenA -> DEX1 -> TokenB -> DEX2 -> TokenC。
// - Hop (跳): 路径中的一步，即通过一个DEX进行一次交换。
// - Liquidity (流动性): DEX池中可供交易的代币数量，是衡量池健康度的重要指标。
// - DFS (Depth-First Search, 深度优先搜索): 一种图遍历算法，用于探索所有可能的路径。
// - Pegged Coins (锚定币): 其价值与某种法定货币或其他资产锚定的加密货币 (如USDC, USDT)。
// - Native Coin (原生代币): 区块链平台的内置代币，如Sui网络中的SUI。

// --- 声明子模块 ---
// 每个 `mod` 声明都指向同级目录下的一个同名 `.rs` 文件或一个同名目录下的 `mod.rs` 文件。
mod aftermath;        // Aftermath DEX 实现
mod blue_move;        // BlueMove DEX/NFT市场 实现 (可能通过聚合器)
mod cetus;            // Cetus DEX 实现
mod deepbook_v2;      // DeepBook V2 (订单簿) 实现 (可能通过聚合器)
mod flowx_clmm;       // FlowX CLMM 实现
mod indexer_searcher; // DEX索引器搜索器实现 (用于从外部服务发现DEX池)
mod kriya_amm;        // KriyaDEX AMM 实现
mod kriya_clmm;       // KriyaDEX CLMM 实现
mod navi;             // Navi 借贷协议实现 (也可能用于获取代币或利率)
mod shio;             // Shio DEX 实现 (可能较新或不太常见)
mod trade;            // 交易执行、模拟、路径组合等核心逻辑
mod turbos;           // Turbos Finance DEX 实现
mod utils;            // DeFi模块内部的辅助工具函数

// --- 引入标准库及第三方库 ---
use std::{
    collections::{HashMap, HashSet}, // HashMap用于存储键值对 (如coin_type -> Vec<Dex>), HashSet用于存储唯一元素
    fmt,                             // 用于格式化输出 (实现Debug, Display trait)
    hash::Hash,                      // 用于使类型可哈希 (作为HashMap的键)
    sync::Arc,                       // 原子引用计数，用于安全地共享对象 (如DexSearcher, Trader)
};

use ::utils::coin; // 从外部 `utils` crate (注意是 `::utils` 而非 `super::utils`) 引入代币相关工具
use dex_indexer::types::Protocol; // 从 `dex_indexer` crate 引入 Protocol 枚举
use eyre::{bail, ensure, Result}; // 错误处理库 `eyre`
pub use indexer_searcher::IndexerDexSearcher; // 重新导出 `IndexerDexSearcher`，使其在 `defi` 模块外更易访问
use object_pool::ObjectPool; // 对象池，用于管理模拟器实例
use simulator::{SimulateCtx, Simulator}; // 模拟器上下文和Simulator trait
use sui_sdk::SUI_COIN_TYPE; // SUI原生代币的类型字符串 ("0x2::sui::SUI")
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型
    transaction::{Argument, TransactionData},      // Sui交易构建类型
};
use tokio::task::JoinSet; // Tokio任务集合，用于并发执行异步任务
use tracing::Instrument; // `tracing`库的扩展，用于追踪异步代码块
use trade::{FlashResult, TradeResult}; // 从子模块 `trade` 引入交易结果和闪电贷结果类型
pub use trade::{Path, TradeCtx, TradeType, Trader}; // 重新导出 `trade` 子模块中的核心类型

use crate::{config::pegged_coin_types, types::Source}; // 从当前crate的根模块引入配置和自定义类型

// --- DeFi模块常量定义 ---
// 路径中允许的最大跳数 (DEX数量)。限制跳数有助于控制路径搜索的复杂度和执行成本。
const MAX_HOP_COUNT: usize = 2;
// 从索引器为每种代币获取的池的最大数量。用于限制处理的池数量，优先处理流动性较高的池。
const MAX_POOL_COUNT: usize = 10;
// 池的最小流动性门槛。低于此流动性的池在路径搜索中可能被忽略。
const MIN_LIQUIDITY: u128 = 1000; // 这个值可能需要根据代币的精度调整，1000对于SUI可能太小

// Cetus聚合器的Sui包ID。许多DEX交互可能是通过这个聚合器进行的。
pub const CETUS_AGGREGATOR: &str = "0x11451575c775a3e633437b827ecbc1eb51a5964b0302210b28f5b89880be21a2";

/// `DexSearcher` Trait (DEX搜索器接口)
///
/// 定义了查找DEX实例的通用行为。
/// `Send + Sync`约束表示实现该trait的类型可以安全地在线程间发送和共享。
#[async_trait::async_trait] // 表明trait中的方法可以是异步的
pub trait DexSearcher: Send + Sync {
    /// `find_dexes` 方法
    ///
    /// 根据输入的代币类型（和可选的输出代币类型）查找所有相关的DEX实例。
    ///
    /// 参数:
    /// - `coin_in_type`: 输入代币的类型字符串 (例如 "0x2::sui::SUI")。
    /// - `coin_out_type`: (可选) 输出代币的类型字符串。如果为 `None`，则查找所有包含 `coin_in_type` 的池。
    ///
    /// 返回:
    /// - `Result<Vec<Box<dyn Dex>>>`: 一个包含动态分发的具体DEX实例的向量。
    ///   `Box<dyn Dex>` 允许存储不同类型的DEX，只要它们都实现了 `Dex` trait。
    async fn find_dexes(&self, coin_in_type: &str, coin_out_type: Option<String>) -> Result<Vec<Box<dyn Dex>>>;

    /// `find_test_path` 方法 (主要用于测试)
    ///
    /// 根据一个给定的对象ID序列（代表一个交易路径中的连续池子），
    /// 构建并返回一个 `Path` 对象。
    async fn find_test_path(&self, path: &[ObjectID]) -> Result<Path>;
}

/// `Dex` Trait (去中心化交易所接口)
///
/// 定义了与单个DEX池交互的通用行为。
/// `Send + Sync + CloneBoxedDex` 约束：
/// - `Send + Sync`: 可在线程间安全传递和共享。
/// - `CloneBoxedDex`: 一个自定义trait (见下文)，使得 `Box<dyn Dex>` 特征对象可以被克隆。
#[async_trait::async_trait]
pub trait Dex: Send + Sync + CloneBoxedDex {
    /// `support_flashloan` 方法
    ///
    /// 返回一个布尔值，指示该DEX是否支持闪电贷功能。
    /// 默认实现为 `false`。具体DEX需要重写此方法如果它们支持闪电贷。
    fn support_flashloan(&self) -> bool {
        false
    }

    /// `extend_flashloan_tx` 方法 (用于发起闪电贷)
    ///
    /// 将发起闪电贷的操作添加到现有的交易上下文 (`TradeCtx`) 中。
    ///
    /// 参数:
    /// - `_ctx`: 可变的交易上下文。
    /// - `_amount`: 希望借入的代币数量。
    ///
    /// 返回:
    /// - `Result<FlashResult>`: 包含借出的代币 (`coin_out`) 和闪电贷回执 (`receipt`) 的 `Argument`。
    ///   默认实现返回错误，表示不支持闪电贷。
    async fn extend_flashloan_tx(&self, _ctx: &mut TradeCtx, _amount: u64) -> Result<FlashResult> {
        bail!("此DEX不支持闪电贷") // `bail!` 来自 `eyre` 库，用于快速返回错误
    }

    /// `extend_repay_tx` 方法 (用于偿还闪电贷)
    ///
    /// 将偿还闪电贷的操作添加到现有的交易上下文 (`TradeCtx`) 中。
    ///
    /// 参数:
    /// - `_ctx`: 可变的交易上下文。
    /// - `_coin`: 用于偿还的代币的 `Argument`。
    /// - `_flash_res`: 从 `extend_flashloan_tx` 返回的 `FlashResult`。
    ///
    /// 返回:
    /// - `Result<Argument>`: 可能代表找零的代币，或一个空结果。
    ///   默认实现返回错误。
    async fn extend_repay_tx(&self, _ctx: &mut TradeCtx, _coin: Argument, _flash_res: FlashResult) -> Result<Argument> {
        bail!("此DEX不支持闪电贷的偿还操作")
    }

    /// `extend_trade_tx` 方法 (用于常规交换)
    ///
    /// 将常规的代币交换操作添加到现有的交易上下文 (`TradeCtx`) 中。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文。
    /// - `sender`: 交易发送者的Sui地址。
    /// - `coin_in`: 代表输入代币的 `Argument`。
    /// - `amount_in`: (可选) 输入代币的数量。某些DEX实现可能需要它，而另一些则直接使用 `coin_in` 的全部面额。
    ///
    /// 返回:
    /// - `Result<Argument>`: 代表从交换中获得的输出代币的 `Argument`。
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        sender: SuiAddress,
        coin_in: Argument,
        amount_in: Option<u64>,
    ) -> Result<Argument>;

    // --- Getter 方法 ---
    fn coin_in_type(&self) -> String; // 返回当前交易方向的输入代币类型字符串
    fn coin_out_type(&self) -> String; // 返回当前交易方向的输出代币类型字符串
    fn protocol(&self) -> Protocol;   // 返回DEX的协议类型 (如 Cetus, Turbos)
    fn liquidity(&self) -> u128;     // 返回池的流动性估值
    fn object_id(&self) -> ObjectID; // 返回DEX池对象的ID

    /// `flip` 方法
    ///
    /// 翻转DEX实例的交易方向 (即交换输入代币和输出代币)。
    /// 这会修改DEX实例内部的状态。
    fn flip(&mut self);

    // --- 调试和测试用的方法 ---
    /// `is_a2b` 方法
    ///
    /// 判断当前交易方向是否为 "A to B" (例如，从池中的token0到token1)。
    /// 这个方向的定义可能因DEX而异，通常用于选择调用哪个具体的合约函数 (如 `swap_a2b` vs `swap_b2a`)。
    fn is_a2b(&self) -> bool;

    /// `swap_tx` 方法 (主要用于测试)
    ///
    /// 构建一个完整的、独立的Sui交易数据 (`TransactionData`)，用于执行一次简单的交换。
    /// 不涉及复杂的路径组合或闪电贷。
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData>;
}

/// `CloneBoxedDex` Trait (克隆Boxed Dex接口)
///
/// 这是一个辅助trait，用于使得 `Box<dyn Dex>` 特征对象能够被克隆。
/// Rust的特征对象本身不支持 `Clone` trait，除非通过这种方式显式实现。
pub trait CloneBoxedDex {
    fn clone_boxed(&self) -> Box<dyn Dex>;
}

/// 为所有实现了 `Dex` 和 `Clone` (以及生命周期 `'static`) 的类型 `T` 实现 `CloneBoxedDex`。
impl<T> CloneBoxedDex for T
where
    T: 'static + Dex + Clone, // `'static` 生命周期表示类型T不包含任何非静态引用
{
    fn clone_boxed(&self) -> Box<dyn Dex> {
        Box::new(self.clone()) // 调用T的具体clone方法，然后包装到Box中
    }
}

/// 为 `Box<dyn Dex>` 实现 `Clone` trait。
/// 当调用 `clone()` 在一个 `Box<dyn Dex>` 上时，它会实际调用 `clone_boxed()` 方法。
impl Clone for Box<dyn Dex> {
    fn clone(&self) -> Box<dyn Dex> {
        self.clone_boxed()
    }
}

// --- 为 `Box<dyn Dex>` 实现比较和哈希相关的 traits ---
// 这使得 `Box<dyn Dex>` 可以在需要这些特性的数据结构（如HashMap, HashSet）中使用，
// 或者进行比较操作。比较和哈希是基于DEX池的 `object_id()`。

impl PartialEq for Box<dyn Dex> {
    fn eq(&self, other: &Self) -> bool {
        self.object_id() == other.object_id() // 基于池对象ID判断是否相等
    }
}

impl Eq for Box<dyn Dex> {} // Eq是一个标记trait，表示 `a == a` 总是为true (由PartialEq保证)

impl Hash for Box<dyn Dex> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.object_id().hash(state); // 使用池对象ID进行哈希
    }
}

/// 为 `Box<dyn Dex>` 实现 `fmt::Debug` trait，用于调试时打印DEX信息。
impl fmt::Debug for Box<dyn Dex> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 打印协议名称、池对象ID、输入代币类型和输出代币类型
        write!(
            f,
            "{}(Pool: {}, In: {}, Out: {})", // 简化了格式，更易读
            self.protocol(),
            self.object_id(),
            self.coin_in_type(),
            self.coin_out_type()
        )
    }
}

/// `Defi` 结构体
///
/// 封装了与DeFi协议交互的顶层逻辑。
/// 它持有一个 `DexSearcher` (用于发现DEX) 和一个 `Trader` (用于执行和模拟交易)。
#[derive(Clone)] // Defi结构体本身也可以被克隆 (因为其成员都是Arc包裹的，克隆成本低)
pub struct Defi {
    dex_searcher: Arc<dyn DexSearcher>, // 共享的DEX搜索器实例
    trader: Arc<Trader>,               // 共享的交易执行器/模拟器实例
}

impl Defi {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `Defi` 实例。
    ///
    /// 参数:
    /// - `http_url`: Sui RPC节点的URL，传递给 `DexSearcher` 和 `Trader` 进行初始化。
    /// - `simulator_pool`: 共享的模拟器对象池。
    pub async fn new(http_url: &str, simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>) -> Result<Self> {
        // 初始化 IndexerDexSearcher
        let dex_searcher = IndexerDexSearcher::new(http_url, Arc::clone(&simulator_pool)).await?;
        // 初始化 Trader
        let trader_instance = Trader::new(simulator_pool).await?; // Trader也改为trader_instance

        Ok(Self {
            dex_searcher: Arc::new(dex_searcher),
            trader: Arc::new(trader_instance), // trader_instance
        })
    }

    /// `find_dexes` 方法 (已废弃或内部使用，标记为 dead_code)
    ///
    /// 直接调用 `dex_searcher` 的同名方法。
    #[allow(dead_code)] // 允许存在未使用的代码
    pub async fn find_dexes(&self, coin_in_type: &str, coin_out_type: Option<String>) -> Result<Vec<Box<dyn Dex>>> {
        self.dex_searcher.find_dexes(coin_in_type, coin_out_type).await
    }

    /// `find_sell_paths` 方法
    ///
    /// 查找从指定的 `coin_in_type` 开始，通过一系列DEX交换，最终换回SUI原生代币的交易路径。
    ///
    /// 逻辑:
    /// 1. 如果输入代币已经是SUI，则返回一个空路径 (表示无需交换)。
    /// 2. 使用类似广度优先搜索 (BFS) 或迭代加深的方式（通过 `stack` 和 `MAX_HOP_COUNT` 控制）来发现多跳路径。
    ///    - 在每一跳 (hop)，为当前栈中的代币查找可以将其交易出去的DEX。
    ///    - 优先查找能直接换成SUI的DEX，或者能换成其他锚定币的DEX (如果不是最后一跳)。
    ///    - 对找到的DEX列表进行流动性过滤和数量限制。
    ///    - 将下一跳可能的代币类型加入 `new_stack`。
    /// 3. 收集所有中间步骤的 `coin_type -> Vec<Dex>` 映射到 `all_hops` HashMap中。
    /// 4. 使用DFS (深度优先搜索) 算法，基于 `all_hops` 构建出所有有效的、不超过 `MAX_HOP_COUNT` 的、
    ///    最终能换回SUI的完整交易路径 (`Vec<Vec<Box<dyn Dex>>>`)。
    /// 5. 将这些路径包装成 `Path` 对象返回。
    ///
    /// 参数:
    /// - `coin_in_type`: 输入代币的类型字符串。
    ///
    /// 返回:
    /// - `Result<Vec<Path>>`: 包含所有找到的卖出路径的向量。
    pub async fn find_sell_paths(&self, coin_in_type: &str) -> Result<Vec<Path>> {
        // 如果输入代币就是SUI，则无需寻找路径，直接返回一个默认的空路径。
        // Path::default() 可能代表一个不包含任何DEX的路径。
        if coin::is_native_coin(coin_in_type) {
            return Ok(vec![Path::default()]);
        }

        // `all_hops`: HashMap，键是代币类型字符串，值是连接到该代币的、可作为下一步交易的DEX列表。
        let mut all_hops = HashMap::new();
        // `stack`: 用于迭代搜索的代币类型栈。初始时只包含输入的代币类型。
        let mut stack = vec![coin_in_type.to_string()];
        // `visited`: HashSet，记录已经处理过的代币类型，防止循环搜索和重复工作。
        let mut visited = HashSet::new();
        // `visited_dexes`: HashSet，记录已经添加到 `all_hops` 中的DEX的ObjectID，避免重复添加同一个DEX。
        let mut visited_dexes = HashSet::new();

        // 最多进行 `MAX_HOP_COUNT` 次跳跃（即路径最多包含 `MAX_HOP_COUNT` 个DEX）。
        for nth_hop in 0..MAX_HOP_COUNT {
            let is_last_hop = nth_hop == MAX_HOP_COUNT - 1; // 判断是否是允许的最后一跳
            let mut new_stack = vec![]; // 用于存储下一轮要处理的代币类型

            // 处理当前层级 (stack) 中的所有代币类型
            while let Some(current_coin_type) = stack.pop() {
                // 如果该代币类型已经访问过，或者已经是SUI (目标代币)，则跳过。
                if visited.contains(&current_coin_type) || coin::is_native_coin(&current_coin_type) {
                    continue;
                }
                visited.insert(current_coin_type.clone()); // 标记为已访问

                // 确定在当前跳的目标输出代币类型：
                // - 如果是锚定币 (pegged_coin_types) 或者已经是最后一跳，则目标输出是SUI。
                // - 否则，目标输出是 `None` (表示查找所有可能的输出代币)。
                let target_coin_out_type = if pegged_coin_types().contains(current_coin_type.as_str()) || is_last_hop {
                    Some(SUI_COIN_TYPE.to_string())
                } else {
                    None
                };

                // 从DEX搜索器查找从 `current_coin_type` 出发，目标为 `target_coin_out_type` 的DEX列表。
                let mut dexes_from_current_coin = if let Ok(dexes) = self.dex_searcher.find_dexes(&current_coin_type, target_coin_out_type).await {
                    dexes
                } else {
                    continue; // 如果查找失败，则跳过当前代币类型
                };

                // 过滤掉流动性过低的DEX
                dexes_from_current_coin.retain(|dex| dex.liquidity() >= MIN_LIQUIDITY);

                // 如果找到的DEX数量过多，进行筛选：
                // - 优先保留尚未访问过的DEX。
                // - 按流动性降序排序。
                // - 截取前 `MAX_POOL_COUNT` 个。
                if dexes_from_current_coin.len() > MAX_POOL_COUNT {
                    dexes_from_current_coin.retain(|dex| !visited_dexes.contains(&dex.object_id()));
                    dexes_from_current_coin.sort_by_key(|dex| std::cmp::Reverse(dex.liquidity())); // Reverse实现降序
                    dexes_from_current_coin.truncate(MAX_POOL_COUNT);
                }

                if dexes_from_current_coin.is_empty() {
                    continue; // 如果没有符合条件的DEX，则跳过
                }

                // 对于每个找到的DEX：
                for dex in &dexes_from_current_coin {
                    let out_coin_type = dex.coin_out_type();
                    // 如果该DEX的输出代币类型尚未访问过，则加入到下一轮的处理栈中。
                    if !visited.contains(&out_coin_type) {
                        new_stack.push(out_coin_type.clone());
                    }
                    visited_dexes.insert(dex.object_id()); // 记录已访问的DEX
                }
                // 将当前代币类型及其可达的DEX列表存入 `all_hops`。
                all_hops.insert(current_coin_type.clone(), dexes_from_current_coin);
            } // 结束 while stack.pop()

            if is_last_hop { // 如果已经是最后一跳，则停止搜索更深的路径
                break;
            }

            stack = new_stack; // 更新stack为下一轮要处理的代币
        } // 结束 for nth_hop

        // --- 使用DFS从 `all_hops` 构建完整路径 ---
        let mut routes = vec![]; // 存储所有找到的完整路径
        let mut current_path_segment = vec![]; // DFS过程中当前的路径段
        // 从初始的 `coin_in_type` 开始进行DFS
        dfs(coin_in_type, &mut current_path_segment, &all_hops, &mut routes);

        // 将 `Vec<Vec<Box<dyn Dex>>>` 转换为 `Vec<Path>`
        Ok(routes.into_iter().map(Path::new).collect())
    }

    /// `find_buy_paths` 方法
    ///
    /// 查找从SUI原生代币开始，购买到指定的 `coin_out_type` 的交易路径。
    /// 实现方式是：
    /// 1. 调用 `find_sell_paths(coin_out_type)` 找到从 `coin_out_type` 卖出到SUI的路径。
    /// 2. 将这些路径反转。
    /// 3. 将路径中每个DEX实例的方向也翻转 (`dex.flip()`)。
    ///
    /// 参数:
    /// - `coin_out_type`: 目标购买的代币类型字符串。
    ///
    /// 返回:
    /// - `Result<Vec<Path>>`: 包含所有找到的购买路径的向量。
    pub async fn find_buy_paths(&self, coin_out_type: &str) -> Result<Vec<Path>> {
        let mut paths = self.find_sell_paths(coin_out_type).await?;
        for path in &mut paths {
            path.path.reverse(); // 反转路径中DEX的顺序
            for dex in &mut path.path {
                dex.flip(); // 翻转每个DEX的交易方向 (in <-> out)
            }
        }
        Ok(paths)
    }

    /// `find_best_path_exact_in` 方法
    ///
    /// 在一组给定的交易路径 (`paths`) 中，为指定的输入金额 (`amount_in`)，
    /// 通过模拟交易找到能产生最佳输出结果（通常是最大化输出代币数量或利润）的路径。
    ///
    /// 参数:
    /// - `paths`: 一个包含多个 `Path` 对象的切片，代表候选的交易路径。
    /// - `sender`: 交易发送者的Sui地址。
    /// - `amount_in`: 输入代币的数量。
    /// - `trade_type`: 交易类型 (如 Swap, Flashloan)。
    /// - `gas_coins`: 用于支付Gas的代币对象引用列表。
    /// - `sim_ctx`: 模拟上下文，包含当前纪元信息 (如Gas价格)。
    ///
    /// 返回:
    /// - `Result<PathTradeResult>`: 包含最佳路径及其交易结果的 `PathTradeResult`。
    pub async fn find_best_path_exact_in(
        &self,
        paths: &[Path],
        sender: SuiAddress,
        amount_in: u64,
        trade_type: TradeType,
        gas_coins: &[ObjectRef],
        sim_ctx: &SimulateCtx,
    ) -> Result<PathTradeResult> {
        let mut joinset = JoinSet::new(); // 用于并发模拟多条路径

        for (idx, path) in paths.iter().enumerate() {
            if path.is_empty() { // 跳过空路径
                continue;
            }

            // 克隆所需的状态以传递给异步任务
            let trader_clone = Arc::clone(&self.trader); // Trader是Arc包裹的，克隆是廉价的
            let path_clone = path.clone();
            let gas_coins_clone = gas_coins.to_vec();
            let sim_ctx_clone = sim_ctx.clone();

            // 为每条路径的模拟创建一个异步任务
            joinset.spawn(
                async move {
                    // 调用 Trader 的 get_trade_result 方法进行模拟
                    let result = trader_clone
                        .get_trade_result(&path_clone, sender, amount_in, trade_type, gas_coins_clone, sim_ctx_clone)
                        .await;
                    (idx, result) // 返回路径索引和模拟结果
                }
                .in_current_span(), // 继承当前的tracing span
            );
        }

        // 初始化最佳结果
        let (mut best_idx, mut best_trade_res) = (0, TradeResult::default());
        // 收集并处理并发模拟的结果
        while let Some(Ok((idx, trade_res_result))) = joinset.join_next().await {
            match trade_res_result { // trade_res_result 是 Result<TradeResult, Error>
                Ok(current_trade_res) => {
                    // 如果当前路径的模拟结果优于已知的最佳结果，则更新最佳结果。
                    // `TradeResult` 需要实现 `PartialOrd` 以便比较。
                    if current_trade_res > best_trade_res {
                        best_idx = idx;
                        best_trade_res = current_trade_res;
                    }
                }
                Err(_error) => {
                    // 如果某条路径模拟失败，可以选择记录错误。
                    // tracing::error!(path = ?paths[idx], ?error, "交易模拟错误");
                }
            }
        }

        // 确保找到的最佳交易结果的输出金额大于0。
        ensure!(best_trade_res.amount_out > 0, "最佳路径的输出金额为零");

        // 使用最佳路径和其模拟结果创建 `PathTradeResult`。
        Ok(PathTradeResult::new(paths[best_idx].clone(), amount_in, best_trade_res))
    }

    /// `build_final_tx_data` 方法
    ///
    /// 构建最终的可执行Sui交易数据 (`TransactionData`)。
    /// 通常用于包含闪电贷操作的套利交易。
    ///
    /// 参数:
    /// - `sender`: 交易发送者地址。
    /// - `amount_in`: 初始输入金额 (或闪电贷金额)。
    /// - `path`: 选定的最佳交易路径 (`&Path`)。
    /// - `gas_coins`: Gas币列表。
    /// - `gas_price`: 当前Gas价格。
    /// - `source`: 交易来源信息 (例如MEV竞价相关数据)。
    ///
    /// 返回:
    /// - `Result<TransactionData>`: 构建好的交易数据。
    pub async fn build_final_tx_data(
        &self,
        sender: SuiAddress,
        amount_in: u64,
        path: &Path,
        gas_coins: Vec<ObjectRef>,
        gas_price: u64,
        source: Source,
    ) -> Result<TransactionData> {
        // 调用 Trader 的 get_flashloan_trade_tx 方法来构建包含闪电贷的完整交易PTB。
        // 这个方法会处理：借款 -> 执行路径上的交换 -> 偿还借款。
        let (tx_data, _expected_profit) = self // _expected_profit 未使用
            .trader
            .get_flashloan_trade_tx(path, sender, amount_in, gas_coins, gas_price, source)
            .await?;

        Ok(tx_data)
    }
}

/// `dfs` (Depth-First Search, 深度优先搜索) 辅助函数
///
/// 用于从 `hops` (一个表示代币之间可通过DEX连接的图) 中构建所有可能的交易路径。
///
/// 参数:
/// - `coin_type`: 当前DFS路径末端的代币类型。
/// - `current_path_segment`: 当前正在构建的路径段 (`&mut Vec<Box<dyn Dex>>`)。
/// - `hops`: 一个HashMap，`key`是代币类型字符串，`value`是可从该代币交易出去的DEX列表。
/// - `routes`: 一个可变引用，用于存储所有找到的完整路径 (`&mut Vec<Vec<Box<dyn Dex>>>`)。
fn dfs(
    coin_type: &str,
    current_path_segment: &mut Vec<Box<dyn Dex>>,
    hops: &HashMap<String, Vec<Box<dyn Dex>>>,
    routes: &mut Vec<Vec<Box<dyn Dex>>>,
) {
    // 终止条件1: 如果当前代币是SUI (原生/目标代币)，则表示一条完整路径已找到。
    if coin::is_native_coin(coin_type) {
        routes.push(current_path_segment.clone()); // 将当前路径段的克隆加入到结果中
        return;
    }
    // 终止条件2: 如果路径长度达到或超过最大跳数限制，则停止深入。
    if current_path_segment.len() >= MAX_HOP_COUNT {
        return;
    }
    // 终止条件3: 如果当前代币类型在 `hops` 中没有对应的可交易DEX，则无法继续。
    if !hops.contains_key(coin_type) {
        return;
    }

    // 遍历从当前 `coin_type` 出发的所有DEX
    for dex in hops.get(coin_type).unwrap() { // unwrap() 因为上面已经 contains_key 检查
        current_path_segment.push(dex.clone()); // 将当前DEX加入路径段 (回溯时会移除)
        // 递归调用DFS，以该DEX的输出代币作为新的当前代币类型
        dfs(&dex.coin_out_type(), current_path_segment, hops, routes);
        current_path_segment.pop(); // 回溯：将当前DEX从路径段中移除，尝试其他分支
    }
}

/// `PathTradeResult` 结构体
///
/// 封装了一条特定路径在给定输入金额下的交易模拟结果。
#[derive(Debug, Clone)]
pub struct PathTradeResult {
    pub path: Path,         // 交易路径
    pub amount_in: u64,     // 输入金额
    pub amount_out: u64,    // 输出金额
    pub gas_cost: i64,      // Gas成本 (有符号整数，因为可能是估算)
    pub cache_misses: u64,  // 模拟过程中的缓存未命中次数
}

impl PathTradeResult {
    /// `new` 构造函数
    pub fn new(path: Path, amount_in: u64, trade_res: TradeResult) -> Self {
        Self {
            path,
            amount_in,
            amount_out: trade_res.amount_out,
            gas_cost: trade_res.gas_cost,
            cache_misses: trade_res.cache_misses,
        }
    }

    /// `profit` 方法
    ///
    /// 计算这条路径的利润。利润计算逻辑：
    /// - 只有当路径的输入代币 (`coin_in_type`) 和输出代币 (`coin_out_type`) 都是SUI时，
    ///   才计算 `amount_out - amount_in - gas_cost` 作为利润。这对应于纯粹的SUI增值套利。
    /// - 如果路径的输入是SUI，但输出不是SUI（例如，用SUI购买了其他代币），
    ///   则利润被视为负的输入金额减去Gas成本 ( `-amount_in - gas_cost` )。
    ///   这可能表示一种成本计算，或者假设目标是积累SUI。
    /// - 其他情况（例如输入不是SUI）利润为0。这个逻辑可能需要根据具体的套利策略调整。
    ///
    /// 返回:
    /// - `i128`: 计算出的利润值。
    pub fn profit(&self) -> i128 {
        if self.path.coin_in_type() == SUI_COIN_TYPE { // 如果输入是SUI
            if self.path.coin_out_type() == SUI_COIN_TYPE { // 且输出也是SUI (SUI -> ... -> SUI)
                // 利润 = 输出SUI - 输入SUI - Gas成本
                return self.amount_out as i128 - self.amount_in as i128 - self.gas_cost as i128;
            }
            // 如果输入是SUI，但输出不是SUI (例如购买资产)
            // 利润被视为负的投入成本 (amount_in + gas_cost)
            // 这表示以SUI计价的净支出。
            0i128 - self.gas_cost as i128 - self.amount_in as i128 // 注意这里是 i128
        } else {
            // 如果输入不是SUI，当前逻辑不计算利润 (返回0)。
            // 这可能意味着只关注以SUI开始和结束的套利，或者需要其他方式来评估非SUI本位的利润。
            0
        }
    }
}

/// 为 `PathTradeResult` 实现 `fmt::Display` trait，用于打印输出。
impl fmt::Display for PathTradeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PathTradeResult {{ amount_in: {}, amount_out: {}, profit: {}, path: {:?} ... }}",
            self.amount_in,
            self.amount_out,
            self.profit(),
            self.path // Path本身也需要实现Debug
        )
    }
}


// --- 测试模块 ---
#[cfg(test)]
mod tests {

    use simulator::HttpSimulator; // HTTP模拟器，用于连接真实RPC节点进行测试
    use tracing::info; // 日志库

    use super::*; // 导入外部模块 (defi::mod.rs) 的所有公共成员
    use crate::config::tests::TEST_HTTP_URL; // 测试用的RPC URL

    /// `test_find_sell_paths` 测试函数
    ///
    /// 测试 `Defi::find_sell_paths` 方法是否能为给定的非SUI代币找到卖出到SUI的路径。
    #[tokio::test]
    async fn test_find_sell_paths() {
        // 初始化日志系统
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        // 创建模拟器对象池 (这里使用HttpSimulator)
        let simulator_pool = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(HttpSimulator::new(&TEST_HTTP_URL, &None).await) as Box<dyn Simulator> })
        }));

        // 创建Defi实例
        let defi = Defi::new(TEST_HTTP_URL, simulator_pool).await.unwrap();

        // 定义一个测试用的输入代币类型 (例如 OCEAN 代币)
        let coin_in_type = "0xa8816d3a6e3136e86bc2873b1f94a15cadc8af2703c075f2d546c2ae367f4df9::ocean::OCEAN";
        // 查找卖出路径
        let paths = defi.find_sell_paths(coin_in_type).await.unwrap();
        
        // 断言至少找到一条路径 (如果测试环境中有流动性)
        assert!(!paths.is_empty(), "未找到任何卖出路径");

        // 打印找到的路径 (用于调试或观察)
        for path in paths {
            info!(?path, "找到的卖出路径 (sell path)"); // ?path 使用Debug格式打印Path
        }
    }

    /// `test_find_buy_paths` 测试函数
    ///
    /// 测试 `Defi::find_buy_paths` 方法是否能为给定的非SUI代币找到从SUI买入的路径。
    #[tokio::test]
    async fn test_find_buy_paths() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let simulator_pool = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(HttpSimulator::new(&TEST_HTTP_URL, &None).await) as Box<dyn Simulator> })
        }));

        let defi = Defi::new(TEST_HTTP_URL, simulator_pool).await.unwrap();

        // 定义一个测试用的目标输出代币类型 (例如 OCEAN 代币)
        let coin_out_type = "0xa8816d3a6e3136e86bc2873b1f94a15cadc8af2703c075f2d546c2ae367f4df9::ocean::OCEAN";
        // 查找买入路径
        let paths = defi.find_buy_paths(coin_out_type).await.unwrap();

        assert!(!paths.is_empty(), "未找到任何买入路径");
        
        for path in paths {
            info!(?path, "找到的买入路径 (buy path)");
        }
    }
}
