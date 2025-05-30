// 该文件 `pool_ids.rs` 定义了一个命令行工具，主要有两个功能：
// 1. 生成一个包含Sui链上与DEX池及全局系统相关的对象ID列表的文件。
//    这个列表 (`pool_related_ids.txt`) 可能被 `DBSimulator` (数据库模拟器) 用来预加载这些对象到本地缓存中，
//    从而在模拟交易时减少对RPC节点的实际查询，提高模拟速度和效率。
// 2. 测试这个预加载的对象列表在模拟交易时的效果或正确性。
//
// 文件概览:
// - `Args` 结构体: 定义了此工具的命令行参数，如输出文件路径、RPC URL、测试模式开关、模拟参数等。
// - `supported_protocols()`: 返回一个包含所有当前支持的DEX协议的列表。
// - `run()`: 主函数，根据命令行参数执行相应操作。
//   - 如果不是测试模式，则会连接DEX索引器，获取所有支持协议的池信息及其相关对象ID，
//     结合一些已知的全局对象ID，并将它们写入到指定的输出文件中。
//   - 如果是测试模式 (`args.test` 为 true)，则调用 `test_pool_related_objects()`。
// - `global_ids()`: 返回一个包含Sui系统级全局对象ID（如框架包ID、时钟对象、系统状态对象等）
//   以及其他重要全局对象（如Wormhole）的ID集合。
// - `test_pool_related_objects()`: 一个测试函数，它会：
//   - 加载 `pool_related_ids.txt` 文件中的对象ID。
//   - (可选) 移除指定的某些对象ID（用于测试删除或排除特定对象的效果）。
//   - 使用这些对象ID创建一个 `SimulateCtx` (模拟上下文)，其中包含了这些预加载的对象。
//   - 使用 `Trader` 和给定的路径、输入金额等参数，在这个模拟上下文中执行一次交易模拟。
//   - 打印模拟结果，用于验证预加载对象是否按预期工作。
// - `pool_related_objects()`: 一个辅助函数，用于从指定的文件路径读取对象ID列表，
//   并通过模拟器获取这些对象的详细信息 (`ObjectReadResult`)，以便用于 `SimulateCtx`。
//
// Sui/DeFi概念:
// - Object ID (对象ID): Sui上每个对象的唯一标识符。
// - DBSimulator (数据库模拟器): 一种交易模拟器，它可能在本地维护一个Sui链状态的数据库副本。
//   通过预加载常用的对象到这个数据库中，可以避免在每次模拟时都通过RPC从链上获取这些对象。
// - SimulateCtx (模拟上下文): 在执行交易模拟时，提供给模拟器的上下文信息，
//   包括当前的纪元信息、Gas价格、以及一组需要覆盖或预设的链上对象状态 (`override_objects`)。
// - DexIndexer (DEX索引器): 一个外部服务或库，用于发现和索引不同DEX协议的池信息。
// - InputObjectKind: 在构建交易时，用于指定输入对象的类型（如共享可变对象、私有对象等）。
// - ObjectReadResult: 封装了读取一个对象的结果，包括其`InputObjectKind`和对象数据。

// 引入标准库及第三方库
use std::collections::HashSet; // 用于存储唯一的对象ID字符串
use std::fs;                   // 文件系统操作，如读写文件
use std::str::FromStr;         // 用于从字符串转换 (例如SuiAddress, ObjectID)
use std::sync::Arc;            // 原子引用计数

use clap::Parser; // `clap` crate，用于解析命令行参数
use dex_indexer::{types::Protocol, DexIndexer}; // DEX索引器客户端和协议类型
use eyre::Result; // `eyre`库，用于错误处理
use mev_logger::LevelFilter; // 日志级别过滤器 (来自自定义的 `mev_logger`)
use object_pool::ObjectPool; // 对象池，用于管理模拟器实例
use simulator::{DBSimulator, SimulateCtx, Simulator}; // 数据库模拟器、模拟上下文、模拟器trait
use std::fs::File; // 文件操作
use std::io::{BufRead, BufReader, BufWriter, Write}; // 带缓冲的读写器
use sui_sdk::types::{ // Sui SDK中定义的一些常量对象ID
    BRIDGE_PACKAGE_ID, DEEPBOOK_PACKAGE_ID, MOVE_STDLIB_PACKAGE_ID, SUI_AUTHENTICATOR_STATE_OBJECT_ID,
    SUI_BRIDGE_OBJECT_ID, SUI_CLOCK_OBJECT_ID, SUI_DENY_LIST_OBJECT_ID, SUI_FRAMEWORK_PACKAGE_ID,
    SUI_RANDOMNESS_STATE_OBJECT_ID, SUI_SYSTEM_PACKAGE_ID, SUI_SYSTEM_STATE_OBJECT_ID,
};
use sui_sdk::SuiClientBuilder; // Sui客户端构建器
use sui_types::base_types::{ObjectID, SuiAddress}; // Sui基本类型
use sui_types::object::{Object, Owner}; // Sui对象和所有者类型
use sui_types::transaction::{InputObjectKind, ObjectReadResult}; // Sui交易输入对象类型和对象读取结果
use tracing::info; // `tracing`库，用于日志记录

// 从当前crate的其他模块引入
use crate::common::get_latest_epoch; // 获取最新纪元信息的函数
use crate::defi::{DexSearcher, IndexerDexSearcher, TradeType, Trader}; // DeFi相关的trait和结构体
use crate::HttpConfig; // 通用的HTTP配置结构体 (在main.rs中定义)

/// `Args` 结构体
///
/// 定义了 `pool_ids` 子命令的命令行参数。
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// 输出文件的路径，用于存储收集到的对象ID列表。
    /// 默认值为 "./pool_related_ids.txt"。
    #[clap(long, default_value = "./pool_related_ids.txt")]
    pub result_path: String,

    /// HTTP相关的配置 (例如Sui RPC URL)。
    /// `#[command(flatten)]` 表示将 `HttpConfig` 中的字段直接作为当前命令的参数。
    #[command(flatten)]
    pub http_config: HttpConfig,

    /// 是否仅运行测试模式。
    /// 如果为true，则会执行 `test_pool_related_objects()` 函数。
    #[clap(long, help = "仅运行测试")]
    pub test: bool,

    /// (测试模式参数) 是否在模拟时启用回退机制 (fallback)。
    /// `DBSimulator::new_test(with_fallback)` 可能根据此参数有不同行为。
    #[clap(long, help = "模拟时启用回退")]
    pub with_fallback: bool,

    /// (测试模式参数) 模拟交易的输入金额。
    /// 默认值为 10,000,000 MIST (0.01 SUI)。
    #[clap(long, default_value = "10000000")]
    pub amount_in: u64,

    /// (测试模式参数) 用于测试的交易路径，由逗号分隔的对象ID组成。
    /// 例如: "pool_id1,pool_id2,pool_id3"
    #[clap(
        long,
        default_value = "0x3c3dd05e348fba5d8bf6958369cc3b33c8e8be85c96e10b1ca6413ad1b2d7787,0xe356c686eb19972e076b6906de12354a1a7ce1b09691416e9d852b04fd21b9a6,0xade90c3bc407eaa34068129d63bba5d1cf7889a2dbaabe5eb9b3efbbf53891ea,0xda49f921560e39f15d801493becf79d47c89fb6db81e0cbbe7bf6d3318117a00"
    )]
    pub path: String,

    /// (测试模式参数, 可选) 在模拟前需要从预加载对象列表中删除的对象ID，由逗号分隔。
    /// 用于测试排除某些对象对模拟结果的影响。
    #[clap(long, help = "模拟前删除的对象ID列表")]
    pub delete_objects: Option<String>,
}

/// `supported_protocols` 函数
///
/// 返回一个包含所有当前已集成的、需要为其收集对象ID的DEX协议的列表。
fn supported_protocols() -> Vec<Protocol> {
    vec![
        Protocol::Cetus,
        Protocol::Turbos,
        Protocol::KriyaAmm,
        Protocol::BlueMove,
        Protocol::KriyaClmm,
        Protocol::FlowxClmm,
        Protocol::Navi,       // Navi虽然是借贷协议，但其关键对象ID也可能需要预加载
        Protocol::Aftermath,
        // 注意：DeepBookV2 没有在这里列出，可能是因为它不通过常规的 `get_all_pools` 获取，
        // 或者其相关对象已包含在 `global_ids()` 中。
    ]
}

/// `run` 函数 (子命令的主入口)
///
/// 根据命令行参数执行操作：生成对象ID列表文件，或运行测试。
///
/// 参数:
/// - `args`: 解析后的命令行参数 (`Args`结构体)。
///
/// 返回:
/// - `Result<()>`: 如果成功则返回Ok，否则返回错误。
pub async fn run(args: Args) -> Result<()> {
    // 初始化日志系统
    mev_logger::init_console_logger_with_directives(
        Some(LevelFilter::INFO), // 设置默认日志级别为INFO
        &[ // 为特定模块设置更详细的日志级别 (用于调试)
            "arb=debug", // arb模块设为debug
            // "dex_indexer=warn",
            // "simulator=trace",
            // "sui_types=trace",
            // "sui_move_natives_latest=trace",
            // "sui_execution=warn",
        ],
    );

    // 如果指定了 `--test` 参数，则执行测试逻辑并返回。
    if args.test {
        return test_pool_related_objects(args).await;
    }

    // --- 生成对象ID列表文件的逻辑 ---
    let result_path = args.result_path; // 输出文件路径
    let rpc_url = args.http_config.rpc_url; // Sui RPC URL

    // 初始化DEX索引器客户端和数据库模拟器 (用于获取对象信息)
    let dex_indexer = DexIndexer::new(&rpc_url).await?;
    // `DBSimulator::new_default_slow()` 可能连接到一个持久化的数据库实例来获取对象数据。
    let simulator: Arc<dyn Simulator> = Arc::new(DBSimulator::new_default_slow().await);

    // 尝试删除已存在的旧结果文件 (如果存在)
    let _ = fs::remove_file(&result_path); // 忽略删除失败的错误
    // 创建新的结果文件
    let file = File::create(&result_path)?;
    let mut writer = BufWriter::new(file); // 使用带缓冲的写入器以提高效率

    // 加载已存在于文件中的ID (如果文件非空且可读)，以支持增量更新。
    // 注意：由于上面 `fs::remove_file` 的存在，这里通常会从一个空文件开始。
    // 如果希望是增量更新，则不应首先删除文件。
    let mut object_ids: HashSet<String> = match fs::read_to_string(&result_path) {
        Ok(contents) => contents.lines().map(|line| line.to_string()).collect(),
        Err(_) => HashSet::new(), // 如果文件不存在或读取失败，则从空集合开始
    };


    // 遍历所有支持的协议
    for protocol in supported_protocols() {
        // 添加与协议本身相关的对象ID (例如全局配置对象、工厂对象等)
        // `protocol.related_object_ids()` 是 `Protocol` 枚举的一个方法 (可能通过trait实现)
        object_ids.extend(protocol.related_object_ids().await?);

        // Navi的资金池不由 `dex_indexer` 的 `get_all_pools` 管理，其关键对象已在上面添加。
        if protocol == Protocol::Navi {
            continue;
        }

        // 获取该协议下的所有池
        if let Ok(pools) = dex_indexer.get_all_pools(&protocol) { // 修改：处理Result
            for pool in pools {
                // 添加与每个池相关的对象ID (例如池本身、LP代币对象等)
                // `pool.related_object_ids()` 是 `Pool` 结构体的一个方法
                object_ids.extend(pool.related_object_ids(Arc::clone(&simulator)).await);
            }
        } else {
            // 如果获取某协议的池失败，可以记录一个警告或错误
            tracing::warn!("未能获取协议 {:?} 的池列表", protocol);
        }
    }

    // 添加所有全局系统对象ID
    object_ids.extend(global_ids());

    // 将所有收集到的唯一对象ID写入文件，每行一个。
    let all_ids_vec: Vec<String> = object_ids.into_iter().collect(); // HashSet转为Vec以排序或稳定输出(可选)
                                                                    // 如果需要稳定输出顺序，可以在这里排序: all_ids_vec.sort();
    writeln!(writer, "{}", all_ids_vec.join("\n"))?; // 用换行符连接所有ID并写入

    writer.flush()?; //确保所有缓冲内容都写入文件

    info!("🎉 成功将池及相关对象ID写入到 {}", result_path);

    Ok(())
}

/// `global_ids` 函数
///
/// 返回一个包含Sui系统级全局对象ID和一些其他重要全局对象ID的集合。
/// 这些ID通常是固定的或广为人知的。
fn global_ids() -> HashSet<String> {
    // Sui系统框架和核心对象的ID (从sui_sdk::types导入的常量)
    let mut result_set = vec![
        MOVE_STDLIB_PACKAGE_ID,        // Move标准库包ID ("0x1")
        SUI_FRAMEWORK_PACKAGE_ID,      // Sui框架包ID ("0x2")
        SUI_SYSTEM_PACKAGE_ID,         // Sui系统包ID ("0x3")
        BRIDGE_PACKAGE_ID,             // Sui桥接相关包ID (可能指Wormhole或其他官方桥)
        DEEPBOOK_PACKAGE_ID,           // DeepBook包ID
        SUI_SYSTEM_STATE_OBJECT_ID,    // Sui系统状态对象ID ("0x5")
        SUI_CLOCK_OBJECT_ID,           // 时钟对象ID ("0x6")
        SUI_AUTHENTICATOR_STATE_OBJECT_ID, // 认证器状态对象ID ("0x7")
        SUI_RANDOMNESS_STATE_OBJECT_ID,  // 随机数状态对象ID ("0x8")
        SUI_BRIDGE_OBJECT_ID,          // Sui桥对象ID
        SUI_DENY_LIST_OBJECT_ID,       // Sui拒绝列表对象ID (用于封禁等)
    ]
    .into_iter()
    .map(|id| id.to_string()) // 将ObjectID常量转换为String
    .collect::<HashSet<String>>();

    // 添加其他已知的全局重要对象的ID
    // 例如，Wormhole核心状态对象等
    result_set.insert("0x5306f64e312b581766351c07af79c72fcb1cd25147157fdc2f8ad76de9a3fb6a".to_string()); // Wormhole 主状态对象 (示例)
    result_set.insert("0x26efee2b51c911237888e5dc6702868abca3c7ac12c53f76ef8eba0697695e3d".to_string()); // 可能是另一个Wormhole相关对象

    result_set
}

/// `test_pool_related_objects` 异步函数 (测试模式的主逻辑)
///
/// 该函数用于测试预加载的对象列表在实际交易模拟中的效果。
///
/// 步骤:
/// 1. 定义测试参数 (发送者地址、输入金额、交易路径等)。
/// 2. 初始化 `IndexerDexSearcher` 和 `Trader`。
/// 3. 从 `args.result_path` (通常是 `pool_related_ids.txt`) 加载对象ID列表，
///    并获取这些对象的 `ObjectReadResult` (包含对象数据和元数据)。
/// 4. (可选) 根据 `args.delete_objects` 从预加载列表中移除某些对象。
/// 5. 使用这些预加载对象创建一个 `SimulateCtx`。
/// 6. 调用 `Trader::get_trade_result` 在此上下文中模拟一笔闪电贷交易。
/// 7. 打印模拟结果。
async fn test_pool_related_objects(args: Args) -> Result<()> {
    // --- 测试数据定义 ---
    let sender = SuiAddress::from_str("0xac5bceec1b789ff840d7d4e6ce4ce61c90d190a7f8c4f4ddf0bff6ee2413c33c").unwrap(); // 一个固定的测试发送者地址
    let amount_in = args.amount_in; // 从命令行参数获取输入金额

    // 从命令行参数解析交易路径 (逗号分隔的ObjectID字符串)
    let path_obj_ids = args
        .path
        .split(',')
        .map(|obj_id_str| ObjectID::from_hex_literal(obj_id_str).unwrap())
        .collect::<Vec<_>>();

    let with_fallback = args.with_fallback; // 是否启用模拟器回退
    let rpc_url = args.http_config.rpc_url.clone(); // RPC URL

    // 创建模拟器对象池 (用于初始化Trader和IndexerDexSearcher)
    // `DBSimulator::new_test(with_fallback)` 创建一个测试用的数据库模拟器。
    let simulator_pool = Arc::new(ObjectPool::new(1, move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { Box::new(DBSimulator::new_test(with_fallback).await) as Box<dyn Simulator> })
    }));

    // 初始化DEX搜索器，并根据对象ID路径构建实际的交易路径 (`Path` 对象)
    let dex_searcher: Arc<dyn DexSearcher> = Arc::new(IndexerDexSearcher::new(&rpc_url, Arc::clone(&simulator_pool)).await?);
    let trade_path = dex_searcher.find_test_path(&path_obj_ids).await?;
    info!(?with_fallback, ?amount_in, ?trade_path, ?args.delete_objects, "测试数据初始化完毕");
    // --- 测试数据定义结束 ---

    // 创建Sui客户端并获取最新纪元信息 (用于模拟上下文)
    let sui_client = SuiClientBuilder::default().build(&rpc_url).await?;
    let epoch_info = get_latest_epoch(&sui_client).await?;

    // 加载 `pool_related_ids.txt` 文件中的对象作为预加载对象。
    let mut override_objects_for_sim = pool_related_objects(&args.result_path).await?;
    // 如果命令行指定了要删除的对象ID，则从预加载列表中移除它们。
    if let Some(delete_object_ids_str) = args.delete_objects {
        let delete_obj_ids_vec = delete_object_ids_str
            .split(',')
            .map(|obj_id_str| ObjectID::from_hex_literal(obj_id_str).unwrap())
            .collect::<Vec<_>>();
        // `retain` 方法保留使闭包返回true的元素。
        // 这里保留那些ID不在 `delete_obj_ids_vec` 中的对象。
        override_objects_for_sim.retain(|obj_read_result| {
            !delete_obj_ids_vec.contains(&obj_read_result.object_id())
        });
    }

    // 使用预加载对象创建模拟上下文。
    let sim_ctx = SimulateCtx::new(epoch_info, override_objects_for_sim);

    // 初始化Trader并执行交易模拟。
    let trader = Trader::new(simulator_pool).await?;
    let trade_result = trader
        .get_trade_result(&trade_path, sender, amount_in, TradeType::Flashloan, vec![], sim_ctx) // Gas币列表为空vec![]，因为DBSimulator可能不严格检查Gas对象
        .await?;
    info!(?trade_result, "交易模拟结果");

    Ok(())
}

/// `pool_related_objects` 异步辅助函数
///
/// 从指定的文件路径读取对象ID列表，并通过模拟器获取这些对象的 `ObjectReadResult`。
/// `ObjectReadResult` 包含了对象的元数据和数据，可以直接用于填充 `SimulateCtx` 的 `override_objects`。
///
/// 参数:
/// - `file_path`: 包含对象ID列表的文件的路径字符串。
///
/// 返回:
/// - `Result<Vec<ObjectReadResult>>`: 包含所有成功获取的对象信息的向量。
async fn pool_related_objects(file_path: &str) -> Result<Vec<ObjectReadResult>> {
    // 创建一个临时的DBSimulator实例，用于获取对象数据。
    // `new_test(true)` 可能表示使用一个轻量级的、带回退的测试模拟器。
    let simulator: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);
    let file = File::open(file_path)?; // 打开文件
    let reader = BufReader::new(file); // 创建带缓冲的读取器

    let mut results_vec = vec![];
    for line_result in reader.lines() { // 逐行读取文件
        let line_str = line_result?; // 处理可能的IO错误
        let object_id = ObjectID::from_hex_literal(&line_str)?; // 将行内容解析为ObjectID

        // 通过模拟器获取对象数据
        let object_data: Object = if let Some(obj) = simulator.get_object(&object_id).await {
            obj
        } else {
            // 如果模拟器中找不到该对象 (例如，它在链上已被删除或ID无效)，则跳过。
            tracing::warn!("对象ID {} 在模拟器中未找到，已跳过。", object_id);
            continue;
        };

        // 根据对象的所有者类型，确定其 `InputObjectKind`。
        // 这对于构建交易或在模拟器中正确表示对象是必要的。
        let input_object_kind = match object_data.owner() {
            Owner::Shared { initial_shared_version } => InputObjectKind::SharedMoveObject {
                id: object_id,
                initial_shared_version: *initial_shared_version,
                mutable: true, // 假设预加载的共享对象在模拟中可能是可变的
            },
            _ => InputObjectKind::ImmOrOwnedMoveObject(object_data.compute_object_reference()), // 对于私有对象或不可变对象
        };

        // 将 `InputObjectKind` 和对象数据 (`object_data`) 包装成 `ObjectReadResult`。
        // `object_data.into()` 可能会将其转换为 `SuiObjectData`。
        results_vec.push(ObjectReadResult::new(input_object_kind, object_data.into()));
    }

    Ok(results_vec)
}
