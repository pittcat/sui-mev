// 该文件 `main.rs` 是 `relay` (中继) 二进制程序的入口点。
// 这个程序的核心功能是：
// 1. 模拟一个Sui验证者节点的gRPC接口，特别是接收已签名交易的端点。
// 2. 当通过gRPC接收到一笔交易时，它不会将该交易提交到Sui共识或执行，
//    而是提取交易数据和签名。
// 3. 将提取到的交易信息（序列化并Base64编码后）通过一个WebSocket服务器广播给所有连接的客户端。
//
// 这种模式常用于MEV (Miner Extractable Value) 相关的基础设施中，例如：
// - 私有交易中继：用户或机器人将交易发送到此中继，而不是直接发送到公开的内存池，
//   以避免被抢先交易 (front-running) 或三明治攻击 (sandwich attacks)。
// - 交易监听/广播服务：中继可以将接收到的交易信息快速广播给特定的参与者（如搜索者、区块提议者），
//   这些参与者可以基于这些信息执行MEV策略或进行其他操作。
//
// 文件概览:
// - `RELAY_SERVER_URL`, `WS_SERVER_URL`: 定义gRPC服务器和WebSocket服务器监听的地址。
// - `TxMessage` 结构体: 用于封装通过WebSocket广播的交易信息（Base64编码的交易字节和签名）。
// - `Relay` 结构体:
//   - 实现了 `sui_network::api::Validator` trait，使其能响应验证者节点的gRPC请求。
//   - `transaction()` 方法是关键，它接收交易，提取信息，并通过 `watch::Sender` 发送 `TxMessage`。
//     注意：此方法最后返回 "Not implemented" 错误，表明它不执行完整的验证者节点功能。
//   - `start_websocket_server()`: 启动一个WebSocket服务器，监听 `watch` 通道上的 `TxMessage`，
//     并将接收到的消息广播给所有WebSocket客户端。
// - `main()` 函数:
//   - 初始化日志和 `watch` 通道。
//   - 创建 `Relay` 实例。
//   - 在一个单独的Tokio任务中启动WebSocket服务器。
//   - 启动gRPC服务器，监听来自客户端的交易提交请求。
// - `subscribe_websocket_messages()`: (死代码) 一个示例函数，演示如何连接并订阅此中继的WebSocket消息。

// 引入所需的库和模块
use async_trait::async_trait; // `async_trait`宏使得在trait中定义异步方法成为可能
use fastcrypto::encoding::Base64; // Base64编解码
use futures::SinkExt; // 为 Sink (如WebSocket的写端) 提供额外的方法，如 `send`
use futures_util::stream::StreamExt; // 为 Stream (如WebSocket的读端) 提供额外的方法，如 `next`
use serde::Serialize; // `serde`库的 `Serialize` trait，用于将数据结构序列化为JSON等格式
use sui_network::api::{Validator, ValidatorServer}; // Sui网络API，定义了Validator trait和ValidatorServer (gRPC服务)
use sui_types::{
    crypto::ToFromBytes, // 用于签名和密钥的字节转换
    messages_checkpoint::{CheckpointRequest, CheckpointRequestV2, CheckpointResponse, CheckpointResponseV2}, // 检查点相关的消息类型 (gRPC接口的一部分)
    messages_grpc::{ // gRPC消息类型定义
        HandleCertificateRequestV3, HandleCertificateResponseV2, HandleCertificateResponseV3,
        HandleSoftBundleCertificatesRequestV3, HandleSoftBundleCertificatesResponseV3, HandleTransactionRequestV2,
        HandleTransactionResponse, HandleTransactionResponseV2, ObjectInfoRequest, ObjectInfoResponse,
        SubmitCertificateResponse, SystemStateRequest, TransactionInfoRequest, TransactionInfoResponse,
    },
    sui_system_state::SuiSystemState, // Sui系统状态对象类型
    transaction::{CertifiedTransaction, Transaction}, // Sui交易类型 (未签名和已认证/已签名)
};
use tokio::{net::TcpListener, sync::watch}; // Tokio库：TCP监听器 (用于WebSocket), `watch` 通道 (单生产者多消费者通道)
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message as WsMessage}; // WebSocket库
use tracing::{debug, error, info}; // 日志库

// gRPC服务器监听的地址和协议。
// "/ip4/0.0.0.0/tcp/9000/http" 是一个多地址 (multiaddr) 格式，表示监听所有IPv4接口的9000端口，使用HTTP (承载gRPC)。
const RELAY_SERVER_URL: &str = "/ip4/0.0.0.0/tcp/9000/http";
// WebSocket服务器监听的地址和端口。
const WS_SERVER_URL: &str = "0.0.0.0:9001";

/// `TxMessage` 结构体
///
/// 用于封装通过WebSocket广播的交易信息。
#[derive(Debug, Clone, Serialize, Default)] // Default用于watch::channel初始化
pub struct TxMessage {
    tx_bytes: String,        // Base64编码的原始交易数据字节 (TransactionData部分)
    signatures: Vec<String>, // Base64编码的签名列表
}

/// `Relay` 结构体
///
/// 实现了 `Validator` trait，并包含一个 `watch::Sender` 用于将接收到的交易信息广播出去。
pub struct Relay {
    tx_sender: watch::Sender<TxMessage>, // `watch` 通道的发送端
}

impl Relay {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `Relay` 实例。
    ///
    /// 参数:
    /// - `tx_sender`: `watch::Sender<TxMessage>`，用于发送提取到的交易信息。
    pub fn new(tx_sender: watch::Sender<TxMessage>) -> Self {
        Relay { tx_sender }
    }

    /// `start_websocket_server` 异步静态方法
    ///
    /// 启动WebSocket服务器。
    /// 服务器会监听指定的 `WS_SERVER_URL`。
    /// 每当有新的WebSocket客户端连接时，它会为该客户端创建一个新的Tokio任务。
    /// 在该任务中，客户端会订阅 `tx_sender` (通过 `tx_sender.subscribe()` 创建一个 `Receiver`)。
    /// 当 `tx_sender` 上有新的 `TxMessage` 发送时，所有订阅的客户端都会收到该消息，
    /// 并通过WebSocket将其发送出去 (序列化为JSON文本格式)。
    ///
    /// 参数:
    /// - `tx_sender`: 从 `main` 函数传递过来的 `watch::Sender<TxMessage>`，用于广播交易消息。
    async fn start_websocket_server(tx_sender: watch::Sender<TxMessage>) {
        info!("WebSocket服务器正在 {} 上运行", WS_SERVER_URL);
        // 绑定TCP监听器到指定地址
        let listener = TcpListener::bind(WS_SERVER_URL).await.unwrap();
        // 循环接受新的TCP连接
        while let Ok((stream, _)) = listener.accept().await {
            let tx_sender_clone = tx_sender.clone(); // 克隆tx_sender以移动到新任务
            //为每个连接创建一个新的Tokio任务来处理WebSocket通信
            tokio::spawn(async move {
                // 执行WebSocket握手
                let ws_stream = accept_async(stream).await.unwrap();
                // 将WebSocket流分割为写端 (sink) 和读端 (stream)
                let (mut write_half, _) = ws_stream.split();
                // 订阅 watch 通道以接收 TxMessage 更新
                let mut tx_receiver = tx_sender_clone.subscribe();
                // 循环等待 watch 通道内容变化
                while tx_receiver.changed().await.is_ok() { // changed() 会在发送端发送新值后返回Ok
                    // 获取通道中最新的 TxMessage (borrow() 不消耗值)
                    let tx_message_to_send = tx_receiver.borrow().clone();
                    // 将 TxMessage 序列化为JSON字符串，然后包装成WebSocket文本消息
                    let ws_msg_to_send = WsMessage::Text(serde_json::to_string(&tx_message_to_send).unwrap());
                    info!("🔥 中继发送WebSocket消息: {:?}", ws_msg_to_send);
                    // 通过WebSocket的写端发送消息
                    if let Err(e) = write_half.send(ws_msg_to_send).await {
                        error!("发送WebSocket消息失败: {:?}, 连接可能已关闭。", e);
                        break; // 发送失败则退出循环，结束此客户端的处理任务
                    }
                }
                // 当 `tx_receiver.changed().await` 返回 `Err` 时，表示发送端已关闭，循环结束。
                debug!("WebSocket客户端的watch通道已关闭或发生错误。");
            });
        }
    }
}

/// 为 `Relay` 结构体实现 `sui_network::api::Validator` trait。
/// 这使得 `Relay` 可以响应Sui验证者节点通常会处理的gRPC请求。
/// 在这个实现中，只有 `transaction` 方法有实际逻辑，其他方法都返回 "Not implemented"。
#[async_trait]
impl Validator for Relay {
    /// `transaction` 方法 (处理已签名的交易提交)
    ///
    /// 这是 `Validator` trait中用于接收客户端提交的已签名交易 (`Transaction`) 的方法。
    /// 此实现会：
    /// 1. 记录接收到的请求。
    /// 2. 从 `Transaction` 中提取原始交易数据 (`TransactionData`) 和签名。
    /// 3. 将它们Base64编码并包装到 `TxMessage` 中。
    /// 4. 通过 `self.tx_sender` (watch通道) 发送这个 `TxMessage`。
    ///    所有通过 `start_websocket_server` 连接的WebSocket客户端都会收到这个消息。
    /// 5. **重要**: 最后返回一个 `tonic::Status::internal("Not implemented")` 错误。
    ///    这意味着此中继服务本身并不打算完整处理或将交易提交到Sui网络共识。
    ///    它仅仅是“拦截”交易，提取信息，然后通过其他渠道（WebSocket）分发出去。
    ///    发送交易给此gRPC端点的客户端会收到一个错误响应。
    async fn transaction(
        &self,
        request: tonic::Request<Transaction>, // 接收到的gRPC请求，包含一个已签名的Sui交易
    ) -> Result<tonic::Response<HandleTransactionResponse>, tonic::Status> {
        info!("🧀 中继服务接收到gRPC交易请求: {:?}", request);

        let signed_transaction = request.into_inner(); // 获取请求中的Transaction对象

        // 提取交易数据 (TransactionData) 并序列化为BCS字节，然后Base64编码。
        let tx_data_bytes_b64 = Base64::from_bytes(
            &bcs::to_bytes(signed_transaction.data().transaction_data()).unwrap()
        ).encoded();

        // 提取所有签名，并将每个签名转换为Base64编码的字符串。
        let signatures_b64: Vec<String> = signed_transaction
            .data()
            .tx_signatures() // 获取交易签名列表
            .iter()
            .map(|sig| Base64::from_bytes(sig.as_bytes()).encoded()) // 对每个签名进行Base64编码
            .collect();

        // 创建 TxMessage
        let tx_message_to_broadcast = TxMessage {
            tx_bytes: tx_data_bytes_b64,
            signatures: signatures_b64,
        };

        // 通过 watch 通道发送 TxMessage。
        // 如果没有订阅者 (WebSocket客户端)，`send` 会返回 `Err`，但消息仍会被存储在watch通道中供未来的订阅者使用。
        if self.tx_sender.send(tx_message_to_broadcast).is_err() {
            // 这通常发生在还没有WebSocket客户端连接并订阅时，或者所有订阅者都已断开。
            // 对于watch通道，即使没有活跃接收者，send也会成功更新通道中的值。
            // send返回Err意味着通道已关闭（所有接收者都已drop）。
            debug!("💤 没有WebSocket订阅者，或者watch通道已关闭。");
        }

        // 返回一个错误响应，表明此方法未完全实现（即中继不直接处理交易到链上）。
        // 客户端（如sui client CLI或SDK）提交交易到此端点时会收到此错误。
        Err(tonic::Status::internal("中继服务不直接处理交易，仅转发"))
    }

    // --- Validator trait 的其他方法 ---
    // 以下所有方法都是 `Validator` trait 的一部分，但在这个中继实现中，它们都没有实际功能，
    // 只是简单地返回 "Not implemented" 错误。
    // 这表明此中继服务专注于拦截和转发 `transaction` 调用，而不提供其他验证者节点的功能。

    async fn transaction_v2(
        &self,
        _request: tonic::Request<HandleTransactionRequestV2>,
    ) -> Result<tonic::Response<HandleTransactionResponseV2>, tonic::Status> {
        error!("方法 transaction_v2 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn submit_certificate(
        &self,
        _request: tonic::Request<CertifiedTransaction>,
    ) -> Result<tonic::Response<SubmitCertificateResponse>, tonic::Status> {
        error!("方法 submit_certificate 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn handle_certificate_v2(
        &self,
        _request: tonic::Request<CertifiedTransaction>,
    ) -> Result<tonic::Response<HandleCertificateResponseV2>, tonic::Status> {
        error!("方法 handle_certificate_v2 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn handle_certificate_v3(
        &self,
        _request: tonic::Request<HandleCertificateRequestV3>,
    ) -> Result<tonic::Response<HandleCertificateResponseV3>, tonic::Status> {
        error!("方法 handle_certificate_v3 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn handle_soft_bundle_certificates_v3(
        &self,
        _request: tonic::Request<HandleSoftBundleCertificatesRequestV3>,
    ) -> Result<tonic::Response<HandleSoftBundleCertificatesResponseV3>, tonic::Status> {
        error!("方法 handle_soft_bundle_certificates_v3 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn object_info(
        &self,
        _request: tonic::Request<ObjectInfoRequest>,
    ) -> Result<tonic::Response<ObjectInfoResponse>, tonic::Status> {
        error!("方法 object_info 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn transaction_info(
        &self,
        _request: tonic::Request<TransactionInfoRequest>,
    ) -> Result<tonic::Response<TransactionInfoResponse>, tonic::Status> {
        error!("方法 transaction_info 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn checkpoint(
        &self,
        _request: tonic::Request<CheckpointRequest>,
    ) -> Result<tonic::Response<CheckpointResponse>, tonic::Status> {
        error!("方法 checkpoint 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn checkpoint_v2(
        &self,
        _request: tonic::Request<CheckpointRequestV2>,
    ) -> Result<tonic::Response<CheckpointResponseV2>, tonic::Status> {
        error!("方法 checkpoint_v2 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }

    async fn get_system_state_object(
        &self,
        _request: tonic::Request<SystemStateRequest>,
    ) -> Result<tonic::Response<SuiSystemState>, tonic::Status> {
        error!("方法 get_system_state_object 未实现");
        Err(tonic::Status::internal("Not implemented"))
    }
}

/// `main` 函数 (程序主入口)
///
/// `#[tokio::main]` 宏将 `main` 函数转换为一个异步函数，并使用Tokio运行时来执行它。
#[tokio::main]
async fn main() {
    // 初始化日志系统，设置 "relay" 模块的日志级别为 debug。
    mev_logger::init_console_logger_with_directives(None, &["relay=debug"]);

    // 创建一个 `watch` 通道。
    // `sender` 是发送端，`_` (下划线) 表示我们不直接使用初始的接收端（新的接收端会通过 `sender.subscribe()` 创建）。
    // `TxMessage::default()` 用于提供通道的初始值。
    let (tx_msg_sender, _) = watch::channel(TxMessage::default());
    // 创建 `Relay` 实例，将 `watch` 通道的发送端传递给它。
    let relay_service = Relay::new(tx_msg_sender.clone()); // 克隆sender以移动到WebSocket服务

    // 在一个新的Tokio任务中启动WebSocket服务器。
    // `tx_msg_sender` 被移动到这个新任务中。
    tokio::spawn(async move {
        Relay::start_websocket_server(tx_msg_sender).await;
    });

    // --- (可选) 测试代码：启动一个WebSocket客户端来订阅消息 ---
    // 这部分代码被注释掉了，但在开发和测试时可以取消注释，
    // 以验证WebSocket服务器是否能正确广播消息。
    // tokio::spawn(async move {
    //     subscribe_websocket_messages().await;
    // });

    // --- 启动gRPC服务器 ---
    // 使用 `mysten_network::config::Config` 构建gRPC服务器配置。
    let server = mysten_network::config::Config::new()
        .server_builder() // 获取服务器构建器
        .add_service(ValidatorServer::new(relay_service)) // 将 `Relay` 实例包装成 `ValidatorServer` 并添加到服务中
        .bind(&RELAY_SERVER_URL.parse().unwrap(), None) // 绑定到指定的gRPC地址和端口
        .await
        .unwrap(); // 处理可能的绑定错误

    info!("gRPC中继服务器正在 {} 上运行", server.local_addr());
    // 启动服务器并开始处理请求。`serve()` 是一个异步方法，会一直运行直到服务器关闭。
    server.serve().await.unwrap(); // 处理可能的服务器运行错误
}

/// `subscribe_websocket_messages` (死代码，用于测试)
///
/// 一个示例函数，演示如何连接到本地运行的WebSocket服务器并接收消息。
#[allow(dead_code)] // 允许存在未使用的代码
async fn subscribe_websocket_messages() {
    let ws_server_address = "ws://localhost:9001"; // WebSocket服务器地址
    // 异步连接到WebSocket服务器
    let (ws_stream, _) = tokio_tungstenite::connect_async(ws_server_address)
        .await
        .expect("无法连接到WebSocket服务器"); // 连接失败则panic
    info!("已成功连接到WebSocket服务器: {}", ws_server_address);

    // 分割WebSocket流为读端和写端 (这里只需要读端)
    let (_, mut read_half) = ws_stream.split();
    // 循环从读端接收消息
    while let Some(message_result) = read_half.next().await {
        match message_result {
            Ok(msg) => info!("✅ WebSocket订阅者接收到消息: {:?}", msg), // 成功接收消息
            Err(e) => error!("WebSocket接收错误: {:?}", e), // 发生错误
        }
    }
    debug!("WebSocket订阅者与服务器的连接已关闭。");
}
