// 该文件 `lib.rs` 是 `utils` crate (库) 的根文件和主入口。
// `utils` crate 的设计目标是提供一系列在整个套利机器人项目中可能被多个其他crate或模块
// 共用的通用工具函数和辅助功能。
//
// **文件概览 (File Overview)**:
// 这个文件是 `utils` 工具库的“总指挥部”。它做了两件主要的事情：
// 1.  **声明并导出子模块 (Declaring and Exporting Submodules)**:
//     通过 `pub mod coin;` 这样的语句，它将 `coin.rs`, `heartbeat.rs` 等文件定义为自己的子模块，
//     并使其内容可以被其他使用了 `utils` crate 的项目所访问。
//     例如，其他项目可以通过 `utils::coin::get_gas_coin_refs()` 来调用 `coin.rs` 中定义的函数。
//
// 2.  **定义通用工具函数 (Defining Common Utility Functions)**:
//     -   `set_panic_hook()`: 设置一个全局的panic钩子，用于在程序发生panic（意外崩溃）时，
//         捕获panic信息，并尝试通过Telegram将这些错误信息发送出去，同时也记录到日志中。
//         这对于在生产环境中快速发现和诊断问题非常有用。
//     -   `send_panic_to_telegram()`: (私有辅助函数) 实际负责将panic信息格式化并发送到Telegram。
//         它会处理命令行参数（可能包含敏感信息如私钥，所以会进行脱敏处理）和panic的详细信息。
//         **特别注意**: 此函数内部也使用了 `run_in_tokio!` 类似的逻辑 (通过 `Handle::try_current()` 判断并可能创建新的Tokio运行时)
//         来异步发送Telegram消息，确保即使在非异步或不同异步上下文的panic处理器中也能工作。
//     -   `current_time_ms()`: 返回当前的Unix时间戳（自1970年1月1日以来的毫秒数）。
//     -   `new_test_sui_client()`: (异步函数) 创建并返回一个新的 `SuiClient` 实例，
//         但它尝试连接的RPC URL是**空字符串 `""`**。这通常意味着它依赖于 `SuiClientBuilder` 的默认行为
//         （例如，从环境变量 `SUI_RPC_URL` 读取，或者如果 `SUI_GATEWAY_URL` 也存在，则可能报错）。
//         此函数名中带有 "test"，表明它主要用于测试环境，并且调用者需要确保RPC URL已通过其他方式（如环境变量）正确配置。
//         如果URL无效，`.unwrap()` 会导致panic。
//
// **核心功能和组件 (Core Functionalities and Components)**:
//
// -   **Panic Hook (Panic钩子)**:
//     -   Rust程序在遇到不可恢复的错误时会发生“panic”。默认情况下，panic信息会打印到标准错误输出。
//     -   `std::panic::set_hook()` 允许开发者自定义panic发生时的行为。
//     -   `set_panic_hook()` 函数就利用了这一点，它设置了一个钩子：
//         1.  获取当前线程名和panic发生的位置（文件名和行号）。
//         2.  获取panic信息本身。
//         3.  获取当前程序的命令行参数，并对长度超过32个字符的参数进行脱敏处理（替换为"[REDACTED]"），
//             这主要是为了防止私钥等敏感信息意外泄露到日志或Telegram消息中。
//         4.  将这些信息格式化为一个错误消息字符串。
//         5.  调用 `send_panic_to_telegram()` 将格式化后的错误消息发送出去。
//         6.  使用 `tracing::error!` 将错误消息记录到日志系统中。
//
// -   **异步Telegram发送 (Asynchronous Telegram Sending in Panic Hook)**:
//     -   `send_panic_to_telegram()` 函数本身不是异步的（因为它是在panic钩子这种同步上下文中被调用的），
//         但它需要调用一个异步的 `telegram_dispatcher.send_message()` 方法来发送Telegram消息。
//     -   为了解决这个问题，它使用了与 `run_in_tokio!` 宏 (在 `strategy/mod.rs` 中定义) 类似的逻辑：
//         -   `Handle::try_current()`: 检查当前线程是否已经在Tokio运行时上下文中。
//         -   如果是，则根据运行时类型（单线程或多线程）使用 `std::thread::scope` + 新的 `CurrentThread` 运行时，
//             或者 `tokio::task::block_in_place` + `handle.block_on` 来阻塞执行异步发送。
//         -   如果不是，则创建一个新的 `CurrentThread` Tokio运行时，并在其中 `block_on` 执行异步发送。
//         这种处理确保了无论panic发生在何种线程（Tokio管理的异步线程或普通的同步线程），
//         都能尝试异步发送Telegram消息。
//
// -   **Telegram常量**:
//     -   `send_panic_to_telegram()` 函数中使用了 `R2D2_TELEGRAM_BOT_TOKEN`, `CHAT_MONEY_PRINTER`, `CHAT_MONEY_PRINTER_THREAD_ERROR_REPORT` 这些常量。
//         它们是从当前crate的 `telegram` 子模块中导入的，定义了用于发送错误报告的Telegram机器人的Token、目标聊天ID和话题ID。
//
// **用途 (Purpose in Project)**:
// -   **代码复用**: 将这些通用的功能（如获取时间、创建测试客户端、统一的panic处理）放在 `utils` crate中，
//     可以避免在项目的其他部分重复编写相同的代码。
// -   **标准化**: 提供标准的panic处理行为，确保所有panic都能被记录并通过Telegram通知。
// -   **便捷性**: 封装一些常用操作，如 `current_time_ms()`，方便其他模块调用。

// --- 声明并导出子模块 ---
// `pub mod coin;` 表示当前crate中有一个名为 `coin` 的子模块，其代码在 `coin.rs` 文件中。
// `pub` 关键字使得 `coin` 模块及其公共项可以被其他使用了 `utils` crate 的项目访问。
pub mod coin;      // 与代币操作相关的工具函数
pub mod heartbeat; // 心跳服务相关逻辑
pub mod link;      // (可能) 用于生成Sui浏览器链接或其他链接的工具
pub mod object;    // 与Sui对象操作相关的工具函数
pub mod telegram;  // 与Telegram消息发送相关的常量或工具

// 引入 burberry 框架中用于Telegram消息发送的组件。
use burberry::executor::telegram_message::{escape, MessageBuilder, TelegramMessageDispatcher};
// 引入 Sui SDK 中的 SuiClient (RPC客户端) 和 SuiClientBuilder (客户端构建器)。
use sui_sdk::{SuiClient, SuiClientBuilder};
// 引入 Tokio 运行时相关的组件，用于在同步代码中执行异步操作。
use tokio::runtime::{Builder as TokioRuntimeBuilder, Handle as TokioHandle, RuntimeFlavor as TokioRuntimeFlavor};
// 引入 tracing 库的 error! 宏，用于记录错误日志。
use tracing::error;

// 从当前crate的 `telegram` 子模块中导入所有公共项 (主要是Telegram相关的常量)。
use crate::telegram::*;

/// `set_panic_hook` 函数
///
/// 设置一个全局的panic钩子，用于捕获程序中发生的panic，
/// 记录详细信息，并通过Telegram发送通知。
pub fn set_panic_hook() {
    // `std::panic::set_hook` 注册一个新的panic处理器。
    // `Box::new(move |info| { ... })` 创建一个实现了 `Fn(PanicInfo)` 的闭包，并将其装箱。
    // `move` 关键字确保闭包捕获其环境中的变量（如果有的话）的所有权。
    std::panic::set_hook(Box::new(move |panic_info| {
        // --- 1. 获取命令行参数并进行脱敏处理 ---
        // `std::env::args()` 获取程序启动时的命令行参数。
        let command_line_args = std::env::args()
            .map(|arg_str| {
                // 如果参数长度超过32个字符，则替换为 "[REDACTED]"，以防止私钥等敏感信息泄露。
                if arg_str.len() > 32 {
                    "[REDACTED]".to_string()
                } else {
                    arg_str
                }
            })
            .collect::<Vec<_>>() // 收集为字符串向量
            .join(" "); // 用空格连接成单个字符串

        // --- 2. 获取发生panic的线程信息 ---
        let current_thread = std::thread::current();
        let thread_name = current_thread.name().unwrap_or("<未命名线程>"); // 获取线程名，如果未命名则使用默认值

        // --- 3. 获取panic的核心消息 ---
        // `panic_info.payload()` 返回一个 `&dyn Any + Send`，代表panic时传递的实际值。
        // 通常它是一个字符串字面量 (`&'static str`) 或一个 `String` 对象。
        let panic_message = match panic_info.payload().downcast_ref::<&'static str>() {
            Some(static_str_payload) => *static_str_payload, // 如果是 &'static str
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(string_payload) => &**string_payload, // 如果是 String (注意解引用)
                None => "Box<Any>", // 如果是其他未知类型，则使用 "Box<Any>" 作为占位符
            },
        };

        // --- 4. 获取panic发生的位置信息 (文件名和行号) ---
        let error_message_formatted = match panic_info.location() {
            Some(location_info) => { // 如果能获取到位置信息
                format!(
                    "线程 '{}' 发生panic: '{}', 位置: {}:{}",
                    thread_name,
                    panic_message,
                    location_info.file(), // 文件名
                    location_info.line(), // 行号
                )
            }
            None => { // 如果获取不到位置信息
                format!("线程 '{}' 发生panic: '{}'", thread_name, panic_message)
            }
        };

        // --- 5. 发送panic信息到Telegram ---
        send_panic_to_telegram(&command_line_args, &error_message_formatted);
        // --- 6. 将panic信息记录到日志系统中 ---
        // `target: "panic_hook"` 指定日志的来源目标，方便过滤和查找。
        error!(target: "panic_hook", "捕获到Panic: {}", error_message_formatted);
    }));
}

/// `send_panic_to_telegram` 函数 (私有辅助函数)
///
/// 负责将格式化后的panic信息通过Telegram发送出去。
/// 它会处理在不同Tokio运行时上下文中异步发送消息的情况。
fn send_panic_to_telegram(cmdline_str: &str, msg_str: &str) {
    // 创建一个Telegram消息调度器实例。
    // `None` 参数可能表示使用默认的配置或不使用特定的HTTP客户端/重试策略。
    let telegram_dispatcher = TelegramMessageDispatcher::new(None, None, None);
    // 格式化要发送的Telegram消息内容，包含命令行参数和panic错误信息。
    // `escape` 函数用于转义Markdown特殊字符，防止在Telegram中显示错乱。
    let escaped_message_text = escape(&format!("命令: {:?}\n错误: {:?}", cmdline_str, msg_str));
    // 使用 `MessageBuilder` 构建Telegram消息对象。
    let telegram_message_to_send = MessageBuilder::new()
        .bot_token(R2D2_TELEGRAM_BOT_TOKEN) // 使用预定义的机器人Token (在 telegram.rs 中定义)
        .chat_id(CHAT_MONEY_PRINTER)      // 使用预定义的目标聊天ID
        .thread_id(CHAT_MONEY_PRINTER_THREAD_ERROR_REPORT) // 使用预定义的错误报告话题ID
        .text(&escaped_message_text)         // 设置消息文本
        .disable_link_preview(true)          // 禁用链接预览
        .disable_notification(true)          // (可选) 禁用消息通知，使其成为静默消息
        .build();                            // 构建消息

    // --- 在当前线程的Tokio运行时中或新建一个运行时来异步发送消息 ---
    // 这是因为 `std::panic::set_hook` 的闭包是同步执行的，但发送Telegram消息是异步操作。
    match TokioHandle::try_current() { // 尝试获取当前线程的Tokio运行时句柄
        Ok(tokio_handle_ref) => { // 如果当前线程在Tokio运行时上下文中
            match tokio_handle_ref.runtime_flavor() { // 检查运行时类型
                TokioRuntimeFlavor::CurrentThread => { // 如果是单线程运行时 (CurrentThread)
                    // 在单线程运行时中直接 `block_on` 可能会导致问题（例如，如果它本身就是被 `block_on` 调用的）。
                    // 因此，创建一个新的标准库线程，并在该线程中创建一个新的 `CurrentThread` Tokio运行时来执行异步发送。
                    // `std::thread::scope` 用于确保新线程在其作用域结束前完成。
                    std::thread::scope(move |scoped_thread| {
                        scoped_thread.spawn(move || { // 创建并启动新线程
                            TokioRuntimeBuilder::new_current_thread() // 创建新的单线程Tokio运行时
                                .enable_all() // 启用所有Tokio特性 (如IO, time)
                                .build()
                                .unwrap() // 假设运行时构建成功
                                .block_on(async move { // 在新运行时中阻塞执行异步发送
                                    telegram_dispatcher.send_message(telegram_message_to_send).await; // 发送消息
                                })
                        })
                        .join() // 等待新线程完成
                        .unwrap(); // 处理线程join的Result
                    })
                },
                _ => { // 如果是多线程运行时 (例如 TokioRuntimeFlavor::MultiThread)
                    // `tokio::task::block_in_place` 允许在异步任务中执行阻塞代码，而不会阻塞整个线程池。
                    // 它会将当前任务从Tokio调度器中移出，执行闭包中的阻塞代码，然后再移回。
                    tokio::task::block_in_place(move || {
                        tokio_handle_ref.block_on(async move { // 使用当前运行时的句柄来阻塞执行异步发送
                            telegram_dispatcher.send_message(telegram_message_to_send).await;
                        })
                    })
                },
            }
        }
        Err(_) => { // 如果当前线程不在Tokio运行时上下文中 (例如，panic发生在非Tokio管理的线程)
            // 创建一个新的 `CurrentThread` Tokio运行时来执行异步发送。
            TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move { // 阻塞执行异步发送
                    telegram_dispatcher.send_message(telegram_message_to_send).await;
                })
        },
    }
}

/// `current_time_ms` 函数
///
/// 返回当前的Unix时间戳（自1970年1月1日UTC以来的毫秒数）。
///
/// 返回:
/// - `u64`: 毫秒级Unix时间戳。
pub fn current_time_ms() -> u64 {
    std::time::SystemTime::now() // 获取当前系统时间
        .duration_since(std::time::UNIX_EPOCH) // 计算自UNIX纪元以来的时长
        .unwrap() // `duration_since` 在时间早于UNIX_EPOCH时返回Err，这里假设不会发生
        .as_millis() as u64 // 将时长转换为毫秒，并强制转换为u64
}

/// `new_test_sui_client` 异步函数
///
/// 创建并返回一个新的 `SuiClient` 实例，用于测试。
/// **重要**: 它尝试连接的RPC URL是空字符串 `""`。
/// 这依赖于 `SuiClientBuilder` 的默认行为，例如从环境变量 `SUI_RPC_URL` 读取。
/// 如果没有有效的RPC URL配置，此函数在 `.build("").await` 时会失败并panic (因为 `.unwrap()`)。
///
/// 返回:
/// - `SuiClient`: 新创建的Sui客户端实例。
pub async fn new_test_sui_client() -> SuiClient {
    SuiClientBuilder::default() // 使用默认的SuiClientBuilder配置
        .build("") // 尝试构建客户端，连接到空字符串URL (依赖默认行为或环境变量)
        .await // 等待异步构建完成
        .unwrap() // 如果构建失败 (例如URL无效或无法连接)，则panic
}

[end of crates/utils/src/lib.rs]
