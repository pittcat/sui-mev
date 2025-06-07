// 该文件 `shio_executor.rs` (位于 `shio` crate中) 定义了 `ShioExecutor` 结构体。
// `ShioExecutor` 负责将套利机器人构建的MEV竞价 (bid) 编码并通过一个异步通道发送出去，
// 这个通道的接收端在 `shio_conn` 模块中，负责将竞价信息通过WebSocket实际提交给Shio服务器。
// 它实现了 `burberry::Executor` trait，使其可以被集成到 `burberry::Engine` 事件处理引擎中，
// 专门处理类型为 `(TransactionData, u64, TransactionDigest)` 的动作，这种动作代表一个Shio竞价请求。
//
// **文件概览 (File Overview)**:
// 这个文件定义了一个叫做 `ShioExecutor` 的“竞价提交员”。
// 当套利机器人的“大脑”（策略模块）决定要对一个Shio MEV机会进行竞价时，它会产生一个包含
// 套利交易本身 (`TransactionData`)、愿意支付的竞价金额 (`u64`)、以及原始机会交易的摘要 (`TransactionDigest`)
// 的“指令包”。`ShioExecutor` 的工作就是接收这个指令包，把它包装成Shio服务器能看懂的格式，
// 然后通过“电话线”（异步通道）把它发给负责实际网络通信的 `shio_conn` 模块。
//
// **核心组件 (Core Components)**:
// 1.  **`ShioExecutor` 结构体**:
//     -   `keypair: SuiKeyPair`: 用于对套利交易 (`TransactionData`) 进行签名的Sui密钥对。
//         虽然竞价信息本身可能不需要签名，但其附带的套利交易通常需要签名。
//     -   `bid_sender: Sender<Value>`: 这是一个异步通道 (`async_channel`) 的发送端。
//         `ShioExecutor` 将编码好的竞价信息（一个 `serde_json::Value` 对象）通过这个发送端
//         发送给 `shio_conn` 模块中的连接管理任务。
//
// 2.  **`ShioExecutor::new()` 异步构造函数**:
//     -   创建一个新的 `ShioExecutor` 实例。
//     -   参数:
//         -   `keypair`: Sui密钥对。
//         -   `bid_sender`: 从 `shio_conn::new_shio_conn()` 获取的竞价发送通道的发送端。
//
// 3.  **`ShioExecutor::encode_bid()` 异步方法**:
//     -   **功能**: 将一个套利交易、竞价金额和机会交易摘要编码为一个 `serde_json::Value` 对象，
//         这个JSON对象是符合Shio服务器提交竞价API要求的格式。
//     -   **参数**:
//         -   `tx_data: TransactionData`: 套利者构建的、希望被优先打包的套利交易。
//         -   `bid_amount: u64`: 套利者愿意为这个机会支付的竞价金额（小费）。
//         -   `opp_tx_digest: TransactionDigest`: 原始机会交易的摘要。
//     -   **实现**:
//         1.  将 `tx_data` 序列化为BCS字节流，然后进行Base64编码，得到 `tx_b64`。
//         2.  对 `tx_data` 进行签名：
//             a.  将 `tx_data` 包裹在 `IntentMessage` 中（增加安全性）。
//             b.  将 `IntentMessage` 序列化为BCS字节流 (`raw_tx`)。
//             c.  计算 `raw_tx` 的摘要 (`digest`)。
//             d.  使用 `self.keypair` 对 `digest`进行签名，得到 `sig`。
//         3.  构建一个JSON对象，包含以下字段：
//             -   `oppTxDigest`: 机会交易摘要的Base58编码字符串。
//             -   `bidAmount`: 竞价金额 (u64)。
//             -   `txData`: 套利交易的Base64编码字符串。
//             -   `sig`: 套利交易的签名 (签名的具体格式取决于 `keypair.sign()` 返回的类型，这里直接用，JSON宏会处理其序列化)。
//     -   **返回**: `Result<Value>`，表示编码后的JSON对象。
//
// 4.  **`Executor<(TransactionData, u64, TransactionDigest)>` trait 实现**:
//     -   这使得 `ShioExecutor` 可以被 `burberry::Engine` 用作一个特定类型动作的执行器。
//         这里的动作类型是一个元组 `(TransactionData, u64, TransactionDigest)`，正好对应 `encode_bid` 的输入参数。
//     -   **`name()`**: 返回执行器的名称 "ShioExecutor"。
//     -   **`execute()`**: 这是 `Executor` trait的核心方法。
//         -   它接收一个包含 `(tx_data, bid_amount, opp_tx_digest)` 的元组作为动作。
//         -   调用 `self.encode_bid()` 将这些信息编码为Shio服务器期望的JSON格式。
//         -   通过 `self.bid_sender.send(bid_json).await?` 将编码后的JSON竞价信息异步发送到通道中。
//           如果发送失败（例如通道已关闭），则返回错误。
//
// **工作流程 (Workflow)**:
// 1.  `ArbStrategy` (套利策略模块) 决定对一个Shio MEV机会进行竞价。
// 2.  策略模块会创建一个 `Action::ShioSubmitBid((tx_data, bid_amount, opp_tx_digest))` 动作。
// 3.  `burberry::Engine` 将这个动作分发给注册为处理此动作类型的 `ShioExecutor`。
// 4.  `ShioExecutor::execute()` 方法被调用。
// 5.  `execute()` 调用 `self.encode_bid()` 来：
//     a.  对 `tx_data` 进行签名。
//     b.  将 `tx_data` (Base64编码后)、签名、`bid_amount` 和 `opp_tx_digest` (Base58编码后) 组装成一个JSON对象。
// 6.  `execute()` 将这个JSON对象通过 `self.bid_sender` 发送出去。
// 7.  在 `shio_conn` 模块中运行的后台任务会从 `bid_receiver` 接收到这个JSON对象，
//     并将其通过WebSocket发送给Shio服务器。
//
// **关键点 (Key Points)**:
// -   **签名**: 套利交易 `tx_data` 在编码为竞价信息之前被正确地签名。
// -   **编码**: 竞价信息被编码为特定的JSON格式，以符合Shio服务器的API要求。
// -   **异步发送**: 竞价信息通过异步通道发送，实现了与实际网络提交的解耦。

// 引入 async_channel 库的 Sender 类型，用于异步多生产者多消费者通道的发送端。
use async_channel::Sender;
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

/// `ShioExecutor` 结构体
///
/// 负责对包含套利逻辑的交易数据进行签名，将其与竞价信息一起编码，
/// 并通过异步通道发送给 `shio_conn` 模块以提交到Shio MEV协议服务器。
pub struct ShioExecutor {
    keypair: SuiKeyPair,       // 用于签名套利交易的Sui密钥对
    bid_sender: Sender<Value>, // 异步通道的发送端，用于将编码后的竞价信息 (JSON Value) 发送给连接管理器
}

impl ShioExecutor {
    /// `new` 异步构造函数
    ///
    /// 创建一个新的 `ShioExecutor` 实例。
    ///
    /// 参数:
    /// - `keypair`: 用于签名交易的 `SuiKeyPair`。
    /// - `bid_sender`: 一个 `Sender<Value>`，从 `shio_conn::new_shio_conn()` 返回，
    ///                 用于将编码后的竞价发送到WebSocket连接管理任务。
    ///
    /// 返回:
    /// - `Self`: 新创建的 `ShioExecutor` 实例。
    pub async fn new(keypair: SuiKeyPair, bid_sender: Sender<Value>) -> Self {
        Self { keypair, bid_sender }
    }

    /// `encode_bid` 异步方法
    ///
    /// 将套利交易数据、竞价金额和相关的机会交易摘要编码为Shio服务器期望的JSON格式。
    ///
    /// 参数:
    /// - `tx_data`: 套利者构建的 `TransactionData`，这笔交易通常包含套利操作和支付给验证者的竞价金额。
    /// - `bid_amount`: 为此机会明确出价的金额 (u64类型，以SUI的最小单位MIST计)。
    /// - `opp_tx_digest`: 原始机会交易的 `TransactionDigest`。
    ///
    /// 返回:
    /// - `Result<Value>`: 成功则返回编码后的 `serde_json::Value` 对象，否则返回错误。
    pub async fn encode_bid(
        &self,
        tx_data: TransactionData,       // 注意：这里接收的是 tx_data 的所有权
        bid_amount: u64,
        opp_tx_digest: TransactionDigest,
    ) -> Result<Value> {
        // 1. 将原始 `TransactionData` (不包含意图包装) 序列化为BCS字节流，然后进行Base64编码。
        //    这个 `tx_b64` 是 Shio 竞价API中 `txData` 字段需要的值。
        let tx_bytes = bcs::to_bytes(&tx_data)?;
        let tx_b64 = Base64::from_bytes(&tx_bytes).encoded();

        // 2. 对 `TransactionData` 进行签名，以生成提交给Shio API的 `sig` 字段。
        let signature_string = {
            // a. 创建 IntentMessage: 将 tx_data 包裹在标准的 Sui 交易意图中。
            //    注意：这里 `tx_data` 被移动，所以上面的 `bcs::to_bytes(&tx_data)` 如果在之后调用，
            //    就需要克隆 `tx_data`。当前顺序是正确的。
            let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
            // b. 将 IntentMessage 序列化为BCS字节流。这是实际要被签名的数据的哈希。
            let raw_tx_to_sign = bcs::to_bytes(&intent_msg)?;
            // c. 计算序列化后的 IntentMessage 的摘要 (哈希)。
            let digest_to_be_signed = {
                let mut hasher = sui_types::crypto::DefaultHash::default(); // 使用Sui的默认哈希算法
                hasher.update(raw_tx_to_sign); // 注意：这里没有克隆 raw_tx_to_sign，如果它后续还需使用则应克隆
                hasher.finalize().digest // 计算哈希
            };

            // d. 使用实例持有的 `self.keypair` 对摘要进行签名。
            //    `keypair.sign()` 返回一个实现了 `Signature` trait 的类型。
            //    签名的结果需要被序列化 (例如Base64编码) 以包含在JSON中。
            //    `sig.as_bytes()` 获取签名的原始字节，然后进行Base64编码。
            //    (这里直接用了 `sig`，`json!` 宏会处理 `Signature::to_string()` 或类似序列化)
            //    **修正/确认**: `json!` 宏通常会调用类型的 `Serialize` 实现。
            //    Sui的 `Signature` 类型 (如 `Ed25519Signature`) 应该实现了 `Serialize`，
            //    它可能会序列化为Base64字符串或特定的字符串表示。
            //    如果Shio API期望的是纯Base64编码的签名字节，那么应该显式编码：
            //    `Base64::from_bytes(self.keypair.sign(&digest_to_be_signed).as_bytes()).encoded()`
            //    当前代码直接将 `Signature` 对象放入 `json!` 宏，依赖其默认序列化行为。
            self.keypair.sign(&digest_to_be_signed)
        };

        // 3. 构建并返回JSON对象。
        Ok(json!({ // 使用 json! 宏创建 serde_json::Value
            "oppTxDigest": opp_tx_digest.base58_encode(), // 机会交易摘要，使用Base58编码
            "bidAmount": bid_amount,                      // 竞价金额 (u64)
            "txData": tx_b64,                             // 套利交易的Base64编码字符串
            "sig": signature_string,                      // 套利交易的签名 (依赖Signature的Serialize实现)
        }))
    }
}

/// 为 `ShioExecutor` 实现 `burberry::Executor` trait。
/// Executor的泛型参数 `(TransactionData, u64, TransactionDigest)` 定义了此执行器能处理的动作类型。
#[async_trait]
impl Executor<(TransactionData, u64, TransactionDigest)> for ShioExecutor {
    /// `name` 方法 (来自 `Executor` trait)
    ///
    /// 返回执行器的名称。
    fn name(&self) -> &str {
        "ShioExecutor"
    }

    /// `execute` 方法 (来自 `Executor` trait)
    ///
    /// 执行一个Shio竞价动作。
    /// 它接收一个包含套利交易数据、竞价金额和机会交易摘要的元组，
    /// 调用 `self.encode_bid` 将其编码为JSON，然后通过 `self.bid_sender` 通道异步发送出去。
    ///
    /// 参数:
    /// - `action_tuple`: 一个元组 `(TransactionData, u64, TransactionDigest)`，
    ///                   包含了执行Shio竞价所需的所有信息。
    ///
    /// 返回:
    /// - `Result<()>`: 如果编码和发送操作成功，则返回Ok。如果过程中发生错误（例如编码失败、通道发送失败），则返回错误。
    async fn execute(
        &self,
        action_tuple: (TransactionData, u64, TransactionDigest), // 解构元组参数
    ) -> Result<()> {
        // 从元组中解构出各个部分
        let (tx_data_val, bid_amount_val, opp_tx_digest_val) = action_tuple;

        // 调用 `encode_bid` 方法将信息编码为JSON格式的竞价。
        let bid_json_payload = self.encode_bid(tx_data_val, bid_amount_val, opp_tx_digest_val).await?;
        // 通过异步通道 `self.bid_sender` 发送编码后的JSON竞价。
        // `shio_conn` 模块中的后台任务会接收这个JSON并通过WebSocket将其提交给Shio服务器。
        // `.await?` 用于等待发送操作完成并处理可能的发送错误 (例如通道已关闭)。
        self.bid_sender.send(bid_json_payload).await?;
        Ok(())
    }
}

[end of crates/shio/src/shio_executor.rs]
