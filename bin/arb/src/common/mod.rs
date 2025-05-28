// 该文件是 `common` 模块的入口文件 (通常命名为 `mod.rs` 在Rust中)。
// `common` 模块通常包含项目中多个其他模块都会用到的通用工具、函数或子模块。
// 这种组织方式有助于代码的复用和模块化。
//
// 文件概览:
// 1. 声明了两个子模块: `notification` 和 `search`。
//    - `pub mod notification;`: 表示 `notification` 是一个公共子模块，其内容可以被外部访问。
//      这个模块可能包含发送通知 (例如通过Telegram, Email等) 的功能。
//    - `pub mod search;`: 表示 `search` 也是一个公共子模块。
//      这个模块可能包含通用的搜索算法或与搜索相关的工具函数，
//      例如在 `arb.rs` 中看到的黄金分割搜索 (`golden_section_search_maximize`) 可能就定义在这里或其子模块。
// 2. 定义了一个异步公共函数 `get_latest_epoch`。
//    这个函数用于从Sui区块链获取最新的纪元 (Epoch) 信息。

// 声明公共子模块 `notification`
// 这行代码会告诉Rust编译器在同级目录下或 `common` 目录下寻找 `notification.rs` 或 `notification/mod.rs` 文件，
// 并将其内容作为 `common::notification` 模块加载。
pub mod notification;

// 声明公共子模块 `search`
// 类似地，这会加载 `search.rs` 或 `search/mod.rs`。
pub mod search;

// 引入所需的库和类型
use eyre::Result; // eyre库，用于更方便的错误处理。`Result` 是一个通用的结果类型。
use simulator::SimEpoch; // 从 `simulator` crate 中引入 `SimEpoch` 结构体。
                         // `SimEpoch` 可能是一个简化的或模拟器专用的Sui纪元信息表示。
use sui_sdk::SuiClient; // 从 `sui_sdk` crate 中引入 `SuiClient`。
                        // `SuiClient` 是与Sui区块链RPC节点交互的主要客户端结构。

/// `get_latest_epoch` 是一个异步函数，用于从Sui网络获取最新的纪元信息，
/// 并将其转换为模拟器可以使用的 `SimEpoch` 格式。
///
/// 纪元 (Epoch) 在Sui网络中是一个重要的时间单位，它与验证者集的变更、质押奖励的分配等系统级操作相关。
/// 对于套利机器人或链上交互程序，了解当前的纪元信息（例如当前的gas价格）可能很重要。
///
/// 参数:
/// - `sui`: 一个对 `SuiClient` 的引用 (`&SuiClient`)。`SuiClient` 实例用于与Sui节点通信。
///          使用引用避免了所有权的转移，允许调用者之后继续使用这个 `SuiClient`。
///
/// 返回:
/// - `Result<SimEpoch>`:
///   - 如果成功获取并转换了纪元信息，则返回 `Ok(SimEpoch)`，其中包含最新的纪元数据。
///   - 如果在过程中发生任何错误 (例如网络问题、RPC调用失败、反序列化错误等)，则返回 `Err(...)`，
///     其中包含具体的错误信息 (由 `eyre` 库包装)。
pub async fn get_latest_epoch(sui: &SuiClient) -> Result<SimEpoch> {
    // 步骤1: 调用Sui SDK的治理API (governance_api) 来获取最新的Sui系统状态 (Sui System State)。
    // `get_latest_sui_system_state()` 是一个异步方法，因此需要 `.await`。
    // `sui.governance_api()` 返回一个可以调用治理相关RPC方法的API客户端。
    // `?` 操作符用于错误传播：如果 `get_latest_sui_system_state().await` 返回一个 `Err`，
    // 那么整个 `get_latest_epoch` 函数会立即返回这个错误。
    let sys_state = sui.governance_api().get_latest_sui_system_state().await?;

    // 步骤2: 将从Sui SDK获取的 `SuiSystemState` (这里是 `sys_state`) 转换为 `SimEpoch` 类型。
    // `SimEpoch::from(sys_state)` 表示 `SimEpoch` 类型实现了 `From<SuiSystemState>` trait，
    // 允许直接进行这种类型转换。这通常用于将RPC返回的详细类型转换为模拟器或应用程序内部使用的更简洁的类型。
    Ok(SimEpoch::from(sys_state)) // 返回转换后的 `SimEpoch`，包装在 `Ok` 中。
}
