// 该文件 `executor.rs` 定义了 `PublicTxExecutor`，它负责执行（提交到Sui链上）
// 由套利逻辑或其他部分构建好的交易 (`TransactionData`)。
// "Public" 可能意味着它通过公共Sui RPC节点提交交易。
//
// 文件概览:
// 1. `PublicTxExecutor` 结构体:
//    - 持有一个 `SuiClient` 实例，用于与Sui RPC节点通信。
//    - 持有一个 `SuiKeyPair`，包含了签名交易所需的公私钥对。
// 2. `new()` 方法: `PublicTxExecutor` 的构造函数，初始化Sui客户端。
// 3. `execute_tx()` 方法: 核心的交易执行逻辑。它接收一个 `TransactionData`，
//    对其进行签名，然后通过Sui客户端的 `quorum_driver_api` 将签名后的交易提交到链上执行。
//    返回交易的响应 (`SuiTransactionBlockResponse`)。
// 4. 实现了 `burberry::Executor<TransactionData>` trait:
//    - `name()`: 返回执行器的名称。
//    - `execute()`: `Executor` trait要求的方法，接收一个 `TransactionData` (在这里是 `action`)，
//      调用 `execute_tx()` 执行它，并记录执行结果的日志。
//
// Sui概念:
// - TransactionData: 代表一个未签名的Sui交易。它包含了交易的所有指令、参数、Gas设置等。
// - SuiKeyPair: 一个包含了Sui账户的公钥和对应私钥的结构。私钥用于对交易进行签名。
// - IntentMessage (意图消息): 在对交易数据进行签名之前，Sui会将其包装在一个 `IntentMessage` 中。
//   这是一种安全措施，确保签名者清楚他们正在签的是什么类型的消息 (这里是 `Intent::sui_transaction()`)。
// - BCS (Binary Canonical Serialization): Sui用于序列化交易数据和其他核心类型的二进制格式。
// - Transaction Digest (交易摘要/哈希): 通过对序列化后的交易数据（通常是`IntentMessage`包装后的）进行哈希计算，
//   得到一个唯一的摘要值。这个摘要用于唯一标识一笔交易，并且签名是针对这个摘要进行的。
// - GenericSignature (通用签名): Sui支持多种签名方案，`GenericSignature` 是一个枚举，可以包装不同类型的签名。
// - Transaction (已签名交易): 包含了 `TransactionData` 和对其摘要的签名。这是最终提交到链上的形式。
// - Quorum Driver API: Sui客户端中用于与Sui网络的验证者（通过Quorum Driver）交互以提交和确认交易的API。
// - SuiTransactionBlockResponse: 执行交易后，Sui节点返回的响应，包含了交易摘要、执行效果 (effects)、
//   事件、对象变更等详细信息。

// 引入所需的库和模块
use async_trait::async_trait; // `async_trait`宏使得在trait中定义异步方法成为可能
use burberry::Executor; // 从外部 `burberry` crate 引入 `Executor` trait。
                        // `burberry` 可能是一个自定义的框架或库，用于定义可执行的任务或动作。
use eyre::Result; // `eyre`库，用于更方便的错误处理
use fastcrypto::hash::HashFunction; // `fastcrypto`库提供的哈希函数接口
use shared_crypto::intent::{Intent, IntentMessage}; // Sui共享的加密库，用于处理交易意图
use sui_json_rpc_types::{SuiTransactionBlockResponse, SuiTransactionBlockResponseOptions}; // Sui RPC相关的类型，如交易块响应和选项
use sui_sdk::{SuiClient, SuiClientBuilder}; // Sui SDK，用于与Sui节点交互
use sui_types::{
    crypto::{Signer, SuiKeyPair}, // Sui加密相关的类型，如签名者接口 (`Signer`) 和密钥对 (`SuiKeyPair`)
    signature::GenericSignature,    // 通用签名类型
    transaction::{Transaction, TransactionData}, // Sui交易数据和已签名交易的类型
};
use tracing::info; // `tracing`库，用于日志记录

/// `PublicTxExecutor` 结构体
///
/// 负责签名并执行Sui交易。
pub struct PublicTxExecutor {
    sui: SuiClient,         // Sui RPC客户端，用于与Sui网络通信
    keypair: SuiKeyPair,    // 签名交易所需的密钥对 (包含公钥和私钥)
}

impl PublicTxExecutor {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `PublicTxExecutor` 实例。
    ///
    /// 参数:
    /// - `rpc_url`: Sui RPC节点的URL字符串。
    /// - `keypair`: 用于签名交易的 `SuiKeyPair`。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `PublicTxExecutor` 实例。
    pub async fn new(rpc_url: &str, keypair: SuiKeyPair) -> Result<Self> {
        // 使用提供的RPC URL构建一个新的SuiClient实例。
        let sui_client = SuiClientBuilder::default().build(rpc_url).await?;
        Ok(Self { sui: sui_client, keypair })
    }

    /// `execute_tx` 方法
    ///
    /// 对给定的 `TransactionData`进行签名，并将其提交到Sui网络执行。
    ///
    /// 参数:
    /// - `tx_data`: 要执行的未签名交易数据 (`TransactionData`)。
    ///
    /// 返回:
    /// - `Result<SuiTransactionBlockResponse>`: 包含交易执行结果的响应。
    pub async fn execute_tx(&self, tx_data: TransactionData) -> Result<SuiTransactionBlockResponse> {
        // 步骤 1: 创建交易意图 (IntentMessage)。
        // 这是Sui为了安全和清晰性要求的一个步骤，确保签名者知道他们正在签的是一笔Sui交易。
        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);

        // 步骤 2: 将意图消息序列化为BCS字节流。
        // BCS是Sui标准的二进制序列化格式。
        let raw_tx_bytes = bcs::to_bytes(&intent_msg)?;

        // 步骤 3: 计算序列化后的交易数据的摘要 (哈希值)。
        // 签名是针对这个摘要进行的。
        let digest_to_sign = {
            let mut hasher = sui_types::crypto::DefaultHash::default(); // 获取Sui默认的哈希算法实现
            hasher.update(raw_tx_bytes.clone()); // 更新哈希器的状态 (注意：这里克隆了raw_tx_bytes，如果数据量大可能考虑优化)
                                                // 但对于交易数据通常还好。
            hasher.finalize().digest // 完成哈希计算并获取32字节的摘要结果
        };

        // 步骤 4: 使用私钥对摘要进行签名。
        // `self.keypair` (SuiKeyPair) 实现了 `Signer` trait，提供了 `sign` 方法。
        let signature = self.keypair.sign(&digest_to_sign);

        // 步骤 5: 创建已签名的交易 (`Transaction`)。
        // 它将原始的交易数据 (`intent_msg.value`，即 `tx_data`) 和签名组合在一起。
        // `GenericSignature::Signature(sig)` 将具体的签名包装成Sui通用的签名类型。
        let signed_transaction = Transaction::from_generic_sig_data(intent_msg.value, vec![GenericSignature::Signature(signature)]);

        // 步骤 6: 设置交易响应选项。
        // `SuiTransactionBlockResponseOptions::default()` 通常请求包含交易效果 (effects) 等基本信息。
        // 可以根据需要配置更详细的选项，例如请求返回原始输入对象、对象变更等。
        let response_options = SuiTransactionBlockResponseOptions::default();

        // 步骤 7: 通过Sui客户端的 `quorum_driver_api` 提交已签名的交易到链上执行。
        // `execute_transaction_block` 方法会等待Sui网络就该交易达成共识（或超时/出错）。
        // `None` 作为第三个参数表示请求执行模式为默认 (通常是等待最终确认 `WaitForEffectsCert`)。
        let transaction_response = self
            .sui
            .quorum_driver_api()
            .execute_transaction_block(signed_transaction, response_options, None)
            .await?;

        // 返回交易执行的响应。
        Ok(transaction_response)
    }
}

/// 为 `PublicTxExecutor` 实现 `burberry::Executor<TransactionData>` trait。
/// 这使得 `PublicTxExecutor` 可以被用在 `burberry` 框架中作为一种特定类型动作（这里是`TransactionData`）的执行器。
#[async_trait]
impl Executor<TransactionData> for PublicTxExecutor {
    /// `name` 方法 (来自 `Executor` trait)
    ///
    /// 返回执行器的名称。
    fn name(&self) -> &str {
        "PublicTxExecutor"
    }

    /// `execute` 方法 (来自 `Executor` trait)
    ///
    /// 执行一个给定的动作 (`action`，这里是 `TransactionData`)。
    ///
    /// 参数:
    /// - `action`: 要执行的 `TransactionData`。
    ///
    /// 返回:
    /// - `Result<()>`: 如果执行（包括提交和记录日志）成功，则返回Ok(())，否则返回错误。
    async fn execute(&self, action: TransactionData) -> Result<()> {
        // 调用内部的 `execute_tx` 方法实际执行交易。
        let response = self.execute_tx(action).await?;
        // 获取交易摘要的Base58编码字符串，用于日志记录。
        let digest_str = response.digest.base58_encode();

        // 记录日志：包含交易摘要和交易是否成功 (`status_ok()`)。
        info!(digest = %digest_str, status_ok = ?response.status_ok(), "已执行交易 (Executed tx)");
        Ok(())
    }
}
