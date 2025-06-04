// 该文件 `start_bot.rs` 是套利机器人程序的主要启动和运行逻辑。
// 它负责初始化所有必要的组件，如事件收集器、交易执行器、模拟器和核心套利策略，
// 然后使用一个引擎（可能是`burberry::Engine`）来驱动这些组件协同工作。
//
// **文件概览 (File Overview)**:
// 这个 `start_bot.rs` 文件是整个套利机器人的“总装车间”和“发动机点火器”。
// 当你在命令行中指示机器人“启动！”（通常是通过 `arb start-bot ...` 命令），这个文件里的代码就会被执行。
// 它的核心任务是把机器人运行所需的所有零件（模块/组件）都准备好、组装起来，然后启动整个系统。
//
// **主要步骤 (Main Steps)**:
// 1.  **读取配置 (Reading Configuration)**:
//     -   机器人需要很多配置信息才能正确运行，比如：
//         -   你的Sui账户私钥（用来签名交易）。
//         -   连接哪个Sui RPC节点（用来和区块链通信）。
//         -   从哪里收集交易信息（比如Shio协议的地址、本地节点的套接字路径）。
//         -   模拟交易时用哪个数据库（如果用DBSimulator）。
//     -   这些配置信息大部分通过命令行参数（`Args` 结构体）传递进来。
//
// 2.  **初始化核心组件 (Initializing Core Components)**:
//     -   **事件收集器 (Event Collectors)**: 这些是机器人的“耳朵”，负责从不同地方监听Sui网络上发生的事情。
//         -   `ShioCollector`: 监听Shio MEV协议的事件（比如新的出价机会）。
//         -   `PublicTxCollector`: 监听所有公开广播的Sui交易。
//         -   `PrivateTxCollector`: 监听通过特定渠道（Relay WebSocket）私下广播的交易（通常用于MEV）。
//     -   **交易执行器 (Transaction Executors)**: 这些是机器人的“手”，负责把机器人决定要做的操作（比如提交一笔套利交易或MEV竞价）实际发送出去。
//         -   `ShioExecutor` / `ShioRPCExecutor`: 把MEV竞价提交到Shio协议。
//         -   `PublicTxExecutor`: 把普通的Sui交易提交到链上。
//         -   `TelegramMessageDispatcher`: 通过Telegram（一个即时通讯软件）发送通知消息（比如机器人状态、发现的套利机会等）。
//     -   **交易模拟器 (Transaction Simulators)**: 这是机器人的“沙盘”或“排练场”。在真正花钱执行交易之前，机器人会用模拟器来“彩排”一下，
//         看看这笔交易如果真的执行了，大概会是什么结果（能不能赚钱、花多少Gas费等）。这有助于避免亏钱的交易。
//         -   `DBSimulator`: 一种高性能的模拟器，它在本地电脑上维护一个Sui链状态的数据库副本。模拟速度快，适合正式运行。
//         -   `HttpSimulator`: 通过Sui RPC节点进行模拟，速度较慢，现在主要用于测试或作为备选。
//         -   `ReplaySimulator`: 一种特殊的模拟器，可能用于回放历史数据或在特定场景下进行分析。
//     -   **套利策略 (Arbitrage Strategy - `ArbStrategy`)**: 这是机器人的“大脑”，包含了发现套利机会、计算最佳路径、构建交易的核心智能。
//
// 3.  **组装引擎 (Assembling the Engine)**:
//     -   所有上面初始化好的收集器、执行器和策略，都会被添加到一个叫做 `Engine`（来自 `burberry` 库）的中心协调器里。
//     -   这个引擎就像一个总调度台，负责把从“耳朵”（收集器）听到的信息传递给“大脑”（策略），再把“大脑”的决定传递给“手”（执行器）去执行。
//
// 4.  **启动附加服务 (Starting Additional Services)**:
//     -   **心跳服务 (Heartbeat Service)**: 定期向外部报告机器人还在正常运行，像心跳一样。这对于监控机器人是否健康很重要。
//
// 5.  **运行引擎 (Running the Engine)**:
//     -   最后，调用 `engine.run_and_join()`，整个机器人系统就正式启动并开始工作了。它会一直运行，直到被手动停止或发生严重错误。
//
// **Sui区块链、MEV及机器人相关的概念解释 (Sui Blockchain, MEV, and Bot-related Concepts)**:
//
// -   **事件驱动架构 (Event-Driven Architecture)**:
//     这个机器人很可能采用事件驱动的模式工作。这意味着它的行为是由发生的“事件”来触发的。
//     例如：一个新的公开交易被收集到（事件）-> 策略分析这个交易看是否有套利机会（处理）-> 如果有机会，构建套利交易并让执行器提交（动作）。
//     `burberry::Engine` 可能就是这样一个事件驱动框架。
//
// -   **MEV (Miner Extractable Value / Maximal Extractable Value)**:
//     指验证者（在PoS链如Sui中）或矿工（在PoW链中）通过其在交易排序、打包等方面的特权所能获得的额外利润。
//     套利机器人本身就是一种MEV的寻找者。
//     -   **Shio Protocol**: 这是一个专门为MEV设计的协议，允许搜索者（如套利机器人）对交易的优先打包权进行“竞价”。
//         机器人如果发现一个高利润的MEV机会，可能会通过Shio提交一个出价（bid），希望能让自己的交易被优先打包，从而成功捕获这个机会。
//         `ShioCollector` 和 `ShioExecutor` 就是用于与Shio协议交互的组件。
//     -   **Relay WebSocket / Private Transactions**: 为了防止自己的套利交易被其他MEV机器人“抢跑”（front-running）或“三明治攻击”（sandwich attack），
//         一些MEV搜索者会选择不把交易公开广播到公共内存池（mempool），而是通过私密的WebSocket通道直接发送给区块生产者或中继服务。
//         `PrivateTxCollector` 就是用来监听这种私有交易流的。
//
// -   **模拟的重要性 (Importance of Simulation)**:
//     在DeFi和MEV领域，交易执行的原子性（要么全部成功，要么全部失败）和链上状态的复杂性，使得“事前模拟”变得极其重要。
//     -   **成本效益**: 模拟可以避免因错误判断而导致的真实资金损失和Gas费浪费。
//     -   **机会验证**: 确认一个理论上的套利机会在实际执行时是否真的有利可图（考虑到滑点、Gas、交易顺序等因素）。
//     -   **参数优化**: 可以通过多次模拟来调整交易参数（如输入金额）以期达到最大利润。
//     `DBSimulator` 的高性能对于需要快速、大量模拟的套利策略尤为关键。
//
// -   **并发与异步 (Concurrency and Asynchronicity)**:
//     机器人需要同时处理来自多个收集器的事件、执行多个模拟、可能还要与其他外部服务交互。
//     Rust的异步编程模型（`async/await`）和像Tokio这样的运行时，使得编写高并发、高性能的网络应用成为可能。
//     代码中大量的 `async fn` 和 `.await` 就是异步编程的体现。

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::{
    sync::Arc, // 原子引用计数 (Atomic Reference Counting, for shared ownership across threads)
    time::{Duration, Instant}, // 时间处理 (Time handling utilities)
};

use ::utils::heartbeat; // 从外部 `utils` crate 引入心跳服务 (Import heartbeat service from external `utils` crate)
use burberry::{executor::telegram_message::TelegramMessageDispatcher, map_collector, map_executor, Engine}; // `burberry`引擎框架相关组件
                                                                                                            // `burberry` engine framework related components
use clap::Parser; // `clap` crate，用于解析命令行参数 (clap crate, for parsing command-line arguments)
use eyre::Result; // `eyre`库，用于错误处理 (eyre library, for error handling)
use object_pool::ObjectPool; // 对象池，用于管理模拟器实例 (Object pool, for managing simulator instances)
use shio::{new_shio_collector_and_executor, ShioRPCExecutor}; // `shio` crate，用于与Shio MEV协议交互
                                                              // `shio` crate, for interacting with the Shio MEV protocol
use simulator::{DBSimulator, HttpSimulator, ReplaySimulator, Simulator}; // 各种模拟器实现 (Various simulator implementations)
use sui_types::{base_types::SuiAddress, crypto::SuiKeyPair}; // Sui基本类型和加密类型 (Sui basic types and cryptography types)
use tracing::{info, warn}; // `tracing`库，用于日志记录 (tracing library, for logging)

// 从当前crate的其他模块引入 (Import from other modules in the current crate)
use crate::{
    collector::{PrivateTxCollector, PublicTxCollector}, // 各种事件收集器 (Various event collectors)
    executor::PublicTxExecutor,                         // 公开交易执行器 (Public transaction executor)
    strategy::ArbStrategy,                              // 套利策略核心逻辑 (Arbitrage strategy core logic)
    types::{Action, Event},                              // 自定义的Action和Event枚举 (Custom Action and Event enums)
    HttpConfig,                                         // 通用的HTTP配置 (在main.rs中定义) (Common HTTP configuration, defined in main.rs)
};

/// `Args` 结构体 (启动机器人子命令的参数)
/// (Args struct (Parameters for the start-bot subcommand))
///
/// 定义了通过命令行启动套利机器人时可以接受的参数。
/// (Defines the parameters that can be accepted when starting the arbitrage bot via the command line.)
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// Sui账户的私钥字符串 (Base64编码)。
    /// (Sui account's private key string (Base64 encoded).)
    /// 用于签名交易。会从环境变量 `SUI_PRIVATE_KEY` 读取。
    /// (Used for signing transactions. Will be read from the environment variable `SUI_PRIVATE_KEY`.)
    #[arg(long, env = "SUI_PRIVATE_KEY")]
    pub private_key: String,

    /// Shio执行器是否使用RPC提交竞价。
    /// (Whether the Shio executor uses RPC to submit bids.)
    /// 如果为false，可能使用WebSocket或其他方式。
    /// (If false, WebSocket or other methods might be used.)
    #[arg(long, help = "Shio执行器使用RPC提交竞价 (Shio executor uses RPC to submit bids)")]
    pub shio_use_rpc: bool,

    /// HTTP相关的配置 (例如Sui RPC URL)。
    /// (HTTP related configuration (e.g., Sui RPC URL).)
    #[command(flatten)]
    pub http_config: HttpConfig,

    /// 收集器相关的配置。
    /// (Collector related configuration.)
    #[command(flatten)]
    collector_config: CollectorConfig,

    /// 数据库模拟器相关的配置。
    /// (Database simulator related configuration.)
    #[command(flatten)]
    db_sim_config: DbSimConfig,

    /// 工作线程和内部队列相关的配置。
    /// (Configuration related to worker threads and internal queues.)
    #[command(flatten)]
    worker_config: WorkerConfig,
}

/// `CollectorConfig` 结构体 (收集器配置参数)
/// (CollectorConfig struct (Collector configuration parameters))
#[derive(Clone, Debug, Parser)]
struct CollectorConfig {
    /// 中继交易收集器的WebSocket URL (可选)。
    /// (WebSocket URL for the relay transaction collector (optional).)
    /// 用于接收私下广播的交易。如果提供，则会启用 `PrivateTxCollector`。
    /// (Used for receiving privately broadcast transactions. If provided, `PrivateTxCollector` will be enabled.)
    /// 通常与 `public_tx_collector` (通过tx_socket_path) 互斥。
    /// (Usually mutually exclusive with `public_tx_collector` (via tx_socket_path).)
    #[arg(long)]
    pub relay_ws_url: Option<String>,

    /// Shio事件收集器的WebSocket URL (可选)。
    /// (WebSocket URL for the Shio event collector (optional).)
    /// 用于从Shio协议接收事件（如新的拍卖轮次、高出价等）。
    /// (Used for receiving events from the Shio protocol (e.g., new auction rounds, high bids, etc.).)
    #[arg(long)]
    pub shio_ws_url: Option<String>,

    /// 公开交易收集器的本地IPC套接字路径。
    /// (Local IPC socket path for the public transaction collector.)
    /// 用于从本地运行的Sui节点（或类似服务）接收所有新提交的交易。
    /// (Used for receiving all newly submitted transactions from a locally running Sui node (or similar service).)
    /// 默认值为 "/tmp/sui_tx.sock"。会从环境变量 `SUI_TX_SOCKET_PATH` 读取。
    /// (Default value is "/tmp/sui_tx.sock". Will be read from the environment variable `SUI_TX_SOCKET_PATH`.)
    #[arg(long, env = "SUI_TX_SOCKET_PATH", default_value = "/tmp/sui_tx.sock")]
    pub tx_socket_path: String,
}

/// `DbSimConfig` 结构体 (数据库模拟器配置参数)
/// (DbSimConfig struct (Database simulator configuration parameters))
#[derive(Clone, Debug, Parser)]
struct DbSimConfig {
    /// 数据库模拟器所需的Sui数据库路径。
    /// (Path to the Sui database required by the database simulator.)
    /// 默认值为 "/home/ubuntu/sui/db/live/store"。会从环境变量 `SUI_DB_PATH` 读取。
    /// (Default value is "/home/ubuntu/sui/db/live/store". Will be read from the environment variable `SUI_DB_PATH`.)
    #[arg(long, env = "SUI_DB_PATH", default_value = "/home/ubuntu/sui/db/live/store")]
    pub db_path: String,

    /// Sui全节点配置文件的路径 (例如 fullnode.yaml)。
    /// (Path to the Sui full node configuration file (e.g., fullnode.yaml).)
    /// 数据库模拟器可能需要此文件来获取网络配置或其他信息。
    /// (The database simulator might need this file to get network configuration or other information.)
    /// 默认值为 "/home/ubuntu/sui/fullnode.yaml"。会从环境变量 `SUI_CONFIG_PATH` 读取。
    /// (Default value is "/home/ubuntu/sui/fullnode.yaml". Will be read from the environment variable `SUI_CONFIG_PATH`.)
    #[arg(long, env = "SUI_CONFIG_PATH", default_value = "/home/ubuntu/sui/fullnode.yaml")]
    pub config_path: String,

    /// 数据库模拟器监听此套接字以接收来自Sui节点的缓存更新。
    /// (The database simulator listens on this socket to receive cache updates from the Sui node.)
    /// Sui节点在对象状态变化时，会通过此套接字通知模拟器更新其本地缓存。
    /// (When object states change, the Sui node notifies the simulator via this socket to update its local cache.)
    /// 默认值为 "/tmp/sui_cache_updates.sock"。会从环境变量 `SUI_UPDATE_CACHE_SOCKET` 读取。
    /// (Default value is "/tmp/sui_cache_updates.sock". Will be read from the environment variable `SUI_UPDATE_CACHE_SOCKET`.)
    #[arg(long, env = "SUI_UPDATE_CACHE_SOCKET", default_value = "/tmp/sui_cache_updates.sock")]
    pub update_cache_socket: String,

    /// 包含预加载对象ID列表的文件的路径。
    /// (Path to the file containing the list of preloaded object IDs.)
    /// 数据库模拟器会预加载这些对象到其缓存中。
    /// (The database simulator will preload these objects into its cache.)
    /// 默认值为 "/home/ubuntu/suiflow-relay/pool_related_ids.txt"。会从环境变量 `SUI_PRELOAD_PATH` 读取。
    /// (Default value is "/home/ubuntu/suiflow-relay/pool_related_ids.txt". Will be read from the environment variable `SUI_PRELOAD_PATH`.)
    #[arg(
        long,
        env = "SUI_PRELOAD_PATH",
        default_value = "/home/ubuntu/suiflow-relay/pool_related_ids.txt"
    )]
    pub preload_path: String,

    /// 是否使用数据库模拟器。
    /// (Whether to use the database simulator.)
    /// 如果为false，则可能使用HttpSimulator。
    /// (If false, HttpSimulator might be used.)
    #[arg(long, default_value_t = false)]
    pub use_db_simulator: bool,

    /// (DBSimulator) 数据库追赶（catchup）操作的间隔时间（秒）。
    /// ((DBSimulator) Interval time (in seconds) for database catchup operations.)
    /// DBSimulator可能需要定期从链上同步最新的状态以避免数据陈旧。
    /// (DBSimulator might need to periodically synchronize the latest state from the chain to avoid stale data.)
    #[arg(long, default_value_t = 60)]
    pub catchup_interval: u64,
}

/// `WorkerConfig` 结构体 (工作组件配置参数)
/// (WorkerConfig struct (Worker component configuration parameters))
#[derive(Clone, Debug, Parser)]
struct WorkerConfig {
    /// 处理事件（公开交易、私有交易、Shio事件）的工作线程数量。
    /// (Number of worker threads for processing events (public transactions, private transactions, Shio events).)
    /// 8个通常足够。
    /// (8 is usually sufficient.)
    #[arg(long, default_value_t = 8)]
    pub workers: usize,

    /// 模拟器对象池中的模拟器实例数量。
    /// (Number of simulator instances in the simulator object pool.)
    #[arg(long, default_value_t = 32)]
    pub num_simulators: usize,

    /// 最近已处理套利机会的最大数量。
    /// (Maximum number of recently processed arbitrage opportunities.)
    /// 如果一个新的机会与最近 `max_recent_arbs` 次已处理的机会相似（例如涉及相同的代币或池），
    /// 则可能会被忽略，以避免重复或过于频繁地处理相同的机会。
    /// (If a new opportunity is similar to the last `max_recent_arbs` processed opportunities (e.g., involving the same tokens or pools),
    /// it might be ignored to avoid duplicate or overly frequent processing of the same opportunity.)
    #[arg(long, default_value_t = 20)]
    pub max_recent_arbs: usize,

    /// (ReplaySimulator) 专用回放模拟器的短间隔和长间隔（毫秒）。
    /// ((ReplaySimulator) Short and long intervals (in milliseconds) for the dedicated replay simulator.)
    /// 短间隔，例如50毫秒。
    /// (Short interval, e.g., 50 milliseconds.)
    #[arg(long, default_value_t = 50)]
    pub dedicated_short_interval: u64,

    /// 长间隔，例如200毫秒。
    /// (Long interval, e.g., 200 milliseconds.)
    #[arg(long, default_value_t = 200)]
    pub dedicated_long_interval: u64,
}

/// `run` 函数 (启动机器人子命令的主入口)
/// (run function (Main entry point for the start-bot subcommand))
///
/// 初始化并运行套利机器人。
/// (Initializes and runs the arbitrage bot.)
///
/// 参数 (Parameters):
/// - `args`: 解析后的命令行参数 (`Args`结构体)。
///           (Parsed command-line arguments (`Args` struct).)
///
/// 返回 (Returns):
/// - `Result<()>`: 如果成功则返回Ok，否则返回错误。
///                 (Returns Ok if successful, otherwise returns an error.)
pub async fn run(args: Args) -> Result<()> {
    // 设置自定义的panic钩子，可能用于在程序崩溃时记录更详细的信息或执行清理操作。
    // (Set a custom panic hook, possibly for logging more detailed information or performing cleanup on program crash.)
    utils::set_panic_hook();
    // 初始化日志系统，使用白名单模块进行过滤。
    // (Initialize the logging system, using a whitelist of modules for filtering.)
    // "mainnet"可能是环境标识，"sui-arb"是应用名称。
    // ("mainnet" might be an environment identifier, "sui-arb" is the application name.)
    // "arb", "utils", "shio" 等模块会以默认级别记录，"cache_metrics=debug" 表示cache_metrics模块以debug级别记录。
    // (Modules like "arb", "utils", "shio" will log at the default level, "cache_metrics=debug" means the cache_metrics module logs at debug level.)
    mev_logger::init_with_whitelisted_modules(
        "mainnet",
        "sui-arb".to_string(),
        &["arb", "utils", "shio", "cache_metrics=debug"],
    );

    // 从提供的私钥字符串解码Sui密钥对。
    // (Decode the Sui keypair from the provided private key string.)
    let keypair = SuiKeyPair::decode(&args.private_key)?;
    // 从密钥对获取公钥。
    // (Get the public key from the keypair.)
    let pubkey = keypair.public();
    // 从公钥派生Sui地址，作为机器人的操作地址（攻击者地址）。
    // (Derive the Sui address from the public key, to be used as the bot's operator address (attacker address).)
    let attacker = SuiAddress::from(&pubkey);

    // 记录启动信息，包含关键配置。
    // (Log startup information, including key configurations.)
    info!(
        "启动套利机器人 (start_bot)，攻击者地址 (Attacker Address): {}, HTTP配置 (HTTP Config): {:#?}, 收集器配置 (Collector Config): {:#?}, DB模拟器配置 (DB Sim Config): {:#?}, Worker配置 (Worker Config): {:#?}",
        attacker, args.http_config, args.collector_config, args.db_sim_config, args.worker_config
    );

    // --- 初始化引擎和组件 ---
    // (Initialize engine and components)
    let rpc_url = args.http_config.rpc_url.clone(); // RPC URL的克隆，因为后续会多次使用 (Clone RPC URL as it will be used multiple times)
    let db_path = args.db_sim_config.db_path.clone();
    let tx_socket_path = args.collector_config.tx_socket_path.clone();
    let config_path = args.db_sim_config.config_path.clone();
    let update_cache_socket = args.db_sim_config.update_cache_socket.clone();
    let preload_path = args.db_sim_config.preload_path.clone();

    // 创建 `burberry::Engine` 实例，用于管理和运行所有组件。
    // (Create a `burberry::Engine` instance to manage and run all components.)
    let mut engine = Engine::default();
    info!("Burberry引擎已创建。"); // 日志：引擎创建

    // --- 配置收集器 (Collectors) ---
    // (Configure Collectors)
    // 如果配置了Shio WebSocket URL，则初始化Shio收集器和执行器。
    // (If Shio WebSocket URL is configured, initialize Shio collector and executor.)
    if let Some(ref ws_url) = args.collector_config.shio_ws_url {
        info!("配置Shio收集器和执行器，WS URL: {}", ws_url); // 日志：配置Shio
        let (shio_collector, shio_executor) =
            new_shio_collector_and_executor(keypair.copy(), Some(ws_url.clone()), None).await; // keypair.copy()因为后续可能还用
                                                                                               // (keypair.copy() because it might be used later)
        // `map_collector!` 是一个宏，用于将具体收集器的输出事件映射为引擎通用的 `Event` 枚举类型。
        // (`map_collector!` is a macro used to map output events of a specific collector to the engine's generic `Event` enum type.)
        // 这里将 `shio_collector` 产生的事件映射为 `Event::Shio`。
        // (Here, events produced by `shio_collector` are mapped to `Event::Shio`.)
        engine.add_collector(map_collector!(shio_collector, Event::Shio));

        // 根据配置选择Shio执行器类型 (RPC或WebSocket)。
        // (Select Shio executor type (RPC or WebSocket) based on configuration.)
        if args.shio_use_rpc {
            info!("Shio执行器使用RPC模式。"); // 日志：Shio RPC模式
            let shio_rpc_executor = ShioRPCExecutor::new(SuiKeyPair::decode(&args.private_key)?);
            // `map_executor!` 将引擎通用的 `Action` 枚举类型映射为具体执行器能处理的动作。
            // (`map_executor!` maps the engine's generic `Action` enum type to actions that a specific executor can handle.)
            // 这里将 `Action::ShioSubmitBid` 映射给 `shio_rpc_executor`。
            // (Here, `Action::ShioSubmitBid` is mapped to `shio_rpc_executor`.)
            engine.add_executor(map_executor!(shio_rpc_executor, Action::ShioSubmitBid));
        } else {
            info!("Shio执行器使用默认模式 (可能为WebSocket)。"); // 日志：Shio默认模式
            engine.add_executor(map_executor!(shio_executor, Action::ShioSubmitBid));
        }
    } else {
        // 如果没有配置Shio，则默认使用公开交易收集器。
        // (If Shio is not configured, use the public transaction collector by default.)
        info!("未配置Shio WS URL，将使用PublicTxCollector，路径: {}", tx_socket_path); // 日志：使用PublicTxCollector
        let public_tx_collector = PublicTxCollector::new(&tx_socket_path);
        engine.add_collector(Box::new(public_tx_collector)); // PublicTxCollector直接产生 Event::PublicTx
                                                             // (PublicTxCollector directly produces Event::PublicTx)
    }

    // 添加公开交易执行器。
    // (Add public transaction executor.)
    info!("配置PublicTxExecutor，RPC URL: {}", rpc_url); // 日志：配置PublicTxExecutor
    engine.add_executor(map_executor!(
        PublicTxExecutor::new(&rpc_url, SuiKeyPair::decode(&args.private_key)?).await?, // 注意每次都解码私钥 (Note: private key is decoded each time)
        Action::ExecutePublicTx
    ));

    // 如果配置了中继WebSocket URL，则初始化私有交易收集器。
    // (If relay WebSocket URL is configured, initialize private transaction collector.)
    if let Some(ref relay_ws_url) = args.collector_config.relay_ws_url {
        info!("配置PrivateTxCollector，Relay WS URL: {}", relay_ws_url); // 日志：配置PrivateTxCollector
        let private_tx_collector = PrivateTxCollector::new(relay_ws_url);
        engine.add_collector(Box::new(private_tx_collector)); // PrivateTxCollector直接产生 Event::PrivateTx
                                                               // (PrivateTxCollector directly produces Event::PrivateTx)
    }

    // --- 配置模拟器对象池 (Simulator Pool) ---
    // (Configure Simulator Object Pool)
    // 根据配置选择使用DBSimulator还是HttpSimulator。
    // (Select DBSimulator or HttpSimulator based on configuration.)
    info!("配置模拟器池，数量: {}, 使用DBSimulator: {}", args.worker_config.num_simulators, args.db_sim_config.use_db_simulator); // 日志：配置模拟器池
    let simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>> = Arc::new(match args.db_sim_config.use_db_simulator {
        true => {
            info!("正在为模拟器池创建DBSimulator实例..."); // 日志：创建DBSimulator
            // 使用DBSimulator (Use DBSimulator)
            let db_path_clone = db_path.clone(); // 克隆String以移动到闭包 (Clone String to move into closure)
            let config_path_clone = config_path.clone();
            let update_cache_socket_clone = update_cache_socket.clone();
            let preload_path_clone = preload_path.clone();
            ObjectPool::new(args.worker_config.num_simulators, move || {
                // 在新的tokio运行时中同步地执行异步的模拟器初始化。
                // (Synchronously execute asynchronous simulator initialization in a new tokio runtime.)
                // 这是因为ObjectPool的初始化函数是同步的。
                // (This is because ObjectPool's initialization function is synchronous.)
                tokio::runtime::Runtime::new().unwrap().block_on(async {
                    let start_time = Instant::now();
                    let simulator = Box::new(
                        DBSimulator::new_slow( // new_slow表示可能初始化较慢但功能完整的版本
                            &db_path_clone,
                            &config_path_clone,
                            Some(&update_cache_socket_clone),
                            Some(&preload_path_clone),
                        )
                        .await,
                    ) as Box<dyn Simulator>;
                    info!(elapsed_ms = %start_time.elapsed().as_millis(), "DBSimulator实例已为对象池初始化完毕。"); // 日志：DBSimulator实例初始化耗时
                    simulator
                })
            })
        }
        false => {
            // 使用HttpSimulator (已弃用，仅用于测试)
            // (Use HttpSimulator (deprecated, for testing only))
            warn!("HttpSimulator已弃用，请仅用于测试目的。(HttpSimulator is deprecated, please use for testing purposes only.)");
            let rpc_url_clone = rpc_url.clone();
            let ipc_path_clone = args.http_config.ipc_path.clone(); // ipc_path也是HttpConfig的一部分
                                                                    // (ipc_path is also part of HttpConfig)

            ObjectPool::new(args.worker_config.num_simulators, move || {
                let rpc_url_inner_clone = rpc_url_clone.clone(); // 再次克隆以移动到更内层的异步块
                                                                // (Clone again to move into inner async block)
                let ipc_path_inner_clone = ipc_path_clone.clone();
                tokio::runtime::Runtime::new().unwrap().block_on(async move {
                    Box::new(HttpSimulator::new(&rpc_url_inner_clone, &ipc_path_inner_clone).await)
                        as Box<dyn Simulator>
                })
            })
        }
    });

    // `Arc::new` 用于将 `ObjectPool` 包裹起来，如果 `simulator_pool` 需要被 `ArbStrategy` 等多个地方共享。
    // (`Arc::new` is used to wrap `ObjectPool` if `simulator_pool` needs to be shared by multiple places like `ArbStrategy`.)
    // 如果 `ObjectPool` 本身已经是 `Arc` 或者其 `get()` 方法返回 `Arc` 包装的实例，则可能不需要再包一层。
    // (If `ObjectPool` itself is already `Arc` or its `get()` method returns an `Arc`-wrapped instance, an additional layer might not be needed.)
    // 从 `ArbStrategy::new` 的签名看，它需要 `Arc<ObjectPool<...>>`。
    // (Judging from `ArbStrategy::new`'s signature, it requires `Arc<ObjectPool<...>>`.)

    // --- 配置策略自身使用的模拟器 (own_simulator) 和专用回放模拟器 (dedicated_simulator) ---
    // (Configure simulator for strategy's own use (own_simulator) and dedicated replay simulator (dedicated_simulator))
    // `own_simulator` 可能用于策略内部的一些特定、不适合从池中获取模拟器的场景。
    // (`own_simulator` might be used for specific scenarios within the strategy where fetching from the pool is not suitable.)
    info!("配置策略自身模拟器 (own_simulator)..."); // 日志：配置own_simulator
    let own_simulator_instance: Arc<dyn Simulator> = if args.db_sim_config.use_db_simulator {
        Arc::new(DBSimulator::new_slow(&db_path, &config_path, Some(&update_cache_socket), Some(&preload_path)).await)
    } else {
        warn!("HttpSimulator已弃用，策略自身模拟器也将使用它。(HttpSimulator is deprecated, strategy's own simulator will also use it.)");
        let ipc_path_for_own = args.http_config.ipc_path.clone();
        Arc::new(HttpSimulator::new(&rpc_url, &ipc_path_for_own).await)
    };

    // `dedicated_simulator` (ReplaySimulator) 可能用于特定的回放或分析任务，与主模拟池分开。
    // (`dedicated_simulator` (ReplaySimulator) might be used for specific replay or analysis tasks, separate from the main simulation pool.)
    info!("配置专用回放模拟器 (dedicated_simulator)..."); // 日志：配置dedicated_simulator
    let dedicated_simulator_instance: Option<Arc<ReplaySimulator>> = if args.db_sim_config.use_db_simulator {
        Some(Arc::new(
            ReplaySimulator::new_slow(
                &db_path,
                &config_path,
                Duration::from_millis(args.worker_config.dedicated_long_interval), // 长轮询间隔
                Duration::from_millis(args.worker_config.dedicated_short_interval),// 短轮询间隔
            )
            .await,
        ))
    } else {
        info!("未使用DBSimulator，因此不创建专用回放模拟器。"); // 日志：不创建回放模拟器
        None // 如果不用DBSimulator，则不创建ReplaySimulator (If not using DBSimulator, don't create ReplaySimulator)
    };

    info!("模拟器池和策略专用模拟器已初始化。模拟器池地址: {:p}", simulator_pool); // 打印对象池的指针地址以确认

    // --- 初始化并添加套利策略 (ArbStrategy) ---
    // (Initialize and add Arbitrage Strategy (ArbStrategy))
    info!("初始化ArbStrategy..."); // 日志：初始化ArbStrategy
    let arb_strategy = ArbStrategy::new(
        attacker,                           // 机器人操作者地址 (Bot operator address)
        simulator_pool,                     // 共享的模拟器对象池 (Shared simulator object pool)
        own_simulator_instance,             // 策略自身使用的模拟器 (Simulator for strategy's own use)
        args.worker_config.max_recent_arbs, // 最近套利机会的记忆参数 (Memory parameter for recent arbitrage opportunities)
        &rpc_url,                           // RPC URL
        args.worker_config.workers,         // 工作线程数 (Number of worker threads)
        dedicated_simulator_instance,       // (可选的) 专用回放模拟器 ((Optional) dedicated replay simulator)
    )
    .await;
    engine.add_strategy(Box::new(arb_strategy)); // 将策略添加到引擎 (Add strategy to the engine)
    info!("ArbStrategy已添加至引擎。"); // 日志：策略已添加

    // --- 添加Telegram通知执行器 ---
    // (Add Telegram notification executor)
    info!("配置Telegram通知执行器..."); // 日志：配置Telegram执行器
    engine.add_executor(map_executor!(
        TelegramMessageDispatcher::new_without_error_report(), // 创建一个不报告错误的TG调度器
                                                               // (Create a TG dispatcher that doesn't report errors)
        Action::NotifyViaTelegram // 将 Action::NotifyViaTelegram 映射给它 (Map Action::NotifyViaTelegram to it)
    ));
    info!("Telegram通知执行器已添加。"); // 日志：TG执行器已添加

    // 启动心跳服务，每30秒发送一次心跳信号，应用名称为 "sui-arb"。
    // (Start heartbeat service, sending a heartbeat signal every 30 seconds, application name "sui-arb".)
    heartbeat::start("sui-arb", Duration::from_secs(30));
    info!("心跳服务已启动，应用名: sui-arb, 间隔: 30s"); // 日志：心跳服务启动

    // --- 运行引擎 ---
    // (Run the engine)
    // `engine.run_and_join()` 会启动所有收集器、策略和执行器，并使它们开始工作。
    // (`engine.run_and_join()` will start all collectors, strategies, and executors, and make them begin working.)
    // 它会阻塞直到引擎停止或发生不可恢复的错误。
    // (It will block until the engine stops or an unrecoverable error occurs.)
    info!("机器人引擎即将启动... (Bot engine is about to start...)");
    engine.run_and_join().await.unwrap(); // unwrap() 如果引擎运行出错则panic (unwrap() will panic if the engine errors out)

    Ok(())
}

[end of bin/arb/src/start_bot.rs]
