// 该文件 `start_bot.rs` 是套利机器人程序的主要启动和运行逻辑。
// 它负责初始化所有必要的组件，如事件收集器、交易执行器、模拟器和核心套利策略，
// 然后使用一个引擎（可能是`burberry::Engine`）来驱动这些组件协同工作。
//
// 文件概览:
// 1. `Args` 结构体: 定义了启动机器人所需的命令行参数，包括私钥、RPC URL、各种组件的配置等。
// 2. `CollectorConfig`, `DbSimConfig`, `WorkerConfig`: 分别定义了收集器、数据库模拟器和工作线程相关的配置参数。
// 3. `run()` 函数:
//    - 初始化日志系统和panic钩子。
//    - 解析私钥，获取攻击者（机器人操作者）的Sui地址。
//    - 根据配置初始化各种事件收集器 (Collectors):
//      - `ShioCollector`: 用于从Shio协议（MEV竞价）收集事件。
//      - `PublicTxCollector`: 用于从本地套接字收集公开的Sui交易。
//      - `PrivateTxCollector`: 用于从WebSocket收集私有的中继交易。
//    - 初始化各种执行器 (Executors):
//      - `ShioExecutor` / `ShioRPCExecutor`: 用于向Shio协议提交竞价。
//      - `PublicTxExecutor`: 用于执行公开的Sui交易。
//      - `TelegramMessageDispatcher`: 用于通过Telegram发送通知。
//    - 初始化模拟器 (`Simulator`):
//      - 可以选择使用 `DBSimulator` (基于本地数据库，性能较高) 或 `HttpSimulator` (基于RPC，已弃用，主要用于测试)。
//      - `DBSimulator` 需要数据库路径、全节点配置路径、缓存更新套接字和预加载对象列表。
//      - `ReplaySimulator`: 一种特殊的模拟器，可能用于回放和分析历史数据或特定场景。
//    - 初始化核心套利策略 (`ArbStrategy`):
//      - 传入攻击者地址、模拟器池、RPC URL、工作线程数等配置。
//    - 将所有收集器、执行器和策略添加到 `burberry::Engine` 实例中。
//    - 启动一个心跳服务 (`heartbeat::start`)，用于监控机器人健康状况。
//    - 调用 `engine.run_and_join()` 启动并运行整个机器人引擎。
//
// Sui/MEV/Bot概念:
// - Event Collector (事件收集器): 负责从不同来源（如链上事件、WebSocket流、IPC套接字）监听并收集相关事件（如新交易、价格更新等）。
// - Executor (执行器): 负责执行某种类型的动作（如提交交易、发送通知）。
// - Simulator (模拟器): 用于在实际执行交易前，模拟交易在当前链状态下的执行结果，以预测利润、Gas消耗等。
//   - DBSimulator: 使用本地数据库副本进行模拟，速度快。
//   - HttpSimulator: 通过RPC调用远程节点进行模拟，速度慢，通常仅用于测试。
//   - ReplaySimulator: 可能用于回放历史交易或在特定状态下进行模拟。
// - Strategy (策略): 套利机器人的核心逻辑，定义了如何分析收集到的事件、发现套利机会、构建交易并决定是否执行。
// - Engine (引擎): 一个驱动框架 (如 `burberry::Engine`)，负责协调收集器、策略和执行器的工作流程。
//   通常是事件驱动的：收集器产生事件 -> 引擎将事件分发给策略 -> 策略分析并产生动作 -> 引擎将动作分发给执行器。
// - Heartbeat (心跳): 一个定期发送的信号，用于表明程序仍在正常运行，常用于监控系统。
// - Shio Protocol: 一个用于MEV竞价的协议，机器人可以通过它提交出价以获得交易优先权。
// - Relay WebSocket: 一个WebSocket服务，用于私下广播交易，常见于MEV场景，避免交易被公开内存池中的机器人抢先。

// 引入标准库及第三方库
use std::{
    sync::Arc, // 原子引用计数
    time::{Duration, Instant}, // 时间处理
};

use ::utils::heartbeat; // 从外部 `utils` crate 引入心跳服务
use burberry::{executor::telegram_message::TelegramMessageDispatcher, map_collector, map_executor, Engine}; // `burberry`引擎框架相关组件
use clap::Parser; // `clap` crate，用于解析命令行参数
use eyre::Result; // `eyre`库，用于错误处理
use object_pool::ObjectPool; // 对象池，用于管理模拟器实例
use shio::{new_shio_collector_and_executor, ShioRPCExecutor}; // `shio` crate，用于与Shio MEV协议交互
use simulator::{DBSimulator, HttpSimulator, ReplaySimulator, Simulator}; // 各种模拟器实现
use sui_types::{base_types::SuiAddress, crypto::SuiKeyPair}; // Sui基本类型和加密类型
use tracing::{info, warn}; // `tracing`库，用于日志记录

// 从当前crate的其他模块引入
use crate::{
    collector::{PrivateTxCollector, PublicTxCollector}, // 各种事件收集器
    executor::PublicTxExecutor,                         // 公开交易执行器
    strategy::ArbStrategy,                              // 套利策略核心逻辑
    types::{Action, Event},                              // 自定义的Action和Event枚举
    HttpConfig,                                         // 通用的HTTP配置 (在main.rs中定义)
};

/// `Args` 结构体 (启动机器人子命令的参数)
///
/// 定义了通过命令行启动套利机器人时可以接受的参数。
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// Sui账户的私钥字符串 (Base64编码)。
    /// 用于签名交易。会从环境变量 `SUI_PRIVATE_KEY` 读取。
    #[arg(long, env = "SUI_PRIVATE_KEY")]
    pub private_key: String,

    /// Shio执行器是否使用RPC提交竞价。
    /// 如果为false，可能使用WebSocket或其他方式。
    #[arg(long, help = "Shio执行器使用RPC提交竞价")]
    pub shio_use_rpc: bool,

    /// HTTP相关的配置 (例如Sui RPC URL)。
    #[command(flatten)]
    pub http_config: HttpConfig,

    /// 收集器相关的配置。
    #[command(flatten)]
    collector_config: CollectorConfig,

    /// 数据库模拟器相关的配置。
    #[command(flatten)]
    db_sim_config: DbSimConfig,

    /// 工作线程和内部队列相关的配置。
    #[command(flatten)]
    worker_config: WorkerConfig,
}

/// `CollectorConfig` 结构体 (收集器配置参数)
#[derive(Clone, Debug, Parser)]
struct CollectorConfig {
    /// 中继交易收集器的WebSocket URL (可选)。
    /// 用于接收私下广播的交易。如果提供，则会启用 `PrivateTxCollector`。
    /// 通常与 `public_tx_collector` (通过tx_socket_path) 互斥。
    #[arg(long)]
    pub relay_ws_url: Option<String>,

    /// Shio事件收集器的WebSocket URL (可选)。
    /// 用于从Shio协议接收事件（如新的拍卖轮次、高出价等）。
    #[arg(long)]
    pub shio_ws_url: Option<String>,

    /// 公开交易收集器的本地IPC套接字路径。
    /// 用于从本地运行的Sui节点（或类似服务）接收所有新提交的交易。
    /// 默认值为 "/tmp/sui_tx.sock"。会从环境变量 `SUI_TX_SOCKET_PATH` 读取。
    #[arg(long, env = "SUI_TX_SOCKET_PATH", default_value = "/tmp/sui_tx.sock")]
    pub tx_socket_path: String,
}

/// `DbSimConfig` 结构体 (数据库模拟器配置参数)
#[derive(Clone, Debug, Parser)]
struct DbSimConfig {
    /// 数据库模拟器所需的Sui数据库路径。
    /// 默认值为 "/home/ubuntu/sui/db/live/store"。会从环境变量 `SUI_DB_PATH` 读取。
    #[arg(long, env = "SUI_DB_PATH", default_value = "/home/ubuntu/sui/db/live/store")]
    pub db_path: String,

    /// Sui全节点配置文件的路径 (例如 fullnode.yaml)。
    /// 数据库模拟器可能需要此文件来获取网络配置或其他信息。
    /// 默认值为 "/home/ubuntu/sui/fullnode.yaml"。会从环境变量 `SUI_CONFIG_PATH` 读取。
    #[arg(long, env = "SUI_CONFIG_PATH", default_value = "/home/ubuntu/sui/fullnode.yaml")]
    pub config_path: String,

    /// 数据库模拟器监听此套接字以接收来自Sui节点的缓存更新。
    /// Sui节点在对象状态变化时，会通过此套接字通知模拟器更新其本地缓存。
    /// 默认值为 "/tmp/sui_cache_updates.sock"。会从环境变量 `SUI_UPDATE_CACHE_SOCKET` 读取。
    #[arg(long, env = "SUI_UPDATE_CACHE_SOCKET", default_value = "/tmp/sui_cache_updates.sock")]
    pub update_cache_socket: String,

    /// 包含预加载对象ID列表的文件的路径。
    /// 数据库模拟器会预加载这些对象到其缓存中。
    /// 默认值为 "/home/ubuntu/suiflow-relay/pool_related_ids.txt"。会从环境变量 `SUI_PRELOAD_PATH` 读取。
    #[arg(
        long,
        env = "SUI_PRELOAD_PATH",
        default_value = "/home/ubuntu/suiflow-relay/pool_related_ids.txt"
    )]
    pub preload_path: String,

    /// 是否使用数据库模拟器。
    /// 如果为false，则可能使用HttpSimulator。
    #[arg(long, default_value_t = false)]
    pub use_db_simulator: bool,

    /// (DBSimulator) 数据库追赶（catchup）操作的间隔时间（秒）。
    /// DBSimulator可能需要定期从链上同步最新的状态以避免数据陈旧。
    #[arg(long, default_value_t = 60)]
    pub catchup_interval: u64,
}

/// `WorkerConfig` 结构体 (工作组件配置参数)
#[derive(Clone, Debug, Parser)]
struct WorkerConfig {
    /// 处理事件（公开交易、私有交易、Shio事件）的工作线程数量。
    /// 8个通常足够。
    #[arg(long, default_value_t = 8)]
    pub workers: usize,

    /// 模拟器对象池中的模拟器实例数量。
    #[arg(long, default_value_t = 32)]
    pub num_simulators: usize,

    /// 最近已处理套利机会的最大数量。
    /// 如果一个新的机会与最近 `max_recent_arbs` 次已处理的机会相似（例如涉及相同的代币或池），
    /// 则可能会被忽略，以避免重复或过于频繁地处理相同的机会。
    #[arg(long, default_value_t = 20)]
    pub max_recent_arbs: usize,

    /// (ReplaySimulator) 专用回放模拟器的短间隔和长间隔（毫秒）。
    /// 短间隔，例如50毫秒。
    #[arg(long, default_value_t = 50)]
    pub dedicated_short_interval: u64,

    /// 长间隔，例如200毫秒。
    #[arg(long, default_value_t = 200)]
    pub dedicated_long_interval: u64,
}

/// `run` 函数 (启动机器人子命令的主入口)
///
/// 初始化并运行套利机器人。
///
/// 参数:
/// - `args`: 解析后的命令行参数 (`Args`结构体)。
///
/// 返回:
/// - `Result<()>`: 如果成功则返回Ok，否则返回错误。
pub async fn run(args: Args) -> Result<()> {
    // 设置自定义的panic钩子，可能用于在程序崩溃时记录更详细的信息或执行清理操作。
    utils::set_panic_hook();
    // 初始化日志系统，使用白名单模块进行过滤。
    // "mainnet"可能是环境标识，"sui-arb"是应用名称。
    // "arb", "utils", "shio" 等模块会以默认级别记录，"cache_metrics=debug" 表示cache_metrics模块以debug级别记录。
    mev_logger::init_with_whitelisted_modules(
        "mainnet",
        "sui-arb".to_string(),
        &["arb", "utils", "shio", "cache_metrics=debug"],
    );

    // 从提供的私钥字符串解码Sui密钥对。
    let keypair = SuiKeyPair::decode(&args.private_key)?;
    // 从密钥对获取公钥。
    let pubkey = keypair.public();
    // 从公钥派生Sui地址，作为机器人的操作地址（攻击者地址）。
    let attacker = SuiAddress::from(&pubkey);

    // 记录启动信息，包含关键配置。
    info!(
        "启动套利机器人 (start_bot)，攻击者地址: {}, HTTP配置: {:#?}, 收集器配置: {:#?}, DB模拟器配置: {:#?}, Worker配置: {:#?}",
        attacker, args.http_config, args.collector_config, args.db_sim_config, args.worker_config
    );

    // --- 初始化引擎和组件 ---
    let rpc_url = args.http_config.rpc_url.clone(); // RPC URL的克隆，因为后续会多次使用
    let db_path = args.db_sim_config.db_path.clone();
    let tx_socket_path = args.collector_config.tx_socket_path.clone();
    let config_path = args.db_sim_config.config_path.clone();
    let update_cache_socket = args.db_sim_config.update_cache_socket.clone();
    let preload_path = args.db_sim_config.preload_path.clone();

    // 创建 `burberry::Engine` 实例，用于管理和运行所有组件。
    let mut engine = Engine::default();

    // --- 配置收集器 (Collectors) ---
    // 如果配置了Shio WebSocket URL，则初始化Shio收集器和执行器。
    if let Some(ref ws_url) = args.collector_config.shio_ws_url {
        let (shio_collector, shio_executor) =
            new_shio_collector_and_executor(keypair.copy(), Some(ws_url.clone()), None).await; // keypair.copy()因为后续可能还用
        // `map_collector!` 是一个宏，用于将具体收集器的输出事件映射为引擎通用的 `Event` 枚举类型。
        // 这里将 `shio_collector` 产生的事件映射为 `Event::Shio`。
        engine.add_collector(map_collector!(shio_collector, Event::Shio));

        // 根据配置选择Shio执行器类型 (RPC或WebSocket)。
        if args.shio_use_rpc {
            let shio_rpc_executor = ShioRPCExecutor::new(SuiKeyPair::decode(&args.private_key)?);
            // `map_executor!` 将引擎通用的 `Action` 枚举类型映射为具体执行器能处理的动作。
            // 这里将 `Action::ShioSubmitBid` 映射给 `shio_rpc_executor`。
            engine.add_executor(map_executor!(shio_rpc_executor, Action::ShioSubmitBid));
        } else {
            engine.add_executor(map_executor!(shio_executor, Action::ShioSubmitBid));
        }
    } else {
        // 如果没有配置Shio，则默认使用公开交易收集器。
        let public_tx_collector = PublicTxCollector::new(&tx_socket_path);
        engine.add_collector(Box::new(public_tx_collector)); // PublicTxCollector直接产生 Event::PublicTx
    }

    // 添加公开交易执行器。
    engine.add_executor(map_executor!(
        PublicTxExecutor::new(&rpc_url, SuiKeyPair::decode(&args.private_key)?).await?, // 注意每次都解码私钥
        Action::ExecutePublicTx
    ));

    // 如果配置了中继WebSocket URL，则初始化私有交易收集器。
    if let Some(ref relay_ws_url) = args.collector_config.relay_ws_url {
        let private_tx_collector = PrivateTxCollector::new(relay_ws_url);
        engine.add_collector(Box::new(private_tx_collector)); // PrivateTxCollector直接产生 Event::PrivateTx
    }

    // --- 配置模拟器对象池 (Simulator Pool) ---
    // 根据配置选择使用DBSimulator还是HttpSimulator。
    let simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>> = Arc::new(match args.db_sim_config.use_db_simulator {
        true => {
            // 使用DBSimulator
            let db_path_clone = db_path.clone(); // 克隆String以移动到闭包
            let config_path_clone = config_path.clone();
            let update_cache_socket_clone = update_cache_socket.clone();
            let preload_path_clone = preload_path.clone();
            ObjectPool::new(args.worker_config.num_simulators, move || {
                // 在新的tokio运行时中同步地执行异步的模拟器初始化。
                // 这是因为ObjectPool的初始化函数是同步的。
                tokio::runtime::Runtime::new().unwrap().block_on(async {
                    let start_time = Instant::now();
                    let simulator = Box::new(
                        DBSimulator::new_slow(
                            &db_path_clone,
                            &config_path_clone,
                            Some(&update_cache_socket_clone),
                            Some(&preload_path_clone),
                        )
                        .await,
                    ) as Box<dyn Simulator>;
                    info!(elapsed = ?start_time.elapsed(), "DBSimulator实例已初始化");
                    simulator
                })
            })
        }
        false => {
            // 使用HttpSimulator (已弃用，仅用于测试)
            warn!("HttpSimulator已弃用，请仅用于测试目的。");
            let rpc_url_clone = rpc_url.clone();
            let ipc_path_clone = args.http_config.ipc_path.clone(); // ipc_path也是HttpConfig的一部分

            ObjectPool::new(args.worker_config.num_simulators, move || {
                let rpc_url_inner_clone = rpc_url_clone.clone(); // 再次克隆以移动到更内层的异步块
                let ipc_path_inner_clone = ipc_path_clone.clone();
                tokio::runtime::Runtime::new().unwrap().block_on(async move {
                    Box::new(HttpSimulator::new(&rpc_url_inner_clone, &ipc_path_inner_clone).await)
                        as Box<dyn Simulator>
                })
            })
        }
    });

    // `Arc::new` 用于将 `ObjectPool` 包裹起来，如果 `simulator_pool` 需要被 `ArbStrategy` 等多个地方共享。
    // 如果 `ObjectPool` 本身已经是 `Arc` 或者其 `get()` 方法返回 `Arc` 包装的实例，则可能不需要再包一层。
    // 从 `ArbStrategy::new` 的签名看，它需要 `Arc<ObjectPool<...>>`。

    // --- 配置策略自身使用的模拟器 (own_simulator) 和专用回放模拟器 (dedicated_simulator) ---
    // `own_simulator` 可能用于策略内部的一些特定、不适合从池中获取模拟器的场景。
    let own_simulator_instance: Arc<dyn Simulator> = if args.db_sim_config.use_db_simulator {
        Arc::new(DBSimulator::new_slow(&db_path, &config_path, Some(&update_cache_socket), Some(&preload_path)).await)
    } else {
        warn!("HttpSimulator已弃用，策略自身模拟器也将使用它。");
        let ipc_path_for_own = args.http_config.ipc_path.clone();
        Arc::new(HttpSimulator::new(&rpc_url, &ipc_path_for_own).await)
    };

    // `dedicated_simulator` (ReplaySimulator) 可能用于特定的回放或分析任务，与主模拟池分开。
    let dedicated_simulator_instance: Option<Arc<ReplaySimulator>> = if args.db_sim_config.use_db_simulator {
        Some(Arc::new(
            ReplaySimulator::new_slow(
                &db_path,
                &config_path,
                Duration::from_millis(args.worker_config.dedicated_long_interval),
                Duration::from_millis(args.worker_config.dedicated_short_interval),
            )
            .await,
        ))
    } else {
        None // 如果不用DBSimulator，则不创建ReplaySimulator
    };

    info!("模拟器池已初始化: {:?}", simulator_pool); // 打印对象池的Debug信息 (可能只是其内部结构)

    // --- 初始化并添加套利策略 (ArbStrategy) ---
    let arb_strategy = ArbStrategy::new(
        attacker,                           // 机器人操作者地址
        simulator_pool,                     // 共享的模拟器对象池
        own_simulator_instance,             // 策略自身使用的模拟器
        args.worker_config.max_recent_arbs, // 最近套利机会的记忆参数
        &rpc_url,                           // RPC URL
        args.worker_config.workers,         // 工作线程数
        dedicated_simulator_instance,       // (可选的) 专用回放模拟器
    )
    .await;
    engine.add_strategy(Box::new(arb_strategy)); // 将策略添加到引擎

    // --- 添加Telegram通知执行器 ---
    engine.add_executor(map_executor!(
        TelegramMessageDispatcher::new_without_error_report(), // 创建一个不报告错误的TG调度器
        Action::NotifyViaTelegram // 将 Action::NotifyViaTelegram 映射给它
    ));

    // 启动心跳服务，每30秒发送一次心跳信号，应用名称为 "sui-arb"。
    heartbeat::start("sui-arb", Duration::from_secs(30));

    // --- 运行引擎 ---
    // `engine.run_and_join()` 会启动所有收集器、策略和执行器，并使它们开始工作。
    // 它会阻塞直到引擎停止或发生不可恢复的错误。
    info!("机器人引擎即将启动...");
    engine.run_and_join().await.unwrap(); // unwrap() 如果引擎运行出错则panic

    Ok(())
}
