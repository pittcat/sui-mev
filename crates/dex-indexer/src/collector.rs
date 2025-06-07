// 该文件 `collector.rs` (位于 `dex-indexer` crate中) 定义了一个名为 `QueryEventCollector` 的事件收集器。
// 这个收集器的功能非常简单：它不监听外部的实时事件源（如WebSocket或IPC套接字），
// 而是作为一个定时器，定期地产生一个特定的事件 (`Event::QueryEventTrigger`)。
//
// 文件概览:
// - `QueryEventCollector` 结构体:
//   - `tick_interval`: 一个 `Duration` 类型，表示定时器触发的时间间隔。
// - `new()` 方法: `QueryEventCollector` 的构造函数，初始化 `tick_interval` (默认为10秒)。
// - `Collector<Event>` trait 实现:
//   - `name()`: 返回收集器的名称 "QueryEventCollector"。
//   - `get_event_stream()`: 返回一个异步事件流。这个流会使用 `tokio::time::interval_at`
//     来创建一个定时器。每当定时器到达一个时间点 (`tick().await`)，流就会 `yield` (产生)
//     一个 `Event::QueryEventTrigger` 事件。
//
// 工作原理和用途:
// `QueryEventCollector` 的主要目的是在系统中引入一个周期性的触发机制。
// 当这个收集器被添加到 `burberry::Engine` (或其他事件处理引擎) 中时，
// 引擎会定期从其事件流中接收到 `Event::QueryEventTrigger` 事件。
//
// 接收到这个触发事件的策略 (Strategy) 可以执行以下操作：
// - 定期查询 `DexIndexer` 服务以获取最新的DEX池信息。
// - 执行周期性的状态检查或维护任务。
// - 重新评估某些缓存的套利机会或路径。
//
// 这种基于时间的轮询机制是获取非实时更新数据或执行周期性任务的常见模式，
// 特别是当直接的事件推送机制不可用或不适用时。

// 引入所需的库和模块
use burberry::{async_trait, Collector, CollectorStream}; // 从 `burberry` 框架引入 Collector trait 和相关类型
                                                        // `Collector` 定义了事件收集器的通用接口。
                                                        // `CollectorStream` 是收集器产生的事件流的类型别名。
use eyre::Result; // `eyre`库，用于错误处理
use tokio::time::{interval_at, Duration, Instant}; // Tokio库的时间处理功能：
                                                  // `interval_at`: 创建一个在指定时间点开始并按固定间隔触发的定时器。
                                                  // `Duration`: 表示时间间隔。
                                                  // `Instant`: 表示一个时间点。

use crate::types::Event; // 从当前 `dex-indexer` crate 的 `types` 模块引入 `Event` 枚举。
                         // `Event::QueryEventTrigger` 是此收集器将产生的事件类型。

/// `QueryEventCollector` 结构体
///
/// 一个简单的事件收集器，它按固定的时间间隔产生 `Event::QueryEventTrigger` 事件。
pub struct QueryEventCollector {
    tick_interval: Duration, // 定时器触发的时间间隔
}

impl QueryEventCollector {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `QueryEventCollector` 实例。
    /// 默认的 `tick_interval` 设置为10秒。
    pub fn new() -> Self {
        Self {
            tick_interval: Duration::from_secs(10), // 初始化时间间隔为10秒
        }
    }
}

/// 为 `QueryEventCollector` 实现 `burberry::Collector<Event>` trait。
#[async_trait] // 因为 `get_event_stream` 是异步的
impl Collector<Event> for QueryEventCollector {
    /// `name` 方法 (来自 `Collector` trait)
    ///
    /// 返回收集器的名称。
    fn name(&self) -> &str {
        "QueryEventCollector"
    }

    /// `get_event_stream` 方法 (来自 `Collector` trait)
    ///
    /// 返回一个异步事件流 (`CollectorStream<'_, Event>`)。
    /// 这个流会无限地、周期性地产生 `Event::QueryEventTrigger` 事件。
    ///
    /// 返回:
    /// - `Result<CollectorStream<'_, Event>>`: 成功则返回一个固定在堆上的异步事件流。
    ///   在这个简单实现中，它总是返回 `Ok(...)`。
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Event>> {
        // 创建一个定时器 (`Interval`)。
        // `Instant::now() + self.tick_interval` 表示第一次触发是在当前时间点之后的 `tick_interval` 时长。
        // 之后，每隔 `self.tick_interval` 会再次触发。
        let mut timer_interval = interval_at(Instant::now() + self.tick_interval, self.tick_interval);

        // 使用 `async_stream::stream!` 宏创建一个异步流。
        // 这个宏使得编写异步流更加简洁。
        let event_stream = async_stream::stream! {
            loop { // 无限循环
                // `timer_interval.tick().await` 会异步等待直到下一个定时器时间点到达。
                timer_interval.tick().await;
                // 当定时器触发时，`yield` (产生) 一个 `Event::QueryEventTrigger` 事件。
                // 这个事件会被事件处理引擎捕获并分发给相应的策略。
                yield Event::QueryEventTrigger;
            }
        };

        // 将创建的异步流固定 (pin) 到堆上并返回。
        // `Box::pin` 是创建 `Pin<Box<dyn Stream>>` (即 `CollectorStream`) 的常用方式。
        Ok(Box::pin(event_stream))
    }
}
