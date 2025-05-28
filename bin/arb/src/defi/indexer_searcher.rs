// 该文件 `indexer_searcher.rs` 实现了 `DexSearcher` trait，
// 它负责从一个外部的“DEX索引器服务”（DexIndexer）获取原始的交易池数据，
// 然后根据这些数据创建并返回具体的DEX交互对象（如Cetus, Turbos, Aftermath等实例）。
// 这个模块是套利机器人动态发现和适配不同DEX协议的关键。
//
// 文件概览:
// 1. `INDEXER`: 一个静态的 `OnceCell<Arc<DexIndexer>>`，用于全局单次初始化并缓存 `DexIndexer` 客户端。
//    `DexIndexer` 可能是一个外部服务或库，它监控Sui链上的DEX活动，并提供查询这些DEX池子信息的API。
// 2. `IndexerDexSearcher` 结构体: 实现了 `DexSearcher` trait。它持有一个 `DexIndexer` 客户端的引用
//    和一个模拟器对象池 (`simulator_pool`)。模拟器用于获取具体的链上对象状态以初始化DEX实例。
// 3. `new()` 方法: `IndexerDexSearcher` 的构造函数。
// 4. `new_dexes()` (私有辅助异步函数): 接收一个从索引器获取的原始 `Pool` 信息，
//    根据 `pool.protocol` 字段（如 Protocol::Cetus, Protocol::Turbos 等），
//    异步创建对应协议的具体DEX实现（例如 `Cetus::new(...)`, `Turbos::new(...)`）。
//    返回一个包含具体DEX实例的向量 ( `Vec<Box<dyn Dex>>` )。
//    一个原始 `Pool` 可能对应多个可交易的 `Dex` 实例 (例如 Aftermath 的多币种池)。
// 5. `find_dexes()` 方法 (实现 `DexSearcher` trait):
//    - 根据输入的 `token_in_type` 和可选的 `token_out_type`，从 `DexIndexer` 查询相关的池列表。
//    - 并发地（使用 `JoinSet`）为每个查询到的池调用 `new_dexes()` 来创建具体的DEX实例。
//    -收集所有创建成功的DEX实例并返回。
// 6. `find_test_path()` 方法 (实现 `DexSearcher` trait):
//    - 根据一个给定的对象ID路径 ( `path: &[ObjectID]` )，为路径中的每个ID创建一个DEX实例，
//      并组装成一个 `Path` 对象。这主要用于测试或特定的路径发现。
//
// 工作流程:
// 套利逻辑 -> 调用 `IndexerDexSearcher::find_dexes(token_A, Some(token_B))`
//  -> `IndexerDexSearcher` 向 `DexIndexer` 服务查询所有包含 token_A 和 token_B 的池
//  -> `DexIndexer` 返回原始 `Pool` 对象列表
//  -> `IndexerDexSearcher` 遍历每个 `Pool` 对象
//     -> 对每个 `Pool`，调用 `new_dexes()`
//        -> `new_dexes()` 根据 `pool.protocol` 调用相应的具体DEX构造函数 (如 `Cetus::new()`)
//           -> 具体DEX构造函数使用模拟器从链上获取详细状态并初始化
//        -> 返回 `Box<dyn Dex>`
//  -> `IndexerDexSearcher` 收集所有 `Box<dyn Dex>` 并返回给套利逻辑。

// 引入标准库及第三方库
use dex_indexer::{ // `dex_indexer` crate，提供了与DEX索引服务交互的功能
    types::{Pool, Protocol}, // `Pool`是索引器返回的原始池信息结构，`Protocol`是DEX协议的枚举类型
    DexIndexer,               // DEX索引器客户端
};
use eyre::{bail, ensure, OptionExt, Result}; // 错误处理库
use object_pool::ObjectPool; // 对象池，用于管理和复用模拟器实例
use simulator::Simulator; // 交易模拟器接口
use std::sync::Arc; // 原子引用计数，用于安全共享 `DexIndexer` 和模拟器实例
use sui_sdk::SUI_COIN_TYPE; // SUI原生代币的类型字符串
use sui_types::base_types::ObjectID; // Sui对象ID类型
use tokio::sync::OnceCell; // Tokio异步单次初始化单元，用于全局缓存 `DexIndexer`
use tokio::task::JoinSet; // Tokio任务集合，用于并发执行多个异步任务

// 从当前项目的 `defi` 模块中引入各种具体的DEX实现和核心trait/结构体
use super::{ // `super` 指代父模块 (`defi`)
    aftermath::Aftermath, // Aftermath DEX实现
    cetus::Cetus,         // Cetus DEX实现
    deepbook_v2::DeepbookV2, // DeepBook V2 实现
    flowx_clmm::FlowxClmm, // FlowX CLMM 实现
    turbos::Turbos,       // Turbos DEX实现
    Dex,                  // 通用DEX接口 (trait)
    DexSearcher,          // DEX搜索器接口 (trait)
    Path,                 // 交易路径结构体
};
// 从当前crate的 `defi` 模块引入其他DEX实现
use crate::defi::{blue_move::BlueMove, kriya_amm::KriyaAmm, kriya_clmm::KriyaClmm};

// 全局静态 `DexIndexer` 实例的缓存。
// `OnceCell` 确保 `DexIndexer` 在整个程序生命周期中只被初始化一次。
// `Arc` 允许多个地方安全地共享这个单例。
static INDEXER: OnceCell<Arc<DexIndexer>> = OnceCell::const_new();

/// `IndexerDexSearcher` 结构体
///
/// 实现了 `DexSearcher` trait，通过查询 `DexIndexer` 服务来发现和构建DEX实例。
#[derive(Clone)] // 允许克隆 `IndexerDexSearcher` 实例
pub struct IndexerDexSearcher {
    simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>, // 共享的模拟器对象池
    indexer: Arc<DexIndexer>,                             // 共享的DEX索引器客户端
}

impl IndexerDexSearcher {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `IndexerDexSearcher` 实例。
    /// 它会异步初始化（如果尚未初始化）全局的 `DexIndexer` 客户端。
    ///
    /// 参数:
    /// - `http_url`: DEX索引器服务可能需要的HTTP RPC URL (用于其内部的Sui客户端)。
    /// - `simulator_pool`: 一个共享的模拟器对象池，用于获取链上状态以初始化具体的DEX实例。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `IndexerDexSearcher` 实例。
    pub async fn new(http_url: &str, simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>) -> Result<Self> {
        // 获取或初始化全局的 DexIndexer 实例。
        // `get_or_init` 接收一个异步闭包，该闭包在 `INDEXER` 首次被访问时执行。
        let indexer_instance = INDEXER
            .get_or_init(|| async {
                // 在异步闭包内创建 DexIndexer 实例。
                // `.unwrap()` 用于简化，实际生产代码中应处理 `DexIndexer::new` 可能返回的错误。
                let indexer = DexIndexer::new(http_url).await.unwrap();
                Arc::new(indexer) // 将新创建的实例包装在 Arc 中
            })
            .await // 等待初始化完成
            .clone(); // 克隆 Arc 指针

        Ok(Self {
            simulator_pool,
            indexer: indexer_instance,
        })
    }
}

/// `new_dexes` (私有异步辅助函数)
///
/// 根据从索引器获取的单个 `Pool` 信息，创建对应的一个或多个具体DEX交互实例。
///
/// 参数:
/// - `simulator`: 一个模拟器实例 (`Arc<Box<dyn Simulator>>`)，用于获取链上对象数据。
/// - `pool`: 从 `DexIndexer` 获取的原始池信息 (`&Pool`)。
/// - `token_in_type`: 当前希望交易的输入代币类型字符串。
/// - `token_out_type`: (可选) 当前希望交易的输出代币类型字符串。
///   某些DEX (如Aftermath的多币种池) 在初始化时如果提供了输出代币，可以只创建特定交易对的实例。
///   如果为 `None`，则可能为输入代币与池中所有其他代币的组合都创建实例。
///
/// 返回:
/// - `Result<Vec<Box<dyn Dex>>>`: 一个包含动态分发的具体DEX实例的向量。
///   `Box<dyn Dex>` 表示这是一个特征对象，可以是任何实现了 `Dex` trait 的具体类型。
async fn new_dexes(
    simulator: Arc<Box<dyn Simulator>>, // 注意：这里接收的是 Arc<Box<dyn Simulator>>，而不是 ObjectPool
    pool_info: &Pool,
    token_in_type: &str,
    token_out_type: Option<String>,
) -> Result<Vec<Box<dyn Dex>>> {
    // 根据 `pool_info.protocol` 枚举值，匹配并调用相应DEX类型的构造函数。
    let dex_instances: Vec<Box<dyn Dex>> = match pool_info.protocol {
        Protocol::Turbos => {
            let dex = Turbos::new(simulator, pool_info, token_in_type).await?;
            vec![Box::new(dex) as Box<dyn Dex>] // 将具体类型包装为特征对象
        }
        Protocol::Cetus => {
            let dex = Cetus::new(simulator, pool_info, token_in_type).await?;
            vec![Box::new(dex) as Box<dyn Dex>]
        }
        Protocol::Aftermath => {
            // Aftermath::new 本身返回 Vec<Aftermath>，因为一个Aftermath池对象可能代表多个交易对。
            // 所以需要 `into_iter().map(...).collect()` 来转换。
            Aftermath::new(simulator, pool_info, token_in_type, token_out_type)
                .await?
                .into_iter()
                .map(|dex| Box::new(dex) as Box<dyn Dex>)
                .collect()
        }
        Protocol::FlowxClmm => {
            let dex = FlowxClmm::new(simulator, pool_info, token_in_type).await?;
            vec![Box::new(dex) as Box<dyn Dex>]
        }
        Protocol::KriyaAmm => {
            let dex = KriyaAmm::new(simulator, pool_info, token_in_type).await?;
            vec![Box::new(dex) as Box<dyn Dex>]
        }
        Protocol::KriyaClmm => {
            let dex = KriyaClmm::new(simulator, pool_info, token_in_type).await?;
            vec![Box::new(dex) as Box<dyn Dex>]
        }
        Protocol::DeepbookV2 => {
            let dex = DeepbookV2::new(simulator, pool_info, token_in_type).await?;
            vec![Box::new(dex) as Box<dyn Dex>]
        }
        Protocol::BlueMove => {
            let dex = BlueMove::new(simulator, pool_info, token_in_type).await?;
            vec![Box::new(dex) as Box<dyn Dex>]
        }
        // 如果遇到不支持的协议类型，则返回错误。
        // `bail!` 是 `eyre` 库中用于快速返回错误的宏。
        _ => bail!("不支持的协议类型: {:?}", pool_info.protocol),
    };

    Ok(dex_instances)
}

/// 为 `IndexerDexSearcher` 实现 `DexSearcher` trait。
#[async_trait::async_trait] // 因为 `DexSearcher` trait 中的方法是异步的
impl DexSearcher for IndexerDexSearcher {
    /// `find_dexes` 方法
    ///
    /// 根据输入的代币类型（和可选的输出代币类型）查找所有相关的DEX实例。
    ///
    /// 参数:
    /// - `token_in_type`: 输入代币的类型字符串。
    /// - `token_out_type`: (可选) 输出代币的类型字符串。如果为 `None`，则查找所有包含 `token_in_type` 的池。
    ///
    /// 返回:
    /// - `Result<Vec<Box<dyn Dex>>>`: 包含所有找到并成功初始化的DEX实例的向量。
    async fn find_dexes(&self, token_in_type: &str, token_out_type: Option<String>) -> Result<Vec<Box<dyn Dex>>> {
        // 步骤 1: 从 `DexIndexer` 获取原始池列表。
        // 如果提供了 `token_out_type`，则按交易对查询 (`get_pools_by_token01`)。
        // 否则，按单个代币查询 (`get_pools_by_token`)。
        let raw_pools = if let Some(ref out_type) = token_out_type { // 使用 ref 避免消耗 token_out_type
            self.indexer.get_pools_by_token01(token_in_type, out_type)
        } else {
            self.indexer.get_pools_by_token(token_in_type)
        };

        // 确保从索引器获取到了池数据。
        // `ok_or_eyre` 用于将 Option 转换为 Result，如果 Option 是 None 则返回错误。
        // 这里改为 `ensure!` 宏，如果 `raw_pools` 是 `None`，则返回一个包含上下文信息的错误。
        ensure!(
            raw_pools.is_some(),
            "未从索引器找到相关的池, 输入代币: {}, 输出代币: {:?}",
            token_in_type,
            token_out_type
        );

        // 步骤 2: 并发地为每个原始池创建具体的DEX实例。
        let mut join_set = JoinSet::new(); // 创建一个任务集合用于并发执行
        for pool_data in raw_pools.unwrap() { // unwrap() 因为上面已经 ensure! 过 is_some()
            // 从模拟器对象池中获取一个模拟器实例。
            // `self.simulator_pool.get()` 返回的是 `Arc<Box<dyn Simulator>>` 的克隆，是廉价的。
            let simulator_instance = self.simulator_pool.get();
            let token_in_str = token_in_type.to_string(); // 将 &str 转换为 String 以满足 'static 生命周期要求 (如果需要)
            let token_out_opt_str = token_out_type.clone(); // 克隆 Option<String>

            // 为每个池的初始化过程创建一个异步任务。
            join_set.spawn(async move {
                // 在异步任务中调用 `new_dexes`。
                // `pool_data` 需要被移动到任务中。
                new_dexes(simulator_instance, &pool_data, &token_in_str, token_out_opt_str).await
            });
        }

        // 步骤 3: 收集并发任务的结果。
        let mut resulting_dex_list = Vec::new();
        while let Some(join_result) = join_set.join_next().await { // 等待下一个任务完成
            match join_result { // `join_result` 是 `Result<TaskOutput, JoinError>`
                Ok(new_dexes_call_result) => { // 任务本身没有panic或被取消
                    match new_dexes_call_result { // `new_dexes_call_result` 是 `new_dexes` 的 `Result<Vec<Box<dyn Dex>>>`
                        Ok(dex_batch) => resulting_dex_list.extend(dex_batch), // 成功则将DEX实例列表追加到总列表
                        Err(_error) => {
                            // 如果某个池初始化失败 (例如，协议不支持、链上对象解析错误等)，
                            //可以选择记录错误并继续处理其他池，而不是让整个搜索失败。
                            // trace!(?error, "无效的池或初始化DEX实例失败"); // 使用 trace! 避免在正常情况下过多日志
                        }
                    }
                }
                Err(join_error) => {
                    // 任务执行出错 (例如panic)
                    eprintln!("并发初始化DEX任务执行错误: {:?}", join_error);
                    // 或者选择传播这个错误: return Err(eyre!(join_error));
                }
            }
        }
        Ok(resulting_dex_list)
    }

    /// `find_test_path` 方法 (主要用于测试)
    ///
    /// 根据一个给定的对象ID序列（代表一个交易路径中的连续池子），
    /// 构建并返回一个 `Path` 对象。
    ///
    /// 参数:
    /// - `path_object_ids`: 一个包含多个 `ObjectID` 的切片，每个ID代表路径中的一个池。
    ///
    /// 返回:
    /// - `Result<Path>`: 包含按顺序初始化的DEX实例的 `Path` 对象。
    async fn find_test_path(&self, path_object_ids: &[ObjectID]) -> Result<Path> {
        let mut dexes_on_path = vec![]; // 用于存储路径上的DEX实例
        // 假设路径的第一个输入代币是SUI，后续的输入代币由前一个DEX的输出决定。
        let mut current_coin_in_type = SUI_COIN_TYPE.to_string();

        for pool_id in path_object_ids {
            let simulator_instance = self.simulator_pool.get();
            // 从索引器获取指定ID的池信息。
            let pool_info = self.indexer.get_pool_by_id(pool_id).ok_or_eyre(format!("测试路径中池ID {} 未在索引器中找到", pool_id))?;
            
            // 为该池创建DEX实例。
            // `new_dexes` 返回 `Vec<Box<dyn Dex>>`，对于测试路径，我们通常期望一个池ID只对应一个明确的交易对。
            // `.pop().unwrap()` 假设 `new_dexes` 对于这个场景总是返回包含一个元素的向量。
            // 如果 `new_dexes` 可能返回空向量或多个元素，这里的逻辑需要更健壮。
            // (例如，如果 `token_out_type` 是 `None`，Aftermath会返回多个实例)
            // 为了测试路径，通常会预先知道路径上每一步的输出代币，或者 `new_dexes` 需要能处理这种情况。
            // 假设这里 `token_out_type` 为 `None` 是为了让 `new_dexes` 自动推断或创建所有可能的交易对，
            // 然后我们选择其中一个 (例如，第一个，或基于某种逻辑)。
            // `.pop().unwrap()` 会取最后一个，这可能不总是正确的，除非 `new_dexes` 的返回顺序有保证
            // 或者对于这条路径，每个池只对应一个我们关心的 `Dex` 实例。
            // **修正**: 更好的做法是，如果路径定义了 token_out，则应该传入。
            // 如果没有，`new_dexes` 返回的 `Vec` 需要选择。对于一条确定的测试路径，
            // 通常每一步的输入输出都是确定的。
            // 这里的 `None` 作为 `token_out_type` 传给 `new_dexes`，意味着 `new_dexes` 内部需要有逻辑
            // 来确定这条路径上下一步的 `coin_out_type`。
            // 从 `dex.coin_out_type()` 获取输出代币作为下一步的输入代币。
            let mut created_dex_instances = new_dexes(simulator_instance, &pool_info, &current_coin_in_type, None).await?;
            if created_dex_instances.is_empty() {
                bail!("为池 {} 和输入代币 {} 创建DEX实例失败，得到空列表", pool_id, current_coin_in_type);
            }
            // TODO: 如果 created_dex_instances 有多个，需要选择正确的那个。
            // 例如，如果路径是 A->B->C，当前输入是A，池是A/B和A/D的池，需要选择A/B的那个。
            // 这里的实现是取第一个，这可能不总是正确，除非 new_dexes 返回的顺序有特定含义或只有一个元素。
            // 假设对于测试路径，通常一个池ID和输入币会唯一确定一个输出币（或我们关心的那个）。
            let dex_instance = created_dex_instances.remove(0); // 取第一个作为示例

            current_coin_in_type = dex_instance.coin_out_type(); // 更新下一步的输入代币类型
            dexes_on_path.push(dex_instance); // 将DEX实例添加到路径中
        }

        Ok(Path { path: dexes_on_path })
    }
}
