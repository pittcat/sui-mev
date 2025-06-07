// 该文件 `shio_collector.rs` (位于 `shio` crate中) 定义了 `ShioCollector` 结构体。
// `ShioCollector` 负责从 Shio MEV 协议的事件源 (通常是一个WebSocket feed) 接收MEV机会信息 (`ShioItem`)。
// 它实现了 `burberry::Collector` trait，使其可以被集成到 `burberry::Engine` 事件处理引擎中。
//
// **文件概览 (File Overview)**:
// 这个文件定义了一个叫做 `ShioCollector` 的“信息收集员”。
// Shio是一个MEV（矿工可提取价值）相关的协议，它会通过一个WebSocket服务（像一个直播间）
// 实时广播新出现的MEV机会。`ShioCollector` 的工作就是连接到这个“直播间”，
// 接收这些机会信息（封装在 `ShioItem` 里），然后把它们传递给机器人的其他部分（比如策略模块）进行分析。
//
// **核心组件 (Core Components)**:
// 1.  **`ShioCollector` 结构体**:
//     -   `receiver: Receiver<ShioItem>`: 这是一个异步通道 (`async_channel`) 的接收端。
//         `ShioCollector` 并不直接管理WebSocket连接的建立和消息解析，这些底层工作由 `shio_conn` 模块（特别是 `new_shio_conn` 函数）完成。
//         `new_shio_conn` 函数会建立连接，并在一个独立的异步任务中持续从WebSocket接收和解析消息，
//         然后通过一个异步通道将解析后的 `ShioItem` 发送出来。`ShioCollector` 持有的就是这个通道的接收端。
//
// 2.  **构造函数 (Constructors)**:
//     -   **`new_without_executor(wss_url: String, num_retries: Option<u32>) -> Self`**:
//         这个构造函数会创建一个新的 `ShioCollector`，并**主动**调用 `new_shio_conn` 来建立一个新的WebSocket连接。
//         它接收WebSocket服务器的URL和可选的连接重试次数。
//         函数名中的 `_without_executor` 暗示它创建的 `ShioCollector` 是独立的，可能不与一个匹配的 `ShioExecutor`
//         （用于提交竞价）共享同一个底层的 `bid_sender` 通道。
//         它只关心接收机会信息，不发送任何东西。因此，它会丢弃 `new_shio_conn` 返回的 `bid_sender`。
//         日志中会打印一条警告 "only reading from shio feed, not sending any bids"。
//     -   **`new(receiver: Receiver<ShioItem>) -> Self`**:
//         这个构造函数更通用，它直接接收一个已经创建好的 `Receiver<ShioItem>`。
//         这允许 `ShioCollector` 与其他组件（例如 `ShioExecutor`）共享同一个由 `new_shio_conn` 创建的底层连接和通道。
//         `shio` crate 的 `lib.rs` 中的 `new_shio_collector_and_executor()` 函数就是使用了这种方式，
//         确保收集器和执行器使用匹配的通道。
//
// 3.  **`Collector<ShioItem>` trait 实现**:
//     -   **`name()`**: 返回收集器的名称 "ShioCollector"。
//     -   **`get_event_stream()`**: 这是 `Collector` trait的核心方法。它返回一个异步事件流 (`CollectorStream<'_, ShioItem>`)。
//         -   **实现**: 它使用 `async_stream::stream!` 宏来创建一个流。
//         -   这个流会进入一个无限循环，在循环中异步地从 `self.receiver` (克隆一份以在流中拥有所有权) 接收 `ShioItem`。
//         -   每当成功接收到一个 `ShioItem`，流就会通过 `yield item;` 将其“产生”出来。
//         -   如果从通道接收时发生错误 (例如通道关闭，通常意味着 `shio_conn` 中的连接任务已终止)，
//             `recv().await` 会返回 `Err`，循环会中断。此时，代码中用 `panic!` 来指示流意外结束。
//             在生产环境中，这里可能需要更优雅的错误处理或重启机制。
//
// **工作流程 (Workflow)**:
// 1.  当 `ShioCollector` (通常是通过 `new_shio_collector_and_executor` 在 `shio/lib.rs` 中被创建) 被添加到 `burberry::Engine` 时，
//     引擎会调用其 `get_event_stream()` 方法来获取事件流。
// 2.  `get_event_stream()` 返回的流会连接到由 `shio_conn::new_shio_conn()` 创建的内部异步通道的接收端。
// 3.  `shio_conn` 模块中的连接任务负责：
//     a.  连接到Shio的WebSocket服务器 (`SHIO_FEED_URL`)。
//     b.  异步地从WebSocket接收原始消息。
//     c.  解析这些消息，尝试将它们转换为 `ShioItem`。
//     d.  通过异步通道的发送端将解析成功的 `ShioItem` 发送出去。
// 4.  `ShioCollector` 的事件流中的 `self.receiver.clone().recv().await` 语句会接收到这些 `ShioItem`。
// 5.  接收到的 `ShioItem` 通过 `yield item;` 从流中发出，供引擎的策略模块 (`ArbStrategy`) 处理。
//
// **测试模块 (`tests`)**:
// -   包含一个 `test_shio_collector` 异步测试函数。
// -   它使用 `ShioCollector::new_without_executor` 创建一个独立的收集器实例，连接到默认的 `SHIO_FEED_URL`。
// -   然后它从事件流中获取事件，并根据事件类型 (`AuctionStarted`, `AuctionEnded`, `Dummy`) 打印信息。
// -   这个测试可以用来验证与Shio feed的连接是否正常，以及是否能正确接收和识别不同类型的 `ShioItem`。

// 从当前crate的 shio_conn 模块引入 new_shio_conn 函数。
use crate::shio_conn::new_shio_conn;
// 从当前crate的 types 模块引入 ShioItem 结构体。
use crate::types::ShioItem;
// 引入 async_channel 库的 Receiver 类型，用于异步多生产者多消费者通道的接收端。
use async_channel::Receiver;
// 引入 burberry 框架的 Collector trait, CollectorStream 类型别名, 和 async_trait 宏。
use burberry::{async_trait, Collector, CollectorStream};
// 引入 eyre 库的 Result 类型，用于错误处理。
use eyre::Result;
// 引入 tracing 库的 warn! 宏，用于记录警告级别的日志。
use tracing::warn;

/// `ShioCollector` 结构体
///
/// 负责从Shio MEV协议的事件源收集 `ShioItem` (MEV机会信息)。
/// 它通过一个异步通道的接收端 (`Receiver<ShioItem>`) 来获取这些条目，
/// 而实际的WebSocket连接和消息解析由 `shio_conn` 模块处理。
pub struct ShioCollector {
    receiver: Receiver<ShioItem>, // 异步通道的接收端，用于接收 ShioItem
}

// 注释：表明只有一个到WebSocket服务器的连接。
// 这暗示了 `new_shio_conn` 会管理这个单一连接，并可能通过克隆发送/接收端来支持多个收集器/执行器。
impl ShioCollector {
    /// `new_without_executor` 构造函数 (异步)
    ///
    /// 创建一个新的 `ShioCollector` 实例，并**主动**建立一个新的到Shio WebSocket feed的连接。
    /// 这个方法主要用于只需要从Shio feed读取数据而不需要提交竞价的场景（例如，纯粹的监控或数据分析）。
    ///
    /// 参数:
    /// - `wss_url`: Shio WebSocket feed的URL字符串。
    /// - `num_retries`: (可选) 连接失败时的重试次数。如果为 `None`，则使用 `new_shio_conn` 中的默认值 (例如3次)。
    ///
    /// 返回:
    /// - `Self`: 新创建的 `ShioCollector` 实例。
    pub async fn new_without_executor(wss_url: String, num_retries: Option<u32>) -> Self {
        // 记录一条警告日志，说明这个收集器实例只能读取数据，不能发送竞价。
        warn!("仅从Shio feed读取数据，不发送任何竞价 (only reading from shio feed, not sending any bids)");
        // 调用 `new_shio_conn` 建立连接并获取通道的发送端 (用于发送竞价，这里被忽略) 和接收端 (用于接收ShioItem)。
        // `_` 忽略了 `bid_sender`，因为此收集器不发送竞价。
        let (_, shio_item_receiver) = new_shio_conn(wss_url, num_retries.unwrap_or(3)).await;
        // 使用接收端创建 `ShioCollector`。
        Self { receiver: shio_item_receiver }
    }

    /// `new` 构造函数
    ///
    /// 创建一个新的 `ShioCollector` 实例，使用一个**已经存在**的 `Receiver<ShioItem>`。
    /// 这种方式允许 `ShioCollector` 与其他组件 (如 `ShioExecutor`) 共享同一个底层的Shio连接和通道。
    /// `shio` crate的 `lib.rs` 中的 `new_shio_collector_and_executor` 函数就是这样做的。
    ///
    /// 参数:
    /// - `receiver`: 一个 `Receiver<ShioItem>`，用于接收来自Shio连接的MEV机会。
    ///
    /// 返回:
    /// - `Self`: 新创建的 `ShioCollector` 实例。
    pub fn new(receiver: Receiver<ShioItem>) -> Self {
        Self { receiver }
    }
}

/// 为 `ShioCollector` 实现 `burberry::Collector<ShioItem>` trait。
/// 这使得 `ShioCollector` 可以被 `burberry::Engine` 用作一个事件源。
#[async_trait] // 因为 `get_event_stream` 是异步的
impl Collector<ShioItem> for ShioCollector {
    /// `name` 方法 (来自 `Collector` trait)
    ///
    /// 返回收集器的名称。
    fn name(&self) -> &str {
        "ShioCollector"
    }

    /// `get_event_stream` 方法 (来自 `Collector` trait)
    ///
    /// 返回一个异步事件流 (`CollectorStream<'_, ShioItem>`)。
    /// 这个流会持续地从内部的 `receiver` 通道中接收 `ShioItem` 并将它们 `yield` (产生) 出来。
    ///
    /// 返回:
    /// - `Result<CollectorStream<'_, ShioItem>>`: 成功则返回一个固定在堆上的异步事件流。
    ///   在这个实现中，它总是返回 `Ok(...)`，因为流的创建本身不会失败。
    ///   流的实际运行中如果发生错误 (例如通道关闭)，会导致流结束或panic。
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, ShioItem>> {
        // 使用 `async_stream::stream!` 宏创建一个异步流。
        let event_stream = async_stream::stream! {
            // 无限循环，以持续从通道接收项目。
            loop {
                // 克隆接收端 `self.receiver.clone()`。
                // `async_channel::Receiver` 的克隆是轻量级的，它们共享同一个底层的通道。
                // 在流的每次迭代中克隆可能是为了确保流的 `Future` 是 `Send` (如果 `Receiver` 本身不是 `Sync` 但其克隆是 `Send`)，
                // 或者仅仅是为了在 `recv().await` 调用中转移所有权。
                // 实际上，`async_channel::Receiver` 是 `Send + Sync` 的，所以可以直接 `&self.receiver` 或在外部克隆一次。
                // 这里的 `clone()` 对于 `async_channel` 来说是廉价的。
                match self.receiver.clone().recv().await { // 异步等待从通道接收一个 ShioItem
                    Ok(item) => { // 如果成功接收到
                        yield item; // 则将该 item 从流中产生出来
                    }
                    Err(_) => { // 如果接收失败 (例如通道已关闭)
                        // 通道关闭通常意味着 Shio 连接的另一端 (shio_conn 中的任务) 已经停止。
                        // 这被视为一个严重错误，因为收集器无法再获取新的事件。
                        // 因此，这里选择 panic 来使程序崩溃并提示问题。
                        // 在更健壮的生产系统中，这里可能需要更复杂的错误处理或重启逻辑。
                        panic!("ShioCollector 的事件流意外结束 (可能是通道已关闭) (ShioCollector stream ended unexpectedly, channel might be closed)");
                    }
                }
            }
        };

        // 将创建的异步流固定 (pin) 到堆上并返回。
        // `Box::pin` 是创建 `Pin<Box<dyn Stream>>` (即 `CollectorStream`) 的常用方式。
        Ok(Box::pin(event_stream))
    }
}

// --- 测试模块 ---
#[cfg(test)] // 表示这部分代码仅在 `cargo test` 时编译和执行
mod tests {

    use super::*; // 导入外部模块 (shio_collector.rs) 的所有公共成员
    use crate::SHIO_FEED_URL; // 从crate根导入默认的Shio feed URL
    use futures::StreamExt; // 引入 StreamExt trait 以使用 `next()` 方法

    /// `test_shio_collector` 测试函数
    ///
    /// 这个异步测试函数用于验证 `ShioCollector` 是否能成功连接到Shio的WebSocket feed
    /// 并正确接收和识别不同类型的 `ShioItem`。
    ///
    /// 测试命令示例:
    /// `cargo test --package shio --lib -- shio_collector::tests::test_shio_collector --exact --show-output --nocapture`
    /// - `--package shio --lib`: 指定测试 shio 这个库crate。
    /// - `-- shio_collector::tests::test_shio_collector`: 指定运行这个特定的测试函数。
    /// - `--exact`: 精确匹配测试函数名。
    /// - `--show-output`: 显示测试过程中的标准输出 (例如 `println!` 的内容)。
    /// - `--nocapture`: 不捕获标准输出和标准错误，让它们直接打印到控制台。
    #[tokio::test]
    async fn test_shio_collector() {
        // 初始化一个简单的控制台日志记录器，方便查看测试过程中的日志输出。
        // `None` 表示使用默认的日志级别 (通常是INFO或WARN)。
        mev_logger::init_console_logger(None);

        // 使用 `ShioCollector::new_without_executor` 创建一个收集器实例。
        // 它会连接到 `SHIO_FEED_URL`。
        // `Some(0)` 表示如果连接失败，不进行重试 (num_retries = 0)。
        let collector = ShioCollector::new_without_executor(SHIO_FEED_URL.to_string(), Some(0)).await;
        // 获取事件流
        let mut event_stream = collector.get_event_stream().await.unwrap();

        // 循环从事件流中获取项目。
        // `stream.next().await` 会异步等待下一个事件。当流结束时，它会返回 `None`。
        while let Some(item) = event_stream.next().await {
            // 根据接收到的 `ShioItem` 的类型进行匹配并打印信息。
            // 这是一个简单的测试，只打印类型名称或整个项目，实际应用中会有更复杂的处理逻辑。
            match item {
                ShioItem::AuctionStarted { .. } => { // 如果是拍卖开始事件
                    println!("接收到Shio事件: {:#?}", item.type_name()); // 打印事件类型名
                }
                ShioItem::AuctionEnded { .. } => { // 如果是拍卖结束事件
                    println!("接收到Shio事件: {:#?}", item.type_name());
                }
                ShioItem::Dummy(_) => { // 如果是Dummy事件 (可能用于心跳或测试)
                    println!("接收到Shio事件: {:#?}", item); // 打印整个Dummy事件的内容
                }
            }
            // 这个测试会一直运行，直到WebSocket连接中断或手动停止测试。
            // 在实际的CI环境中，可能需要设置超时或只处理有限数量的事件。
        }
    }
}

[end of crates/shio/src/shio_collector.rs]
