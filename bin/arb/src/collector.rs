// 该文件定义了不同类型的“收集器”(Collectors)，它们用于从不同来源收集Sui区块链上的事件数据，
// 例如公开的交易效果 (Transaction Effects) 和私下广播的交易 (Private Transactions)。
// 这些收集器是套利机器人（或其他链上监控工具）获取实时信息的重要组成部分。
//
// 概念解释:
// - Collector (收集器): 一个抽象概念，代表一个能产生事件流（stream of events）的组件。
//   这里的事件可以是交易、日志或其他链上活动。
// - Event (事件): 在这个上下文中，通常指Sui区块链上的交易及其相关数据。
// - Transaction Effects (交易效果): 一笔交易在链上执行后产生的结果，例如状态变更、创建的对象、发出的事件等。
// - Public Transactions (公开交易): 通过常规Sui RPC节点广播和可见的交易。
// - Private Transactions (私有交易): 可能通过专门的通道（如MEV中继器）发送，不立即公开广播的交易，
//   通常用于MEV（Miner Extractable Value，矿工可提取价值）场景，以避免被抢先交易。
// - IPC (Inter-Process Communication, 进程间通信): 一种允许不同程序在同一台机器上交换数据的方法。
//   这里的 `PublicTxCollector` 使用本地套接字 (local socket) 进行IPC，可能从一个本地运行的Sui节点或其他服务接收数据。
// - WebSocket (WS): 一种网络通信协议，允许双向、实时的消息传递。
//   这里的 `PrivateTxCollector` 使用WebSocket连接到一个中继服务器 (relay server) 来接收私有交易。

// 引入所需的库和模块
use burberry::{async_trait, Collector, CollectorStream}; // burberry是一个自定义的库，提供了Collector trait和相关类型
use eyre::Result; // 用于错误处理的库
use fastcrypto::encoding::{Base64, Encoding}; // 用于Base64编码和解码 (私有交易数据可能是Base64编码的)
use futures::stream::StreamExt; // 为Stream（异步迭代器）提供额外的操作方法
use interprocess::local_socket::{ // 用于本地进程间通信 (IPC)
    tokio::{prelude::*, Stream as LocalSocketStream}, // Tokio集成的本地套接字流
    GenericNamespaced, // 用于创建有命名空间的本地套接字名称
};
use serde::Deserialize; // 用于反序列化数据 (例如从JSON或bincode格式)
use sui_json_rpc_types::{SuiEvent, SuiTransactionBlockEffects}; // Sui RPC定义的事件和交易效果类型
use sui_types::{effects::TransactionEffects, transaction::TransactionData}; // Sui核心库定义的交易效果和交易数据类型
use tokio::{io::AsyncReadExt, pin, time}; // Tokio库，用于异步编程 (读取、固定异步值、时间操作)
use tracing::{debug, error}; // 用于日志记录 (调试信息、错误信息)

use crate::types::Event; // 从当前项目中引入自定义的Event枚举类型

// --- PublicTxCollector ---
// PublicTxCollector 用于收集公开广播的Sui交易效果和相关事件。
// 它通过本地IPC套接字连接到一个数据源 (可能是本地Sui节点的全节点流服务)。
pub struct PublicTxCollector {
    path: String, // 本地IPC套接字的路径 (例如 "/tmp/sui_tx_socket")
}

impl PublicTxCollector {
    /// 创建一个新的 `PublicTxCollector` 实例。
    ///
    /// 参数:
    /// - `path`: IPC套接字的路径字符串。
    pub fn new(path: &str) -> Self {
        Self { path: path.to_string() } // 将路径字符串转换为String类型并存储
    }

    /// 异步连接到本地IPC套接字。
    ///
    /// 返回:
    /// - `Result<LocalSocketStream>`: 成功则返回一个本地套接字流，否则返回错误。
    async fn connect(&self) -> Result<LocalSocketStream> {
        // 将路径字符串转换为特定于平台的命名空间名称
        let name = self.path.as_str().to_ns_name::<GenericNamespaced>()?;
        // 尝试连接到该名称的本地套接字
        let conn = LocalSocketStream::connect(name).await?;
        Ok(conn) // 返回连接成功的流
    }
}

// 为 `PublicTxCollector` 实现 `Collector` trait。
// `Collector` trait 定义了一个收集器应有的行为，主要是提供一个事件流。
#[async_trait] // 表示这个trait的方法可以是异步的
impl Collector<Event> for PublicTxCollector {
    /// 返回收集器的名称。
    fn name(&self) -> &str {
        "PublicTxCollector"
    }

    /// 获取一个包含 `Event` 的异步流 (CollectorStream)。
    /// 这个流会持续不断地产生新的事件，直到发生错误或流关闭。
    ///
    /// 返回:
    /// - `Result<CollectorStream<'_, Event>>`: 成功则返回一个固定在堆上的异步事件流，否则返回错误。
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Event>> {
        // 首次尝试连接到IPC套接字
        let mut conn = self.connect().await?;
        // 用于存储接下来要读取的数据块长度的缓冲区
        // Sui交易效果和事件数据通常以 "长度 + 数据" 的方式发送
        let mut effects_len_buf = [0u8; 4]; // 4字节用于存储u32长度 (交易效果)
        let mut events_len_buf = [0u8; 4];  // 4字节用于存储u32长度 (相关事件)

        // 使用 `async_stream::stream!` 宏创建一个异步流。
        // 这个宏使得编写异步流更加简洁。
        let stream = async_stream::stream! {
            // 无限循环，持续尝试从连接中读取数据
            loop {
                // `tokio::select!` 宏用于同时等待多个异步操作，当任何一个完成时即继续。
                // 这里主要等待 `conn.read_exact` 完成，或者在没有数据时短暂休眠 (通过 else 分支)。
                tokio::select! {
                    // 尝试从连接中精确读取4个字节到 effects_len_buf (获取交易效果数据的长度)
                    result = conn.read_exact(&mut effects_len_buf) => {
                        // 如果读取失败 (例如连接断开)
                        if result.is_err() {
                            debug!("读取交易效果数据长度失败，尝试重连...");
                            // 尝试重新连接IPC套接字
                            conn = self.connect().await.expect("无法重连到交易套接字");
                            continue; // 继续下一次循环，尝试重新读取
                        }

                        // 将读取到的4字节 (大端序) 转换为u32类型的长度值
                        let effects_len = u32::from_be_bytes(effects_len_buf);
                        // 根据长度创建一个缓冲区来存储实际的交易效果数据
                        let mut effects_buf = vec![0u8; effects_len as usize];
                        // 从连接中精确读取 `effects_len` 字节到 `effects_buf`
                        if conn.read_exact(&mut effects_buf).await.is_err() {
                            debug!("读取交易效果数据失败，尝试重连...");
                            conn = self.connect().await.expect("无法重连到交易套接字");
                            continue;
                        }

                        // --- 类似地读取Sui事件数据 ---
                        // 读取事件数据长度
                        if conn.read_exact(&mut events_len_buf).await.is_err() {
                            debug!("读取Sui事件数据长度失败，尝试重连...");
                            conn = self.connect().await.expect("无法重连到交易套接字");
                            continue;
                        }
                        let events_len = u32::from_be_bytes(events_len_buf);
                        // 根据长度创建缓冲区
                        let mut events_buf = vec![0u8; events_len as usize];
                        // 读取事件数据
                        if conn.read_exact(&mut events_buf).await.is_err() {
                            debug!("读取Sui事件数据失败，尝试重连...");
                            conn = self.connect().await.expect("无法重连到交易套接字");
                            continue;
                        }

                        // --- 反序列化数据 ---
                        // 将读取到的原始字节数据 `effects_buf` 反序列化为 `TransactionEffects` 类型。
                        // `bincode` 是一个二进制序列化/反序列化库。
                        let tx_effects: TransactionEffects = match bincode::deserialize(&effects_buf) {
                            Ok(tx_effects) => tx_effects, // 反序列化成功
                            Err(e) => {
                                error!("无效的交易效果数据: {:?}, 跳过该条目。", e);
                                continue; // 如果数据无效，记录错误并跳过
                            }
                        };

                        // 将读取到的原始字节数据 `events_buf` 反序列化为 `Vec<SuiEvent>` 类型。
                        // `serde_json` 用于JSON序列化/反序列化。这里假设事件数据是JSON格式。
                        let events: Vec<SuiEvent> = if events_len == 0 {
                            vec![] // 如果长度为0，则表示没有事件
                        } else {
                            match serde_json::from_slice(&events_buf) {
                                Ok(events) => events, // 反序列化成功
                                Err(e) => {
                                    error!("无效的Sui事件数据: {:?}, 跳过该条目。", e);
                                    continue; // 如果数据无效，记录错误并跳过
                                }
                            }
                        };

                        // 将 `TransactionEffects` (来自sui_types核心库) 转换为 `SuiTransactionBlockEffects` (来自sui_json_rpc_types)。
                        // 这可能是因为下游处理逻辑期望的是RPC兼容的类型。
                        if let Ok(sui_tx_effects) = SuiTransactionBlockEffects::try_from(tx_effects) {
                            // `yield` 关键字用于从异步流中产生一个值。
                            // 这里产生一个 `Event::PublicTx` 类型的事件，包含交易效果和相关Sui事件。
                            yield Event::PublicTx(sui_tx_effects, events);
                        }
                    }
                    // `else` 分支在 `tokio::select!` 中，如果没有其他分支立即准备好，则执行此分支。
                    // 这里用于在没有新数据时进行短暂休眠，避免CPU空转。
                    else => {
                        time::sleep(time::Duration::from_millis(10)).await; // 休眠10毫秒
                    }
                }
            }
        };

        // 将创建的异步流固定 (pin) 到堆上并返回。
        // `Box::pin` 是创建 `Pin<Box<dyn Stream>>` 的常用方式。
        Ok(Box::pin(stream))
    }
}

// --- PrivateTxCollector ---
// PrivateTxCollector 用于收集私下广播的Sui交易。
// 它通过WebSocket连接到一个MEV中继器或其他私有交易分发服务。

/// `TxMessage` 结构体用于表示从私有交易中继器接收到的原始交易消息。
/// 通常，私有交易数据会以某种序列化格式（如JSON）封装，其中包含Base64编码的交易字节。
#[derive(Debug, Clone, Deserialize, Default)] // 自动派生Debug, Clone, Deserialize, Default traits
pub struct TxMessage {
    tx_bytes: String, // Base64编码的原始交易字节字符串
}

/// 实现 `TryFrom<TxMessage>` for `TransactionData`，
/// 允许将 `TxMessage` 转换为Sui核心库的 `TransactionData` 类型。
impl TryFrom<TxMessage> for TransactionData {
    type Error = eyre::Error; // 定义转换可能发生的错误类型

    /// 执行转换操作。
    fn try_from(tx_message: TxMessage) -> Result<Self> {
        // 步骤1: 将Base64编码的字符串 `tx_message.tx_bytes` 解码为原始字节序列。
        let tx_bytes = Base64::decode(&tx_message.tx_bytes)?; // `?` 用于错误传播
        // 步骤2: 将原始字节序列反序列化为 `TransactionData` 类型。
        // `bcs` (Binary Canonical Serialization) 是Sui用于交易序列化的格式。
        let tx_data: TransactionData = bcs::from_bytes(&tx_bytes)?;
        Ok(tx_data) // 返回转换成功的 `TransactionData`
    }
}

/// `PrivateTxCollector` 结构体定义。
pub struct PrivateTxCollector {
    ws_url: String, // WebSocket中继服务器的URL
}

impl PrivateTxCollector {
    /// 创建一个新的 `PrivateTxCollector` 实例。
    ///
    /// 参数:
    /// - `ws_url`: WebSocket服务器的URL字符串。
    pub fn new(ws_url: &str) -> Self {
        Self {
            ws_url: ws_url.to_string(), // 存储URL
        }
    }
}

// 为 `PrivateTxCollector` 实现 `Collector` trait。
#[async_trait]
impl Collector<Event> for PrivateTxCollector {
    /// 返回收集器的名称。
    fn name(&self) -> &str {
        "PrivateTxCollector"
    }

    /// 获取一个包含 `Event` 的异步流。
    /// 这个流会从WebSocket连接接收消息，解析它们，并产生 `Event::PrivateTx` 事件。
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Event>> {
        // 尝试连接到指定的WebSocket服务器。
        // `tokio_tungstenite::connect_async` 返回一个元组，包含WebSocket流和HTTP响应。
        let (ws_stream, _) = tokio_tungstenite::connect_async(&self.ws_url)
            .await
            .expect("无法连接到中继服务器 (Relay Server)"); // 连接失败则panic

        // 将WebSocket流 `ws_stream` 分割为发送端 (sink) 和接收端 (stream)。
        // 这里我们只需要接收端 `read` 来监听来自服务器的消息。
        let (_, read) = ws_stream.split();

        // 使用 `async_stream::stream!` 创建异步流。
        let stream = async_stream::stream! {
            // `pin!` 宏用于固定 `read` 流，因为 `next()` 方法需要被固定的流。
            pin!(read);
            // 循环等待WebSocket消息
            while let Some(message_result) = read.next().await { // `next()` 从流中获取下一条消息
                let message = match message_result {
                    Ok(msg) => msg, // 成功获取消息
                    Err(e) => {
                        error!("中继WebSocket错误: {:?}, 继续尝试接收。", e);
                        continue; // 如果发生错误，记录并尝试处理下一条消息
                    }
                };

                // 将接收到的消息 (通常是文本格式的JSON) 解析为 `TxMessage` 结构体。
                // `message.to_text().unwrap()` 将WebSocket消息转换为文本字符串。
                // `serde_json::from_str` 将JSON字符串反序列化为 `TxMessage`。
                // `.unwrap()` 在这里使用可能导致panic，实际生产代码中应进行更健壮的错误处理。
                let tx_message: TxMessage = match message.to_text() {
                    Some(text_message) => {
                        match serde_json::from_str(text_message) {
                            Ok(tm) => tm,
                            Err(e) => {
                                error!("从文本解析TxMessage失败: {:?}, 原始文本: '{}'", e, text_message);
                                continue;
                            }
                        }
                    },
                    None => {
                        // 如果消息不是文本格式 (例如二进制)，则记录并跳过
                        debug!("接收到非文本WebSocket消息，已跳过。消息: {:?}", message);
                        continue;
                    }
                };

                // 将 `TxMessage` 转换为 `TransactionData`。
                let tx_data = match TransactionData::try_from(tx_message) {
                    Ok(tx_data) => tx_data, // 转换成功
                    Err(e) => {
                        error!("无效的交易消息 (TxMessage转换TransactionData失败): {:?}, 跳过。", e);
                        continue; // 如果转换失败，记录错误并跳过
                    }
                };

                // 产生一个 `Event::PrivateTx` 事件，包含解析后的 `TransactionData`。
                yield Event::PrivateTx(tx_data);
            }
        };

        // 返回固定在堆上的异步事件流。
        Ok(Box::pin(stream))
    }
}
