// 该文件 `executor.rs` 定义了 `PublicTxExecutor`，它负责执行（提交到Sui链上）
// 由套利逻辑或其他部分构建好的交易 (`TransactionData`)。
// "Public" 可能意味着它通过公共Sui RPC节点提交交易。
//
// **文件概览 (File Overview)**:
// 这个文件定义了一个名为 `PublicTxExecutor` 的结构体，它的主要工作是把已经准备好的交易发送到Sui区块链上去执行。
// 想象一下，套利机器人是“大脑”，它分析市场并决定要做什么交易；而 `PublicTxExecutor` 就是“手臂”，负责把大脑的决定（交易指令）实际地递交给Sui网络。
//
// 1.  **`PublicTxExecutor` 结构体**:
//     -   **`sui: SuiClient`**:  一个 `SuiClient` 实例。这就像一个电话，`PublicTxExecutor` 用它来和Sui网络（通过RPC节点）沟通。
//     -   **`keypair: SuiKeyPair`**: 用户的密钥对（公钥和私钥）。私钥是用来给交易“签名”的，就像在一张支票上签名一样，证明这笔交易确实是你发起的。
//
// 2.  **`new()` 方法**:
//     这是 `PublicTxExecutor` 的“制造工厂”（构造函数）。当你需要一个新的执行器时，就调用这个方法。
//     它会帮你设置好与Sui网络的连接（初始化 `SuiClient`）。
//
// 3.  **`execute_tx()` 方法**:
//     这是执行交易的核心“动作”。它接收一个 `TransactionData`（包含了交易的所有细节，但还没签名），然后：
//     -   **签名 (Signs)**: 用你的私钥给这个交易签名。
//     -   **提交 (Submits)**: 通过 `SuiClient` 把签好名的交易发送到Sui网络。
//     -   **返回响应 (Returns Response)**: Sui网络执行完交易后会给一个回执 (`SuiTransactionBlockResponse`)，告诉你交易成功了还是失败了，以及具体的结果。
//
// 4.  **`Executor<TransactionData>` trait 实现**:
//     这部分让 `PublicTxExecutor` 能够符合一个叫做 `Executor` 的“规范”或“接口”（由 `burberry` 库定义）。
//     这意味着 `PublicTxExecutor` 可以被更通用的框架使用，只要那个框架知道如何使用符合 `Executor` 规范的东西。
//     -   **`name()`**: 返回这个执行器的名字，比如 "PublicTxExecutor"。
//     -   **`execute()`**: 这是 `Executor` 规范要求的方法。它接收一个 `TransactionData`，调用上面的 `execute_tx()` 来执行，并把执行结果记录到日志里。
//
// **Sui区块链和MEV相关的概念解释 (Sui Blockchain and MEV-related Concepts)**:
//
// -   **`TransactionData` (交易数据)**:
//     可以看作是一份详细的“交易订单”，里面写清楚了你要做什么操作。例如：
//     -   你要调用哪个智能合约的哪个函数？
//     -   传递给函数的参数是什么？
//     -   你愿意支付多少Gas（手续费）？
//     但此时，这份订单还没有你的“签名”，所以还不能正式提交。
//
// -   **`SuiKeyPair` (Sui密钥对)**:
//     每个Sui账户都有一对密钥：一个公钥和一个私钥。
//     -   **公钥 (Public Key)**：像你的银行账号，可以公开给别人，别人可以用它来给你转账或验证你的签名。
//     -   **私钥 (Private Key)**：像你的银行卡密码或签名章，必须严格保密。它用来对交易进行数字签名，证明这笔交易确实是由你授权的。
//     `SuiKeyPair` 结构体就保存了这两个密钥。
//
// -   **`IntentMessage` (意图消息)**:
//     在Sui中，直接对原始的 `TransactionData` 签名是不够安全的。为了防止一些潜在的攻击（比如重放攻击或交易内容被恶意篡改后签名），
//     Sui要求在签名之前，先把 `TransactionData` 用一个“信封”包起来，这个信封就是 `IntentMessage`。
//     信封上会写明“这是一笔Sui交易”（`Intent::sui_transaction()`），这样签名者就非常清楚自己签的是什么性质的东西。
//     这增加了签名的明确性和安全性。
//
// -   **BCS (Binary Canonical Serialization)**:
//     发音类似 "bikes"。这是一种将程序中的数据结构（比如 `IntentMessage`）转换成一串二进制字节（0和1组成的序列）的标准方法。
//     为什么要转换呢？因为计算机网络传输数据、或者把数据存到磁盘上时，都需要的是这种二进制字节流。
//     BCS确保同样的交易数据，无论在哪台计算机上，用哪种编程语言处理，只要遵循BCS规范，转换出来的二进制字节流都是完全一样的。
//     这对于保证交易的唯一性和可验证性非常重要。
//
// -   **Transaction Digest (交易摘要/哈希)**:
//     你可以把“摘要”或“哈希”想象成一个交易的“指纹”。
//     它是通过一个特殊的数学函数（哈希函数，比如SHA256）对序列化后的交易数据（通常是 `IntentMessage` 包裹后的BCS字节流）进行计算，得到的一个固定长度的、几乎唯一的字符串（通常是32字节的数字）。
//     -   **唯一性**：即使交易数据有任何一点微小的改动，计算出来的摘要也会完全不同。
//     -   **固定长度**：无论原始交易数据多大，摘要的长度总是固定的。
//     数字签名实际上并不是对整个庞大的交易数据进行签名，而是对这个小巧的“指纹”（摘要）进行签名。这样做更高效，也同样安全。
//
// -   **`GenericSignature` (通用签名)**:
//     Sui支持多种不同的数字签名算法（比如ED25519、ECDSA等）。`GenericSignature` 是一个通用的“容器”，
//     它可以存放用任何一种Sui支持的算法生成的签名。这使得Sui系统在处理签名时更灵活。
//
// -   **`Transaction` (已签名交易)**:
//     这个结构体代表了一笔最终可以被提交到Sui网络上去执行的交易。它包含了两个主要部分：
//     1.  原始的交易数据（`TransactionData`）。
//     2.  对该交易数据摘要的数字签名（`GenericSignature`）。
//     只有同时具备这两部分，Sui网络才会认为这是一笔合法的、可以被处理的交易。
//
// -   **Quorum Driver API (共识驱动API)**:
//     这是 `SuiClient` 提供的一套高级接口，用于和Sui网络的验证者们（Validators）进行交互，以提交交易并等待交易被确认。
//     “Quorum”（法定人数）指的是需要足够数量的验证者就交易的有效性达成一致。
//     这个API封装了与验证者通信、等待共识等复杂的底层逻辑。
//
// -   **`SuiTransactionBlockResponse` (Sui交易块响应)**:
//     当你的交易被Sui网络执行完毕后（无论成功还是失败），Sui节点会返回一个 `SuiTransactionBlockResponse` 对象。
//     这个对象里包含了关于这笔交易执行结果的各种详细信息，比如：
//     -   交易摘要（`digest`）：这笔交易的唯一指纹。
//     -   执行效果（`effects`）：交易对链上状态产生的具体影响（例如，哪些对象被创建/修改/删除，哪些事件被触发）。
//     -   相关事件（`events`）：交易执行过程中发出的所有事件日志。
//     -   对象变更（`object_changes`）：如果请求了，会包含交易前后对象状态的详细变化。
//     -   执行状态（`status_ok()`）：一个布尔值，简单明了地告诉你交易是否成功执行。
//     套利机器人会仔细分析这个响应，特别是 `effects`，来确认套利是否成功、利润是多少等。

// 引入所需的库和模块 (Import necessary libraries and modules)
use async_trait::async_trait; // `async_trait`宏使得在trait中定义异步方法成为可能
                              // The `async_trait` macro makes it possible to define asynchronous methods in traits.
use burberry::Executor; // 从外部 `burberry` crate 引入 `Executor` trait。
                        // `burberry` 可能是一个自定义的框架或库，用于定义可执行的任务或动作。
                        // Import the `Executor` trait from the external `burberry` crate.
                        // `burberry` might be a custom framework or library for defining executable tasks or actions.
use eyre::Result; // `eyre`库，用于更方便的错误处理
                  // The `eyre` library, for more convenient error handling.
use fastcrypto::hash::HashFunction; // `fastcrypto`库提供的哈希函数接口
                                    // Hashing function interface provided by the `fastcrypto` library.
use shared_crypto::intent::{Intent, IntentMessage}; // Sui共享的加密库，用于处理交易意图
                                                    // Sui's shared cryptography library, used for handling transaction intents.
use sui_json_rpc_types::{SuiTransactionBlockResponse, SuiTransactionBlockResponseOptions}; // Sui RPC相关的类型，如交易块响应和选项
                                                                                      // Sui RPC related types, such as transaction block response and options.
use sui_sdk::{SuiClient, SuiClientBuilder}; // Sui SDK，用于与Sui节点交互
                                           // Sui SDK, for interacting with a Sui node.
use sui_types::{
    crypto::{Signer, SuiKeyPair}, // Sui加密相关的类型，如签名者接口 (`Signer`) 和密钥对 (`SuiKeyPair`)
                                 // Sui cryptography related types, like the `Signer` trait and `SuiKeyPair`.
    signature::GenericSignature,    // 通用签名类型 (Generic signature type)
    transaction::{Transaction, TransactionData}, // Sui交易数据和已签名交易的类型
                                                // Types for Sui transaction data and signed transactions.
};
use tracing::info; // `tracing`库，用于日志记录
                   // The `tracing` library, for logging.

/// `PublicTxExecutor` 结构体
/// (PublicTxExecutor struct)
///
/// 负责签名并执行Sui交易。
/// (Responsible for signing and executing Sui transactions.)
pub struct PublicTxExecutor {
    sui: SuiClient,         // Sui RPC客户端，用于与Sui网络通信
                            // Sui RPC client, used to communicate with the Sui network.
    keypair: SuiKeyPair,    // 签名交易所需的密钥对 (包含公钥和私钥)
                            // Keypair (public and private key) required for signing transactions.
}

impl PublicTxExecutor {
    /// `new` 构造函数
    /// (new constructor)
    ///
    /// 创建一个新的 `PublicTxExecutor` 实例。
    /// (Creates a new instance of `PublicTxExecutor`.)
    ///
    /// 参数 (Parameters):
    /// - `rpc_url`: Sui RPC节点的URL字符串。
    ///              (String for the Sui RPC node URL.)
    /// - `keypair`: 用于签名交易的 `SuiKeyPair`。
    ///              (`SuiKeyPair` used for signing transactions.)
    ///
    /// 返回 (Returns):
    /// - `Result<Self>`: 成功则返回 `PublicTxExecutor` 实例。
    ///                   (Returns a `PublicTxExecutor` instance if successful.)
    pub async fn new(rpc_url: &str, keypair: SuiKeyPair) -> Result<Self> {
        // 使用提供的RPC URL构建一个新的SuiClient实例。
        // (Build a new SuiClient instance using the provided RPC URL.)
        let sui_client = SuiClientBuilder::default().build(rpc_url).await?;
        info!("PublicTxExecutor 初始化成功，连接到 RPC: {}", rpc_url); // 日志：执行器初始化成功
        Ok(Self { sui: sui_client, keypair })
    }

    /// `execute_tx` 方法
    /// (execute_tx method)
    ///
    /// 对给定的 `TransactionData`进行签名，并将其提交到Sui网络执行。
    /// (Signs the given `TransactionData` and submits it to the Sui network for execution.)
    ///
    /// 参数 (Parameters):
    /// - `tx_data`: 要执行的未签名交易数据 (`TransactionData`)。
    ///              (Unsigned transaction data (`TransactionData`) to be executed.)
    ///
    /// 返回 (Returns):
    /// - `Result<SuiTransactionBlockResponse>`: 包含交易执行结果的响应。
    ///                                       (Response containing the transaction execution result.)
    pub async fn execute_tx(&self, tx_data: TransactionData) -> Result<SuiTransactionBlockResponse> {
        // 步骤 1: 创建交易意图 (IntentMessage)。
        // 这是Sui为了安全和清晰性要求的一个步骤，确保签名者知道他们正在签的是一笔Sui交易。
        // (Step 1: Create a transaction intent (IntentMessage).)
        // (This is a step required by Sui for security and clarity, ensuring the signer knows they are signing a Sui transaction.)
        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data);
        // 拿到的 tx_data 会被 intent_msg 包裹，intent_msg.value 就是原始的 tx_data

        // 步骤 2: 将意图消息序列化为BCS字节流。
        // BCS是Sui标准的二进制序列化格式。
        // (Step 2: Serialize the intent message into a BCS byte stream.)
        // (BCS is Sui's standard binary serialization format.)
        let raw_tx_bytes = bcs::to_bytes(&intent_msg)?;

        // 步骤 3: 计算序列化后的交易数据的摘要 (哈希值)。
        // 签名是针对这个摘要进行的。
        // (Step 3: Calculate the digest (hash value) of the serialized transaction data.)
        // (The signature is performed on this digest.)
        let digest_to_sign = {
            let mut hasher = sui_types::crypto::DefaultHash::default(); // 获取Sui默认的哈希算法实现 (Get Sui's default hash algorithm implementation)
            hasher.update(raw_tx_bytes.clone()); // 更新哈希器的状态 (注意：这里克隆了raw_tx_bytes，如果数据量大可能考虑优化)
                                                // (Update the hasher's state. Note: `raw_tx_bytes` is cloned here; consider optimization if data is large,
                                                //  but it's usually fine for transaction data.)
            hasher.finalize().digest // 完成哈希计算并获取32字节的摘要结果 (Complete the hash calculation and get the 32-byte digest result)
        };
        info!("交易摘要待签名 (Digest to sign): {:?}", digest_to_sign); // 日志：记录待签名的摘要

        // 步骤 4: 使用私钥对摘要进行签名。
        // `self.keypair` (SuiKeyPair) 实现了 `Signer` trait，提供了 `sign` 方法。
        // (Step 4: Sign the digest using the private key.)
        // (`self.keypair` (SuiKeyPair) implements the `Signer` trait, which provides the `sign` method.)
        let signature = self.keypair.sign(&digest_to_sign);
        info!("交易已签名 (Transaction signed)."); // 日志：交易已签名

        // 步骤 5: 创建已签名的交易 (`Transaction`)。
        // 它将原始的交易数据 (`intent_msg.value`，即 `tx_data`) 和签名组合在一起。
        // `GenericSignature::Signature(sig)` 将具体的签名包装成Sui通用的签名类型。
        // (Step 5: Create a signed transaction (`Transaction`).)
        // (It combines the original transaction data (`intent_msg.value`, which is `tx_data`) and the signature.)
        // (`GenericSignature::Signature(sig)` wraps the specific signature into Sui's generic signature type.)
        let signed_transaction = Transaction::from_generic_sig_data(intent_msg.value, vec![GenericSignature::Signature(signature)]);

        // 步骤 6: 设置交易响应选项。
        // `SuiTransactionBlockResponseOptions::default()` 通常请求包含交易效果 (effects) 等基本信息。
        // 可以根据需要配置更详细的选项，例如请求返回原始输入对象、对象变更等。
        // (Step 6: Set transaction response options.)
        // (`SuiTransactionBlockResponseOptions::default()` usually requests basic information including transaction effects.)
        // (More detailed options can be configured as needed, e.g., requesting original input objects, object changes, etc.)
        let response_options = SuiTransactionBlockResponseOptions::default().with_effects().with_events(); // 请求效果和事件
        info!("交易响应选项已设置 (Response options set): {:?}", response_options);

        // 步骤 7: 通过Sui客户端的 `quorum_driver_api` 提交已签名的交易到链上执行。
        // `execute_transaction_block` 方法会等待Sui网络就该交易达成共识（或超时/出错）。
        // `None` 作为第三个参数表示请求执行模式为默认 (通常是等待最终确认 `WaitForEffectsCert`)。
        // (Step 7: Submit the signed transaction to the chain for execution via the Sui client's `quorum_driver_api`.)
        // (The `execute_transaction_block` method waits for the Sui network to reach consensus on the transaction (or timeout/error).)
        // (`None` as the third argument indicates the default request execution mode (usually `WaitForEffectsCert`).)
        info!("准备执行交易 (digest: {:?}) (Preparing to execute transaction)", signed_transaction.digest());
        let transaction_response = self
            .sui
            .quorum_driver_api()
            .execute_transaction_block(signed_transaction, response_options, None)
            .await?;
        info!("交易执行完毕，收到响应 (digest: {}) (Transaction execution finished, response received)", transaction_response.digest.base58_encode());

        // 返回交易执行的响应。
        // (Return the transaction execution response.)
        Ok(transaction_response)
    }
}

/// 为 `PublicTxExecutor` 实现 `burberry::Executor<TransactionData>` trait。
/// 这使得 `PublicTxExecutor` 可以被用在 `burberry` 框架中作为一种特定类型动作（这里是`TransactionData`）的执行器。
/// (Implement the `burberry::Executor<TransactionData>` trait for `PublicTxExecutor`.)
/// (This allows `PublicTxExecutor` to be used within the `burberry` framework as an executor for a specific type of action (`TransactionData` here).)
#[async_trait]
impl Executor<TransactionData> for PublicTxExecutor {
    /// `name` 方法 (来自 `Executor` trait)
    /// (`name` method (from the `Executor` trait))
    ///
    /// 返回执行器的名称。
    /// (Returns the name of the executor.)
    fn name(&self) -> &str {
        "PublicTxExecutor"
    }

    /// `execute` 方法 (来自 `Executor` trait)
    /// (`execute` method (from the `Executor` trait))
    ///
    /// 执行一个给定的动作 (`action`，这里是 `TransactionData`)。
    /// (Executes a given action (`action`, which is `TransactionData` here).)
    ///
    /// 参数 (Parameters):
    /// - `action`: 要执行的 `TransactionData`。
    ///             (The `TransactionData` to be executed.)
    ///
    /// 返回 (Returns):
    /// - `Result<()>`: 如果执行（包括提交和记录日志）成功，则返回Ok(())，否则返回错误。
    ///                 (Returns `Ok(())` if execution (including submission and logging) is successful, otherwise returns an error.)
    async fn execute(&self, action: TransactionData) -> Result<()> {
        let tx_digest_for_log = action.digest(); // 获取原始 TransactionData 的摘要用于日志
        info!("PublicTxExecutor 开始执行交易 (digest from TransactionData: {:?})", tx_digest_for_log);
        // 调用内部的 `execute_tx` 方法实际执行交易。
        // (Call the internal `execute_tx` method to actually execute the transaction.)
        let response = self.execute_tx(action).await?;
        // 获取交易摘要的Base58编码字符串，用于日志记录。
        // (Get the Base58 encoded string of the transaction digest for logging.)
        let digest_str = response.digest.base58_encode();

        // 记录日志：包含交易摘要和交易是否成功 (`status_ok()`)。
        // (Log: includes the transaction digest and whether the transaction was successful (`status_ok()`)).
        info!(digest = %digest_str, status_ok = ?response.status_ok(), "已执行交易 (Executed tx via Executor trait)");
        if !response.status_ok() {
            // 如果交易执行失败，记录更详细的错误信息
            error!("交易执行失败 (digest: {}). 效果: {:?}, 事件: {:?}", digest_str, response.effects, response.errors);
            // 可以考虑将 response.errors 或 response.effects 中的错误信息包装到 eyre::Report 中返回
            // 例如: return Err(eyre::eyre!("交易执行失败: {:?}", response.errors));
        }
        Ok(())
    }
}

[end of bin/arb/src/executor.rs]
