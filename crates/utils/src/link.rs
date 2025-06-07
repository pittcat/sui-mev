// 该文件 `link.rs` (位于 `utils` crate中) 提供了一系列辅助函数，
// 用于生成指向 SuiScan (一个Sui区块链浏览器) 上特定资源的Markdown格式的链接。
// 这些函数使得在日志、通知（如Telegram消息）或其他文本文档中，
// 能够方便地创建可点击的链接，引导用户到SuiScan查看相关的链上信息。
//
// **文件概览 (File Overview)**:
// 这个文件是 `utils` 工具库中的一个“链接生成器”。
// SuiScan (suiscan.xyz) 是一个网站，你可以在上面查看Sui区块链上的各种信息，比如交易详情、对象状态、账户活动等。
// 这个文件里的函数就是帮你快速生成指向SuiScan特定页面的Markdown链接。
// Markdown是一种轻量级的标记语言，常用于格式化文本，生成的链接在支持Markdown的平台（如Telegram、GitHub）上是可点击的。
//
// **核心功能和组件 (Core Functionalities and Components)**:
//
// 1.  **`SCAN_URL` 常量**:
//     -   定义了SuiScan主网浏览器URL的基础部分 (`https://suiscan.xyz/mainnet`)。
//     -   所有生成的链接都会基于这个URL。如果将来需要指向测试网或其他浏览器，只需修改这个常量。
//
// 2.  **链接生成函数**:
//     每个函数都遵循类似的模式：
//     -   接收一个或多个与Sui链上资源相关的标识符（如交易摘要、对象ID、地址等）和一个可选的 `tag` (标签)。
//     -   使用 `format!` 宏构建一个Markdown链接字符串，格式为 `"[标签文本](URL)"`。
//     -   **标签文本 (`tag_str` 或 `tag`)**:
//         -   如果调用时提供了 `tag: Some(String)`，则使用提供的字符串作为链接的显示文本。
//         -   如果 `tag` 是 `None`，则会使用资源标识符本身（例如交易摘要、对象ID的字符串形式）作为显示文本。
//     -   **URL**: 根据 `SCAN_URL` 和特定资源的路径规则（如 `/tx/`, `/object/`）以及传入的标识符来构建。
//
//     **具体的链接生成函数包括**:
//     -   **`tx(digest: &TransactionDigest, tag: Option<String>) -> String`**:
//         生成指向特定交易详情页面的链接。
//         URL格式: `https://suiscan.xyz/mainnet/tx/{TransactionDigest}`
//
//     -   **`object(object_id: ObjectID, tag: Option<String>) -> String`**:
//         生成指向特定对象详情页面的链接。
//         URL格式: `https://suiscan.xyz/mainnet/object/{ObjectID}`
//
//     -   **`account(address: &SuiAddress, tag: Option<String>) -> String`**:
//         生成指向特定账户资产组合页面的链接。
//         URL格式: `https://suiscan.xyz/mainnet/account/{SuiAddress}/portfolio`
//
//     -   **`coin(coin_type: &str, tag: Option<String>) -> String`**:
//         生成指向特定代币类型相关交易列表页面的链接。
//         URL格式: `https://suiscan.xyz/mainnet/coin/{CoinTypeString}/txs`
//
//     -   **`checkpoint(digest: &Digest, number: SequenceNumber) -> String`**:
//         生成指向特定检查点详情页面的链接。
//         `Digest` 通常指检查点的摘要，`SequenceNumber` 是检查点的序列号（高度）。
//         链接文本会显示检查点的序列号。
//         URL格式: `https://suiscan.xyz/mainnet/checkpoint/{Digest}`
//
// **用途 (Purpose)**:
// -   **日志增强**: 在日志中输出这些链接，方便开发者快速跳转到SuiScan查看相关的链上细节。
// -   **通知消息**: 在通过Telegram或其他方式发送套利成功或失败的通知时，可以包含这些链接，
//     让用户能够方便地核实交易或查看涉及的对象。
// -   **自动化报告**: 在生成自动化报告或分析结果时，嵌入这些链接可以提高报告的交互性和可用性。
//
// **Markdown链接格式**:
// Markdown中的链接语法是 `[显示文本](URL)`。
// 例如，`[点击查看交易](https://suiscan.xyz/mainnet/tx/...)` 会显示为 "点击查看交易"，点击后会跳转到指定的URL。

// 引入Sui核心类型库中的相关类型。
use sui_types::{
    base_types::{ObjectID, SequenceNumber, SuiAddress}, // ObjectID, 对象版本号, Sui地址
    digests::{Digest, TransactionDigest},               // 通用摘要 (用于检查点), 交易摘要
};

/// `SCAN_URL` 常量
///
/// 定义了SuiScan主网区块链浏览器的基础URL。
/// 所有生成的链接都将以此URL为前缀。
const SCAN_URL: &str = "https://suiscan.xyz/mainnet";

/// `tx` 函数 (生成交易链接)
///
/// 根据交易摘要 (`TransactionDigest`) 生成一个指向SuiScan上该交易详情页面的Markdown链接。
///
/// 参数:
/// - `digest`: 要链接到的交易的 `&TransactionDigest`。
/// - `tag`: (可选) `Option<String>`，用作链接的显示文本。
///          如果为 `None`，则使用交易摘要本身作为显示文本。
///
/// 返回:
/// - `String`: Markdown格式的链接字符串。
///   例如: `"[WQ34...XgyE](https://suiscan.xyz/mainnet/tx/WQ346mGc8sLjtcBPBfJNvTxCWar7U7Fsow9rTkmXgyE)"`
///   或者如果提供了tag: `"[我的交易](https://suiscan.xyz/mainnet/tx/WQ346mGc8sLjtcBPBfJNvTxCWar7U7Fsow9rTkmXgyE)"`
pub fn tx(digest: &TransactionDigest, tag: Option<String>) -> String {
    format!(
        // Markdown链接格式: [显示文本](URL)
        "[{tag_str}]({prefix}/tx/{digest})",
        // 如果提供了tag，则使用它；否则，使用交易摘要的字符串表示作为显示文本。
        tag_str = tag.unwrap_or_else(|| format!("{}", digest)),
        prefix = SCAN_URL, // 基础URL
        digest = digest,   // 交易摘要 (会自动调用其Display实现)
    )
}

/// `object` 函数 (生成对象链接)
///
/// 根据对象ID (`ObjectID`) 生成一个指向SuiScan上该对象详情页面的Markdown链接。
///
/// 参数:
/// - `object_id`: 要链接到的对象的 `ObjectID`。
/// - `tag`: (可选) `Option<String>`，用作链接的显示文本。
///          如果为 `None`，则使用对象ID本身作为显示文本。
///
/// 返回:
/// - `String`: Markdown格式的链接字符串。
///   例如: `"[0xb8d...0105](https://suiscan.xyz/mainnet/object/0xb8d7d9e66a60c239e7a60110efcf8de6c705580ed924d0dde141f4a0e2c90105)"`
pub fn object(object_id: ObjectID, tag: Option<String>) -> String {
    format!(
        "[{tag_str}]({prefix}/object/{object_id})",
        tag_str = tag.unwrap_or_else(|| format!("{}", object_id)), // 如果tag为None，则使用ObjectID作为文本
        prefix = SCAN_URL,
        object_id = object_id, // 对象ID
    )
}

/// `account` 函数 (生成账户链接)
///
/// 根据Sui地址 (`SuiAddress`) 生成一个指向SuiScan上该账户资产组合 (`portfolio`) 页面的Markdown链接。
///
/// 参数:
/// - `address`: 要链接到的账户的 `&SuiAddress`。
/// - `tag`: (可选) `Option<String>`，用作链接的显示文本。
///          如果为 `None`，则使用账户地址本身作为显示文本。
///
/// 返回:
/// - `String`: Markdown格式的链接字符串。
///   例如: `"[0xac5b...c33c](https://suiscan.xyz/mainnet/account/0xac5bceec1b789ff840d7d4e6ce4ce61c90d190a7f8c4f4ddf0bff6ee2413c33c/portfolio)"`
pub fn account(address: &SuiAddress, tag: Option<String>) -> String {
    format!(
        "[{tag}]({prefix}/account/{address}/portfolio)", // 注意URL路径中包含了 "/portfolio"
        tag = tag.unwrap_or_else(|| format!("{}", address)), // 如果tag为None，则使用SuiAddress作为文本
        address = address, // 账户地址
        prefix = SCAN_URL,
    )
}

/// `coin` 函数 (生成代币类型链接)
///
/// 根据代币的完整类型字符串 (`coin_type`) 生成一个指向SuiScan上该代币类型相关交易列表页面的Markdown链接。
///
/// 参数:
/// - `coin_type`: 代币的类型字符串 (例如 "0x2::sui::SUI" 或 "0x...::ocean::OCEAN")。
/// - `tag`: (可选) `Option<String>`，用作链接的显示文本。
///          如果为 `None`，则使用代币类型字符串本身作为显示文本。
///
/// 返回:
/// - `String`: Markdown格式的链接字符串。
///   例如: `"[0xa88...::ocean::OCEAN](https://suiscan.xyz/mainnet/coin/0xa8816d3a6e3136e86bc2873b1f94a15cadc8af2703c075f2d546c2ae367f4df9::ocean::OCEAN/txs)"`
pub fn coin(coin_type: &str, tag: Option<String>) -> String {
    format!(
        "[{tag}]({prefix}/coin/{coin_type}/txs)", // 注意URL路径中包含了 "/txs"
        tag = tag.unwrap_or_else(|| coin_type.to_string()), // 如果tag为None，则使用coin_type作为文本
        coin_type = coin_type, // 代币类型字符串
        prefix = SCAN_URL,
    )
}

/// `checkpoint` 函数 (生成检查点链接)
///
/// 根据检查点的摘要 (`Digest`) 和序列号 (`SequenceNumber`) 生成一个指向SuiScan上该检查点详情页面的Markdown链接。
/// 链接的显示文本将是检查点的序列号。
///
/// 参数:
/// - `digest`: 检查点的 `&Digest`。
/// - `number`: 检查点的 `SequenceNumber` (即检查点高度)。
///
/// 返回:
/// - `String`: Markdown格式的链接字符串。
///   例如: `"[12345](https://suiscan.xyz/mainnet/checkpoint/AYWtSh7XWdBiRaEyh4oq3pxoaPmLkPJ6U1LBdwHuEXT)"`
pub fn checkpoint(digest: &Digest, number: SequenceNumber) -> String {
    format!(
        "[{number}]({prefix}/checkpoint/{digest})", // 显示文本是检查点编号
        number = number,   // 检查点序列号
        digest = digest,   // 检查点摘要
        prefix = SCAN_URL,
    )
}

[end of crates/utils/src/link.rs]
