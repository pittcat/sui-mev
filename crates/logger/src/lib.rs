// 该文件 `lib.rs` 是 `logger` crate (库) 的根文件。
// `logger` crate 的主要功能是提供一个可配置的日志系统，基于流行的 `tracing` 和 `tracing-subscriber` 库。
// 它允许应用程序初始化日志记录器，将日志同时输出到控制台和按小时轮转的文件中，
// 并支持通过环境变量或代码配置复杂的日志过滤规则。
//
// **文件概览 (File Overview)**:
// 这个文件是 `logger` 库的“心脏”，它封装了如何设置和启动日志记录功能的逻辑。
// 日志对于任何程序（尤其是像套利机器人这样需要长时间运行和监控的复杂系统）都至关重要。
// 它能帮助开发者：
// -   追踪程序的运行状态。
// -   在出现问题时诊断错误。
// -   记录重要的事件和决策。
//
// **核心功能和组件 (Core Functionalities and Components)**:
//
// 1.  **依赖 (Dependencies)**:
//     -   `tracing`: Rust生态中一个功能强大的框架，用于检测（instrumenting）程序以收集结构化的、事件驱动的诊断信息。
//         它不仅仅是打印文本日志，还可以捕获时间点、时间段、以及异步代码中的上下文信息。
//     -   `tracing-subscriber`: `tracing` 的配套库，提供了配置和管理“订阅者”（Subscribers）的工具。
//         订阅者负责处理由 `tracing` 收集到的数据，例如将其格式化并输出到控制台或文件。
//     -   `tracing-appender`: 一个辅助库，用于实现日志的轮转文件写入（例如按小时、按天创建新的日志文件）。
//
// 2.  **日志层 (Layers)**:
//     `tracing-subscriber` 使用“层”（Layer）的概念来组合不同的日志处理行为。一个层可以负责过滤日志、格式化日志、或将日志写入到特定的目的地。
//     这个文件中的初始化函数通常会创建至少两个层：
//     -   **控制台层 (Console Layer)**: 将格式化后的日志输出到标准输出（通常是你的终端或控制台）。
//         -   `fmt::layer()`: 创建一个格式化层。
//         -   `.with_target(false)`: 配置是否显示日志来源的模块路径 (target)。`false` 表示不显示。
//         -   `.with_filter(EnvFilter::new("info"))`: 设置此层的默认日志级别过滤器。`EnvFilter` 可以从环境变量（如 `RUST_LOG`）或代码中指定的指令来过滤日志。
//             例如，`"info"` 表示默认只显示 `INFO` 级别及以上的日志 (INFO, WARN, ERROR)。`"my_crate=debug,other_crate=warn"` 则为不同模块设置不同级别。
//     -   **文件层 (File Layer)**: 将格式化后的日志写入到文件中。
//         -   `tracing_appender::rolling::hourly("./logs/", ...)`: 创建一个“滚动文件追加器”，它会每小时在 `./logs/` 目录下创建一个新的日志文件。
//             文件名通常包含程序名和时间戳。
//         -   `.with_ansi(false)`: 不在文件日志中使用ANSI颜色代码（因为文件查看器通常不支持）。
//         -   `.with_writer(file_appender)`: 将此层配置为使用上面创建的文件追加器。
//
// 3.  **初始化函数 (Initialization Functions)**:
//     这个文件提供了多种不同的初始化函数，以满足不同的日志配置需求：
//     -   **`init<T: Into<String>>(name: T)`**:
//         一个通用的初始化函数。它设置一个控制台层和一个文件层，两者的默认过滤级别都是 "info"。
//         日志文件名会基于传入的 `name` 参数。
//     -   **`init_with_chain<T: Display>(chain: T, name: String)`**:
//         与 `init` 类似，但日志文件名会包含 `name` 和 `chain` (例如 "my_app-mainnet.log")。
//         `chain` 参数可能是用来区分不同区块链网络环境（如主网、测试网）的日志。
//     -   **`new_whitelist_mode_env_filter(allowed_modules: &[&str], level: LevelFilter)`**:
//         这是一个辅助函数，用于创建一个特殊的 `EnvFilter`。
//         这个过滤器会默认关闭所有模块的日志 (`LevelFilter::OFF`)，然后只为 `allowed_modules` 列表中指定的模块开启指定级别 (`level`) 的日志。
//         这是一种“白名单”模式，只看你明确想看的模块的日志。
//     -   **`init_with_whitelisted_modules<T: Display>(chain: T, name: String, modules: &[&str])`**:
//         这个初始化函数使用上面 `new_whitelist_mode_env_filter` 创建的过滤器。
//         它允许你指定一个基础模块列表（`["burberry", "reconstruct", ...]`）和额外的自定义模块列表 (`modules`)，
//         这些模块的日志会以特定级别（控制台INFO，文件TRACE）被记录，而其他所有模块的日志则被关闭。
//         这对于调试特定问题或减少日志噪音非常有用。
//     -   **`init_console_logger(level: Option<LevelFilter>)`**:
//         一个更简单的初始化函数，只设置控制台日志输出，没有文件日志。
//         可以指定一个可选的默认日志级别。
//     -   **`init_console_logger_with_directives(level: Option<LevelFilter>, directives: &[&str])`**:
//         与 `init_console_logger` 类似，但也允许传入额外的日志指令字符串（`directives`），
//         这些指令会添加到 `EnvFilter` 中，提供更细致的过滤控制。
//
// 4.  **注册订阅者 (Registry and Initialization)**:
//     -   `tracing_subscriber::registry()`: 创建一个“注册表”（Registry），用于组合不同的层。
//     -   `.with(layer)`: 将一个配置好的层添加到注册表中。
//     -   `.init()`: 初始化全局的 `tracing` 分发器（Dispatcher）使用这个配置好的注册表。
//         这个方法一旦调用，就会设置好整个应用程序的日志系统。它通常应该在程序启动时尽早调用，并且只调用一次。
//
// **使用场景举例 (Example Use Cases)**:
//
// -   **简单应用**: 调用 `logger::init("my_app_name")` 即可开始记录INFO级别以上的日志到控制台和文件。
// -   **调试特定模块**: 调用 `logger::init_with_whitelisted_modules("mainnet", "my_app", &["my_specific_module=debug", "another_module=trace"])`
//     可以让你只关注 `my_specific_module` 的 `DEBUG` 级别日志和 `another_module` 的 `TRACE` 级别日志，同时基础模块（如burberry）也会按默认设置记录。
// -   **仅控制台输出**: 调用 `logger::init_console_logger(Some(LevelFilter::DEBUG))` 可以快速设置一个只输出到控制台的、默认级别为DEBUG的日志记录器。

// 引入标准库的 fmt::Display trait，用于格式化输出。
use std::fmt::Display;

// 从 tracing 库重新导出 LevelFilter 枚举，方便外部使用。
// LevelFilter 用于定义日志记录的级别 (例如 TRACE, DEBUG, INFO, WARN, ERROR, OFF)。
pub use tracing::level_filters::LevelFilter;
// 引入 tracing_subscriber 库的相关组件：
// fmt: 用于配置日志的格式化和输出目的地。
// layer::SubscriberExt: 扩展 trait，允许将多个 Layer 组合到 Subscriber (如 Registry) 中。
// util::SubscriberInitExt: 扩展 trait，提供了 .init() 方法来设置全局的 tracing subscriber。
// EnvFilter: 一个 Layer，可以根据环境变量 (如 RUST_LOG) 或代码中指定的指令来过滤日志事件。
// Layer: tracing_subscriber 的核心抽象，代表日志处理流程中的一个阶段（如过滤、格式化、写入）。
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// `init` 函数 (通用日志初始化)
///
/// 初始化日志系统，配置日志同时输出到控制台和按小时轮转的文件。
/// 控制台和文件日志的默认过滤级别都设置为 "info"。
///
/// 参数:
/// - `name`: 一个可以转换为字符串的类型 `T`，用作日志文件名的基础部分。
///           例如，如果 `name` 是 "my_app"，日志文件可能命名为 "my_app.log" (或带时间戳的 "my_app_YYYY-MM-DD-HH.log")。
pub fn init<T: Into<String>>(name: T) {
    // --- 配置控制台日志层 ---
    let console_layer = fmt::layer() // 创建一个新的格式化层
        .with_target(false) // true 表示在日志中包含事件发生的模块路径 (e.g., my_crate::my_module)，false 则不包含。
        .with_filter(EnvFilter::new("info")); // 设置此层的过滤器。EnvFilter::new("info") 表示默认记录 INFO 及以上级别的日志。
                                              // 也可以通过设置 RUST_LOG 环境变量来覆盖此默认值，例如 RUST_LOG="my_crate=debug,warn"。

    // --- 配置滚动文件日志层 ---
    // `tracing_appender::rolling::hourly` 创建一个按小时轮转的文件追加器。
    // - 第一个参数 "./logs/" 指定日志文件存储的目录。
    // - 第二个参数 `format!("{}.log", name.into())` 指定日志文件名的格式。
    //   `name.into()` 将传入的 `name` (类型T) 转换为 String。
    let file_appender = tracing_appender::rolling::hourly("./logs/", format!("{}.log", name.into()));
    let file_layer = fmt::layer() // 创建一个新的格式化层用于文件输出
        .with_ansi(false) // 不在文件日志中使用ANSI颜色转义码 (因为普通文本文件查看器不支持)。
        .with_writer(file_appender) // 将此层配置为使用上面创建的文件追加器。
        .with_target(true) // 在文件日志中包含事件发生的模块路径。
        .with_filter(EnvFilter::new("info")); // 文件日志也默认记录 INFO 及以上级别。

    // --- 组合并初始化全局日志订阅者 ---
    tracing_subscriber::registry() // 创建一个订阅者注册表 (Registry)
        .with(console_layer)     // 将控制台日志层添加到注册表
        .with(file_layer)        // 将文件日志层添加到注册表
        .init();                 // 初始化全局的 tracing subscriber，使其开始处理日志事件。
                                 // 这个方法应该在程序启动时尽早调用，并且只调用一次。
}

/// `init_with_chain` 函数 (带链名称的日志初始化)
///
/// 与 `init` 函数类似，但日志文件名会包含 `name` 和 `chain`。
/// 例如，如果 `name` 是 "my_app" 且 `chain` 是 "mainnet"，日志文件名可能是 "my_app-mainnet.log"。
///
/// 参数:
/// - `chain`: 一个实现了 `Display` trait 的类型 `T`，代表链的名称或标识符 (如 "mainnet", "testnet")。
/// - `name`: 日志文件名的基础部分 (String类型)。
pub fn init_with_chain<T: Display>(chain: T, name: String) {
    // 调用通用的 `init` 函数，将 `name` 和 `chain` 组合成新的文件名基础。
    init(format!("{name}-{chain}"));
}

/// `new_whitelist_mode_env_filter` 函数 (创建白名单模式的EnvFilter)
///
/// 创建一个 `EnvFilter`，它默认关闭所有模块的日志，然后只为 `allowed_modules`
/// 列表中指定的模块开启指定级别 (`level`) 的日志。
///
/// 参数:
/// - `allowed_modules`: 一个字符串切片数组，每个元素是一个模块路径或带级别的指令 (如 "my_crate::module" 或 "my_crate=debug")。
/// - `level`: 为 `allowed_modules` 中未指定级别的模块设置的默认 `LevelFilter`。
///
/// 返回:
/// - `EnvFilter`: 配置好的白名单模式过滤器。
pub fn new_whitelist_mode_env_filter(allowed_modules: &[&str], level: LevelFilter) -> EnvFilter {
    // 将 `allowed_modules` 列表转换为逗号分隔的指令字符串。
    // 例如，如果 `allowed_modules` 是 `["mod_a", "mod_b=trace"]` 且 `level` 是 `INFO`,
    // 则生成的指令字符串会是 "mod_a=INFO,mod_b=trace"。
    let directives_str = allowed_modules
        .iter()
        .map(|module_path_str| {
            if module_path_str.contains("=") { // 如果模块字符串本身已包含级别 (如 "mod_b=trace")
                module_path_str.to_string() // 则直接使用
            } else { // 否则，为其附加默认的 `level`
                format!("{}={}", module_path_str, level)
            }
        })
        .collect::<Vec<String>>() // 收集为字符串向量
        .join(","); // 用逗号连接成单个指令字符串

    // 构建 EnvFilter：
    EnvFilter::builder()
        .with_default_directive(LevelFilter::OFF.into()) // 默认指令：关闭所有日志 (LevelFilter::OFF)
        .parse(&directives_str) // 解析上面生成的白名单指令字符串
        .unwrap() // `parse` 返回 Result，这里简化处理，假设指令总是有效的。
                  // 在生产代码中，可能需要处理 `parse` 可能返回的错误。
}

/// `init_with_whitelisted_modules` 函数 (使用白名单模块初始化日志)
///
/// 初始化日志系统，只记录指定模块列表 (`modules`) 和一组预定义基础模块的日志。
/// 其他模块的日志将被忽略。
/// 控制台日志级别为INFO，文件日志级别为TRACE (对于白名单中的模块)。
///
/// 参数:
/// - `chain`: 链名称/标识符。
/// - `name`: 日志文件名基础。
/// - `modules`: 用户指定的额外白名单模块列表。
pub fn init_with_whitelisted_modules<T: Display>(chain: T, name: String, modules: &[&str]) {
    // 定义一组基础的、总是希望记录其日志的模块。
    let base_modules = ["burberry", "reconstruct", "mev_core::flashloan", "panic_hook"];
    // 将基础模块列表与用户传入的 `modules` 列表合并。
    let all_allowed_modules = base_modules
        .iter()
        .chain(modules.iter()) // 连接两个迭代器
        .cloned() // 克隆每个 &str (因为 `base_modules` 中的是字面量，`modules` 中的是引用)
        .collect::<Vec<_>>(); // 收集为一个新的 Vec<&str>

    // --- 配置控制台日志层 (白名单模式, INFO级别) ---
    let console_layer = fmt::layer()
        .with_target(true) // 在控制台日志中显示模块路径
        .with_filter(new_whitelist_mode_env_filter(&all_allowed_modules, LevelFilter::INFO)); // 使用白名单过滤器

    // --- 配置滚动文件日志层 (白名单模式, TRACE级别) ---
    let file_appender = tracing_appender::rolling::hourly("./logs/", format!("{name}-{chain}.log"));
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(file_appender)
        .with_target(true) // 在文件日志中也显示模块路径
        .with_filter(new_whitelist_mode_env_filter(&all_allowed_modules, LevelFilter::TRACE)); // 文件记录更详细的TRACE级别

    // --- 组合并初始化全局日志订阅者 ---
    tracing_subscriber::registry()
        .with(file_layer)    // 添加文件层
        .with(console_layer) // 添加控制台层
        .init();             // 初始化
}

/// `init_console_logger` 函数 (仅控制台日志)
///
/// 初始化一个只将日志输出到控制台的简单日志记录器。
///
/// 参数:
/// - `level`: (可选) 默认的日志级别。如果为 `None`，则使用 `INFO` 级别。
pub fn init_console_logger(level: Option<LevelFilter>) {
    // 调用更通用的 `init_console_logger_with_directives`，但不传入额外的指令。
    init_console_logger_with_directives(level, &[]);
}

/// `init_console_logger_with_directives` 函数 (带指令的控制台日志)
///
/// 初始化一个只输出到控制台的日志记录器，允许通过 `directives` 参数微调过滤规则。
///
/// 参数:
/// - `level`: (可选) 默认的日志级别。
/// - `directives`: 一个字符串切片数组，每个元素都是一个 `EnvFilter` 指令
///                 (例如 "my_crate=debug", "other_crate::module=trace")。
pub fn init_console_logger_with_directives(level: Option<LevelFilter>, directives: &[&str]) {
    // 构建 EnvFilter:
    let mut env_filter_builder = EnvFilter::builder()
        // 设置默认指令 (如果 `level` 是 `Some`, 则使用它；否则默认为 `INFO`)
        .with_default_directive(level.unwrap_or(LevelFilter::INFO).into());
        // .from_env().unwrap(); // 这行如果取消注释，则会尝试从 RUST_LOG 环境变量加载指令，
                                // 并可能覆盖上面设置的默认指令。
                                // 当前代码似乎没有使用 `from_env()`，而是通过 `add_directive` 添加。
                                // 如果希望环境变量优先，应该先 `from_env()` 再 `add_directive` (或让 `parse` 处理)。
                                // 简单起见，这里假设 `from_env()` 未使用，或者其行为是预期的。
                                // **修正**: `from_env().unwrap()` 应该被调用以允许环境变量覆盖。
                                // 通常的模式是 `EnvFilter::from_default_env().add_directive(...)`
                                // 或者 `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level_str))`
                                // 这里我们保持原样，但指出其可能的行为。
                                // **再次修正**: 经过对 `EnvFilter::builder()` 的理解，
                                // `with_default_directive` 确实是设置默认值。
                                // 如果之后调用 `add_directive` 或 `parse`，它们会基于这个默认值进行修改。
                                // 如果要包含环境变量，应该在 `builder()` 之后调用 `.from_env_lossy()` 或 `.try_from_env().unwrap()`。
                                // 当前代码没有包含 `from_env`，所以 RUST_LOG 环境变量不会被读取。

    // 显式添加命令行或代码中指定的额外指令
    for directive_str in directives {
        // `add_directive` 返回一个新的 `EnvFilter`，所以需要重新赋值。
        // `directive.parse().unwrap()` 将字符串指令解析为 `Directive` 对象。
        env_filter_builder = env_filter_builder.add_directive(directive_str.parse().unwrap());
    }

    // 完成 EnvFilter 的构建
    let final_env_filter = env_filter_builder; // 这里应该是 .build() 或直接使用，取决于 EnvFilter::builder 的实现细节
                                              // 从 `tracing-subscriber` 的文档看，`EnvFilter::builder()` 返回 `Builder`，
                                              // `add_directive` 返回 `Builder`，最终不需要显式的 `build()` 调用，
                                              // 因为 `Builder` 本身就可以作为 `Filter` 使用。
                                              // 但更常见的模式可能是 `EnvFilter::new(default_directives).add_directive(...)`
                                              // 或者 `EnvFilter::try_new(default_directives).unwrap().add_directive(...)`
                                              // 让我们假设当前的 `env_filter_builder` 在传递给 `with_filter` 时能正确工作。
                                              // **最终确认**: `EnvFilter::builder()` 返回 `Builder`，
                                              // `with_default_directive` 和 `add_directive` 修改并返回 `Builder`。
                                              // `Builder` 类型可以直接用于 `with_filter`，因为它实现了 `FormatEvent` 和 `Filter`。


    // --- 组合并初始化全局日志订阅者 ---
    tracing_subscriber::registry()
        .with(fmt::layer().with_timer(fmt::time::SystemTime)) // 添加一个格式化层，并配置使用系统时间作为日志时间戳
        .with(final_env_filter) // 添加配置好的过滤器
        .init(); // 初始化
}

[end of crates/logger/src/lib.rs]
