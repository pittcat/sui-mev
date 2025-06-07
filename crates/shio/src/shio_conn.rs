// 该文件 `shio_conn.rs` (位于 `shio` crate中) 负责管理与 Shio MEV 协议服务器的底层WebSocket连接。
// 它建立连接，处理消息的接收和发送，并在连接断开时进行重试。
//
// **文件概览 (File Overview)**:
// 这个文件是 `shio` 库的“通信兵”。它的核心职责是建立并维护一条到Shio服务器的WebSocket“电话线”。
// 通过这条电话线：
// -   机器人可以接收来自Shio服务器的实时MEV机会信息 (`ShioItem`)。
// -   机器人可以将自己的竞价（bid）发送给Shio服务器。
//
// **核心功能 (Core Functionality)**:
// 1.  **`new_shio_conn()` 异步函数**:
//     -   这是本文件的唯一公共接口，一个工厂函数，用于创建并启动Shio连接管理器。
//     -   **通道创建 (Channel Creation)**:
//         -   它首先创建两个异步通道 (`async_channel`):
//             -   `bid_sender` / `bid_receiver`: 用于将外部（例如 `ShioExecutor`）产生的竞价信息 (`serde_json::Value`) 发送到连接管理任务中，再由后者通过WebSocket发送出去。
//             -   `shio_item_sender` / `shio_item_receiver`: 用于将从WebSocket接收并解析后的 `ShioItem` 从连接管理任务发送给外部使用者（例如 `ShioCollector`）。
//     -   **连接管理任务 (Connection Management Task)**:
//         -   它使用 `tokio::spawn` 在后台启动一个新的异步任务，这个任务是实际的连接管理器。
//         -   **连接与重试逻辑**:
//             -   管理器进入一个无限循环 (`loop`)，尝试连接到指定的 `wss_url`。
//             -   如果连接失败，它会记录错误，并在达到最大重试次数 (`num_retries`) 前每隔5秒进行重试。如果达到最大次数仍失败，则 `panic!`。
//             -   连接成功后，重试计数器 `retry_count` 会被重置。
//         -   **消息处理循环 (`'connected: loop`)**:
//             -   一旦连接成功，它会进入另一个内部无限循环，使用 `tokio::select!` 同时监听两个事件源：
//                 1.  **从 `bid_receiver` 接收竞价**: 如果从这个通道收到了一个 `serde_json::Value` (代表一个竞价)，
//                     它会将其转换为WebSocket文本消息 (`Message::Text`) 并通过 `wss_stream.send()` 发送出去。
//                     如果发送失败（例如WebSocket连接已断开），则跳出内部循环 (`break 'connected;`)，外层循环会尝试重连。
//                 2.  **从 `wss_stream` (WebSocket流) 接收消息**: 如果从WebSocket收到了消息：
//                     -   如果是文本消息 (`Message::Text(text)`):
//                         -   尝试将文本内容解析为 `serde_json::Value`。
//                         -   然后将这个 `Value` 转换为 `ShioItem` (通过 `ShioItem::from(value)`)。
//                         -   最后通过 `shio_item_sender.send()` 将 `ShioItem` 发送出去。
//                     -   如果是Ping消息 (`Message::Ping(val)`): 自动回复一个Pong消息以保持连接活跃。如果发送Pong失败，也跳出内部循环。
//                     -   如果是其他类型的WebSocket消息 (Close, Frame, Pong, Binary): 被认为是意外的，会导致 `panic!`。
//                         （注意：`Pong` 消息通常由WebSocket库自动处理，这里显式panic可能表示不期望收到外部的Pong）。
//                     -   如果接收消息时发生错误: 记录错误并跳出内部循环。
//     -   **返回通道端点 (Returning Channel Endpoints)**:
//         -   `new_shio_conn` 函数本身会立即返回 `(bid_sender, shio_item_receiver)` 这两个通道的端点。
//           外部代码（如 `ShioCollector` 和 `ShioExecutor`）将使用这两个端点与后台的连接管理任务进行通信。
//
// **工作流程 (Workflow)**:
// 1.  调用 `new_shio_conn(url, retries)`。
// 2.  函数创建两对异步通道，并立即返回 `bid_sender` 和 `shio_item_receiver` 给调用者。
// 3.  同时，一个后台Tokio任务被启动，负责：
//     a.  使用传入的 `url` 和 `retries` 尝试连接到Shio WebSocket服务器。
//     b.  如果连接成功，则进入消息处理循环。
//     c.  在消息处理循环中，使用 `tokio::select!`：
//         i.  监听 `bid_receiver`：如果收到竞价，通过WebSocket发送给服务器。
//         ii. 监听WebSocket流：如果收到服务器消息，解析为 `ShioItem` 并通过 `shio_item_sender` 发送出去；处理Ping/Pong。
//     d.  如果WebSocket连接断开或发生错误，则跳出消息处理循环，返回到步骤 (a) 尝试重连。
//
// **关键点 (Key Points)**:
// -   **解耦 (Decoupling)**: 通过使用异步通道，`new_shio_conn` 将WebSocket的连接管理、消息收发和解析逻辑与上层的收集器/执行器逻辑解耦。
//     收集器和执行器只需要与通道交互，而不需要关心底层的网络细节。
// -   **并发安全 (Concurrency Safety)**: `async_channel` 是多生产者多消费者 (MPMC) 通道，可以在异步任务间安全地传递数据。
// -   **自动重连 (Automatic Reconnection)**: 后台任务内置了连接失败后的重试逻辑，增强了连接的健壮性。
// -   **双向通信 (Bidirectional Communication)**: 同时处理从WebSocket接收数据（MEV机会）和向WebSocket发送数据（竞价）。

// 引入 async_channel 库的 Receiver 和 Sender 类型，用于创建异步通道。
use async_channel::{Receiver, Sender};
// 引入 futures 库的 SinkExt 和 StreamExt trait，为 Sink 和 Stream 提供额外的方法。
use futures::{SinkExt, StreamExt};
// 引入 serde_json 的 Value 类型，用于表示通用的JSON值。
use serde_json::Value;
// 引入 tokio_tungstenite 库，用于处理WebSocket连接。Message 枚举代表WebSocket消息类型。
use tokio_tungstenite::tungstenite::Message;
// 引入 tracing 库的 error! 宏，用于记录错误级别的日志。
use tracing::error;

// 从当前crate的根模块引入 ShioItem 结构体。
use crate::ShioItem;

/// `new_shio_conn` 异步函数 (Shio连接工厂)
///
/// 创建并启动一个管理与Shio WebSocket服务器连接的后台任务。
/// 此函数返回一对异步通道的端点，用于与该后台任务进行双向通信：
/// - `Sender<Value>`: 用于向后台任务发送要提交给Shio服务器的竞价信息 (JSON格式)。
/// - `Receiver<ShioItem>`: 用于从后台任务接收来自Shio服务器的MEV机会信息 (`ShioItem`)。
///
/// 参数:
/// - `wss_url`: Shio WebSocket服务器的URL字符串。
/// - `num_retries`: 当连接失败时，后台任务尝试重新连接的最大次数。
///
/// 返回:
/// - `(Sender<Value>, Receiver<ShioItem>)`: 一个元组，包含竞价发送通道的发送端和Shio机会接收通道的接收端。
pub async fn new_shio_conn(wss_url: String, num_retries: u32) -> (Sender<Value>, Receiver<ShioItem>) {
    // 创建第一个异步通道: bid_channel (用于发送竞价)
    // `unbounded()` 创建一个容量不限的通道。
    let (bid_sender_to_external, bid_receiver_for_task) = async_channel::unbounded();
    // 创建第二个异步通道: shio_item_channel (用于接收ShioItem)
    let (shio_item_sender_for_task, shio_item_receiver_for_external) = async_channel::unbounded();

    // 启动一个后台Tokio任务来管理WebSocket连接和消息处理。
    tokio::spawn(async move { // `move` 关键字将所需变量的所有权移入异步块
        // 将通道端点移入任务，并明确其类型 (有助于类型推断和可读性)
        let bid_receiver: Receiver<Value> = bid_receiver_for_task;
        let shio_item_sender: Sender<ShioItem> = shio_item_sender_for_task;
        let wss_server_url = wss_url; // 将 wss_url 的所有权移入

        let mut current_retry_count = 0; // 初始化重试计数器

        // 主循环：负责连接和在连接断开时重试
        loop {
            // 尝试连接到WebSocket服务器
            // `tokio_tungstenite::connect_async` 返回 Result<(WebSocketStream, Response)>
            let (mut websocket_stream, _) = match tokio_tungstenite::connect_async(&wss_server_url).await {
                Ok(connection_result) => { // 连接成功
                    current_retry_count = 0; // 重置重试计数器
                    connection_result // 返回 (WebSocketStream, Response)
                }
                Err(error_val) => { // 连接失败
                    error!("连接到Shio WebSocket服务器失败: {:#}", error_val);
                    if current_retry_count == num_retries { // 如果达到最大重试次数
                        // 触发panic，这将导致后台任务终止。
                        // 依赖此连接的 ShioCollector 和 ShioExecutor 将无法工作。
                        panic!("连接Shio WebSocket服务器失败，已达到最大重试次数 {}", num_retries);
                    }
                    // 等待5秒后重试
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    current_retry_count += 1; // 增加重试计数
                    continue; // 跳过当前循环的剩余部分，直接开始下一次连接尝试
                }
            };

            // 标签化的内部循环：处理已建立的WebSocket连接上的消息收发
            'connected_loop: loop {
                // `tokio::select!` 宏用于同时等待多个异步操作，当其中任何一个完成时执行对应分支。
                tokio::select! {
                    // 分支1: 尝试从 `bid_receiver` 通道接收外部提交的竞价信息
                    // `bid_receiver.recv()` 是异步的，会等待直到有消息或通道关闭。
                    Ok(bid_to_send_val) = bid_receiver.recv() => { // 如果成功接收到竞价 (Value类型)
                        // 将JSON Value转换为字符串，然后包装成WebSocket文本消息
                        let ws_message_to_send = Message::Text(bid_to_send_val.to_string());
                        // 异步发送消息到WebSocket服务器
                        if let Err(send_error) = websocket_stream.send(ws_message_to_send).await {
                            error!("发送竞价消息到Shio WebSocket服务器失败: {:#}", send_error);
                            break 'connected_loop; // 发送失败，可能连接已断开，跳出内部循环以尝试重连
                        }
                    }
                    // 分支2: 尝试从 `websocket_stream` (WebSocket连接) 接收服务器发送的消息
                    // `websocket_stream.next()` 是异步的，获取流中的下一个消息 (Option<Result<Message, Error>>)
                    Some(received_ws_message_result) = websocket_stream.next() => {
                        match received_ws_message_result { // 处理接收到的消息结果
                            Ok(Message::Text(text_payload)) => { // 如果是文本消息
                                // 尝试将文本内容解析为 serde_json::Value
                                let json_value_payload = match serde_json::from_str::<Value>(&text_payload) {
                                    Ok(v) => v,
                                    Err(json_parse_error) => { // JSON解析失败
                                        error!("从Shio WebSocket解析JSON文本消息失败: {}", json_parse_error);
                                        continue; // 继续处理下一条WebSocket消息
                                    }
                                };
                                // 将解析后的JSON Value转换为 ShioItem (通过 ShioItem::from(value) 实现)
                                // 然后通过 `shio_item_sender` 通道发送给外部使用者 (如 ShioCollector)。
                                // `.await.unwrap()` 假设发送总是成功的 (如果接收端已drop则会panic)。
                                shio_item_sender.send(ShioItem::from(json_value_payload)).await.unwrap();
                            }
                            Ok(Message::Ping(ping_payload)) => { // 如果是Ping消息
                                // 自动回复Pong消息以保持连接活跃
                                if let Err(pong_send_error) = websocket_stream.send(Message::Pong(ping_payload)).await {
                                    error!("发送Pong消息失败: {}", pong_send_error);
                                    break 'connected_loop; // 发送失败，可能连接已断开
                                }
                            }
                            Ok(Message::Close(_)) | Ok(Message::Frame(_)) | Ok(Message::Pong(_)) | Ok(Message::Binary(_)) => {
                                // 对于 Close, Frame (底层帧), Pong (通常由库自动处理), Binary (二进制消息)
                                // 这些消息类型在此上下文中被认为是意外的，触发panic。
                                // 这表明Shio feed的协议行为与预期不符。
                                panic!("从Shio WebSocket接收到意外的消息类型: {:?}", received_ws_message_result);
                            }
                            Err(websocket_error) => { // 从WebSocket流接收消息时发生错误
                                error!("从Shio WebSocket接收消息时发生错误: {:?}", websocket_error);
                                break 'connected_loop; // 发生错误，可能连接已断开
                            }
                        }
                    }
                } // 结束 tokio::select!
            } // 结束 'connected_loop (内部消息处理循环)
            // 如果跳出内部循环，意味着WebSocket连接已中断，外层循环会尝试重新连接。
        } // 结束 loop (主连接/重试循环)
    }); // 结束 tokio::spawn (后台连接管理任务)

    // 返回两个通道的外部端点，供 ShioCollector 和 ShioExecutor 使用。
    (bid_sender_to_external, shio_item_receiver_for_external)
}

[end of crates/shio/src/shio_conn.rs]
