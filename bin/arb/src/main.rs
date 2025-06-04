// 该文件 `main.rs` 是 `arb` 二进制程序的入口点。
// Rust程序执行时，会首先调用 `main` 函数。
// 这个文件负责：
// 1. 声明项目中的所有顶层模块 (arb, collector, common, config, defi, executor, pool_ids, start_bot, strategy, types)。
// 2. 使用 `clap` crate 解析命令行参数。
// 3. 根据解析到的子命令，将程序流程分发到相应的模块进行处理。
// 4. 定义一个全局的构建版本号 `BUILD_VERSION`。
//
// **文件概览 (File Overview)**:
// 这个 `main.rs` 文件是整个套利机器人 (`arb`) 程序的“总指挥部”或“起点”。
// 当你运行这个程序时，操作系统会首先找到并执行这里的 `main` 函数。
//
// 主要做的事情 (Key Responsibilities):
// 1.  **模块声明 (Module Declarations)**:
//     像 `mod arb;` 这样的语句，是在告诉Rust编译器：“嘿，我们项目里有一个叫做 `arb` 的模块，它的代码在 `arb.rs` 文件或者 `arb/mod.rs` 目录里，请把它包含进来。”
//     这样，`main.rs` 就可以使用其他模块里定义的函数和结构体了。
//     (Statements like `mod arb;` tell the Rust compiler: "Hey, we have a module named `arb` in our project, its code is in `arb.rs` or `arb/mod.rs`, please include it." This allows `main.rs` to use functions and structs defined in other modules.)
//
// 2.  **解析命令行参数 (Parsing Command-Line Arguments)**:
//     当你从终端运行程序时，可能会附带一些指令，比如 `arb start --live` 或者 `arb run --coin-type "0x2::sui::SUI"`。
//     这些 `--live`、`--coin-type` 就是命令行参数。这个文件使用了一个叫做 `clap` 的“瑞士军刀”库来帮助理解和处理这些参数。
//     (When you run the program from the terminal, you might add instructions like `arb start --live` or `arb run --coin-type "0x2::sui::SUI"`. These `--live`, `--coin-type` are command-line arguments. This file uses a "Swiss Army knife" library called `clap` to help understand and process these arguments.)
//
// 3.  **分发任务 (Dispatching Tasks)**:
//     根据用户在命令行中给出的具体“子命令”（比如 `start` 或 `run`），`main` 函数会决定接下来应该调用哪个模块的哪个函数去干活。
//     就像一个总机，接到不同类型的电话后，会转接到相应的部门。
//     (Based on the specific "subcommand" given by the user in the command line (e.g., `start` or `run`), the `main` function decides which module's function should be called to do the work. Like a switchboard operator routing calls to different departments.)
//
// 4.  **版本号 (Version Number)**:
//     `BUILD_VERSION` 常量保存了这个程序的构建版本信息。这对于开发者来说很有用，比如当用户报告一个问题时，可以知道他们用的是哪个版本的程序。
//     (The `BUILD_VERSION` constant stores the build version information of this program. This is useful for developers, for instance, to know which version of the program a user is using when they report an issue.)
//
// **Sui区块链和MEV相关的概念解释 (Sui Blockchain and MEV-related Concepts)**:
// 虽然 `main.rs` 本身不直接处理复杂的Sui链上逻辑或MEV策略，但它作为程序的入口，会启动那些负责这些功能的模块。
// 因此，理解整个项目的目标（进行Sui链上套利，可能涉及MEV）有助于理解为什么会有 `arb` (套利核心)、`collector` (链上事件收集)、`executor` (交易执行) 等模块。
// (Although `main.rs` itself doesn't directly handle complex Sui on-chain logic or MEV strategies, as the program's entry point, it launches modules responsible for these functions.
// Therefore, understanding the overall project goal (Sui on-chain arbitrage, potentially involving MEV) helps in understanding why modules like `arb` (arbitrage core), `collector` (on-chain event collection), `executor` (transaction execution), etc., exist.)

// --- 声明项目顶层模块 ---
// (Declare top-level modules of the project)
// 每个 `mod` 声明都对应一个同名的 `.rs` 文件或一个同名目录下的 `mod.rs` 文件。
// (Each `mod` declaration corresponds to a `.rs` file of the same name or a `mod.rs` file in a directory of the same name.)
mod arb;             // 套利逻辑核心模块 (可能包含机会发现、路径评估等)
                     // (Arbitrage logic core module (may include opportunity discovery, path evaluation, etc.))
mod collector;       // 事件收集器模块 (如收集公开交易、私有交易)
                     // (Event collector module (e.g., collecting public transactions, private transactions))
mod common;          // 通用工具或共享代码模块
                     // (Common utilities or shared code module)
mod config;          // 全局配置参数模块
                     // (Global configuration parameters module)
mod defi;            // DeFi协议交互模块 (包含各种DEX的实现)
                     // (DeFi protocol interaction module (includes implementations for various DEXs))
mod executor;        // 交易执行器模块
                     // (Transaction executor module)
mod pool_ids;        // 生成池ID列表相关的模块
                     // (Module related to generating pool ID lists)
mod start_bot;       // 启动和管理机器人的模块
                     // (Module for starting and managing the bot)
mod strategy;        // 套利策略定义模块 (可能包含不同类型的套利逻辑)
                     // (Arbitrage strategy definition module (may include different types of arbitrage logic))
mod types;           // 项目中自定义的通用数据类型模块
                     // (Custom general-purpose data types module for the project)

// --- 引入所需的库 ---
// (Import necessary libraries)
use clap::Parser; // `clap` crate，用于解析命令行参数。`Parser` trait用于自动生成参数解析逻辑。
                  // `clap` crate, used for parsing command-line arguments. The `Parser` trait is used to automatically generate argument parsing logic.
use eyre::Result; // `eyre`库，用于更方便的错误处理。`Result`是其核心类型，通常是 `Result<T, eyre::Report>`。
                  // `eyre` library, for more convenient error handling. `Result` is its core type, usually `Result<T, eyre::Report>`.

// `BUILD_VERSION` 常量
// (BUILD_VERSION constant)
//
// 这个常量存储了程序的构建版本号。
// (This constant stores the build version number of the program.)
// `version::build_version!()` 是一个过程宏 (来自可能的 `version` crate 或自定义构建脚本)，
// 它会在编译时获取或生成版本信息 (例如从Git提交哈希、Cargo.toml版本等)。
// (`version::build_version!()` is a procedural macro (from a possible `version` crate or custom build script)
// that obtains or generates version information at compile time (e.g., from Git commit hash, Cargo.toml version, etc.).)
// 这对于追踪程序的不同构建和调试非常有用。
// (This is very useful for tracking different builds of the program and for debugging.)
pub const BUILD_VERSION: &str = version::build_version!();

/// `Args` 结构体 (主命令行参数)
/// (Args struct (Main command-line arguments))
///
/// 使用 `clap::Parser` 宏，定义了程序接受的顶层命令行参数。
/// (Using the `clap::Parser` macro, defines the top-level command-line arguments accepted by the program.)
/// 在这个例子中，它只包含一个子命令字段 `command`。
/// (In this example, it only contains one subcommand field, `command`.)
#[derive(clap::Parser)]
#[command(name = "arb", version = BUILD_VERSION, about = "A Sui arbitrage bot CLI")] // 给主命令设置名称、版本号（从BUILD_VERSION获取）和关于信息
                                                                                    // Set name, version (from BUILD_VERSION), and about information for the main command
pub struct Args {
    // `#[command(subcommand)]` 属性宏表示这个字段是一个子命令。
    // (`#[command(subcommand)]` attribute macro indicates that this field is a subcommand.)
    // `clap` 会根据 `Command` 枚举的定义来解析具体的子命令。
    // (`clap` will parse the specific subcommand based on the definition of the `Command` enum.)
    #[command(subcommand)]
    pub command: Command,
}

/// `HttpConfig` 结构体 (HTTP配置参数)
/// (HttpConfig struct (HTTP configuration parameters))
///
/// 定义了通用的HTTP相关配置，主要用于指定Sui RPC节点的URL。
/// (Defines common HTTP-related configurations, mainly for specifying the Sui RPC node URL.)
/// 这个结构体可以被多个子命令复用。
/// (This struct can be reused by multiple subcommands.)
///
/// `#[command(about = "Common configuration")]` 为这组参数在帮助信息中提供一个描述。
/// (`#[command(about = "Common configuration")]` provides a description for this group of parameters in the help message.)
#[derive(Clone, Debug, Parser)] // Clone和Debug是常用派生，Parser用于clap解析
                                // Clone and Debug are commonly derived traits; Parser is for clap parsing.
#[command(about = "通用HTTP配置 (Common HTTP configuration)")]
pub struct HttpConfig {
    // `#[arg(long, env = "SUI_RPC_URL", default_value = "http://localhost:9000")]`
    // 定义了一个名为 `rpc_url` 的参数：
    // (Defines a parameter named `rpc_url`:)
    // - `long`: 表示它是一个长格式参数，例如 `--rpc-url <VALUE>`。
    //           (Indicates it's a long-form parameter, e.g., `--rpc-url <VALUE>`.)
    // - `env = "SUI_RPC_URL"`: 表示可以从名为 `SUI_RPC_URL` 的环境变量中读取此参数的值。
    //                          (Indicates that the value for this parameter can be read from an environment variable named `SUI_RPC_URL`.)
    // - `default_value = "http://localhost:9000"`: 如果命令行和环境变量都未提供，则使用此默认值。
    //                                              (If neither command line nor environment variable provides it, this default value is used.)
    #[arg(long, env = "SUI_RPC_URL", default_value = "http://localhost:9000")]
    pub rpc_url: String, // Sui RPC节点的URL (URL of the Sui RPC node)

    // `#[arg(long, help = "deprecated")]`
    // 定义了一个名为 `ipc_path` 的可选参数：
    // (Defines an optional parameter named `ipc_path`:)
    // - `long`: 长格式参数 `--ipc-path <VALUE>`。
    //           (Long-form parameter `--ipc-path <VALUE>`.)
    // - `help = "deprecated"`: 在帮助信息中显示此参数已弃用。
    //                          (Displays in the help message that this parameter is deprecated.)
    // `Option<String>` 表示这个参数是可选的。
    // (`Option<String>` means this parameter is optional.)
    #[arg(long, help = "已弃用 (Deprecated)")]
    pub ipc_path: Option<String>, // IPC路径 (已弃用) (IPC path (deprecated))
}

/// `Command` 枚举 (子命令定义)
/// (Command enum (Subcommand definitions))
///
/// 使用 `clap::Subcommand` 宏，定义了程序支持的各个子命令。
/// (Using the `clap::Subcommand` macro, defines the various subcommands supported by the program.)
/// 每个枚举成员都对应一个子命令，并且可以包含该子命令特有的参数结构体。
/// (Each enum member corresponds to a subcommand and can contain a struct for parameters specific to that subcommand.)
#[derive(clap::Subcommand)]
pub enum Command {
    // `StartBot` 子命令: 用于启动套利机器人。
    // (`StartBot` subcommand: Used to start the arbitrage bot.)
    // 它关联了 `start_bot::Args` 结构体，该结构体定义了启动机器人所需的参数。
    // (It is associated with the `start_bot::Args` struct, which defines the parameters required to start the bot.)
    #[command(name="start", about = "启动套利机器人 (Start the arbitrage bot)")] // 给子命令设置名称和描述
    StartBot(start_bot::Args),

    // `Run` 子命令: 可能用于执行一次性的套利分析、特定路径测试或模拟。
    // (`Run` subcommand: May be used for one-off arbitrage analysis, specific path testing, or simulation.)
    // 它关联了 `arb::Args` 结构体。
    // (It is associated with the `arb::Args` struct.)
    #[command(name="run", about = "运行一次性的套利机会查找 (Run a one-time arbitrage opportunity search)")]
    Run(arb::Args),

    // `PoolIds` 子命令: 用于生成包含所有DEX池对象ID及其相关对象ID的文件。
    // (`PoolIds` subcommand: Used to generate a file containing all DEX pool object IDs and their related object IDs.)
    // 这对于初始化或调试非常有用。
    // (This is very useful for initialization or debugging.)
    // 它关联了 `pool_ids::Args` 结构体。
    // (It is associated with the `pool_ids::Args` struct.)
    #[command(name="pool-ids", about = "生成包含所有池对象ID及其底层对象的文件 (Generate a file with all pool object IDs and their underlying objects)")] // 为子命令添加描述
    PoolIds(pool_ids::Args),
}

/// `main` 函数 (程序主入口)
/// (main function (Program main entry point))
///
/// `#[tokio::main]` 宏将 `main` 函数转换为一个异步函数，并使用Tokio运行时来执行它。
/// (`#[tokio::main]` macro converts the `main` function into an asynchronous function and executes it using the Tokio runtime.)
/// Tokio是一个流行的Rust异步运行时。
/// (Tokio is a popular Rust asynchronous runtime.)
///
/// 返回 (Returns):
/// - `Result<()>`: 表示 `main` 函数可能返回一个错误 (由 `eyre` 库处理)。
///                 (Indicates that the `main` function might return an error (handled by the `eyre` library).)
///   如果成功执行完毕，则返回 `Ok(())`。
///   (If execution completes successfully, it returns `Ok(())`.)
#[tokio::main]
async fn main() -> Result<()> {
    // 步骤 1: 解析命令行参数。
    // (Step 1: Parse command-line arguments.)
    // `Args::parse()` 是 `clap` 提供的函数，它会根据 `Args` 结构体的定义来解析
    // 传入程序的命令行参数，并填充 `Args` 实例。
    // (`Args::parse()` is a function provided by `clap` that parses the command-line arguments passed to the program
    // according to the definition of the `Args` struct, and populates an `Args` instance.)
    // 如果参数解析失败（例如，用户提供了无效的参数或格式），`clap` 会自动显示帮助信息并退出程序。
    // (If argument parsing fails (e.g., the user provides invalid arguments or format), `clap` will automatically display help information and exit the program.)
    let args = Args::parse();

    // 步骤 2: 根据解析到的子命令，执行相应的逻辑。
    // (Step 2: Execute the corresponding logic based on the parsed subcommand.)
    // 使用 `match` 语句来匹配 `args.command` 的不同变体。
    // (Use a `match` statement to match different variants of `args.command`.)
    match args.command {
        // 如果是 `StartBot` 子命令，则调用 `start_bot` 模块的 `run` 函数，
        // 并将该子命令特有的参数 `args` (类型为 `start_bot::Args`) 传递给它。
        // (`.await` 用于等待异步的 `run` 函数完成。)
        // (If it's the `StartBot` subcommand, call the `run` function of the `start_bot` module,
        // passing its subcommand-specific arguments `args` (of type `start_bot::Args`) to it.)
        // (`.await` is used to wait for the asynchronous `run` function to complete.)
        Command::StartBot(args) => start_bot::run(args).await,

        // 如果是 `Run` 子命令，则调用 `arb` 模块的 `run` 函数。
        // (If it's the `Run` subcommand, call the `run` function of the `arb` module.)
        Command::Run(args) => arb::run(args).await,

        // 如果是 `PoolIds` 子命令，则调用 `pool_ids` 模块的 `run` 函数。
        // (If it's the `PoolIds` subcommand, call the `run` function of the `pool_ids` module.)
        Command::PoolIds(args) => pool_ids::run(args).await,
    }
    // `match` 语句的结果 (即对应 `run` 函数的 `Result<()>` 返回值) 会作为 `main` 函数的返回值。
    // (The result of the `match` statement (i.e., the `Result<()>` return value of the corresponding `run` function) will be the return value of the `main` function.)
    // 如果任何一个 `run` 函数返回错误，该错误会通过 `?` 操作符（如果 `run` 函数内部使用了的话）
    // 或直接作为 `Result` 从 `main` 函数返回，并可能由 `eyre` 库进行格式化输出。
    // (If any `run` function returns an error, that error will be returned from the `main` function
    // either via the `?` operator (if used inside the `run` function) or directly as a `Result`,
    // and may be formatted for output by the `eyre` library.)
}

[end of bin/arb/src/main.rs]
