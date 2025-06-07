// 该文件 `mod.rs` 是 `db_simulator` 模块的根文件，定义了 `DBSimulator` 及其相关逻辑。
// `DBSimulator` 是一种高性能的Sui交易模拟器，它通过直接访问Sui节点的数据库快照
// (或其复制品) 来模拟交易，而不是通过RPC与远程节点交互。
// 这使得它能够非常快速地执行大量模拟，对于套利机器人的机会发现和路径评估至关重要。
//
// **文件概览 (File Overview)**:
// 这个文件是 `DBSimulator` 的“大本营”。`DBSimulator` 就像一个Sui网络的“本地克隆体”或“沙盘”，
// 但它不是通过网络连接到真实的Sui节点，而是直接读取Sui节点存储数据的方式（通常是RocksDB数据库文件）
// 来获取链上状态。
//
// **核心组件和概念 (Core Components and Concepts)**:
//
// 1.  **子模块声明 (Submodule Declarations)**:
//     -   `override_cache`: 可能定义了 `OverrideCache`，用于在模拟时临时覆盖某些链上对象的状态，
//         这对于模拟MEV机会（基于特定交易执行后的状态）或测试特定场景非常有用。
//     -   `replay_simulator`: 可能定义了 `ReplaySimulator`，一种特殊的模拟器，用于回放历史交易或事件。
//
// 2.  **`DBSimulator` 结构体**:
//     -   `store: Arc<WritebackCache>`: 这是`DBSimulator`的核心，一个对 `WritebackCache` 的共享引用。
//         `WritebackCache` 是Sui执行层的一部分，它在内存中缓存对象，并负责将修改写回到底层的 `AuthorityStore`。
//         通过直接与这个缓存交互，`DBSimulator` 可以快速读取和（在模拟中）修改对象状态。
//     -   `executor: Arc<dyn Executor + Send + Sync>`: 一个Sui交易执行器实例。这是实际执行Move字节码并应用状态更改的组件。
//     -   `protocol_config: ProtocolConfig`: 当前Sui网络的协议配置，包含了各种限制和参数（如max_gas_amount, feature flags等）。
//     -   `metrics: Arc<LimitsMetrics>` 和 `writeback_metrics: Arc<ExecutionCacheMetrics>`: 用于收集模拟过程中的性能指标。
//     -   `with_fallback: bool`: 一个布尔标志。如果为 `true`，`OverrideCache` 在本地缓存未命中时，可能会尝试从其包装的 `WritebackCache` (即真实数据库) 读取。
//         如果为 `false`，则严格只使用 `OverrideCache` 中明确提供的对象。
//
// 3.  **构造函数和初始化 (`new_authority_store`, `new_slow`, `new_default_slow`, `new_test`, `new`)**:
//     -   这些函数提供了多种创建 `DBSimulator` 实例的方式。
//     -   `new_authority_store()`: 核心的初始化步骤，加载Sui节点的 `AuthorityStore` (持久化存储)。
//         需要Sui数据库的路径 (`store_path`) 和节点配置文件路径 (`config_path`)。
//     -   `new_slow()`, `new_default_slow()`: 更高级别的构造函数，封装了 `new_authority_store` 的调用，
//         并增加了对象预加载 (`preload_path`) 和通过Unix套接字进行缓存更新 (`update_socket`) 的功能。
//         “slow”可能指初始化过程较慢，因为它需要加载完整的 `AuthorityStore` 和预加载对象。
//     -   `new_test()`: 一个简化的构造函数，主要用于测试，可以配置 `fallback` 行为。
//     -   `new()`: 最底层的构造函数，接收一个已初始化的 `AuthorityStore` 和其他配置。
//         -   **对象预加载**: 如果提供了 `preload_path` (一个包含对象ID列表的文件路径)，
//             它会读取这些ID并在初始化时将它们对应的对象从数据库加载到 `WritebackCache` 中。
//             这对于加速后续对这些常用对象的访问非常重要。
//         -   **缓存更新线程 (`spawn_update_thread`)**: 如果提供了 `update_socket`，它会启动一个后台线程。
//             这个线程会监听指定的Unix套接字。当外部进程（通常是实际的Sui节点或一个数据同步服务）
//             通过这个套接字发送新的对象数据时，该线程会接收这些数据并更新 `WritebackCache`。
//             这使得 `DBSimulator` 可以在一定程度上保持其本地状态与链上状态的同步，即使它是基于数据库快照的。
//             它还会定期 (例如每天) 调用 `cache_writer.update_underlying(true)` 来将缓存中的写操作持久化到底层数据库，
//             并重新加载预设对象，以防底层数据库被外部更新。
//         -   **协议配置**: 初始化并可能修改 `ProtocolConfig`，例如增加对象缓存相关的限制。
//
// 4.  **对象获取逻辑 (`get_input_objects`, `get_mutated_objects`)**:
//     -   `get_input_objects()`: 根据交易数据 (`TransactionData`) 中声明的输入对象种类 (`InputObjectKind`)，
//         从 `WritebackCache` (或通过fallback机制从底层存储) 中获取这些对象的具体数据 (`ObjectReadResult`)。
//         它需要处理不同类型的输入对象，如Move包、共享对象、不可变或私有可变对象。
//     -   `get_mutated_objects()`: 在交易模拟执行后，从 `InnerTemporaryStore` (包含了交易执行期间所有被修改或新创建的对象)
//         中提取出那些被修改的对象 (`mutated_excluding_gas`)，并将它们转换为 `ObjectReadResult` 列表。
//
// 5.  **`Simulator` trait 实现**:
//     -   **`simulate()`**: 这是核心的模拟方法。
//         -   **准备输入**:
//             -   获取当前epoch信息和覆盖对象列表 (`override_objects`) 从 `SimulateCtx`。
//             -   调用 `get_input_objects()` 获取交易需要的输入对象。
//             -   **模拟Gas币**: 如果交易数据中没有提供Gas币 (`original_gas.is_empty()`)，它会创建一个“模拟的”SUI Gas币对象 (`mock_gas_id`, `gas_obj`)，
//                 并将其添加到输入对象列表和覆盖对象列表中。这使得模拟交易时无需拥有真实的Gas币对象。
//             -   **模拟借入代币**: 如果 `SimulateCtx` 中提供了 `borrowed_coin` (例如，在模拟闪电贷路径的第一步)，
//                 也会将这个模拟的借入代币对象添加到输入和覆盖列表中。
//             -   创建 `OverrideCache`: 将 `override_objects` (包括模拟的Gas币和借入代币) 包装到 `OverrideCache` 中。
//                 `OverrideCache` 会优先从这些覆盖对象中读取数据；如果找不到且 `with_fallback` 为true，则会尝试从底层的 `WritebackCache` 读取。
//             -   再次更新 `input_objects`，确保它们反映了 `OverrideCache` 中的最新状态。
//         -   **执行交易**:
//             -   调用 `self.executor.execute_transaction_to_effects()` 来实际执行交易逻辑。
//                 这个调用是在 `catch_unwind` 中进行的，以捕获Move执行引擎可能发生的panic。
//                 它传入了 `OverrideCache` (作为对象存储)、协议配置、epoch信息、输入对象、Gas数据等。
//         -   **处理结果**:
//             -   从执行结果中获取 `InnerTemporaryStore` (包含写入的对象) 和 `TransactionEffects` (交易效果)。
//             -   调用 `get_mutated_objects()` 获取被修改的对象列表。
//             -   使用 `ExecutedDB` (一个临时的、结合了 `OverrideCache` 和 `InnerTemporaryStore` 的 `ObjectProvider` 实现)
//                 来计算余额变更 (`BalanceChange`) 和格式化事件 (`SuiTransactionBlockEvents`)。
//             -   **余额调整**: 特别处理模拟Gas币和模拟借入代币的余额计算，以确保最终的余额变更能正确反映交易的净效果。
//             -   计算缓存未命中次数。
//             -   返回 `SimulateResult`，其中包含了交易效果、事件、对象变更、余额变更和缓存未命中次数。
//     -   `name()`: 返回 "DBSimulator"。
//     -   `get_object()`: 从 `WritebackCache` 获取对象。
//     -   `get_object_layout()`: 获取对象的Move布局信息。
//
// 6.  **`ExecutedDB` 结构体**:
//     -   一个临时的结构体，实现了 `ObjectProvider` trait。它在模拟后用于从 `OverrideCache` (代表模拟前的状态和覆盖)
//         和 `InnerTemporaryStore` (代表模拟中发生的变化) 中查找对象版本，以便准确计算余额变更和事件。
//
// 7.  **`CacheWriter` trait 和 `preload_objects`, `spawn_update_thread` 函数**:
//     -   这些与通过Unix套接字接收外部对象更新并更新 `WritebackCache` 的机制相关。
//
// **DBSimulator的优势**:
// -   **高性能**: 通过直接访问数据库状态（或其内存缓存）并本地执行交易，避免了RPC调用的网络延迟和开销。
// -   **隔离性**: 可以在不影响真实网络的情况下进行大量测试和模拟。
// -   **状态控制**: 可以通过 `override_objects` 精确控制模拟环境中的对象状态。
//
// **DBSimulator的挑战**:
// -   **状态同步**: 保持本地数据库快照与实时链上状态的同步是一个挑战。`spawn_update_thread` 是一种尝试解决此问题的方法。
// -   **资源需求**: 可能需要较大的磁盘空间来存储Sui节点数据库的副本。
// -   **复杂性**: 初始化和配置相对复杂。

// 声明子模块。
mod override_cache; // 定义了 OverrideCache，用于在模拟时覆盖特定对象的状态。
mod replay_simulator; // 定义了 ReplaySimulator，一种可能用于回放交易的模拟器。

// 从子模块中重新导出 ReplaySimulator，使其可以通过 db_simulator::ReplaySimulator 访问。
pub use replay_simulator::ReplaySimulator;

// 引入标准库的相关模块。
use std::{
    collections::HashSet,                           // HashSet 用于存储唯一的对象ID。
    panic::{catch_unwind, AssertUnwindSafe},       // catch_unwind 用于捕获执行过程中的panic，AssertUnwindSafe 用于在 FFI 或线程边界安全地处理panic。
    path::{Path, PathBuf},                         // Path 和 PathBuf 用于处理文件系统路径。
    str::FromStr,                                 // FromStr trait 用于从字符串转换类型。
    sync::Arc,                                    // Arc (Atomic Reference Counting) 用于线程安全地共享所有权。
};

// 引入异步编程相关的trait。
use async_trait::async_trait;
// 引入 eyre 库，用于更方便的错误处理。
use eyre::Result;
// 引入 Move 核心类型，用于处理Move字节码和对象布局。
use move_core_types::annotated_value::{MoveDatatypeLayout, MoveStructLayout};
// 引入 prometheus 库，用于度量指标收集 (这里是 Registry)。
use prometheus::Registry;
// 引入 Sui 配置相关的类型。
use sui_config::{NodeConfig, PersistedConfig};
// 引入 Sui 核心逻辑相关的类型。
use sui_core::{
    authority::{authority_store_tables::AuthorityPerpetualTables, backpressure::BackpressureManager, AuthorityStore}, // AuthorityStore 是Sui节点的核心存储组件。
    execution_cache::{metrics::ExecutionCacheMetrics, ExecutionCacheWrite, ObjectCacheRead, WritebackCache}, // 执行缓存相关的读写接口和指标。WritebackCache 是内存缓存层。
};
// 引入 Sui 执行引擎。
use sui_execution::Executor;
// 引入 Sui 索引器错误类型。
use sui_indexer::errors::IndexerError;
// 引入 Sui JSON RPC 相关的辅助函数和类型。
use sui_json_rpc::{get_balance_changes_from_effect, ObjectProvider}; // get_balance_changes_from_effect 用于从交易效果计算余额变化。ObjectProvider 是一个提供对象读取的trait。
use sui_json_rpc_types::{BalanceChange, SuiTransactionBlockEffects, SuiTransactionBlockEvents}; // RPC返回的交易效果、事件和余额变化类型。
// 从 Sui SDK 引入 SUI_COIN_TYPE 常量。
use sui_sdk::SUI_COIN_TYPE;
// 引入 Sui 核心类型。
use sui_types::{
    base_types::{ObjectID, SequenceNumber}, // 对象ID, 对象版本号。
    committee::{EpochId, ProtocolVersion},  // Epoch ID, 协议版本。
    digests::TransactionDigest,             // 交易摘要 (哈希)。
    effects::TransactionEffects,            // 交易执行效果。
    error::SuiError,                        // Sui错误类型。
    gas::SuiGasStatus,                      // Gas状态机。
    inner_temporary_store::InnerTemporaryStore, // 交易执行期间的临时对象存储。
    metrics::LimitsMetrics,                 // 限制相关的度量指标。
    object::{MoveObject, Object, Owner, OBJECT_START_VERSION}, // 对象、Move对象、所有者、对象起始版本。
    storage::{BackingPackageStore, ObjectKey, ObjectStore}, // 存储相关的trait和类型。
    supported_protocol_versions::{Chain, ProtocolConfig}, // 支持的协议版本和配置。
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult, ObjectReadResultKind, TransactionData, // 交易输入对象相关的类型。
        TransactionDataAPI, // TransactionData 的API trait。
    },
    TypeTag, // Move类型标签。
};
// 引入 Tokio 的异步IO和Unix套接字。
use tokio::{io::AsyncReadExt, net::UnixStream};
// 引入 tracing 日志库。
use tracing::{debug, error, info};

// 从当前crate的父模块 (simulator) 引入 SimulateCtx, SimulateResult, Simulator trait。
use super::{SimulateCtx, SimulateResult, Simulator};
// 从本地子模块 override_cache 引入 OverrideCache。
use override_cache::OverrideCache;

/// `DBSimulator` 结构体
///
/// 一个高性能的Sui交易模拟器，通过直接访问Sui节点的数据库状态进行模拟。
pub struct DBSimulator {
    /// `store`: 一个对 `WritebackCache` 的共享引用。
    /// `WritebackCache` 是一个内存中的对象缓存，它位于模拟器的执行引擎和底层的 `AuthorityStore` (持久化存储) 之间。
    /// 模拟过程中的对象读取和写入首先通过这个缓存进行。
    pub store: Arc<WritebackCache>,
    /// `executor`: 一个实现了Sui执行逻辑的 `Executor` trait对象的共享引用。
    /// 这是实际执行Move字节码并应用状态更改的核心组件。
    executor: Arc<dyn Executor + Send + Sync>,
    /// `protocol_config`: 当前Sui网络的协议配置。
    /// 包含了各种运行时参数和特性开关，模拟执行时需要遵循这些配置。
    protocol_config: ProtocolConfig,
    /// `metrics`: 用于收集与执行限制相关的度量指标。
    metrics: Arc<LimitsMetrics>,
    /// `writeback_metrics`: 用于收集 `WritebackCache` 性能相关的度量指标。
    writeback_metrics: Arc<ExecutionCacheMetrics>,
    /// `with_fallback`: 一个布尔标志。
    /// 如果为 `true`，当 `OverrideCache` (在模拟过程中使用) 查找对象未命中时，
    /// 会尝试从其包装的 `WritebackCache` (即 `self.store`，代表更广泛的数据库状态) 中读取。
    /// 如果为 `false`，则 `OverrideCache` 只会使用明确提供给它的覆盖对象。
    with_fallback: bool,
}

impl DBSimulator {
    /// `new_authority_store` 异步关联函数
    ///
    /// 根据给定的Sui节点数据库路径 (`store_path`) 和节点配置文件路径 (`config_path`)，
    /// 打开并返回一个 `AuthorityStore` 的共享引用。
    /// `AuthorityStore` 是Sui验证者节点中负责持久化存储所有链上状态（对象、交易、事件等）的核心组件。
    ///
    /// 返回: `Arc<AuthorityStore>`
    pub async fn new_authority_store(store_path: &str, config_path: &str) -> Arc<AuthorityStore> {
        // 从指定的路径加载节点配置 (例如 fullnode.yaml)。
        let config: NodeConfig = PersistedConfig::read(&PathBuf::from(config_path))
            .map_err(|err| err.context(format!("无法打开Sui节点配置文件: {:?}", config_path)))
            .unwrap(); // 如果配置文件读取失败，则panic。

        // 获取创世配置信息。
        let genesis = config.genesis().unwrap().clone();

        // 打开Sui节点的持久化存储表 (RocksDB)。
        // `open_readonly_as_rw` 可能表示以只读模式打开数据库文件，但在内存中允许读写操作的包装。
        // 这对于模拟器来说是合适的，因为它不应该修改原始的节点数据库文件。
        let perpetual_tables = Arc::new(AuthorityPerpetualTables::open_readonly_as_rw(Path::new(store_path)));

        // 使用加载的配置和存储表来打开 `AuthorityStore`。
        // `Registry::new()` 用于Prometheus度量指标。
        AuthorityStore::open(perpetual_tables, &genesis, &config, &Registry::new())
            .await
            .unwrap() // 如果打开失败，则panic。
    }

    /// `new_slow` 异步关联函数
    ///
    /// 创建 `DBSimulator` 的一个“慢速初始化”版本。
    /// "慢速"指的是初始化过程可能较慢，因为它需要加载完整的 `AuthorityStore`，
    /// 并可能预加载大量对象到缓存中。
    ///
    /// 参数:
    /// - `store_path`: Sui节点数据库的路径。
    /// - `config_path`: Sui节点配置文件的路径。
    /// - `update_socket`: (可选) 用于接收外部缓存更新的Unix套接字路径。
    /// - `preload_path`: (可选) 包含对象ID列表的文件路径，这些对象将在初始化时被预加载到缓存。
    pub async fn new_slow(
        store_path: &str,
        config_path: &str,
        update_socket: Option<&str>, // 路径字符串是 &str
        preload_path: Option<&str>,  // 路径字符串是 &str
    ) -> Self {
        // 首先，异步初始化 AuthorityStore。
        let authority_store = Self::new_authority_store(store_path, config_path).await;
        // 然后调用更底层的 `new` 构造函数。
        // `map(PathBuf::from)` 将 Option<&str> 转换为 Option<PathBuf>。
        Self::new(
            authority_store,
            update_socket.map(PathBuf::from),
            preload_path.map(PathBuf::from),
            true, // `with_fallback` 默认为 true
        )
        .await
    }

    /// `new_default_slow` 异步关联函数
    ///
    /// 使用硬编码的默认路径调用 `new_slow`，方便快速创建一个功能完整的 `DBSimulator`。
    /// 这些默认路径通常指向本地开发环境中的Sui节点数据。
    pub async fn new_default_slow() -> Self {
        Self::new_slow(
            "/home/ubuntu/sui/db/live/store", // 默认Sui数据库路径
            "/home/ubuntu/sui/fullnode.yaml",  // 默认Sui节点配置文件路径
            None,                              // 默认不使用缓存更新套接字
            Some("/home/ubuntu/suiflow-relay/pool_related_ids.txt"), // 默认预加载对象列表文件路径
        )
        .await
    }

    /// `new_test` 异步关联函数
    ///
    /// 创建一个主要用于测试的 `DBSimulator` 实例。
    /// 它使用默认的数据库和配置路径，并允许指定 `with_fallback` 行为。
    ///
    /// 参数:
    /// - `fallback`: 是否启用 `OverrideCache` 的回退机制。
    pub async fn new_test(fallback: bool) -> Self {
        // 初始化 AuthorityStore (使用默认路径)
        let authority_store =
            Self::new_authority_store("/home/ubuntu/sui/db/live/store", "/home/ubuntu/sui/fullnode.yaml").await;
        // 调用底层 `new` 构造函数，不指定 update_socket 和 preload_path。
        Self::new(authority_store, None, None, fallback).await
    }

    /// `new` 异步关联函数 (核心构造逻辑)
    ///
    /// `DBSimulator` 的核心构造函数。
    ///
    /// 参数:
    /// - `authority_store`: 一个已初始化的 `AuthorityStore` 的共享引用。
    /// - `update_socket`: (可选) 用于接收外部缓存更新的Unix套接字路径 (`PathBuf`)。
    /// - `preload_path`: (可选) 包含对象ID列表的文件路径 (`PathBuf`)，用于预加载。
    /// - `with_fallback`: 是否启用 `OverrideCache` 的回退机制。
    pub async fn new(
        authority_store: Arc<AuthorityStore>,
        update_socket: Option<PathBuf>,
        preload_path: Option<PathBuf>,
        with_fallback: bool,
    ) -> Self {
        // 初始化执行缓存相关的度量指标。
        let writeback_metrics_instance = Arc::new(ExecutionCacheMetrics::new(&Registry::new()));
        // 初始化一个用于测试的反压管理器 (BackpressureManager)。
        let backpressure_manager_for_tests = BackpressureManager::new_for_tests();

        // 创建 `WritebackCache` 实例。
        // `WritebackCache` 在 `authority_store` 之上提供了一个内存缓存层。
        let writeback_cache_instance = Arc::new(WritebackCache::new(
            &Default::default(), // 可能是某种缓存配置，这里使用默认值
            authority_store.clone(), // 底层存储
            writeback_metrics_instance.clone(), // 度量指标
            backpressure_manager_for_tests, // 反压管理器
        ));

        // 处理对象预加载
        let object_ids_to_preload = if let Some(path_to_preload_file) = preload_path {
            // 如果提供了预加载文件路径，则读取文件内容。
            let object_ids_str_content = std::fs::read_to_string(path_to_preload_file).unwrap(); // 假设文件读取成功
            // 将文件内容按行分割，每行解析为一个ObjectID，并收集到HashSet中以去重。
            object_ids_str_content
                .trim()
                .split("\n")
                .map(|s| ObjectID::from_str(s).unwrap()) // 解析ObjectID，假设格式正确
                .collect::<HashSet<_>>()
        } else {
            HashSet::new() // 如果没有提供预加载文件，则预加载列表为空
        };
        // 将HashSet转换为Vec，因为 `multi_get_objects` 需要Vec。
        let preload_ids_vec = object_ids_to_preload.iter().cloned().collect::<Vec<_>>();

        // 调用 `WritebackCache` 的 `multi_get_objects` 方法来预加载这些对象到缓存中。
        // `_` 忽略返回值，因为这里主要关注预加载的副作用。
        let _ = writeback_cache_instance.multi_get_objects(&preload_ids_vec);
        info!("DBSimulator: 已预加载 {} 个对象到WritebackCache。", preload_ids_vec.len());

        // 如果配置了缓存更新套接字，则启动一个后台线程来监听和处理更新。
        if let Some(socket_path_for_update) = update_socket {
            info!("DBSimulator: 正在为路径 {:?} 启动缓存更新线程...", socket_path_for_update);
            let execution_cache_writer_clone = Arc::clone(&writeback_cache_instance); // 克隆Arc以传递给新线程
            let preload_ids_for_thread = preload_ids_vec.clone(); // 克隆预加载ID列表
            std::thread::Builder::new() // 创建一个标准库线程
                .name("db-update-thread".to_string()) // 为线程命名，方便调试
                .spawn(move || spawn_update_thread(socket_path_for_update, preload_ids_for_thread, execution_cache_writer_clone)) // 启动线程执行 spawn_update_thread 函数
                .unwrap(); // 假设线程创建成功
            info!("DBSimulator: 缓存更新线程已启动。");
        }

        // 获取并可能修改当前Sui网络的协议配置。
        // `ProtocolVersion::MAX` 表示使用当前支持的最新协议版本。
        // `Chain::Mainnet` 通常意味着使用主网的配置参数，但这可能只是一个默认基准。
        let mut current_protocol_config = ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Mainnet);

        // 增加对象运行时缓存相关的限制。这可以允许模拟器在内存中缓存更多对象，
        // 从而在模拟大量交易或访问大量对象时提高性能。
        current_protocol_config.object_runtime_max_num_cached_objects = Some(1000000); // 普通交易缓存对象数上限
        current_protocol_config.object_runtime_max_num_cached_objects_system_tx = Some(1000000); // 系统交易缓存对象数上限
        current_protocol_config.object_runtime_max_num_store_entries = Some(1000000);      // 存储条目上限
        current_protocol_config.object_runtime_max_num_store_entries_system_tx = Some(1000000); // 系统交易存储条目上限
        info!("DBSimulator: 协议配置已更新，对象缓存限制已调整。");

        // 创建Sui执行引擎实例。
        // `sui_execution::executor` 是一个工厂函数，用于创建实现了 `Executor` trait 的对象。
        // - `&current_protocol_config`: 传入协议配置。
        // - `true`: 可能表示启用某些特性或优化。
        // - `None`: 可能是一些可选的特性或回调，这里不使用。
        let sui_executor_instance =
            sui_execution::executor(&current_protocol_config, true, None).expect("创建Sui执行引擎失败");
        info!("DBSimulator: Sui执行引擎已创建。");

        // 返回DBSimulator实例
        Self {
            store: writeback_cache_instance, // 共享的WritebackCache
            executor: sui_executor_instance,   // 共享的Sui执行引擎
            protocol_config: current_protocol_config, // 协议配置
            metrics: Arc::new(LimitsMetrics::new(&Registry::new())), // 限制相关的度量指标
            writeback_metrics: writeback_metrics_instance, // WritebackCache的度量指标
            with_fallback, // 是否启用OverrideCache的回退
        }
    }

    /// `get_input_objects` 方法
    ///
    // 从 `self.store` (WritebackCache) 中获取交易所需的所有输入对象。
    ///
    /// 参数:
    /// - `input_object_kinds`: 一个 `InputObjectKind` 的切片，描述了交易需要哪些输入对象及其种类。
    /// - `epoch_id`: 当前的Epoch ID，用于获取共享对象的特定版本信息。
    ///
    /// 返回:
    /// - `Result<InputObjects, SuiError>`: 包含所有获取到的输入对象数据 (`ObjectReadResult`) 的 `InputObjects` 结构。
    ///   如果任何必需的对象未找到，则返回 `SuiError`。
    pub fn get_input_objects(
        &self,
        input_object_kinds: &[InputObjectKind],
        epoch_id: EpochId,
    ) -> Result<InputObjects, SuiError> {
        // 初始化结果向量，长度与输入种类相同，初始值为None。
        let mut results_vec: Vec<Option<ObjectReadResult>> = vec![None; input_object_kinds.len()];
        // 用于存储需要批量获取的普通对象引用 (ObjectRef)
        let mut object_refs_to_fetch = Vec::with_capacity(input_object_kinds.len());
        // 存储这些普通对象引用在 `input_object_kinds` 中的原始索引，以便后续将获取结果放回正确位置。
        let mut indices_to_fill_later = Vec::with_capacity(input_object_kinds.len());

        // 遍历所有声明的输入对象种类
        for (i, kind) in input_object_kinds.iter().enumerate() {
            match kind {
                // 对于Move包 (MovePackage)，单独通过缓存获取。
                InputObjectKind::MovePackage(id) => {
                    // `self.store.get_package_object(id)` 尝试从WritebackCache获取包对象。
                    // `ok_or_else` 如果返回None (未找到)，则构造一个错误。
                    let package_object = self.store.get_package_object(id)?
                        .ok_or_else(|| SuiError::from(kind.object_not_found_error()))? // 使用kind内置的错误构造器
                        .into(); // 将 Package 转换为 Object
                    results_vec[i] = Some(ObjectReadResult {
                        input_object_kind: *kind, // 输入种类
                        object: ObjectReadResultKind::Object(package_object), // 对象数据
                    });
                }
                // 对于共享对象 (SharedMoveObject)
                InputObjectKind::SharedMoveObject { id, .. } => {
                    match self.store.get_object(id) { // 尝试从WritebackCache获取
                        Some(object_data) => { // 如果找到
                            results_vec[i] = Some(ObjectReadResult::new(*kind, object_data.into()));
                        }
                        None => { // 如果在主缓存中未找到
                            // 检查该共享对象是否在当前epoch已被删除。
                            // `get_last_shared_object_deletion_info` 返回删除时的版本和摘要 (如果已删除)。
                            if let Some((version, digest)) = self.store.get_last_shared_object_deletion_info(id, epoch_id) {
                                // 如果已删除，则标记为 DeletedSharedObject。
                                results_vec[i] = Some(ObjectReadResult {
                                    input_object_kind: *kind,
                                    object: ObjectReadResultKind::DeletedSharedObject(version, digest),
                                });
                            } else {
                                // 如果既不在缓存中，也没有删除记录，则视为未找到。
                                return Err(SuiError::from(kind.object_not_found_error()));
                            }
                        }
                    }
                }
                // 对于不可变或私有可变对象 (ImmOrOwnedMoveObject)
                InputObjectKind::ImmOrOwnedMoveObject(obj_ref) => {
                    // 将其ObjectRef和原始索引添加到待批量获取的列表中。
                    object_refs_to_fetch.push(*obj_ref);
                    indices_to_fill_later.push(i);
                }
            }
        }

        // 批量获取所有ImmOrOwnedMoveObject。
        // `multi_get_objects_by_key` 接收一个ObjectKey列表，返回一个Option<Object>列表。
        let fetched_objects = self
            .store
            .multi_get_objects_by_key(&object_refs_to_fetch.iter().map(ObjectKey::from).collect::<Vec<_>>());
        // 断言返回的对象数量与请求的数量一致。
        assert_eq!(fetched_objects.len(), object_refs_to_fetch.len());

        // 将批量获取的结果填充回 `results_vec` 的正确位置。
        for (original_index, fetched_object_option) in indices_to_fill_later.into_iter().zip(fetched_objects.into_iter()) {
            // **注意**: 这里的逻辑有一个潜在问题。如果 `fetched_object_option` 是 `None` (表示对象未找到)，
            // 那么 `results_vec[original_index]` 会保持为 `None`。
            // 这可能导致后续 `input_results.into_iter().flatten()` 丢失这个条目，
            // 从而使得最终的 `InputObjects` 长度小于 `input_object_kinds.len()`，
            // 这在Sui的交易执行逻辑中通常是不允许的，除非这个对象是可选的或模拟的。
            // 对于非模拟的、必需的输入对象，如果 `multi_get_objects_by_key` 返回 `None`，应该视为错误。
            // 当前代码似乎忽略了这种情况，依赖于后续的 `flatten()`。
            // 如果 `input_object_kinds` 中某个对象是必需的但未找到，执行引擎应该会报错。
            // "ignore mock objects" 的注释可能暗示，如果 `fetched_object_option` 是 `None`，
            // 可能是因为这个对象是模拟的 (例如 `borrowed_coin` 或 `mock_gas`)，并且已在 `override_cache` 中提供，
            // 所以这里从 `self.store` (WritebackCache) 中找不到是正常的。
            // 最终的 `input_objects` 会在 `simulate` 方法中通过 `OverrideCache` 再次更新。
            if let Some(object_data) = fetched_object_option { // 只处理成功获取到的对象
                results_vec[original_index] = Some(ObjectReadResult {
                    input_object_kind: input_object_kinds[original_index],
                    object: ObjectReadResultKind::Object(object_data),
                });
            }
        }
        // `flatten()` 会移除所有 `None` 值，然后收集到 `Vec<ObjectReadResult>`。
        // `into()` 将 `Vec<ObjectReadResult>` 转换为 `InputObjects` 类型。
        Ok(results_vec.into_iter().flatten().collect::<Vec<_>>().into())
    }

    /// `get_mutated_objects` 方法
    ///
    /// 从交易执行后的 `InnerTemporaryStore` (临时存储) 中提取所有被修改的对象 (不包括Gas币)。
    ///
    /// 参数:
    /// - `effects`: 交易的执行效果 (`&TransactionEffects`)，包含了哪些对象被修改。
    /// - `store`: 交易执行期间的临时对象存储 (`&InnerTemporaryStore`)，包含了修改后对象的实际数据。
    ///
    /// 返回:
    /// - `eyre::Result<Vec<ObjectReadResult>>`: 包含所有被修改对象的 `ObjectReadResult` 列表。
    fn get_mutated_objects(
        &self,
        effects: &TransactionEffects,
        store: &InnerTemporaryStore,
    ) -> eyre::Result<Vec<ObjectReadResult>> {
        let mut object_changes_vec = vec![];
        // `effects.mutated_excluding_gas()` 返回一个迭代器，包含所有被修改的 (ObjectRef, Owner) 对，但不包括Gas支付对象。
        for (obj_ref_mutated, owner_after_mutation) in effects.mutated_excluding_gas() {
            // `store.written` 是一个 `HashMap<ObjectID, Object>`，存储了交易执行期间所有被写入（创建或修改）的对象。
            // 从临时存储中获取被修改对象的最新数据。
            if let Some(updated_object_data) = store.written.get(&obj_ref_mutated.0) { // obj_ref_mutated.0 是 ObjectID
                let object_read_result_kind = ObjectReadResultKind::Object(updated_object_data.clone()); // 克隆对象数据

                // 根据修改后的所有者类型，确定其 `InputObjectKind`。
                // 这对于后续如果需要将这些修改后的对象作为另一笔交易的输入时很重要。
                let input_object_kind_after_mutation = match owner_after_mutation {
                    Owner::Shared { initial_shared_version } => InputObjectKind::SharedMoveObject {
                        id: obj_ref_mutated.0, // ObjectID
                        initial_shared_version: *initial_shared_version, // 共享对象的初始版本
                        mutable: true, // 假设它仍然是可变的
                    },
                    _ => InputObjectKind::ImmOrOwnedMoveObject(*obj_ref_mutated), // 对于其他所有者类型 (Address, Immutable, Object)，使用其ObjectRef
                };
                object_changes_vec.push(ObjectReadResult::new(input_object_kind_after_mutation, object_read_result_kind));
            }
            // 如果在 `store.written` 中找不到，可能意味着该对象仅被修改元数据但内容未变，
            // 或者是一个更复杂的情况。当前实现只处理在 `written` 中的对象。
        }
        Ok(object_changes_vec)
    }
}

/// 为 `DBSimulator` 实现 `Simulator` trait。
#[async_trait]
impl Simulator for DBSimulator {
    /// `simulate` 异步方法 (核心模拟逻辑)
    ///
    /// 执行对给定交易数据 (`tx_data`) 的模拟。
    ///
    /// 参数:
    /// - `tx_data_to_simulate`: 要模拟的 `TransactionData`。
    /// - `simulation_context`: `SimulateCtx`，包含了epoch信息、需要覆盖的对象状态列表、以及可选的模拟借入代币。
    ///
    /// 返回:
    /// - `eyre::Result<SimulateResult>`: 包含模拟结果 (`SimulateResult`) 或错误。
    async fn simulate(&self, tx_data_to_simulate: TransactionData, simulation_context: SimulateCtx) -> eyre::Result<SimulateResult> {
        // 记录模拟开始前的缓存未命中次数，用于计算本次模拟引起的额外未命中。
        let cache_misses_count_before_sim = self.writeback_metrics.cache_misses_count();

        // 解构模拟上下文
        let SimulateCtx {
            epoch: current_epoch_info,         // 当前纪元信息
            mut override_objects_list,         // 需要在模拟中覆盖其状态的对象列表 (可变)
            borrowed_coin_info,                // (可选) 模拟的借入代币及其数量
        } = simulation_context;

        // 步骤 1: 获取交易所需的输入对象。
        // `tx_data_to_simulate.input_objects()?` 解析交易数据以获取声明的输入对象种类。
        let mut current_input_objects = self.get_input_objects(&tx_data_to_simulate.input_objects()?, current_epoch_info.epoch_id)?;

        // 步骤 2: 处理模拟Gas币。
        let sender_address = tx_data_to_simulate.sender(); // 获取交易发送者地址
        let original_gas_payment_objects = tx_data_to_simulate.gas().to_vec(); // 获取交易中指定的Gas支付对象

        // 用于模拟的Gas币的固定ObjectID (一个不太可能与真实对象冲突的ID)。
        let mock_gas_object_id =
            ObjectID::from_str("0x0000000000000000000000000000000000000000000000000000000000001337").unwrap();
        // 判断是否需要使用模拟Gas币 (如果原始交易没有指定Gas支付对象)。
        let use_mocked_gas_coin = original_gas_payment_objects.is_empty();
        let (gas_object_refs_for_execution, optional_mock_gas_object) = if use_mocked_gas_coin {
            // 如果使用模拟Gas币：
            info!("DBSimulator: 正在为模拟创建模拟Gas币 (ObjectID: {})", mock_gas_object_id);
            const MIST_PER_SUI: u64 = 1_000_000_000; // 1 SUI = 10^9 MIST
            const DRY_RUN_SUI_BUDGET: u64 = 1_000_000_000; // 模拟时使用的Gas币面额 (10^9 SUI, 非常大)

            let max_coin_value_for_mock = MIST_PER_SUI * DRY_RUN_SUI_BUDGET; // 计算模拟Gas币的总MIST数量
            // 创建一个MoveObject代表这个模拟Gas币。
            let gas_move_object = MoveObject::new_gas_coin(OBJECT_START_VERSION, mock_gas_object_id, max_coin_value_for_mock);
            // 创建一个Object包装这个MoveObject。
            let gas_sui_object = Object::new_move(
                gas_move_object,
                Owner::AddressOwner(sender_address), // 所有者是交易发送者
                TransactionDigest::genesis_marker(), // previous_transaction 使用创世摘要
            );
            let gas_sui_object_ref = gas_sui_object.compute_object_reference(); // 获取其ObjectRef
            // 执行时将使用这个模拟Gas币的引用，并同时返回这个模拟对象本身以便加入覆盖列表。
            (vec![gas_sui_object_ref], Some(gas_sui_object))
        } else {
            // 如果不使用模拟Gas币，则直接使用交易中指定的Gas支付对象，不创建模拟对象。
            (original_gas_payment_objects, None)
        };

        // 步骤 3: 创建Gas状态机。
        // `SuiGasStatus::new` 根据Gas预算、Gas价格和协议配置来初始化。
        let gas_status_machine = match SuiGasStatus::new(tx_data_to_simulate.gas_budget(), tx_data_to_simulate.gas_price(), tx_data_to_simulate.gas_price(), &self.protocol_config)
            .map_err(|e| eyre::eyre!(e)) // 将SuiError转换为eyre::Error
        {
            Ok(status) => status,
            Err(e) => { // 如果Gas参数无效 (例如预算超过最大值)
                info!("DBSimulator: Gas状态创建失败: {:?}", e);
                return Err(e); // 直接返回错误
            }
        };

        // 步骤 4: 将模拟的Gas币和借入代币（如果存在）添加到输入对象列表和覆盖对象列表中。
        if use_mocked_gas_coin {
            let mock_gas_sui_object = optional_mock_gas_object.unwrap(); // 此时一定存在
            let mock_gas_object_read_result = ObjectReadResult {
                input_object_kind: InputObjectKind::ImmOrOwnedMoveObject(mock_gas_sui_object.compute_object_reference()),
                object: ObjectReadResultKind::Object(mock_gas_sui_object),
            };
            // 添加到InputObjects，这样执行引擎能找到它。
            current_input_objects.objects.push(mock_gas_object_read_result.clone());
            // 添加到OverrideCache，这样模拟器会优先使用这个模拟对象。
            override_objects_list.push(mock_gas_object_read_result);
        }

        if let Some((borrowed_sui_coin_object, _borrowed_amount)) = &borrowed_coin_info {
            info!("DBSimulator: 正在将模拟的借入代币 {:?} 添加到覆盖对象", borrowed_sui_coin_object.id());
            let borrowed_coin_object_read_result = ObjectReadResult {
                input_object_kind: InputObjectKind::ImmOrOwnedMoveObject(borrowed_sui_coin_object.compute_object_reference()),
                object: ObjectReadResultKind::Object(borrowed_sui_coin_object.clone()),
            };
            current_input_objects.objects.push(borrowed_coin_object_read_result.clone());
            override_objects_list.push(borrowed_coin_object_read_result);
        }

        // 步骤 5: 创建 `OverrideCache`。
        // `OverrideCache` 允许在模拟时使用 `override_objects_list` 中的对象状态来覆盖
        // 从底层存储 (即 `self.store`，也即 `WritebackCache`) 中读取到的对象状态。
        let override_object_cache = if self.with_fallback {
            // 如果 `with_fallback` 为true，则在OverrideCache未命中时，会尝试从 `self.store` 回退查找。
            OverrideCache::new(Some(self.store.clone()), override_objects_list)
        } else {
            // 否则，严格只使用 `override_objects_list` 中的对象，不回退。
            OverrideCache::new(None, override_objects_list)
        };

        // 步骤 6: 再次更新 `current_input_objects`，确保它们反映了 `OverrideCache` 中的最新状态。
        // 这是因为 `override_objects_list` 可能包含了与 `tx_data_to_simulate.input_objects()`
        // 声明的输入对象ID相同的对象，但具有不同的版本或内容。
        // `OverrideCache` 会确保返回的是被覆盖后的版本。
        for object_read_result_item in current_input_objects.objects.iter_mut() {
            // 只更新ObjectReadResultKind::Object类型，因为Deleted等状态不需要从cache更新数据。
            if let ObjectReadResultKind::Object(object_data_ref) = &object_read_result_item.object {
                // 尝试从OverrideCache中获取该对象ID的最新数据。
                if let Some(overridden_object_data) = (&override_object_cache as &dyn ObjectCacheRead).get_object(&object_data_ref.id()) {
                    // 如果在OverrideCache中找到，则更新 input_objects 中的对象数据。
                    object_read_result_item.object = ObjectReadResultKind::Object(overridden_object_data);
                }
                // 如果在OverrideCache中未找到，则 input_objects 中原有的 (来自self.store的) 数据保持不变。
            }
        }

        // 步骤 7: 准备执行交易。
        let transaction_digest = *tx_data_to_simulate.digest(); // 获取交易摘要
        let transaction_kind = tx_data_to_simulate.into_kind(); // 将TransactionData转换为TransactionKind (消耗tx_data_to_simulate)
        let input_object_kinds_for_exec = current_input_objects.object_kinds().cloned().collect::<Vec<_>>(); // 获取最终的InputObjectKind列表

        let simulate_start_time = std::time::Instant::now(); // 记录模拟执行开始时间

        // 步骤 8: 调用Sui执行引擎的 `execute_transaction_to_effects` 方法。
        // `catch_unwind` 用于捕获Move执行引擎可能发生的panic。
        // `AssertUnwindSafe` 用于在线程边界或FFI边界标记闭包是安全的，即使它可能panic。
        let (temporary_store_after_exec, transaction_effects_result) = catch_unwind(AssertUnwindSafe(|| {
            let (inner_temp_store, _execution_result_status, effects_output, _transaction_auxiliary_data) = self.executor.execute_transaction_to_effects(
                &override_object_cache,         // 对象存储 (优先从OverrideCache读取)
                &self.protocol_config,          // 当前协议配置
                self.metrics.clone(),           // 限制相关的度量指标
                false,                          // 是否为系统交易的检查点执行 (通常为false)
                &HashSet::new(),                // (可能) 用于存储需要延迟加载的对象的集合
                &current_epoch_info.epoch_id,   // 当前Epoch ID
                current_epoch_info.epoch_start_timestamp, // Epoch开始时间戳
                CheckedInputObjects::new_with_checked_transaction_inputs(current_input_objects), // 检查过的输入对象
                gas_object_refs_for_execution,  // Gas支付对象引用列表
                gas_status_machine,             // Gas状态机
                transaction_kind,               // 交易种类 (ProgrammableTransaction等)
                sender_address,                 // 交易发送者
                transaction_digest,             // 交易摘要
            );
            (inner_temp_store, effects_output) // 返回临时存储和交易效果
        }))
        .map_err(|panic_payload| eyre::eyre!("模拟交易时发生Panic: {:?}", panic_payload))?; // 如果发生panic，转换为eyre::Error

        debug!("DBSimulator: 模拟交易 {} 耗时: {:?}", transaction_digest, simulate_start_time.elapsed());

        // 步骤 9: 从临时存储中获取被修改的对象。
        let mutated_objects_after_exec = self.get_mutated_objects(&transaction_effects_result, &temporary_store_after_exec)?;

        // 创建一个临时的 `ExecutedDB` 实例，用于后续计算余额变更和格式化事件。
        // 它结合了模拟前的 `OverrideCache` 状态和模拟中产生的 `temporary_store_after_exec`。
        let executed_db_view = ExecutedDB {
            db: &override_object_cache,
            temp_store: &temporary_store_after_exec,
        };

        // 步骤 10: 计算余额变更。
        // `get_balance_changes_from_effect` 会比较交易前后的对象状态来确定余额变化。
        // 需要忽略模拟Gas币和模拟借入代币的“伪”余额变化。
        let mut final_balance_changes = if !use_mocked_gas_coin { // 如果使用的是真实Gas币
            get_balance_changes_from_effect(
                &executed_db_view,
                &transaction_effects_result,
                input_object_kinds_for_exec,
                borrowed_coin_info.clone().map(|(obj, _)| vec![obj.id()]), // 忽略借入代币的ID
            )
            .await?
        } else { // 如果使用的是模拟Gas币
            let mut ids_to_ignore_in_balance_change = vec![mock_gas_object_id]; // 首先忽略模拟Gas币
            if let Some((borrowed_sui_coin_object, _)) = &borrowed_coin_info { // 如果有借入代币，也忽略它
                ids_to_ignore_in_balance_change.push(borrowed_sui_coin_object.id());
            }
            get_balance_changes_from_effect(&executed_db_view, &transaction_effects_result, input_object_kinds_for_exec, Some(ids_to_ignore_in_balance_change)).await?
        };

        // 步骤 11: (针对闪电贷场景) 调整余额变更，减去借入的金额。
        // 因为 `get_balance_changes_from_effect` 计算的是相对于“模拟开始时”的净变化。
        // 对于闪电贷，我们借入了一笔钱，这笔钱在模拟开始时并不属于我们。
        // 假设最终还款后，我们关心的净利润是相对于“未借款”状态的。
        // `borrowed_coin_info` 包含了原始借入的金额。
        // 如果最终输出的币种与借入的币种相同 (这里硬编码为SUI)，则从该币种的净变化中减去原始借入额。
        // TODO: 当前逻辑硬编码为SUI，应推广到支持任意币种的借贷调整。
        if let Some((_borrowed_sui_coin_object, borrowed_amount_val)) = &borrowed_coin_info {
            let mut found_sui_balance_change = false;
            if let Some(balance_change_entry) = final_balance_changes
                .iter_mut()
                // 找到属于发送者且币种为SUI的余额变更记录
                .find(|bc| bc.owner == Owner::AddressOwner(sender_address) && bc.coin_type.to_string() == SUI_COIN_TYPE)
            {
                found_sui_balance_change = true;
                // 从净变化中减去借入的金额，得到相对于“未借款”状态的真实净利润/亏损。
                balance_change_entry.amount -= *borrowed_amount_val as i128;
            }

            if !found_sui_balance_change { // 如果SUI没有其他净变化，则需要添加一条新的负向余额变更记录
                final_balance_changes.push(BalanceChange {
                    owner: Owner::AddressOwner(sender_address),
                    coin_type: TypeTag::Struct(Box::new(move_core_types::language_storage::StructTag {
                        address: move_core_types::account_address::AccountAddress::TWO, // 0x2
                        module: sui_types::Identifier::new("sui").unwrap(),
                        name: sui_types::Identifier::new("SUI").unwrap(),
                        type_params: vec![],
                    })),
                    amount: -(*borrowed_amount_val as i128), // 净变化为负的借入金额
                });
            }
        }

        // 步骤 12: (针对模拟Gas币场景) 调整SUI余额变更，以反映实际的Gas消耗。
        // 如果使用了模拟Gas币，`get_balance_changes_from_effect` (在忽略模拟Gas币ID后)
        // 可能没有正确计算出SUI的Gas消耗。这里需要手动调整。
        if use_mocked_gas_coin {
            let mut found_sui_balance_for_gas_adj = false;
            // 模拟Gas币的初始面额
            let initial_mock_gas_amount = 1_000_000_000u64 * 1_000_000_000u64;
            // 从临时存储中获取模拟Gas币在交易结束后的最终面额
            let final_mock_gas_amount = temporary_store_after_exec
                .written
                .get(&mock_gas_object_id) // 通过固定ID获取
                .unwrap() // 假设模拟Gas币一定在written中
                .as_coin_maybe() // 转换为CoinOption
                .unwrap() // 假设是Coin
                .value(); // 获取其u64面额

            // 实际消耗的Gas = 初始面额 - 最终面额
            let actual_gas_spent_from_mock = initial_mock_gas_amount - final_mock_gas_amount;

            for balance_change_entry in final_balance_changes.iter_mut() {
                if balance_change_entry.owner == Owner::AddressOwner(sender_address) && balance_change_entry.coin_type.to_string() == SUI_COIN_TYPE {
                    // 从SUI的净变化中再减去实际消耗的Gas。
                    // （如果 `get_balance_changes_from_effect` 已经考虑了非Gas支付对象的SUI消耗，这里可能重复计算，需要小心）
                    // 假设这里的目的是确保SUI余额变化能准确反映Gas支出。
                    balance_change_entry.amount -= actual_gas_spent_from_mock as i128;
                    found_sui_balance_for_gas_adj = true;
                }
            }

            if !found_sui_balance_for_gas_adj { // 如果SUI没有其他净变化，则添加一条新的Gas消耗记录
                final_balance_changes.push(BalanceChange {
                    owner: Owner::AddressOwner(sender_address),
                    coin_type: TypeTag::Struct(Box::new(move_core_types::language_storage::StructTag {
                        address: move_core_types::account_address::AccountAddress::TWO,
                        module: sui_types::Identifier::new("sui").unwrap(),
                        name: sui_types::Identifier::new("SUI").unwrap(),
                        type_params: vec![],
                    })),
                    amount: -(actual_gas_spent_from_mock as i128), // 净变化为负的Gas消耗
                });
            }
        }

        // 步骤 13: 格式化交易事件。
        // `self.executor.type_layout_resolver` 用于解析事件内容中的Move类型。
        let mut type_layout_resolver = self.executor.type_layout_resolver(Box::new(&self.store));
        let formatted_transaction_events =
            SuiTransactionBlockEvents::try_from(temporary_store_after_exec.events, transaction_digest, None, type_layout_resolver.as_mut())?;

        // 步骤 14: 计算本次模拟引起的缓存未命中次数。
        let cache_misses_count_during_sim = self
            .writeback_metrics
            .cache_misses_count()
            .saturating_sub(cache_misses_count_before_sim); // saturating_sub防止下溢

        // 步骤 15: 构建并返回 `SimulateResult`。
        Ok(SimulateResult {
            effects: SuiTransactionBlockEffects::try_from(transaction_effects_result)?, // 将内部TransactionEffects转为RPC类型
            events: formatted_transaction_events,
            object_changes: mutated_objects_after_exec,
            balance_changes: final_balance_changes,
            cache_misses: cache_misses_count_during_sim,
        })
    }

    /// `name` 方法 (来自 `Simulator` trait)
    fn name(&self) -> &str {
        "DBSimulator"
    }

    /// `get_object` 异步方法 (来自 `Simulator` trait)
    ///
    /// 从 `WritebackCache` 中获取指定ObjectID的对象数据。
    async fn get_object(&self, obj_id: &ObjectID) -> Option<Object> {
        self.store.get_object(obj_id)
    }

    /// `get_object_layout` 方法 (来自 `Simulator` trait)
    ///
    /// 获取指定ObjectID的对象的Move结构布局信息。
    /// 这对于反序列化对象的Move内容或理解其字段结构很重要。
    fn get_object_layout(&self, obj_id: &ObjectID) -> Option<MoveStructLayout> {
        // 首先从缓存获取对象本身
        let object_data = self.store.get_object(obj_id)?;
        // 获取对象的Move类型 (Option<MoveObjectType>)
        let object_move_type = object_data.type_().cloned()?; // type_()返回&Option<MoveObjectType>, cloned()得到Option<MoveObjectType>

        // 使用执行引擎的类型布局解析器来获取该类型的带注解的布局。
        let annotated_layout_result = self
            .executor
            .type_layout_resolver(Box::new(&self.store)) // 创建一个临时的类型解析器，其对象源是self.store
            .get_annotated_layout(&object_move_type.into()); // 将MoveObjectType转换为StructTag并获取布局

        match annotated_layout_result {
            Ok(layout_data) => match layout_data { // 如果成功获取布局
                MoveDatatypeLayout::Struct(struct_layout_data) => Some(*struct_layout_data), // 如果是结构体布局，则返回
                _ => None, // 如果是其他数据类型布局 (如Vector, U64等)，则不适用，返回None
            },
            Err(_) => { // 如果获取布局失败
                error!("为对象 {:?} 获取Move布局失败", obj_id);
                None
            }
        }
    }
}

/// `ExecutedDB` 结构体 (临时对象提供者)
///
/// 一个临时的结构体，实现了 `ObjectProvider` trait。
/// 它在模拟交易后，用于从 `OverrideCache` (代表模拟前的状态和覆盖)
/// 和 `InnerTemporaryStore` (代表模拟中发生的变化) 中查找特定版本的对象。
/// 这对于准确计算余额变更和格式化事件至关重要，因为这些操作需要访问对象在交易执行特定阶段的状态。
struct ExecutedDB<'a> {
    db: &'a OverrideCache, // 对OverrideCache的引用 (模拟前状态 + 预设覆盖)
    temp_store: &'a InnerTemporaryStore, // 对InnerTemporaryStore的引用 (交易执行期间的写入)
}

#[async_trait]
impl<'a> ObjectProvider for ExecutedDB<'a> {
    type Error = IndexerError; // 定义错误类型

    /// `get_object` 异步方法 (来自 `ObjectProvider` trait)
    ///
    /// 获取指定ID和版本的对象。
    /// 查找顺序：
    /// 1. `self.db` (OverrideCache) 中是否有用于比较的特定版本对象。
    /// 2. `self.temp_store.input_objects` (交易输入时加载的对象)。
    /// 3. `self.temp_store.written` (交易执行期间写入的对象)。
    /// 4. `self.db` (作为ObjectCacheRead) 中是否有匹配ID和版本的对象。
    async fn get_object(&self, id: &ObjectID, version: &SequenceNumber) -> Result<Object, Self::Error> {
        // 尝试从 OverrideCache (db) 的比较缓存中获取
        if let Some(obj) = self.db.get_versioned_object_for_comparison(id, *version) {
            return Ok(obj);
        }
        // 尝试从临时存储的输入对象中获取
        if let Some(obj) = self.temp_store.input_objects.get(id) {
            if obj.version() == *version {
                return Ok(obj.clone());
            }
        }
        // 尝试从临时存储的写入对象中获取
        if let Some(obj) = self.temp_store.written.get(id) {
            if obj.version() == *version {
                return Ok(obj.clone());
            }
        }
        // 尝试从 OverrideCache (db) 的主缓存中获取
        if let Some(obj) = (self.db as &dyn ObjectCacheRead).get_object_by_key(id, *version) {
            return Ok(obj);
        }
        // 如果都找不到，则返回错误
        Err(IndexerError::GenericError(format!(
            "ExecutedDB: 对象未找到, ID: {:?}, 版本: {}",
            id, version
        )))
    }

    /// `find_object_lt_or_eq_version` 异步方法 (来自 `ObjectProvider` trait)
    ///
    /// 查找ID匹配且版本号小于或等于给定`version`的最新对象。
    /// 查找顺序：
    /// 1. `self.temp_store.written` (交易执行期间写入的对象)。
    /// 2. `self.db` (OverrideCache)。
    async fn find_object_lt_or_eq_version(
        &self,
        id: &ObjectID,
        version: &SequenceNumber,
    ) -> Result<Option<Object>, Self::Error> {
        // 优先从临时存储的写入对象中查找 (因为它们是最新写入的)
        if let Some(obj) = self.temp_store.written.get(id) {
            if obj.version() <= *version { // 如果版本符合条件
                return Ok(Some(obj.clone()));
            }
        }
        // 如果临时存储中没有或版本不符，则从 OverrideCache (db) 中查找
        Ok(self.db.find_object_lt_or_eq_version(*id, *version))
    }
}

/// `CacheWriter` trait (缓存写入器接口)
///
/// 一个组合了 `ExecutionCacheWrite` 和 `ObjectStore` 两个trait的空trait。
/// `ExecutionCacheWrite` 提供了写入执行缓存（如 `WritebackCache`）的方法。
/// `ObjectStore` 提供了通用的对象存储读取方法。
/// 这个trait可能是为了方便地指代同时具备这两种能力的类型。
pub trait CacheWriter: ExecutionCacheWrite + ObjectStore {}

// 为 `WritebackCache` 实现 `CacheWriter` trait。
// 由于 `WritebackCache` 已经分别实现了 `ExecutionCacheWrite` 和 `ObjectStore` (或其父trait)，
// 这个实现是自动满足的，不需要额外的方法体。
impl CacheWriter for WritebackCache {}

/// `preload_objects` 内联函数
///
/// 将一组指定ID的对象从 `cache_writer` (实现了 `ObjectStore`) 中读取出来，
/// 然后再写回 `cache_writer` (通过 `ExecutionCacheWrite::reload_objects`)。
/// 目的是确保这些对象被加载到缓存的“热”路径中。
///
/// 参数:
/// - `cache_writer`: 实现了 `CacheWriter` trait 的对象 (例如 `Arc<WritebackCache>`)。
/// - `preload_ids`: 要预加载的对象ID列表。
#[inline]
pub fn preload_objects(cache_writer: Arc<dyn CacheWriter>, preload_ids: &[ObjectID]) {
    // 批量从缓存/存储中获取这些ID对应的对象
    let preload_objects_data = cache_writer
        .multi_get_objects(&preload_ids)
        .into_iter()
        .filter_map(|obj_option| match obj_option { // 过滤掉未找到的对象
            Some(obj_data) => Some((obj_data.id(), obj_data)), // 保留 (ID, Object) 元组
            None => None,
        })
        .collect::<Vec<_>>();

    // 调用 `reload_objects` 将这些对象重新加载到缓存中。
    // 这可能会更新缓存中的条目或将其标记为最近使用。
    cache_writer.reload_objects(preload_objects_data);
}

/// `spawn_update_thread` 异步函数 (后台缓存更新线程的执行体)
///
/// 这个函数在一个独立的Tokio运行时中执行（通过 `#[tokio::main]` 宏）。
/// 它负责监听一个Unix套接字，接收外部发送的对象更新数据，并用这些数据更新 `cache_writer`。
/// 它还会定期执行缓存的持久化和预加载对象的刷新。
///
/// 参数:
/// - `socket_path`: 要监听的Unix套接字的路径。
/// - `preload_ids`: 需要定期刷新的预加载对象ID列表。
/// - `cache_writer`: 实现了 `CacheWriter` trait 的对象 (例如 `Arc<WritebackCache>`)。
#[tokio::main] // 使用独立的tokio运行时执行此异步函数
async fn spawn_update_thread(socket_path: PathBuf, preload_ids: Vec<ObjectID>, cache_writer: Arc<dyn CacheWriter>) {
    info!("缓存更新线程启动，正在连接到Unix套接字: {:?}", socket_path);
    // 连接到指定的Unix套接字
    let mut unix_socket_stream = match UnixStream::connect(socket_path).await {
        Ok(stream) => stream,
        Err(e) => {
            error!("连接到缓存更新Unix套接字失败: {}", e);
            return; // 连接失败则线程退出
        }
    };
    info!("成功连接到缓存更新Unix套接字。");

    let mut length_buffer = [0u8; 4]; // 用于读取4字节的长度前缀
    let mut last_catch_up_time = std::time::Instant::now(); // 上次执行维护操作的时间点
    loop { // 无限循环以持续接收更新
        // 步骤 1: 读取长度前缀。
        // 假设外部发送的数据格式是：[4字节长度 (小端序u32)][实际数据负载]。
        if let Err(e) = unix_socket_stream.read_exact(&mut length_buffer).await {
            error!("从Unix套接字读取长度前缀失败: {}", e);
            break; // 读取失败，可能连接已断开，退出循环
        }
        let payload_length = u32::from_le_bytes(length_buffer) as usize; // 将字节转换为u32长度

        // 步骤 2: 读取实际数据负载。
        let mut payload_buffer = vec![0u8; payload_length]; // 根据长度创建缓冲区
        if let Err(e) = unix_socket_stream.read_exact(&mut payload_buffer).await {
            error!("从Unix套接字读取数据负载失败: {}", e);
            break; // 读取失败，退出循环
        }

        // 步骤 3: 反序列化数据负载并更新缓存。
        // 假设负载是BCS编码的 `Vec<(ObjectID, Object)>`。
        match bcs::from_bytes::<Vec<(ObjectID, Object)>>(&payload_buffer) {
            Ok(objects_to_reload) => {
                // 调用 `reload_objects` 更新 WritebackCache。
                // 这会将 `objects_to_reload` 中的对象状态写入缓存，覆盖旧值。
                cache_writer.reload_objects(objects_to_reload);
                debug!("DBSimulator缓存已通过Unix套接字更新。");
            }
            Err(e) => {
                error!("反序列化来自Unix套接字的缓存更新数据失败: {}", e);
            }
        }

        // 步骤 4: 定期执行维护操作 (例如，每天一次)。
        if last_catch_up_time.elapsed() > std::time::Duration::from_secs(3600 * 24) { // 检查是否超过24小时
            info!("DBSimulator: 执行每日缓存维护操作...");
            // `update_underlying(true)` 可能将 `WritebackCache` 中的脏数据（已修改但未持久化的）
            // 写回到其底层的 `AuthorityStore` (即Sui节点数据库)。
            // `true` 可能表示强制写回所有内容。
            cache_writer.update_underlying(true);
            // 重新加载预设的对象列表，以确保它们是基于（可能已更新的）底层数据库的最新状态。
            preload_objects(cache_writer.clone(), &preload_ids); // 需要克隆Arc以传递
            info!("DBSimulator: 每日缓存维护操作完成。");
            last_catch_up_time = std::time::Instant::now(); // 更新上次维护时间
        }
    }
    info!("缓存更新线程已停止。");
}

[end of crates/simulator/src/db_simulator/mod.rs]
