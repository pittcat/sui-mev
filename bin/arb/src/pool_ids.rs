// 该文件 `pool_ids.rs` 定义了一个命令行工具，主要有两个功能：
// 1. 生成一个包含Sui链上与DEX池及全局系统相关的对象ID列表的文件。
//    这个列表 (`pool_related_ids.txt`) 可能被 `DBSimulator` (数据库模拟器) 用来预加载这些对象到本地缓存中，
//    从而在模拟交易时减少对RPC节点的实际查询，提高模拟速度和效率。
// 2. 测试这个预加载的对象列表在模拟交易时的效果或正确性。
//
// **文件概览 (File Overview)**:
// 这个 `pool_ids.rs` 文件就像一个“信息收集和测试员”。它的主要工作和Sui区块链上的“对象ID”打交道。
// “对象ID”是Sui上每一个数字资产或智能合约的唯一身份证号码。
//
// 主要功能 (Main Functions):
// 1.  **生成对象ID列表文件 (Generate Object ID List File)**:
//     -   它会去查找Sui网络上所有已知的去中心化交易所（DEX）的“交易池”（Pools）以及一些重要的系统级对象（比如Sui的时钟）。
//     -   然后，它会把这些对象的“身份证号码”（ID）收集起来，写到一个叫做 `pool_related_ids.txt` (或者用户指定其他名字) 的文本文件里，每行一个ID。
//     -   **为什么要做这个？** 有一个叫做 `DBSimulator`（数据库模拟器）的工具，它可以在你的电脑上模拟Sui网络的交易。
//         如果预先把这些常用对象的ID和信息“喂”给 `DBSimulator`，它在模拟交易时就不用每次都去真的Sui网络上查询这些对象了，这样模拟会快很多。
//
// 2.  **测试预加载的对象列表 (Test Preloaded Object List)**:
//     -   这个文件还能测试上面生成的 `pool_related_ids.txt` 文件是不是真的管用。
//     -   它会读取这个文件里的对象ID，用这些ID来“配置”一个模拟环境，然后在这个环境里跑一个模拟交易。
//     -   最后看看模拟交易的结果对不对，以此来验证预加载这些对象是否达到了预期的效果。
//
// **结构解析 (Structure Breakdown)**:
// -   `Args` 结构体: 定义了这个工具运行时可以接受的命令行指令和参数。比如，你可以通过命令行告诉它：
//     -   生成的ID列表文件要保存在哪里 (`result_path`)。
//     -   要连接哪个Sui RPC节点 (`http_config`)。
//     -   是不是要进入“测试模式” (`test`)。
//     -   测试时用多少钱去模拟交易 (`amount_in`)，走哪条交易路径 (`path`) 等。
// -   `supported_protocols()` 函数: 返回一个列表，里面是这个工具认识的所有DEX协议的名字（比如Cetus, Turbos等）。
// -   `run()` 函数: 这是这个工具的“总指挥”，它根据你给的命令行参数来决定是去生成ID列表文件，还是去运行测试。
// -   `global_ids()` 函数: 返回一个列表，里面是一些Sui系统级别的、非常重要的对象的ID，比如Sui框架本身的ID、时钟对象ID等。这些ID通常是固定不变的。
// -   `test_pool_related_objects()` 函数: 如果你用 `--test` 参数运行工具，这个函数就会被调用。它负责上面说的测试预加载列表的逻辑。
// -   `pool_related_objects()` 函数: 一个辅助函数，负责从指定的文件（比如 `pool_related_ids.txt`）里读取对象ID，并获取这些对象的详细信息，以便用于模拟环境。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
//
// -   **Object ID (对象ID)**:
//     在Sui区块链上，一切皆对象（Object）。一个代币、一个NFT、一个智能合约（包括DEX的交易池）都是一个对象，每个对象都有一个全局唯一的ID作为其身份标识。
//     这个ID通常是一个十六进制字符串，例如 `0x123abc...`。
//
// -   **DBSimulator (数据库模拟器)**:
//     这是一个非常重要的工具，尤其对于开发和测试套利机器人而言。它在本地计算机上模拟Sui区块链的行为。
//     -   **作用**：允许开发者在不实际花费Gas费、不影响真实网络的情况下，测试交易逻辑、智能合约交互等。
//     -   **预加载 (Preloading)**：`DBSimulator` 可以预先加载一些常用的链上对象（比如重要的DEX池、代币合约等）到它的本地数据库缓存中。
//         这样，当模拟交易需要访问这些对象时，模拟器可以直接从本地缓存读取，而不需要通过RPC（远程过程调用）去真实的Sui网络节点查询。
//         这大大加快了模拟速度，对于需要大量模拟测试的套利策略开发至关重要。
//         这个 `pool_ids.rs` 工具生成的ID列表文件，就是为了给 `DBSimulator` 提供这个“预加载清单”。
//
// -   **SimulateCtx (模拟上下文 / Simulate Context)**:
//     当你要用模拟器（比如 `DBSimulator`）执行一笔模拟交易时，你需要提供一个“模拟上下文”。
//     这个上下文告诉模拟器当前模拟的环境是怎样的，它通常包含：
//     -   **当前纪元信息 (Epoch Information)**：比如当前的Sui网络纪元号、Gas价格等。
//     -   **预加载/覆盖的对象 (Override Objects)**：这是一组你希望在模拟中使用的特定版本的链上对象。
//         `pool_related_objects()` 函数获取的对象信息就会被用在这里，告诉模拟器：“在这次模拟中，对于这些ID的对象，请使用我提供给你的这些状态数据，而不是去链上查。”
//
// -   **DexIndexer (DEX索引器 / DEX Indexer)**:
//     一个外部的服务或库（这里是 `dex_indexer` crate）。它的作用是扫描Sui区块链，发现并索引（编目）各种去中心化交易所（DEX）的交易池信息。
//     比如，它可以告诉你Cetus上有哪些交易池，每个池里有什么代币，池的ID是什么等等。
//     这个 `pool_ids.rs` 工具会用到 `DexIndexer` 来获取各个DEX协议下的所有池的ID。
//
// -   **InputObjectKind (输入对象类型)**:
//     在Sui上构建一笔交易时，你需要指定交易中用到的每个输入对象（Input Object）的类型。这有助于Sui网络正确地处理这些对象。
//     常见的类型有：
//     -   **SharedMoveObject (共享可变对象)**：例如DEX的交易池，多个用户可以同时与之交互，并且它的状态会改变。需要提供对象的ID、初始共享版本号。
//     -   **ImmOrOwnedMoveObject (不可变或私有可变对象)**：例如你自己拥有的代币，或者是不可变的元数据对象。需要提供对象的引用（ID、版本号、摘要）。
//     `pool_related_objects()` 函数在获取对象信息时，会判断每个对象的类型，并包装成 `ObjectReadResult`。
//
// -   **ObjectReadResult (对象读取结果)**:
//     这个结构体封装了从链上（或模拟器缓存中）读取一个对象的详细结果。它不仅包含了对象本身的数据（`Object`），
//     还包含了这个对象作为交易输入时应该使用的 `InputObjectKind`。这是 `SimulateCtx` 中 `override_objects` 期望的格式。

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::collections::HashSet; // 用于存储唯一的对象ID字符串 (Used for storing unique Object ID strings)
use std::fs;                   // 文件系统操作，如读写文件 (File system operations, like reading/writing files)
use std::str::FromStr;         // 用于从字符串转换 (例如SuiAddress, ObjectID) (Used for converting from strings, e.g., SuiAddress, ObjectID)
use std::sync::Arc;            // 原子引用计数 (Atomic Reference Counting, for shared ownership)

use clap::Parser; // `clap` crate，用于解析命令行参数 (clap crate, for parsing command-line arguments)
use dex_indexer::{types::Protocol, DexIndexer}; // DEX索引器客户端和协议类型 (DEX indexer client and protocol types)
use eyre::Result; // `eyre`库，用于错误处理 (eyre library, for error handling)
use mev_logger::LevelFilter; // 日志级别过滤器 (来自自定义的 `mev_logger`) (Log level filter from custom `mev_logger`)
use object_pool::ObjectPool; // 对象池，用于管理模拟器实例 (Object pool, for managing simulator instances)
use simulator::{DBSimulator, SimulateCtx, Simulator}; // 数据库模拟器、模拟上下文、模拟器trait (Database simulator, simulate context, simulator trait)
use std::fs::File; // 文件操作 (File operations)
use std::io::{BufRead, BufReader, BufWriter, Write}; // 带缓冲的读写器 (Buffered readers/writers)
use sui_sdk::types::{ // Sui SDK中定义的一些常量对象ID (Some constant Object IDs defined in Sui SDK)
    BRIDGE_PACKAGE_ID, DEEPBOOK_PACKAGE_ID, MOVE_STDLIB_PACKAGE_ID, SUI_AUTHENTICATOR_STATE_OBJECT_ID,
    SUI_BRIDGE_OBJECT_ID, SUI_CLOCK_OBJECT_ID, SUI_DENY_LIST_OBJECT_ID, SUI_FRAMEWORK_PACKAGE_ID,
    SUI_RANDOMNESS_STATE_OBJECT_ID, SUI_SYSTEM_PACKAGE_ID, SUI_SYSTEM_STATE_OBJECT_ID,
};
use sui_sdk::SuiClientBuilder; // Sui客户端构建器 (Sui client builder)
use sui_types::base_types::{ObjectID, SuiAddress}; // Sui基本类型 (Sui basic types)
use sui_types::object::{Object, Owner}; // Sui对象和所有者类型 (Sui object and owner types)
use sui_types::transaction::{InputObjectKind, ObjectReadResult}; // Sui交易输入对象类型和对象读取结果 (Sui transaction input object kind and object read result)
use tracing::info; // `tracing`库，用于日志记录 (tracing library, for logging)

// 从当前crate的其他模块引入 (Import from other modules in the current crate)
use crate::common::get_latest_epoch; // 获取最新纪元信息的函数 (Function to get the latest epoch information)
use crate::defi::{DexSearcher, IndexerDexSearcher, TradeType, Trader}; // DeFi相关的trait和结构体 (DeFi related traits and structs)
use crate::HttpConfig; // 通用的HTTP配置结构体 (在main.rs中定义) (Common HTTP configuration struct, defined in main.rs)

/// `Args` 结构体
/// (Args struct)
///
/// 定义了 `pool_ids` 子命令的命令行参数。
/// (Defines command-line arguments for the `pool_ids` subcommand.)
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// 输出文件的路径，用于存储收集到的对象ID列表。
    /// (Path to the output file for storing the collected list of object IDs.)
    /// 默认值为 "./pool_related_ids.txt"。
    /// (Default value is "./pool_related_ids.txt".)
    #[clap(long, default_value = "./pool_related_ids.txt")]
    pub result_path: String,

    /// HTTP相关的配置 (例如Sui RPC URL)。
    /// (HTTP related configuration (e.g., Sui RPC URL).)
    /// `#[command(flatten)]` 表示将 `HttpConfig` 中的字段直接作为当前命令的参数。
    /// (`#[command(flatten)]` means fields from `HttpConfig` are directly used as arguments for the current command.)
    #[command(flatten)]
    pub http_config: HttpConfig,

    /// 是否仅运行测试模式。
    /// (Whether to run in test mode only.)
    /// 如果为true，则会执行 `test_pool_related_objects()` 函数。
    /// (If true, the `test_pool_related_objects()` function will be executed.)
    #[clap(long, help = "仅运行测试 (Run test only)")]
    pub test: bool,

    /// (测试模式参数) 是否在模拟时启用回退机制 (fallback)。
    /// ((Test mode parameter) Whether to enable fallback mechanism during simulation.)
    /// `DBSimulator::new_test(with_fallback)` 可能根据此参数有不同行为。
    /// (`DBSimulator::new_test(with_fallback)` might behave differently based on this parameter.)
    #[clap(long, help = "模拟时启用回退 (Enable fallback during simulation)")]
    pub with_fallback: bool,

    /// (测试模式参数) 模拟交易的输入金额。
    /// ((Test mode parameter) Input amount for transaction simulation.)
    /// 默认值为 10,000,000 MIST (0.01 SUI)。
    /// (Default value is 10,000,000 MIST (0.01 SUI).)
    #[clap(long, default_value = "10000000")]
    pub amount_in: u64,

    /// (测试模式参数) 用于测试的交易路径，由逗号分隔的对象ID组成。
    /// ((Test mode parameter) Trading path for testing, consisting of comma-separated Object IDs.)
    /// 例如 (For example): "pool_id1,pool_id2,pool_id3"
    #[clap(
        long,
        default_value = "0x3c3dd05e348fba5d8bf6958369cc3b33c8e8be85c96e10b1ca6413ad1b2d7787,0xe356c686eb19972e076b6906de12354a1a7ce1b09691416e9d852b04fd21b9a6,0xade90c3bc407eaa34068129d63bba5d1cf7889a2dbaabe5eb9b3efbbf53891ea,0xda49f921560e39f15d801493becf79d47c89fb6db81e0cbbe7bf6d3318117a00"
    )]
    pub path: String,

    /// (测试模式参数, 可选) 在模拟前需要从预加载对象列表中删除的对象ID，由逗号分隔。
    /// ((Test mode parameter, optional) Object IDs to be deleted from the preloaded object list before simulation, comma-separated.)
    /// 用于测试排除某些对象对模拟结果的影响。
    /// (Used for testing the effect of excluding certain objects on simulation results.)
    #[clap(long, help = "模拟前删除的对象ID列表 (List of Object IDs to delete before simulation)")]
    pub delete_objects: Option<String>,
}

/// `supported_protocols` 函数
/// (supported_protocols function)
///
/// 返回一个包含所有当前已集成的、需要为其收集对象ID的DEX协议的列表。
/// (Returns a list of all currently integrated DEX protocols for which object IDs need to be collected.)
fn supported_protocols() -> Vec<Protocol> {
    vec![
        Protocol::Cetus,        // Cetus协议
        Protocol::Turbos,       // Turbos协议
        Protocol::KriyaAmm,     // Kriya DEX的AMM (Automated Market Maker)
        Protocol::BlueMove,     // BlueMove (可能是一个NFT市场，但也可能有AMM池)
        Protocol::KriyaClmm,    // Kriya DEX的CLMM (Concentrated Liquidity Market Maker)
        Protocol::FlowxClmm,    // FlowX Finance的CLMM
        Protocol::Navi,         // Navi Protocol (通常是借贷协议，但其关键对象ID也可能需要预加载，比如预言机、资金池等)
                                // (Navi is a lending protocol, but its key object IDs might also need preloading)
        Protocol::Aftermath,    // Aftermath Finance
        // 注意：DeepBookV2 没有在这里列出，可能是因为它不通过常规的 `get_all_pools` 获取，
        // 或者其相关对象已包含在 `global_ids()` 中。
        // (Note: DeepBookV2 is not listed here, possibly because it's not fetched via the usual `get_all_pools`,
        //  or its related objects are already included in `global_ids()`.)
    ]
}

/// `run` 函数 (子命令的主入口)
/// (run function (Main entry point for the subcommand))
///
/// 根据命令行参数执行操作：生成对象ID列表文件，或运行测试。
/// (Executes operations based on command-line arguments: generates an object ID list file, or runs tests.)
///
/// 参数 (Parameters):
/// - `args`: 解析后的命令行参数 (`Args`结构体)。
///           (Parsed command-line arguments (`Args` struct).)
///
/// 返回 (Returns):
/// - `Result<()>`: 如果成功则返回Ok，否则返回错误。
///                 (Returns Ok if successful, otherwise returns an error.)
pub async fn run(args: Args) -> Result<()> {
    // 初始化日志系统 (Initialize the logging system)
    mev_logger::init_console_logger_with_directives(
        Some(LevelFilter::INFO), // 设置默认日志级别为INFO (Set default log level to INFO)
        &[ // 为特定模块设置更详细的日志级别 (用于调试)
           // (Set more detailed log levels for specific modules (for debugging))
            "arb=debug", // arb模块设为debug (Set arb module to debug)
            // "dex_indexer=warn",
            // "simulator=trace",
            // "sui_types=trace",
            // "sui_move_natives_latest=trace",
            // "sui_execution=warn",
        ],
    );
    info!("pool_ids 工具启动，参数: {:?}", args); // 日志：工具启动及参数

    // 如果指定了 `--test` 参数，则执行测试逻辑并返回。
    // (If the `--test` argument is specified, execute test logic and return.)
    if args.test {
        info!("进入测试模式..."); // 日志：进入测试模式
        return test_pool_related_objects(args).await;
    }

    // --- 生成对象ID列表文件的逻辑 ---
    // (Logic for generating the object ID list file)
    let result_path = args.result_path; // 输出文件路径 (Output file path)
    let rpc_url = args.http_config.rpc_url; // Sui RPC URL

    info!("将从 RPC {} 获取数据，并将结果写入到 {}", rpc_url, result_path); // 日志：数据源和目标文件

    // 初始化DEX索引器客户端和数据库模拟器 (用于获取对象信息)
    // (Initialize DEX indexer client and database simulator (for fetching object information))
    let dex_indexer = DexIndexer::new(&rpc_url).await?;
    // `DBSimulator::new_default_slow()` 可能连接到一个持久化的数据库实例来获取对象数据。
    // (`DBSimulator::new_default_slow()` might connect to a persistent database instance to fetch object data.)
    let simulator: Arc<dyn Simulator> = Arc::new(DBSimulator::new_default_slow().await);
    info!("DEX索引器和模拟器初始化完毕。"); // 日志：组件初始化

    // 尝试删除已存在的旧结果文件 (如果存在)
    // (Attempt to delete the old result file if it exists)
    let _ = fs::remove_file(&result_path); // 忽略删除失败的错误 (Ignore errors if deletion fails)
    // 创建新的结果文件 (Create a new result file)
    let file = File::create(&result_path)?;
    let mut writer = BufWriter::new(file); // 使用带缓冲的写入器以提高效率 (Use a buffered writer for efficiency)

    // 加载已存在于文件中的ID (如果文件非空且可读)，以支持增量更新。
    // (Load IDs already present in the file (if non-empty and readable) to support incremental updates.)
    // 注意：由于上面 `fs::remove_file` 的存在，这里通常会从一个空文件开始。
    // (Note: Due to `fs::remove_file` above, this usually starts from an empty file.)
    // 如果希望是增量更新，则不应首先删除文件。
    // (If incremental updates are desired, the file should not be deleted first.)
    let mut object_ids: HashSet<String> = match fs::read_to_string(&result_path) {
        Ok(contents) => contents.lines().map(|line| line.to_string()).collect(),
        Err(_) => HashSet::new(), // 如果文件不存在或读取失败，则从空集合开始 (If file doesn't exist or read fails, start with an empty set)
    };
    let initial_ids_count = object_ids.len();
    info!("从现有文件加载了 {} 个对象ID (如果文件存在)。", initial_ids_count); // 日志：初始ID数量


    // 遍历所有支持的协议 (Iterate through all supported protocols)
    for protocol in supported_protocols() {
        info!("正在处理协议: {:?}", protocol); // 日志：当前处理的协议
        // 添加与协议本身相关的对象ID (例如全局配置对象、工厂对象等)
        // (Add object IDs related to the protocol itself (e.g., global config objects, factory objects, etc.))
        // `protocol.related_object_ids()` 是 `Protocol` 枚举的一个方法 (可能通过trait实现)
        // (`protocol.related_object_ids()` is a method of the `Protocol` enum (possibly implemented via a trait))
        let protocol_related_ids = protocol.related_object_ids().await?;
        info!("协议 {:?} 相关的对象ID数量: {}", protocol, protocol_related_ids.len());
        object_ids.extend(protocol_related_ids);

        // Navi的资金池不由 `dex_indexer` 的 `get_all_pools` 管理，其关键对象已在上面添加。
        // (Navi's liquidity pools are not managed by `dex_indexer`'s `get_all_pools`; its key objects were added above.)
        if protocol == Protocol::Navi {
            info!("协议 {:?} 是Navi，跳过get_all_pools步骤。", protocol);
            continue;
        }

        // 获取该协议下的所有池 (Get all pools under this protocol)
        if let Ok(pools) = dex_indexer.get_all_pools(&protocol) { // 修改：处理Result (Modified: Handle Result)
            info!("协议 {:?} 下找到 {} 个池。", protocol, pools.len());
            for (i, pool) in pools.iter().enumerate() { // 为池子循环添加日志
                let pool_related_ids_count_before = object_ids.len();
                // 添加与每个池相关的对象ID (例如池本身、LP代币对象等)
                // (Add object IDs related to each pool (e.g., the pool itself, LP token objects, etc.))
                // `pool.related_object_ids()` 是 `Pool` 结构体的一个方法
                // (`pool.related_object_ids()` is a method of the `Pool` struct)
                object_ids.extend(pool.related_object_ids(Arc::clone(&simulator)).await);
                let pool_related_ids_count_after = object_ids.len();
                if i < 5 || i % 100 == 0 { // 日志部分池子的ID收集情况，避免过多日志
                    info!("  处理池 {}/{}: ID {}, 新增 {} 个相关ID。", i+1, pools.len(), pool.pool_id(), pool_related_ids_count_after - pool_related_ids_count_before);
                }
            }
        } else {
            // 如果获取某协议的池失败，可以记录一个警告或错误
            // (If fetching pools for a protocol fails, a warning or error can be logged)
            tracing::warn!("未能获取协议 {:?} 的池列表 (Failed to fetch pool list for protocol {:?})", protocol);
        }
    }

    // 添加所有全局系统对象ID (Add all global system object IDs)
    let global_ids_set = global_ids();
    info!("添加 {} 个全局对象ID。", global_ids_set.len());
    object_ids.extend(global_ids_set);

    // 将所有收集到的唯一对象ID写入文件，每行一个。
    // (Write all collected unique object IDs to the file, one per line.)
    let all_ids_vec: Vec<String> = object_ids.into_iter().collect(); // HashSet转为Vec以排序或稳定输出(可选)
                                                                    // (Convert HashSet to Vec for sorting or stable output (optional))
                                                                    // 如果需要稳定输出顺序，可以在这里排序: all_ids_vec.sort();
                                                                    // (If stable output order is needed, sort here: all_ids_vec.sort();)
    writeln!(writer, "{}", all_ids_vec.join("\n"))?; // 用换行符连接所有ID并写入 (Join all IDs with newline and write)

    writer.flush()?; //确保所有缓冲内容都写入文件 (Ensure all buffered content is written to the file)

    info!("🎉 成功将 {} 个池及相关对象ID写入到 {} (Successfully wrote {} pool and related object IDs to {})", all_ids_vec.len(), result_path, all_ids_vec.len());

    Ok(())
}

/// `global_ids` 函数
/// (global_ids function)
///
/// 返回一个包含Sui系统级全局对象ID和一些其他重要全局对象ID的集合。
/// (Returns a set containing Sui system-level global object IDs and some other important global object IDs.)
/// 这些ID通常是固定的或广为人知的。
/// (These IDs are usually fixed or widely known.)
fn global_ids() -> HashSet<String> {
    // Sui系统框架和核心对象的ID (从sui_sdk::types导入的常量)
    // (IDs of Sui system framework and core objects (constants imported from sui_sdk::types))
    let mut result_set = vec![
        MOVE_STDLIB_PACKAGE_ID,        // Move标准库包ID ("0x1") (Move standard library package ID)
        SUI_FRAMEWORK_PACKAGE_ID,      // Sui框架包ID ("0x2") (Sui framework package ID)
        SUI_SYSTEM_PACKAGE_ID,         // Sui系统包ID ("0x3") (Sui system package ID)
        BRIDGE_PACKAGE_ID,             // Sui桥接相关包ID (可能指Wormhole或其他官方桥) (Sui bridge related package ID (might refer to Wormhole or other official bridges))
        DEEPBOOK_PACKAGE_ID,           // DeepBook包ID (DeepBook package ID)
        SUI_SYSTEM_STATE_OBJECT_ID,    // Sui系统状态对象ID ("0x5") (Sui system state object ID)
        SUI_CLOCK_OBJECT_ID,           // 时钟对象ID ("0x6") (Clock object ID)
        SUI_AUTHENTICATOR_STATE_OBJECT_ID, // 认证器状态对象ID ("0x7") (Authenticator state object ID)
        SUI_RANDOMNESS_STATE_OBJECT_ID,  // 随机数状态对象ID ("0x8") (Randomness state object ID)
        SUI_BRIDGE_OBJECT_ID,          // Sui桥对象ID (Sui bridge object ID)
        SUI_DENY_LIST_OBJECT_ID,       // Sui拒绝列表对象ID (用于封禁等) (Sui deny list object ID (used for banning, etc.))
    ]
    .into_iter()
    .map(|id| id.to_string()) // 将ObjectID常量转换为String (Convert ObjectID constants to String)
    .collect::<HashSet<String>>();

    // 添加其他已知的全局重要对象的ID
    // (Add IDs of other known globally important objects)
    // 例如，Wormhole核心状态对象等
    // (For example, Wormhole core state objects, etc.)
    result_set.insert("0x5306f64e312b581766351c07af79c72fcb1cd25147157fdc2f8ad76de9a3fb6a".to_string()); // Wormhole 主状态对象 (示例) (Wormhole main state object (example))
    result_set.insert("0x26efee2b51c911237888e5dc6702868abca3c7ac12c53f76ef8eba0697695e3d".to_string()); // 可能是另一个Wormhole相关对象 (Possibly another Wormhole related object)

    result_set
}

/// `test_pool_related_objects` 异步函数 (测试模式的主逻辑)
/// (test_pool_related_objects async function (Main logic for test mode))
///
/// 该函数用于测试预加载的对象列表在实际交易模拟中的效果。
/// (This function is used to test the effect of the preloaded object list in actual transaction simulation.)
///
/// 步骤 (Steps):
/// 1. 定义测试参数 (发送者地址、输入金额、交易路径等)。
///    (Define test parameters (sender address, input amount, trade path, etc.).)
/// 2. 初始化 `IndexerDexSearcher` 和 `Trader`。
///    (Initialize `IndexerDexSearcher` and `Trader`.)
/// 3. 从 `args.result_path` (通常是 `pool_related_ids.txt`) 加载对象ID列表，
///    并获取这些对象的 `ObjectReadResult` (包含对象数据和元数据)。
///    (Load the object ID list from `args.result_path` (usually `pool_related_ids.txt`),
///     and fetch `ObjectReadResult` for these objects (containing object data and metadata).)
/// 4. (可选) 根据 `args.delete_objects` 从预加载列表中移除某些对象。
///    ((Optional) Remove certain objects from the preloaded list based on `args.delete_objects`.)
/// 5. 使用这些预加载对象创建一个 `SimulateCtx`。
///    (Create a `SimulateCtx` using these preloaded objects.)
/// 6. 调用 `Trader::get_trade_result` 在此上下文中模拟一笔闪电贷交易。
///    (Call `Trader::get_trade_result` to simulate a flashloan transaction in this context.)
/// 7. 打印模拟结果。
///    (Print the simulation result.)
async fn test_pool_related_objects(args: Args) -> Result<()> {
    info!("开始执行 test_pool_related_objects 函数..."); // 日志：测试函数开始
    // --- 测试数据定义 ---
    // (Test data definition)
    let sender = SuiAddress::from_str("0xac5bceec1b789ff840d7d4e6ce4ce61c90d190a7f8c4f4ddf0bff6ee2413c33c").unwrap(); // 一个固定的测试发送者地址 (A fixed test sender address)
    let amount_in = args.amount_in; // 从命令行参数获取输入金额 (Get input amount from command-line args)

    // 从命令行参数解析交易路径 (逗号分隔的ObjectID字符串)
    // (Parse trade path from command-line args (comma-separated ObjectID strings))
    let path_obj_ids = args
        .path
        .split(',')
        .map(|obj_id_str| ObjectID::from_hex_literal(obj_id_str).unwrap())
        .collect::<Vec<_>>();

    let with_fallback = args.with_fallback; // 是否启用模拟器回退 (Whether to enable simulator fallback)
    let rpc_url = args.http_config.rpc_url.clone(); // RPC URL

    // 创建模拟器对象池 (用于初始化Trader和IndexerDexSearcher)
    // (Create simulator object pool (for initializing Trader and IndexerDexSearcher))
    // `DBSimulator::new_test(with_fallback)` 创建一个测试用的数据库模拟器。
    // (`DBSimulator::new_test(with_fallback)` creates a test database simulator.)
    let simulator_pool = Arc::new(ObjectPool::new(1, move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { Box::new(DBSimulator::new_test(with_fallback).await) as Box<dyn Simulator> })
    }));

    // 初始化DEX搜索器，并根据对象ID路径构建实际的交易路径 (`Path` 对象)
    // (Initialize DEX searcher and build the actual trade path (`Path` object) based on Object ID path)
    let dex_searcher: Arc<dyn DexSearcher> = Arc::new(IndexerDexSearcher::new(&rpc_url, Arc::clone(&simulator_pool)).await?);
    let trade_path = dex_searcher.find_test_path(&path_obj_ids).await?;
    info!(?with_fallback, ?amount_in, ?trade_path, delete_objects_str = ?args.delete_objects, "测试数据初始化完毕 (Test data initialized)");
    // --- 测试数据定义结束 ---
    // (End of test data definition)

    // 创建Sui客户端并获取最新纪元信息 (用于模拟上下文)
    // (Create Sui client and get latest epoch information (for simulation context))
    let sui_client = SuiClientBuilder::default().build(&rpc_url).await?;
    let epoch_info = get_latest_epoch(&sui_client).await?;
    info!("获取到最新纪元信息: {:?}", epoch_info.epoch); // 日志：纪元信息

    // 加载 `pool_related_ids.txt` 文件中的对象作为预加载对象。
    // (Load objects from `pool_related_ids.txt` file as preloaded objects.)
    let mut override_objects_for_sim = pool_related_objects(&args.result_path).await?;
    info!("从 {} 加载了 {} 个待覆盖(预加载)的对象。", args.result_path, override_objects_for_sim.len()); // 日志：预加载对象数量

    // 如果命令行指定了要删除的对象ID，则从预加载列表中移除它们。
    // (If Object IDs to be deleted are specified in command-line args, remove them from the preloaded list.)
    if let Some(delete_object_ids_str) = args.delete_objects {
        info!("准备从预加载对象中删除: {}", delete_object_ids_str); // 日志：待删除对象
        let delete_obj_ids_vec = delete_object_ids_str
            .split(',')
            .map(|obj_id_str| ObjectID::from_hex_literal(obj_id_str).unwrap())
            .collect::<Vec<_>>();
        let count_before_delete = override_objects_for_sim.len();
        // `retain` 方法保留使闭包返回true的元素。
        // (`retain` method keeps elements for which the closure returns true.)
        // 这里保留那些ID不在 `delete_obj_ids_vec` 中的对象。
        // (Here, objects whose IDs are not in `delete_obj_ids_vec` are kept.)
        override_objects_for_sim.retain(|obj_read_result| {
            !delete_obj_ids_vec.contains(&obj_read_result.object_id())
        });
        let count_after_delete = override_objects_for_sim.len();
        info!("从预加载对象中删除了 {} 个对象。删除前: {}, 删除后: {}",
            count_before_delete - count_after_delete,
            count_before_delete,
            count_after_delete
        ); // 日志：删除对象结果
    }

    // 使用预加载对象创建模拟上下文。
    // (Create simulation context using preloaded objects.)
    let sim_ctx = SimulateCtx::new(epoch_info, override_objects_for_sim);
    info!("模拟上下文创建完毕。"); // 日志：模拟上下文创建

    // 初始化Trader并执行交易模拟。
    // (Initialize Trader and execute transaction simulation.)
    let trader = Trader::new(simulator_pool).await?;
    info!("Trader初始化完毕，准备执行模拟交易..."); // 日志：Trader初始化
    let trade_result = trader
        .get_trade_result(&trade_path, sender, amount_in, TradeType::Flashloan, vec![], sim_ctx) // Gas币列表为空vec![]，因为DBSimulator可能不严格检查Gas对象
                                                                                                 // (Gas coin list is empty vec![], as DBSimulator might not strictly check Gas objects)
        .await?;
    info!(?trade_result, "交易模拟结果 (Trade simulation result)"); // 日志：模拟结果

    Ok(())
}

/// `pool_related_objects` 异步辅助函数
/// (pool_related_objects async helper function)
///
/// 从指定的文件路径读取对象ID列表，并通过模拟器获取这些对象的 `ObjectReadResult`。
/// (Reads a list of Object IDs from the specified file path and fetches their `ObjectReadResult` via the simulator.)
/// `ObjectReadResult` 包含了对象的元数据和数据，可以直接用于填充 `SimulateCtx` 的 `override_objects`。
/// (`ObjectReadResult` contains object metadata and data, and can be directly used to populate `SimulateCtx`'s `override_objects`.)
///
/// 参数 (Parameters):
/// - `file_path`: 包含对象ID列表的文件的路径字符串。
///                (Path string of the file containing the list of Object IDs.)
///
/// 返回 (Returns):
/// - `Result<Vec<ObjectReadResult>>`: 包含所有成功获取的对象信息的向量。
///                                   (A vector containing information for all successfully fetched objects.)
async fn pool_related_objects(file_path: &str) -> Result<Vec<ObjectReadResult>> {
    info!("开始从 {} 加载对象信息用于模拟上下文...", file_path); // 日志：开始加载对象
    // 创建一个临时的DBSimulator实例，用于获取对象数据。
    // (Create a temporary DBSimulator instance to fetch object data.)
    // `new_test(true)` 可能表示使用一个轻量级的、带回退的测试模拟器。
    // (`new_test(true)` might indicate using a lightweight test simulator with fallback.)
    let simulator: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);
    let file = File::open(file_path)?; // 打开文件 (Open the file)
    let reader = BufReader::new(file); // 创建带缓冲的读取器 (Create a buffered reader)

    let mut results_vec = vec![];
    let mut line_count = 0;
    let mut found_count = 0;
    for line_result in reader.lines() { // 逐行读取文件 (Read file line by line)
        line_count += 1;
        let line_str = line_result?; // 处理可能的IO错误 (Handle possible IO errors)
        let object_id = match ObjectID::from_hex_literal(&line_str) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("解析对象ID失败: '{}', 错误: {:?}, 已跳过。", line_str, e);
                continue;
            }
        }; // 将行内容解析为ObjectID (Parse line content to ObjectID)


        // 通过模拟器获取对象数据 (Fetch object data via simulator)
        let object_data: Object = if let Some(obj) = simulator.get_object(&object_id).await {
            obj
        } else {
            // 如果模拟器中找不到该对象 (例如，它在链上已被删除或ID无效)，则跳过。
            // (If the object is not found in the simulator (e.g., deleted on-chain or invalid ID), skip it.)
            tracing::warn!("对象ID {} 在模拟器中未找到，已跳过。(Object ID {} not found in simulator, skipped.)", object_id);
            continue;
        };
        found_count += 1;

        // 根据对象的所有者类型，确定其 `InputObjectKind`。
        // (Determine its `InputObjectKind` based on the object's owner type.)
        // 这对于构建交易或在模拟器中正确表示对象是必要的。
        // (This is necessary for building transactions or correctly representing objects in the simulator.)
        let input_object_kind = match object_data.owner() {
            Owner::Shared { initial_shared_version } => InputObjectKind::SharedMoveObject {
                id: object_id,
                initial_shared_version: *initial_shared_version,
                mutable: true, // 假设预加载的共享对象在模拟中可能是可变的 (Assume preloaded shared objects might be mutable in simulation)
            },
            _ => InputObjectKind::ImmOrOwnedMoveObject(object_data.compute_object_reference()), // 对于私有对象或不可变对象 (For private or immutable objects)
        };

        // 将 `InputObjectKind` 和对象数据 (`object_data`) 包装成 `ObjectReadResult`。
        // (Wrap `InputObjectKind` and object data (`object_data`) into `ObjectReadResult`.)
        // `object_data.into()` 可能会将其转换为 `SuiObjectData`。
        // (`object_data.into()` might convert it to `SuiObjectData`.)
        results_vec.push(ObjectReadResult::new(input_object_kind, object_data.into()));
    }
    info!("从 {} 共读取 {} 行，找到并处理了 {} 个对象。", file_path, line_count, found_count); // 日志：对象加载总结

    Ok(results_vec)
}

[end of bin/arb/src/pool_ids.rs]
