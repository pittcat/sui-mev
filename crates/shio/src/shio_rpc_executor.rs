// 该文件 `shio_rpc_executor.rs` (位于 `shio` crate中) 定义了 `ShioRPCExecutor` 结构体。
// `ShioRPCExecutor` 与 `ShioExecutor` 类似，都负责将MEV竞价提交给Shio协议，
// 但不同之处在于 `ShioRPCExecutor` 是通过 **JSON-RPC** 调用来提交竞价，
// 而 `ShioExecutor` (在 `shio_executor.rs` 中定义) 是通过WebSocket连接（经由 `shio_conn` 模块）提交。
// 这为与Shio协议交互提供了另一种可选的通信方式。
//
// **文件概览 (File Overview)**:
// 这个文件定义了一个叫做 `ShioRPCExecutor` 的“竞价提交员（RPC版）”。
// 它的工作和 `ShioExecutor` 一样，都是接收套利机器人的竞价“指令包”，然后把它发给Shio服务器。
// 不同的是，它不走WebSocket“电话线”，而是像写一封“挂号信”（JSON-RPC请求）寄给Shio服务器。
//
// **核心组件 (Core Components)**:
// 1.  **`ShioRPCExecutor` 结构体**:
//     -   `keypair: SuiKeyPair`: 用于对套利交易 (`TransactionData`) 进行签名的Sui密钥对。
//     -   `rpc_client: reqwest::Client`: 一个 `reqwest` HTTP客户端实例，专门用于发送JSON-RPC请求。
//         `reqwest` 是一个流行的Rust HTTP客户端库。
//
// 2.  **`ShioRPCExecutor::new()` 构造函数**:
//     -   创建一个新的 `ShioRPCExecutor` 实例。
//     -   参数:
//         -   `keypair`: Sui密钥对。
//     -   它会内部创建一个新的 `reqwest::Client` 实例。
//
// 3.  **`ShioRPCExecutor::encode_bid()` 异步方法**:
//     -   **功能**: 将套利交易、竞价金额和机会交易摘要编码为一个符合JSON-RPC规范的 `serde_json::Value` 对象，
//         用于调用Shio服务器的 `shio_submitBid` 方法。
//     -   **参数**: 与 `ShioExecutor::encode_bid` 相同。
//     -   **实现**:
//         1.  对 `tx_data` 进行签名（与 `ShioExecutor::encode_bid` 中的签名逻辑完全相同）。
//         2.  将 `tx_data` 序列化为BCS字节流，然后Base64编码。
//         3.  构建一个JSON-RPC请求体 (JSON对象)，包含以下字段：
//             -   `jsonrpc`: "2.0" (JSON-RPC版本)。
//             -   `id`: 1 (请求ID，可以是任意值，用于匹配请求和响应)。
//             -   `method`: "shio_submitBid" (要调用的远程方法名)。
//             -   `params`: 一个JSON数组，包含调用 `shio_submitBid` 方法所需的参数：
//                 -   机会交易摘要的Base58编码字符串。
//                 -   竞价金额 (u64)。
//                 -   套利交易的Base64编码字符串。
//                 -   套利交易的签名。
//     -   **返回**: `Result<Value>`，表示编码后的JSON-RPC请求对象。
//
// 4.  **`Executor<(TransactionData, u64, TransactionDigest)>` trait 实现**:
//     -   与 `ShioExecutor` 一样，使得 `ShioRPCExecutor` 也可以被 `burberry::Engine` 用作处理相同类型竞价动作的执行器。
//     -   **`name()`**: 返回执行器的名称 "ShioRPCExecutor"。
//     -   **`execute()`**:
//         -   接收 `(tx_data, bid_amount, opp_tx_digest)` 元组。
//         -   调用 `self.encode_bid()` 将其编码为JSON-RPC请求对象。
//         -   使用 `self.rpc_client` 向 `SHIO_JSON_RPC_URL` (在 `lib.rs` 中定义的Shio RPC服务器地址)
//             发送一个HTTP POST请求，请求体为编码后的JSON对象。
//         -   记录请求 (`tracing::warn!`) 和服务器的响应状态及内容 (`tracing::warn!`)。
//             （使用 `warn!` 级别可能是为了在日志中突出显示这些重要的网络交互）。
//         -   如果HTTP请求或响应处理成功，则返回 `Ok(())`。
//
// **工作流程 (Workflow)**:
// (与 `ShioExecutor` 类似，但最后一步是通过HTTP POST发送JSON-RPC请求，而不是通过WebSocket通道)
// 1.  策略模块产生 `Action::ShioSubmitBid` 动作。
// 2.  引擎将动作分发给 `ShioRPCExecutor`。
// 3.  `ShioRPCExecutor::execute()` 被调用。
// 4.  `execute()` 调用 `self.encode_bid()` 准备JSON-RPC请求体。
// 5.  `execute()` 使用 `reqwest::Client` 将此JSON-RPC请求发送到Shio服务器的RPC端点。
// 6.  记录服务器响应。
//
// **JSON-RPC**:
// -   一种轻量级的远程过程调用（RPC）协议，使用JSON作为其数据格式。
// -   一个JSON-RPC请求通常包含 `jsonrpc` (版本), `method` (要调用的方法名), `params` (方法参数), 和 `id` (请求ID)。
// -   服务器的响应也会是一个JSON对象，包含 `result` (成功时的方法返回值) 或 `error` (失败时的错误信息)，以及对应的 `id`。

// 引入 burberry 框架的 Executor trait 和 async_trait 宏。
use burberry::{async_trait, Executor};
// 引入 eyre 库的 Result 类型，用于错误处理。
use eyre::Result;
// 引入 fastcrypto 库的 Base64 编码和 HashFunction trait。
use fastcrypto::{encoding::Base64, hash::HashFunction};
// 引入 serde_json 库的 json! 宏 (用于方便地创建JSON Value) 和 Value 类型 (通用的JSON值)。
use serde_json::{json, Value};
// 引入 Sui 共享加密库中的 Intent 和 IntentMessage，用于创建安全的交易意图。
use shared_crypto::intent::{Intent, IntentMessage};
// 引入 Sui 核心类型库中的相关类型。
use sui_types::{
    crypto::{Signer, SuiKeyPair},         // Signer trait (用于签名), SuiKeyPair (密钥对)
    digests::TransactionDigest,           // 交易摘要 (哈希)
    transaction::TransactionData,         // 未签名的交易数据结构
};

// 从当前crate的根模块引入 SHIO_JSON_RPC_URL 常量。
use crate::SHIO_JSON_RPC_URL;

/// `ShioRPCExecutor` 结构体
///
/// 负责对套利交易进行签名，将其与竞价信息一起编码为JSON-RPC请求，
/// 并通过HTTP POST将此请求发送给Shio服务器的JSON-RPC端点。
pub struct ShioRPCExecutor {
    keypair: SuiKeyPair,            // 用于签名套利交易的Sui密钥对
    rpc_client: reqwest::Client,    // `reqwest` HTTP客户端实例，用于发送JSON-RPC请求
}

impl ShioRPCExecutor {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `ShioRPCExecutor` 实例。
    ///
    /// 参数:
    /// - `keypair`: 用于签名交易的 `SuiKeyPair`。
    ///
    /// 返回:
    /// - `Self`: 新创建的 `ShioRPCExecutor` 实例。
    pub fn new(keypair: SuiKeyPair) -> Self {
        // 创建一个新的 reqwest::Client 实例。这个客户端可以被复用以发送多个HTTP请求。
        let rpc_client = reqwest::Client::new();
        Self { keypair, rpc_client }
    }

    /// `encode_bid` 异步方法
    ///
    /// 将套利交易数据、竞价金额和相关的机会交易摘要编码为符合Shio `shio_submitBid` JSON-RPC方法要求的JSON对象。
    ///
    /// 参数:
    /// - `tx_data`: 套利者构建的 `TransactionData`。
    /// - `bid_amount`: 为此机会明确出价的金额 (u64)。
    /// - `opp_tx_digest`: 原始机会交易的 `TransactionDigest`。
    ///
    /// 返回:
    /// - `Result<Value>`: 成功则返回编码后的 `serde_json::Value` JSON-RPC请求对象，否则返回错误。
    pub async fn encode_bid(
        &self,
        tx_data: TransactionData,       // 注意：这里接收的是 tx_data 的所有权
        bid_amount: u64,
        opp_tx_digest: TransactionDigest,
    ) -> Result<Value> {
        // 1. 将原始 `TransactionData` 序列化为BCS字节流，然后进行Base64编码。
        let tx_bytes = bcs::to_bytes(&tx_data)?;
        let tx_b64 = Base64::from_bytes(&tx_bytes).encoded();

        // 2. 对 `TransactionData` 进行签名。
        let signature_object = { // 使用Sui的Signature类型，它应该实现了serde::Serialize
            let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data); // tx_data被移动到intent_msg
            let raw_tx_to_sign = bcs::to_bytes(&intent_msg)?;
            let digest_to_be_signed = {
                let mut hasher = sui_types::crypto::DefaultHash::default();
                hasher.update(raw_tx_to_sign); // 这里没有克隆，如果raw_tx_to_sign后续还需使用则应克隆
                hasher.finalize().digest
            };
            self.keypair.sign(&digest_to_be_signed) // 返回一个 Signature 对象
        };

        // 3. 构建JSON-RPC请求对象。
        Ok(json!({ // 使用 json! 宏创建 serde_json::Value
            "jsonrpc": "2.0", // JSON-RPC版本
            "id": 1,          // 请求ID (可以是任意数字或字符串，用于客户端匹配响应)
            "method": "shio_submitBid", // 要调用的远程方法名
            "params": [ // 参数数组，顺序和类型需与Shio服务器的 shio_submitBid 方法定义一致
                opp_tx_digest.base58_encode(), // 参数1: 机会交易摘要 (Base58编码)
                bid_amount,                    // 参数2: 竞价金额 (u64)
                tx_b64,                        // 参数3: 套利交易的Base64编码字符串
                signature_object,              // 参数4: 套利交易的签名 (依赖Signature的Serialize实现，通常是签名的Base64字符串或其他可序列化形式)
            ]
        }))
    }
}

/// 为 `ShioRPCExecutor` 实现 `burberry::Executor` trait。
/// 执行器处理的动作类型为 `(TransactionData, u64, TransactionDigest)` 元组。
#[async_trait]
impl Executor<(TransactionData, u64, TransactionDigest)> for ShioRPCExecutor {
    /// `name` 方法 (来自 `Executor` trait)
    /// 返回执行器的名称。
    fn name(&self) -> &str {
        "ShioRPCExecutor"
    }

    /// `execute` 方法 (来自 `Executor` trait)
    ///
    /// 执行一个Shio竞价动作，通过JSON-RPC提交。
    ///
    /// 参数:
    /// - `action_tuple`: 包含套利交易数据、竞价金额和机会交易摘要的元组。
    ///
    /// 返回:
    /// - `Result<()>`: 如果编码和HTTP POST请求成功（不一定表示服务器业务逻辑成功），则返回Ok。
    ///   如果过程中发生错误，则返回错误。
    async fn execute(
        &self,
        action_tuple: (TransactionData, u64, TransactionDigest),
    ) -> Result<()> {
        let (tx_data_val, bid_amount_val, opp_tx_digest_val) = action_tuple; // 解构元组

        // 调用 `encode_bid` 方法将信息编码为JSON-RPC请求对象。
        let rpc_bid_payload = self.encode_bid(tx_data_val, bid_amount_val, opp_tx_digest_val).await?;
        // 使用 `tracing::warn!` 记录将要发送的JSON请求体。
        // 使用warn级别可能是为了在日志中使其更显眼，便于调试网络交互。
        tracing::warn!("🧀>> 准备发送Shio RPC竞价请求: {}", rpc_bid_payload);

        // 使用 `self.rpc_client` (reqwest::Client) 发送HTTP POST请求。
        // - URL是 `SHIO_JSON_RPC_URL` (在lib.rs中定义的常量)。
        // - `.json(&rpc_bid_payload)` 将请求体序列化为JSON并设置Content-Type为application/json。
        // - `.send().await?` 发送请求并等待响应，处理网络层面的错误。
        let http_response = self.rpc_client.post(SHIO_JSON_RPC_URL).json(&rpc_bid_payload).send().await?;

        // 获取HTTP响应的状态码。
        let response_status = http_response.status();
        // 获取HTTP响应体为文本。
        let response_text = http_response.text().await?;
        // 使用 `tracing::warn!` 记录响应的状态码和响应体文本。
        tracing::warn!("🧀<<收到Shio RPC响应: 状态码={:?}, 响应体={:?}", response_status, response_text);

        // 这里可以根据 `response_status` 或 `response_text` 中的内容做进一步的成功/失败判断。
        // 例如，检查状态码是否为200 OK，以及响应体中是否有 "error" 字段。
        // 当前实现只是记录响应，并假设只要HTTP请求本身没有错误，就认为是Ok(())。
        // 如果需要更严格的错误处理，例如确保Shio服务器成功接受了bid，则需要解析 `response_text`。
        // if !response_status.is_success() {
        //     return Err(eyre!("Shio RPC请求失败，状态码: {}, 响应: {}", response_status, response_text));
        // }
        // let json_response: Value = serde_json::from_str(&response_text)?;
        // if json_response.get("error").is_some() {
        //     return Err(eyre!("Shio RPC返回错误: {}", json_response["error"]));
        // }

        Ok(())
    }
}

[end of crates/shio/src/shio_rpc_executor.rs]
