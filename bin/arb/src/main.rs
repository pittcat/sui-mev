// 该文件 `main.rs` 是 `arb` 二进制程序的入口点。
// Rust程序执行时，会首先调用 `main` 函数。
// 这个文件负责：
// 1. 声明项目中的所有顶层模块 (arb, collector, common, config, defi, executor, pool_ids, start_bot, strategy, types)。
// 2. 使用 `clap` crate 解析命令行参数。
// 3. 根据解析到的子命令，将程序流程分发到相应的模块进行处理。
// 4. 定义一个全局的构建版本号 `BUILD_VERSION`。
//
// 文件概览:
// - 模块声明: `mod arb;`, `mod collector;` 等，将其他 `.rs` 文件或子目录作为模块引入到项目中。
// - `BUILD_VERSION`: 一个编译时生成的版本号常量，用于标识程序的构建版本。
// - `Args` 结构体: 使用 `clap::Parser` 派生宏定义了程序的主命令行参数结构，它包含一个子命令。
// - `HttpConfig` 结构体: 定义了通用的HTTP配置参数 (如Sui RPC URL)，可以被多个子命令共享或使用。
//   它也使用了 `clap::Parser`，并且可以通过环境变量或命令行参数进行配置。
// - `Command` 枚举: 使用 `clap::Subcommand` 派生宏定义了程序支持的子命令，例如:
//   - `StartBot`: 启动套利机器人 (逻辑在 `start_bot` 模块)。
//   - `Run`: 可能用于执行一次性的套利查找或特定分析 (逻辑在 `arb` 模块)。
//   - `PoolIds`: 生成包含所有池对象ID及其底层对象的文件 (逻辑在 `pool_ids` 模块)。
// - `main` 函数: 异步的 `main` 函数 (使用 `#[tokio::main]`)。
//   - 解析命令行参数。
//   - 使用 `match` 语句根据解析到的子命令，调用相应模块的 `run` 函数来执行具体任务。

// --- 声明项目顶层模块 ---
// 每个 `mod` 声明都对应一个同名的 `.rs` 文件或一个同名目录下的 `mod.rs` 文件。
mod arb;             // 套利逻辑核心模块 (可能包含机会发现、路径评估等)
mod collector;       // 事件收集器模块 (如收集公开交易、私有交易)
mod common;          // 通用工具或共享代码模块
mod config;          // 全局配置参数模块
mod defi;            // DeFi协议交互模块 (包含各种DEX的实现)
mod executor;        // 交易执行器模块
mod pool_ids;        // 生成池ID列表相关的模块
mod start_bot;       // 启动和管理机器人的模块
mod strategy;        // 套利策略定义模块 (可能包含不同类型的套利逻辑)
mod types;           // 项目中自定义的通用数据类型模块

// --- 引入所需的库 ---
use clap::Parser; // `clap` crate，用于解析命令行参数。`Parser` trait用于自动生成参数解析逻辑。
use eyre::Result; // `eyre`库，用于更方便的错误处理。`Result`是其核心类型，通常是 `Result<T, eyre::Report>`。

// `BUILD_VERSION` 常量
//
// 这个常量存储了程序的构建版本号。
// `version::build_version!()` 是一个过程宏 (来自可能的 `version` crate 或自定义构建脚本)，
// 它会在编译时获取或生成版本信息 (例如从Git提交哈希、Cargo.toml版本等)。
// 这对于追踪程序的不同构建和调试非常有用。
pub const BUILD_VERSION: &str = version::build_version!();

/// `Args` 结构体 (主命令行参数)
///
/// 使用 `clap::Parser` 宏，定义了程序接受的顶层命令行参数。
/// 在这个例子中，它只包含一个子命令字段 `command`。
#[derive(clap::Parser)]
pub struct Args {
    // `#[command(subcommand)]` 属性宏表示这个字段是一个子命令。
    // `clap` 会根据 `Command` 枚举的定义来解析具体的子命令。
    #[command(subcommand)]
    pub command: Command,
}

/// `HttpConfig` 结构体 (HTTP配置参数)
///
/// 定义了通用的HTTP相关配置，主要用于指定Sui RPC节点的URL。
/// 这个结构体可以被多个子命令复用。
///
/// `#[command(about = "Common configuration")]` 为这组参数在帮助信息中提供一个描述。
#[derive(Clone, Debug, Parser)] // Clone和Debug是常用派生，Parser用于clap解析
#[command(about = "通用HTTP配置")]
pub struct HttpConfig {
    // `#[arg(long, env = "SUI_RPC_URL", default_value = "http://localhost:9000")]`
    // 定义了一个名为 `rpc_url` 的参数：
    // - `long`: 表示它是一个长格式参数，例如 `--rpc-url <VALUE>`。
    // - `env = "SUI_RPC_URL"`: 表示可以从名为 `SUI_RPC_URL` 的环境变量中读取此参数的值。
    // - `default_value = "http://localhost:9000"`: 如果命令行和环境变量都未提供，则使用此默认值。
    #[arg(long, env = "SUI_RPC_URL", default_value = "http://localhost:9000")]
    pub rpc_url: String, // Sui RPC节点的URL

    // `#[arg(long, help = "deprecated")]`
    // 定义了一个名为 `ipc_path` 的可选参数：
    // - `long`: 长格式参数 `--ipc-path <VALUE>`。
    // - `help = "deprecated"`: 在帮助信息中显示此参数已弃用。
    // `Option<String>` 表示这个参数是可选的。
    #[arg(long, help = "已弃用")]
    pub ipc_path: Option<String>, // IPC路径 (已弃用)
}

/// `Command` 枚举 (子命令定义)
///
/// 使用 `clap::Subcommand` 宏，定义了程序支持的各个子命令。
/// 每个枚举成员都对应一个子命令，并且可以包含该子命令特有的参数结构体。
#[derive(clap::Subcommand)]
pub enum Command {
    // `StartBot` 子命令: 用于启动套利机器人。
    // 它关联了 `start_bot::Args` 结构体，该结构体定义了启动机器人所需的参数。
    StartBot(start_bot::Args),

    // `Run` 子命令: 可能用于执行一次性的套利分析、特定路径测试或模拟。
    // 它关联了 `arb::Args` 结构体。
    Run(arb::Args),

    // `PoolIds` 子命令: 用于生成包含所有DEX池对象ID及其相关对象ID的文件。
    // 这对于初始化或调试非常有用。
    // 它关联了 `pool_ids::Args` 结构体。
    #[command(about = "生成包含所有池对象ID及其底层对象的文件")] // 为子命令添加描述
    PoolIds(pool_ids::Args),
}

/// `main` 函数 (程序主入口)
///
/// `#[tokio::main]` 宏将 `main` 函数转换为一个异步函数，并使用Tokio运行时来执行它。
/// Tokio是一个流行的Rust异步运行时。
///
/// 返回:
/// - `Result<()>`: 表示 `main` 函数可能返回一个错误 (由 `eyre` 库处理)。
///   如果成功执行完毕，则返回 `Ok(())`。
#[tokio::main]
async fn main() -> Result<()> {
    // 步骤 1: 解析命令行参数。
    // `Args::parse()` 是 `clap` 提供的函数，它会根据 `Args` 结构体的定义来解析
    // 传入程序的命令行参数，并填充 `Args` 实例。
    // 如果参数解析失败（例如，用户提供了无效的参数或格式），`clap` 会自动显示帮助信息并退出程序。
    let args = Args::parse();

    // 步骤 2: 根据解析到的子命令，执行相应的逻辑。
    // 使用 `match` 语句来匹配 `args.command` 的不同变体。
    match args.command {
        // 如果是 `StartBot` 子命令，则调用 `start_bot` 模块的 `run` 函数，
        // 并将该子命令特有的参数 `args` (类型为 `start_bot::Args`) 传递给它。
        // `.await` 用于等待异步的 `run` 函数完成。
        Command::StartBot(args) => start_bot::run(args).await,

        // 如果是 `Run` 子命令，则调用 `arb` 模块的 `run` 函数。
        Command::Run(args) => arb::run(args).await,

        // 如果是 `PoolIds` 子命令，则调用 `pool_ids` 模块的 `run` 函数。
        Command::PoolIds(args) => pool_ids::run(args).await,
    }
    // `match` 语句的结果 (即对应 `run` 函数的 `Result<()>` 返回值) 会作为 `main` 函数的返回值。
    // 如果任何一个 `run` 函数返回错误，该错误会通过 `?` 操作符（如果 `run` 函数内部使用了的话）
    // 或直接作为 `Result` 从 `main` 函数返回，并可能由 `eyre` 库进行格式化输出。
}
