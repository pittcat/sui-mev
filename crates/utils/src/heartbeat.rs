// 该文件 `heartbeat.rs` (位于 `utils` crate中) 定义了一个简单的心跳服务。
// 心跳服务通常用于长时间运行的应用程序，以定期发出信号，表明应用程序仍在正常运行。
// 这些信号可以被外部监控系统捕获，用于检测应用程序是否健康，或者在出现问题时触发警报。
//
// **文件概览 (File Overview)**:
// 这个文件是 `utils` 工具库中的一个“健康报告员”。
// 它的主要目的是提供一个简单的方法来启动一个后台任务，这个任务会定期“喊一声”，
// 告诉外界：“我还活着，一切正常！”。
//
// **核心功能 (Key Functions)**:
//
// 1.  **`start<T: Into<String>>(service_id: T, interval: Duration) -> JoinHandle<()>`**:
//     -   **功能**: 启动心跳服务的公共接口。
//     -   **参数**:
//         -   `service_id: T`: 一个可以转换为字符串的泛型参数，用作此心跳服务的唯一标识符。
//             例如，可以是应用程序的名称或特定组件的名称，如 "sui-arb-bot" 或 "data-indexer-service"。
//             这个ID会用于日志记录，也可能用于发送给监控系统的心跳信号中。
//         -   `interval: Duration`: 心跳信号发出的时间间隔 (例如，每30秒发一次)。
//     -   **实现**:
//         1.  将传入的 `service_id` 转换为 `String`。
//         2.  调用 `tokio::spawn(worker(id, interval))` 在Tokio运行时中启动一个新的异步后台任务。
//             `worker` 函数是实际执行心跳逻辑的异步函数。
//         3.  返回这个新创建的Tokio任务的 `JoinHandle<()>`。
//             `JoinHandle` 可以用来等待任务完成（尽管心跳任务通常是无限循环的）或中止任务。
//             返回 `()` 表示这个任务在正常完成时不会返回任何有意义的值。
//
// 2.  **`worker(id: String, interval: Duration)` 异步函数**:
//     -   **功能**: 这是实际执行心跳逻辑的后台任务。
//     -   **参数**:
//         -   `id: String`: 心跳服务的标识符。
//         -   `interval: Duration`: 心跳间隔。
//     -   **实现 (当前为占位符)**:
//         1.  记录一条INFO级别的日志，表明心跳工作线程已为指定的 `id` 启动。
//         2.  **核心逻辑占位符**: `// write your code here`
//             **预期的逻辑应该是**:
//             ```rust
//             let mut timer = tokio::time::interval(interval); // 创建一个定时器
//             loop {
//                 timer.tick().await; // 等待下一个时间点
//                 // 在这里执行实际的心跳操作，例如：
//                 // 1. 记录一条日志，表明服务仍在运行。
//                 //    info!("服务 '{}' 正在发送心跳信号...", id);
//                 // 2. (可选) 向外部监控系统发送一个HTTP请求或消息队列消息。
//                 //    مثال: client.post("https://monitoring.example.com/heartbeat").json(&json!({"service_id": id, "status": "alive"})).send().await;
//                 // 3. (可选) 更新某个共享状态或指标。
//             }
//             ```
//             这个循环会定期执行，每次执行时就代表一次“心跳”。
//
// **用途 (Purpose)**:
// -   **健康监控**: 外部监控系统可以检查心跳信号是否在预期的时间间隔内持续到达。如果心跳停止，
//     监控系统可以假定应用程序或服务遇到了问题，并发出警报通知管理员。
// -   **服务保活**: 在某些环境中（如基于systemd的服务管理），持续的活动（如日志输出或网络请求）
//     可以被用来判断服务是否仍在运行，从而避免服务被错误地认为是僵死而被重启。
// -   **简单状态指示**: 即使没有复杂的监控系统，定期的心跳日志也可以为开发者或操作员提供一个快速判断服务是否仍在运行的简单方法。
//
// **当前实现的局限性**:
// -   `worker` 函数的核心逻辑 (`// write your code here`) 尚未实现。它只是打印了一条启动日志。
//     一个完整的心跳服务需要在这里实现实际的信号发送或状态更新逻辑。
// -   错误处理：如果心跳操作（如发送HTTP请求）失败，当前的 `worker` 函数没有明确的错误处理或重试逻辑。
// -   优雅关闭：当前 `worker` 是一个无限循环，没有提供优雅关闭的机制。
//     如果需要让心跳服务能够被外部信号停止，通常需要结合使用如 `tokio::sync::watch` 或 `tokio::select!` 与一个关闭信号通道。

// 引入标准库的 time::Duration，用于表示时间间隔。
use std::time::Duration;

// 引入 Tokio 库的 JoinHandle，用于表示一个已启动的异步任务的句柄。
use tokio::task::JoinHandle;
// 引入 tracing 库的日志宏 (debug, error, info)。
use tracing::{debug, error, info};

/// `start` 函数 (启动心跳服务)
///
/// 启动一个新的异步后台任务，该任务会定期执行心跳逻辑。
///
/// 参数:
/// - `service_id`: 一个可以转换为字符串的泛型参数 `T`，用作此心跳服务的唯一标识符。
///                 例如，可以是应用程序或组件的名称。
/// - `interval`: `Duration` 类型，指定心跳信号发出的时间间隔。
///
/// 返回:
/// - `JoinHandle<()>`: 返回一个Tokio任务的句柄。
///   可以用来等待任务完成（尽管心跳任务通常是无限循环）或中止任务。
///   `()` 表示任务在正常完成时不返回任何值。
pub fn start<T: Into<String>>(service_id: T, interval: Duration) -> JoinHandle<()> {
    // 将传入的 `service_id` 转换为 `String` 类型。
    let id_str = service_id.into();

    // 使用 `tokio::spawn` 在Tokio运行时中启动一个新的异步任务。
    // `worker(id_str, interval)` 是要执行的异步函数。
    // `move` 关键字将 `id_str` 和 `interval` 的所有权移入异步块。
    tokio::spawn(worker(id_str, interval))
}

/// `worker` 异步函数 (心跳后台任务)
///
/// 这是实际执行心跳逻辑的后台任务。
/// 它应该包含一个循环，在该循环中定期执行“心跳”操作。
///
/// 参数:
/// - `id`: 心跳服务的标识符字符串。
/// - `interval`: 心跳的时间间隔。
async fn worker(id: String, interval: Duration) {
    // 记录一条INFO级别的日志，表明心跳工作线程已为指定的 `id` 启动。
    info!("心跳服务工作线程已为 '{}' 启动，心跳间隔: {:?}", id, interval);

    // --- 实际的心跳逻辑应在此处实现 ---
    // 例如:
    // 1. 创建一个定时器:
    //    let mut timer = tokio::time::interval(interval);
    // 2. 进入无限循环:
    //    loop {
    //        timer.tick().await; // 等待下一个时间点
    //        // 执行心跳操作:
    //        // - 记录日志
    //        // - 调用外部监控API
    //        // - 更新共享状态等
    //        debug!("服务 '{}' 发送心跳...", id);
    //    }
    // 当前实现是一个占位符，只打印启动日志。
    // (The actual heartbeat logic should be implemented here.)
    // (For example:)
    // (1. Create a timer:)
    // (   let mut timer = tokio::time::interval(interval);)
    // (2. Enter an infinite loop:)
    // (   loop {)
    // (       timer.tick().await; // Wait for the next tick)
    // (       // Perform heartbeat action:)
    // (       // - Log a message)
    // (       // - Call an external monitoring API)
    // (       // - Update shared state, etc.)
    // (       debug!("Service '{}' sending heartbeat...", id);)
    // (   })
    // (The current implementation is a placeholder that only prints a startup log.)
    // write your code here
    //
    // 以下是一个更完整的心跳worker示例实现：
    let mut heart_timer = tokio::time::interval(interval);
    loop {
        heart_timer.tick().await; // 等待定时器触发
        // 模拟心跳操作，例如记录日志或调用外部服务
        // 在实际应用中，这里可能会有错误处理和更复杂的逻辑
        if id.is_empty() { // 简单示例：如果ID为空则报错并可能退出（尽管这里不会退出循环）
            error!("心跳服务ID为空，这是一个无效状态。");
        } else {
            // 正常心跳日志
            // 使用 debug! 级别，因为心跳日志可能非常频繁，INFO级别可能过于嘈杂。
            // 对于关键服务，也可以考虑提升到INFO。
            debug!("心跳: 服务 '{}' 仍然存活。", id);
        }

        // 如果需要支持优雅关闭，可以在这里检查一个关闭信号，例如:
        // if shutdown_signal.notified().await {
        //     info!("服务 '{}' 收到关闭信号，心跳服务停止。", id);
        //     break;
        // }
    }
}

[end of crates/utils/src/heartbeat.rs]
