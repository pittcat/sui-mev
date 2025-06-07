// 该文件 `lib.rs` 是 `dex-indexer` crate (库) 的根文件和主入口。
// `dex-indexer` crate 的主要功能是发现、索引和存储Sui区块链上各种去中心化交易所 (DEX) 的流动性池信息。
// 它提供了一个API，使得其他应用程序（如套利机器人）可以方便地查询这些池的数据。
//
// 文件概览:
// 1. 模块声明: 引入了crate内部的各个子模块，如 `blockberry` (与Blockberry API交互),
//    `collector` (事件收集器), `file_db` (基于文件的数据库实现), `protocols` (DEX协议特定逻辑),
//    `strategy` (索引策略), 和 `types` (通用数据类型)。
// 2. `FILE_DB_DIR`: 定义了存储索引数据的默认目录路径 (通常在 "data" 子目录下)。
// 3. `supported_protocols()`: 返回一个包含所有当前支持的、需要被索引的DEX协议的列表。
// 4. `DexIndexer` 结构体: 这是外部用户与此crate交互的主要结构体。
//    - `pool_cache`: 一个 `PoolCache` 实例，用于在内存中缓存池数据，以提供快速查询。
//    - `db`: 一个实现了 `DB` trait 的数据库实例 (这里是 `FileDB`)，用于持久化存储池数据和处理进度。
//    - `_live_indexer_tasks`: 一个 `JoinSet`，用于管理在后台运行的实时索引任务 (例如，通过 `burberry::Engine` 运行)。
//      下划线前缀表示这个字段可能主要用于内部管理，例如确保任务在 `DexIndexer` drop时能够正确清理。
// 5. `DexIndexer::new()`: 异步构造函数。
//    - 初始化Sui客户端、数据库 (`FileDB`)。
//    - 从数据库加载已有的池数据到内存缓存 (`PoolCache`)。
//    - 创建并运行一个 `burberry::Engine` 实例：
//      - 使用 `QueryEventCollector` 定期触发事件。
//      - 使用 `PoolCreatedStrategy` 作为策略，该策略会响应触发事件，从链上或API获取最新的池数据，并将其存入数据库。
//        `strategy.backfill_pools().await?` 会在启动时执行一次历史数据回填。
//      - 使用 `DummyExecutor` 作为执行器，因为此引擎的主要目的是数据索引和处理，而不是执行外部动作。
// 6. 查询方法: 提供多种方法来从 `PoolCache` 或数据库中查询池信息，例如：
//    - `get_pools_by_token()`: 根据单个代币类型查找相关池。
//    - `get_pools_by_token01()`: 根据交易对的两种代币类型查找相关池。
//    - `get_pool_by_id()`: 根据池的ObjectID查找特定池。
//    - `pool_count()`: 获取特定协议的池数量。
//    - `get_all_pools()`: 获取特定协议的所有池列表。
// 7. 工具函数:
//    - `token01_key()`: 为一个代币对 (token0, token1) 生成一个规范化的键 (通常按字典序排序)，用于HashMap等。
//    - `normalize_coin_type()`: 将SUI代币的旧格式地址规范化为官方的 `SUI_COIN_TYPE`。
// 8. `DB` Trait: 定义了数据库操作的通用接口，如 `flush`, `load_token_pools`, `get_processed_cursors` 等。
//    `FileDB` 是这个trait的一个具体实现。
//
// 工作流程:
// 1. 当 `DexIndexer::new()` 被调用时，它会初始化必要的组件。
// 2. `PoolCreatedStrategy` 会首先执行 `backfill_pools()` 来尽可能多地从链上或API获取历史池数据并存入 `FileDB`。
// 3. 同时，`burberry::Engine` 会启动，`QueryEventCollector` 开始定期产生触发事件。
// 4. `PoolCreatedStrategy` 接收到这些触发事件后，会再次查询新的或更新的池数据，并将这些变更通过 `DB::flush()` 写入 `FileDB`。
//    同时，内存中的 `PoolCache` 也会被更新（或者在下次 `DexIndexer::new()` 时从 `FileDB` 重新加载）。
// 5. 外部用户（如套利机器人）可以通过 `DexIndexer` 实例的各种 `get_...` 方法来查询已索引的池数据。

//! Dex索引器
//! 使用方法: 参考单元测试 `test_get_pools`。

// 声明crate内部的模块
mod blockberry;      // 与Blockberry API交互的逻辑
mod collector;       // 事件收集器 (例如定时触发器)
mod file_db;         // 基于文件的数据库实现
mod protocols;       // 各DEX协议特定的数据提取或处理逻辑 (可能未使用或已整合到其他部分)
mod strategy;        // 索引策略 (如何发现和更新池数据)
pub mod types;       // 本地定义的通用类型 (如Pool, Protocol, PoolCache等)

// 引入标准库及第三方库
use std::{
    collections::{HashMap, HashSet}, // 哈希图和哈希集合
    fmt::Debug,                     // Debug trait，用于调试打印
    sync::Arc,                      // 原子引用计数
    time::Instant,                  // 精确时间点，用于计时
};

use burberry::Engine; // `burberry`事件处理引擎
use collector::QueryEventCollector; // 定时查询事件收集器
use eyre::Result; // `eyre`错误处理库
use strategy::PoolCreatedStrategy; // 池创建/更新的处理策略
use sui_sdk::{
    types::{base_types::ObjectID, event::EventID}, // Sui基本类型：对象ID, 事件ID
    SuiClientBuilder, SUI_COIN_TYPE,              // Sui客户端构建器, SUI原生代币类型字符串
};
use tokio::task::JoinSet; // Tokio任务集合，用于管理异步任务
use tracing::info; // 日志库
use types::{DummyExecutor, Event, NoAction, Pool, PoolCache, Protocol}; // 从本地 `types` 模块引入

/// `FILE_DB_DIR` 常量
///
/// 定义了存储索引数据的默认目录路径。
/// `concat!` 宏在编译时连接字符串字面量。
/// `env!("CARGO_MANIFEST_DIR")` 获取当前crate的根目录路径。
/// 所以，数据文件通常会存储在项目的 "data" 子目录下。
pub const FILE_DB_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data");

/// `supported_protocols` 函数
///
/// 返回一个包含所有当前 `dex-indexer` 支持的DEX协议的 `Vec<Protocol>`。
/// 这个列表用于初始化数据库、加载池数据等。
pub fn supported_protocols() -> Vec<Protocol> {
    vec![
        Protocol::Cetus,
        Protocol::Turbos,
        Protocol::Aftermath,
        Protocol::KriyaAmm,
        Protocol::KriyaClmm,
        Protocol::FlowxClmm,
        Protocol::DeepbookV2,
        Protocol::BlueMove,
    ]
}

/// `DexIndexer` 结构体
///
/// DEX索引器的主要结构体，提供了查询DEX池信息的功能。
/// 它在内部管理数据的获取、存储和缓存。
#[derive(Clone)] // 允许克隆 DexIndexer 实例 (内部成员使用Arc，克隆成本低)
pub struct DexIndexer {
    pool_cache: PoolCache, // 内存中的池数据缓存，用于快速查询

    db: Arc<dyn DB>, // 数据库接口的动态分发对象 (这里是 FileDB)，用于持久化存储
                     // `Arc<dyn DB>` 表示这是一个线程安全共享的、实现了DB trait的对象。

    _live_indexer_tasks: Arc<JoinSet<()>>, // 用于管理后台运行的索引任务 (例如 burberry::Engine 的任务集合)。
                                           // 下划线前缀 `_` 表示这个字段主要用于维持任务的生命周期，
                                           // 可能不直接在其方法中主动使用。当 DexIndexer 被drop时，
                                           // Arc的引用计数减少，如果为0，JoinSet也可能被drop，从而清理后台任务。
}

impl DexIndexer {
    /// `new` 异步构造函数
    ///
    /// 创建并初始化一个新的 `DexIndexer` 实例。
    /// 这个过程包括：
    /// 1. 初始化Sui客户端。
    /// 2. 初始化数据库 (例如 `FileDB`)。
    /// 3. 从数据库加载池数据到内存缓存 (`PoolCache`)。
    /// 4. 创建并运行一个内部的 `burberry::Engine` 来处理池数据的实时更新和回填：
    ///    - 使用 `PoolCreatedStrategy` 来发现和保存新的池数据。
    ///    - `strategy.backfill_pools().await?` 会在启动时进行一次历史数据回填。
    ///    - `QueryEventCollector` 用于定期触发策略执行检查更新。
    ///
    /// 参数:
    /// - `http_url`: Sui RPC节点的URL字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `DexIndexer` 实例。
    pub async fn new(http_url: &str) -> Result<Self> {
        // 初始化Sui客户端
        let sui_client = SuiClientBuilder::default().build(http_url).await?;
        // 初始化数据库 (FileDB)，传入数据存储目录和支持的协议列表
        let database_instance = Arc::new(file_db::FileDB::new(FILE_DB_DIR, &supported_protocols())?);

        let timer = Instant::now(); // 开始计时
        info!("正在从数据库加载DEX池数据到内存缓存...");
        // 从数据库加载所有支持协议的池数据到 PoolCache
        let pool_cache_instance = database_instance.load_token_pools(&supported_protocols())?;
        info!(
            elapsed = ?timer.elapsed(), // 记录加载耗时
            token_pools_count = %pool_cache_instance.token_pools.len(), // 按单个代币索引的池数量
            token01_pools_count = %pool_cache_instance.token01_pools.len(), // 按交易对索引的池数量
            "DEX池数据加载到内存缓存完毕。"
        );

        // 创建池创建/更新策略实例
        let pool_indexing_strategy = PoolCreatedStrategy::new(
            Arc::clone(&database_instance), // 共享数据库访问
            Arc::new(sui_client),          // 共享Sui客户端访问 (修改：Arc::new)
            pool_cache_instance.clone(),   // 共享池缓存访问 (用于策略内部可能需要的快速查找)
        )?;
        // 执行一次历史数据回填，确保数据库中有尽可能多的池数据
        pool_indexing_strategy.backfill_pools().await?;

        // --- 构建并运行内部的 burberry::Engine 用于实时索引 ---
        // 这个引擎负责定期触发 PoolCreatedStrategy 来检查新的池或更新。
        // Event: 定义引擎处理的事件类型 (这里是自定义的 dex_indexer::types::Event)
        // NoAction: 定义引擎策略产生的动作类型 (这里是 NoAction，因为策略直接写DB，不产生外部动作)
        let mut live_indexing_engine = Engine::<Event, NoAction>::new();
        // 添加定时查询触发器作为事件收集器
        let query_trigger_collector = QueryEventCollector::new();
        live_indexing_engine.add_collector(Box::new(query_trigger_collector));
        // 添加池索引策略
        live_indexing_engine.add_strategy(Box::new(pool_indexing_strategy));
        // 添加一个虚拟执行器，因为它不产生需要执行的Action
        live_indexing_engine.add_executor(Box::new(DummyExecutor));

        // 运行引擎，它会返回一个 JoinSet，包含了引擎内部所有任务的句柄。
        let background_tasks_join_set = live_indexing_engine.run().await.expect("Burberry引擎启动失败");

        Ok(Self {
            pool_cache: pool_cache_instance,
            db: database_instance,
            _live_indexer_tasks: Arc::new(background_tasks_join_set), // 存储任务句柄
        })
    }

    /// `get_pools_by_token` 方法
    ///
    /// 根据单个代币类型从内存缓存中获取相关的DEX池列表。
    ///
    /// 参数:
    /// - `token_type`: 代币类型字符串。
    ///
    /// 返回:
    /// - `Option<HashSet<Pool>>`: 如果找到，则返回包含多个 `Pool` 对象的 `HashSet` 的克隆；否则返回 `None`。
    pub fn get_pools_by_token(&self, token_type: &str) -> Option<HashSet<Pool>> {
        // `self.pool_cache.token_pools` 是一个 `DashMap<String, HashSet<Pool>>`
        // `.get()` 返回一个 `Option<dashmap::mapref::one::Ref<'_, String, HashSet<Pool>>>`
        // `.map(|p_set_ref| p_set_ref.clone())` 将引用转换为拥有的 `HashSet<Pool>` 克隆。
        self.pool_cache.token_pools.get(token_type).map(|p_set_ref| p_set_ref.value().clone())
    }

    /// `get_pools_by_token01` 方法
    ///
    /// 根据交易对的两种代币类型从内存缓存中获取相关的DEX池列表。
    ///
    /// 参数:
    /// - `token0_type`: 第一个代币的类型字符串。
    /// - `token1_type`: 第二个代币的类型字符串。
    ///
    /// 返回:
    /// - `Option<HashSet<Pool>>`: 如果找到，则返回池集合的克隆；否则返回 `None`。
    pub fn get_pools_by_token01(&self, token0_type: &str, token1_type: &str) -> Option<HashSet<Pool>> {
        // `token01_key` 生成一个规范化的交易对键 (例如，按字典序排序Token类型)
        let pair_key = token01_key(token0_type, token1_type);
        self.pool_cache.token01_pools.get(&pair_key).map(|p_set_ref| p_set_ref.value().clone())
    }

    /// `get_pool_by_id` 方法
    ///
    /// 根据池的ObjectID从内存缓存中获取特定的DEX池信息。
    ///
    /// 参数:
    /// - `pool_id`: 池的ObjectID。
    ///
    /// 返回:
    /// - `Option<Pool>`: 如果找到，则返回 `Pool` 对象的克隆；否则返回 `None`。
    pub fn get_pool_by_id(&self, pool_id: &ObjectID) -> Option<Pool> {
        self.pool_cache.pool_map.get(pool_id).map(|p_ref| p_ref.value().clone())
    }

    /// `pool_count` 方法
    ///
    /// 获取指定协议在数据库中存储的池的数量。
    ///
    /// 参数:
    /// - `protocol`: DEX协议类型。
    ///
    /// 返回:
    /// - `usize`: 池的数量。如果查询失败或协议不存在，默认为0。
    pub fn pool_count(&self, protocol: &Protocol) -> usize {
        self.db.pool_count(protocol).unwrap_or_default()
    }

    /// `get_all_pools` 方法
    ///
    /// 从数据库中获取指定协议的所有池对象列表。
    ///
    /// 参数:
    /// - `protocol`: DEX协议类型。
    ///
    /// 返回:
    /// - `Result<Vec<Pool>>`: 包含所有池对象的向量。
    pub fn get_all_pools(&self, protocol: &Protocol) -> Result<Vec<Pool>> {
        self.db.get_all_pools(protocol)
    }
}

/// `token01_key` 内联函数
///
/// 为一对代币类型 (`token0_type`, `token1_type`) 生成一个规范化的键 (元组)。
/// 规范化的方式是按字典序对两个代币类型进行排序，确保例如 (SUI, USDC) 和 (USDC, SUI)
/// 生成相同的键，这在用作HashMap的键时非常有用，可以避免重复存储同一交易对的信息。
///
/// 参数:
/// - `token0_type`: 第一个代币的类型字符串。
/// - `token1_type`: 第二个代币的类型字符串。
///
/// 返回:
/// - `(String, String)`: 一个包含两个已排序代币类型字符串的元组。
#[inline] // 建议编译器内联此函数，以提高性能
pub fn token01_key(token0_type: &str, token1_type: &str) -> (String, String) {
    if token0_type < token1_type { // 比较字符串的字典序
        (token0_type.to_string(), token1_type.to_string())
    } else {
        (token1_type.to_string(), token0_type.to_string())
    }
}

/// `normalize_coin_type` 内联函数
///
/// 将Sui原生代币SUI的旧格式或全0格式的类型字符串规范化为官方的 `SUI_COIN_TYPE` 常量。
/// 例如，"0x0::sui::SUI" 或 "0x2::sui::SUI" 都会被处理。
/// Sui的地址（包括包ID）有时可能以短格式 (如 "0x2") 或长格式 (补全0) 出现。
///
/// 参数:
/// - `coin_type`: 要规范化的代币类型字符串。
///
/// 返回:
/// - `String`: 规范化后的代币类型字符串。
#[inline]
pub fn normalize_coin_type(coin_type: &str) -> String {
    // SUI代币的特殊长格式表示 (通常是 `0x2::sui::SUI`)
    if coin_type == "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI" {
        SUI_COIN_TYPE.to_string() // 替换为官方常量字符串 "0x2::sui::SUI"
    } else {
        coin_type.to_string() // 其他类型保持不变
    }
}

/// `DB` Trait (数据库接口)
///
/// 定义了与持久化存储交互的通用接口。
/// `Debug + Send + Sync` 约束：
/// - `Debug`: 可调试打印。
/// - `Send + Sync`: 可在线程间安全传递和共享。
pub trait DB: Debug + Send + Sync {
    /// `flush` 方法
    ///
    /// 将一批新的池数据和对应的事件游标持久化。
    fn flush(&self, protocol: &Protocol, pools: &[Pool], cursor: Option<EventID>) -> Result<()>;

    /// `load_token_pools` 方法
    ///
    /// 从持久化存储中加载指定协议的池数据到内存缓存结构 (`PoolCache`)。
    fn load_token_pools(&self, protocols: &[Protocol]) -> Result<PoolCache>;

    /// `get_processed_cursors` 方法
    ///
    /// 获取所有协议当前已处理事件的游标。
    fn get_processed_cursors(&self) -> Result<HashMap<Protocol, Option<EventID>>>;

    /// `pool_count` 方法
    ///
    /// 获取指定协议的池数量。
    fn pool_count(&self, protocol: &Protocol) -> Result<usize>;

    /// `get_all_pools` 方法
    ///
    /// 获取指定协议的所有池列表。
    fn get_all_pools(&self, protocol: &Protocol) -> Result<Vec<Pool>>;
}


// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use super::*; // 导入外部模块 (dex-indexer::lib.rs) 的所有公共成员

    // 测试用的常量 (注意：这些空字符串可能导致测试在实际运行时失败，除非有默认配置或mock)
    pub const TEST_HTTP_URL: &str = ""; // 测试RPC URL，应指向一个有效的Sui节点
    const TOKEN0_TYPE_FOR_TEST: &str = ""; // 测试用代币类型0，应填写实际代币类型
    const TOKEN1_TYPE_FOR_TEST: &str = ""; // 测试用代币类型1

    /// `test_get_pools` 测试函数
    ///
    /// 测试 `DexIndexer` 的池查询功能。
    #[tokio::test] // 声明为异步测试
    async fn test_get_pools() {
        // 注意: `DexIndexer::new` 内部会执行 `backfill_pools`，这可能需要较长时间或依赖外部API。
        // 在单元测试中，可能需要mock掉 `DexIndexer::new` 的部分依赖，或者使用一个预填充的测试数据库。
        // 如果 TEST_HTTP_URL 为空，`SuiClientBuilder::default().build()` 可能会失败。
        if TEST_HTTP_URL.is_empty() {
            println!("警告: TEST_HTTP_URL为空，test_get_pools 可能无法正确执行。");
            return;
        }
        if TOKEN0_TYPE_FOR_TEST.is_empty() || TOKEN1_TYPE_FOR_TEST.is_empty() {
            println!("警告: 测试代币类型为空，test_get_pools 可能无法找到池。");
            // 即使如此，也继续执行以测试空结果的情况或索引器本身的鲁棒性。
        }


        let indexer = DexIndexer::new(TEST_HTTP_URL).await.unwrap();

        // 测试按单个代币查询池
        if let Some(pools_by_token0) = indexer.get_pools_by_token(TOKEN0_TYPE_FOR_TEST) {
            println!("按 {} 查询到的池数量: {}", TOKEN0_TYPE_FOR_TEST, pools_by_token0.len());
            println!("第一个池 (按token0查询): {:?}", pools_by_token0.iter().next());
        } else {
            println!("按 {} 未查询到池。", TOKEN0_TYPE_FOR_TEST);
        }


        // 测试按交易对查询池
        if let Some(pools_by_pair) = indexer.get_pools_by_token01(TOKEN0_TYPE_FOR_TEST, TOKEN1_TYPE_FOR_TEST) {
            println!("按 {}/{} 查询到的池数量: {}", TOKEN0_TYPE_FOR_TEST, TOKEN1_TYPE_FOR_TEST, pools_by_pair.len());
            println!("第一个池 (按交易对查询): {:?}", pools_by_pair.iter().next());
        } else {
            println!("按 {}/{} 未查询到池。", TOKEN0_TYPE_FOR_TEST, TOKEN1_TYPE_FOR_TEST);
        }
    }

    /// `test_normalize_token_type` 测试函数
    ///
    /// 测试 `normalize_coin_type` 函数是否能正确规范化SUI代币类型。
    #[test]
    fn test_normalize_token_type() {
        // 验证长格式的SUI地址是否能被规范化
        assert_eq!(
            normalize_coin_type("0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI"),
            SUI_COIN_TYPE.to_string(), // SUI_COIN_TYPE 是 "0x2::sui::SUI"
            "长格式SUI类型应被规范化"
        );

        // 验证非SUI代币类型是否保持不变
        let other_coin_type = "0x123::mycoin::MYCOIN";
        assert_eq!(
            normalize_coin_type(other_coin_type),
            other_coin_type.to_string(),
            "其他代币类型应保持不变"
        );

        // 验证官方SUI_COIN_TYPE本身是否保持不变
        assert_eq!(
            normalize_coin_type(SUI_COIN_TYPE),
            SUI_COIN_TYPE.to_string(),
            "官方SUI_COIN_TYPE应保持不变"
        );
    }

    /// `test_pools_count` 测试函数
    ///
    /// 测试 `DexIndexer::pool_count` 方法是否能为每个支持的协议返回池数量。
    #[tokio::test]
    async fn test_pools_count() {
        if TEST_HTTP_URL.is_empty() {
            println!("警告: TEST_HTTP_URL为空，test_pools_count 可能无法正确执行。");
            return;
        }
        let indexer = DexIndexer::new(TEST_HTTP_URL).await.unwrap();

        for protocol in supported_protocols() {
            let count_result = indexer.pool_count(&protocol);
            // 打印每个协议的池数量，或者在出错时打印错误信息
            match count_result {
                Ok(count) => println!("协议 {}: 池数量 {}", protocol, count),
                Err(e) => println!("获取协议 {} 池数量失败: {:?}", protocol, e),
            }
        }
    }
}
