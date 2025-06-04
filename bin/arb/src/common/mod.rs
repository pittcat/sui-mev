// 该文件是 `common` 模块的入口文件 (在Rust中，模块的根文件通常命名为 `mod.rs`，或者如果模块是一个目录，则为该目录下的 `mod.rs`)。
// `common` 模块（意为“通用的”、“公共的”）通常包含项目中多个其他模块都会共同用到的一些工具函数、数据结构或子模块。
// 将这些通用功能组织在 `common` 模块中，有助于提高代码的复用性，避免重复编写相同的逻辑，并使项目结构更加清晰和模块化。
//
// **文件概览 (File Overview)**:
// 这个 `mod.rs` 文件是 `common` 模块的“大门”或“目录”。
// `common` 这个名字暗示了它里面放的东西是项目中很多其他部分都会用到的“公共工具”或“共享资源”。
// (This `mod.rs` file is the "main gate" or "directory" for the `common` module.)
// (The name `common` suggests that it contains "common tools" or "shared resources" that many other parts of the project will use.)
//
// 这个文件主要做了两件事 (This file primarily does two things):
//
// 1.  **声明了两个公共的子模块 (Declares two public submodules)**: `notification` 和 `search`。
//     (It declares two public submodules: `notification` and `search`.)
//     -   `pub mod notification;`:
//         `pub` 关键字表示这个子模块是公开的，意味着 `common` 模块外部的代码（例如项目中的其他模块如 `arb` 或 `collector`）
//         可以访问和使用 `notification` 模块中定义的公共项（函数、结构体等）。
//         (The `pub` keyword means this submodule is public, implying that code outside the `common` module (e.g., other modules in the project like `arb` or `collector`)
//          can access and use public items (functions, structs, etc.) defined in the `notification` module.)
//         这行代码会告诉Rust编译器去查找名为 `notification.rs` 的文件（在 `common` 目录下）或者名为 `notification/mod.rs` 的文件（如果 `notification` 是一个子目录），
//         并将其内容作为 `common::notification` 模块加载进来。
//         (This line tells the Rust compiler to look for a file named `notification.rs` (in the `common` directory) or a file named `notification/mod.rs` (if `notification` is a subdirectory),
//          and load its content as the `common::notification` module.)
//         顾名思义，`notification` 模块可能包含了发送各种通知（例如，通过Telegram消息、电子邮件、短信等方式报告套利成功或程序错误）的功能。
//         (As the name suggests, the `notification` module likely contains functionality for sending various notifications (e.g., reporting arbitrage success or program errors via Telegram messages, email, SMS, etc.).)
//
//     -   `pub mod search;`:
//         同样，`pub` 表示 `search` 子模块是公开的。
//         (Similarly, `pub` indicates that the `search` submodule is public.)
//         编译器会查找 `search.rs` 或 `search/mod.rs` 文件。
//         (The compiler will look for a `search.rs` or `search/mod.rs` file.)
//         `search` 模块可能包含了一些通用的搜索算法或与搜索操作相关的工具函数。
//         (The `search` module might contain some general-purpose search algorithms or utility functions related to search operations.)
//         例如，在 `arb.rs` 文件中我们看到了黄金分割搜索算法 (`golden_section_search_maximize`) 的使用，
//         该算法的定义很可能就放在这个 `search` 模块或其更深层的子模块中。
//         (For example, in the `arb.rs` file, we saw the use of the Golden Section Search algorithm (`golden_section_search_maximize`);
//          the definition of this algorithm is likely located in this `search` module or its deeper submodules.)
//
// 2.  **定义了一个异步的公共函数 `get_latest_epoch` (Defines an asynchronous public function `get_latest_epoch`)**。
//     这个函数的作用是从Sui区块链网络获取最新的“纪元”（Epoch）信息。
//     (The purpose of this function is to retrieve the latest "Epoch" information from the Sui blockchain network.)
//     纪元信息对于链上操作（如套利）非常重要，因为它通常包含了当前网络的动态参数，比如当前的gas价格。
//     (Epoch information is very important for on-chain operations (like arbitrage) because it usually contains dynamic parameters of the current network, such as the current gas price.)
//     知道了gas价格，才能更准确地估算交易成本和潜在利润。
//     (Knowing the gas price allows for more accurate estimation of transaction costs and potential profits.)

// 声明公共子模块 `notification`。
// (Declare public submodule `notification`.)
// 这使得 `crate::common::notification::...` 成为有效的路径，可以访问该模块下的公开内容。
// (This makes `crate::common::notification::...` a valid path to access public content within this module.)
pub mod notification;

// 声明公共子模块 `search`。
// (Declare public submodule `search`.)
// 使得 `crate::common::search::...` 成为有效的路径。
// (Makes `crate::common::search::...` a valid path.)
pub mod search;

// 引入所需的库和类型定义。
// (Import necessary libraries and type definitions.)
use eyre::Result; // 从 `eyre` 库引入 `Result` 类型。`eyre` 是一个用于增强错误处理体验的库，
                  // 它提供的 `Result` 通常是 `std::result::Result<T, eyre::Report>` 的别名，
                  // `eyre::Report` 能够包含更丰富的错误上下文信息。
                  // (Import the `Result` type from the `eyre` library. `eyre` is a library for an enhanced error handling experience.
                  //  The `Result` it provides is typically an alias for `std::result::Result<T, eyre::Report>`,
                  //  where `eyre::Report` can contain richer error context information.)
use simulator::SimEpoch; // 从 `simulator` crate (包/库) 中引入 `SimEpoch` 结构体。
                         // `SimEpoch` 很可能是一个简化版的、或者专门为交易模拟器设计的Sui纪元信息表示。
                         // 它可能只包含了模拟套利计算时所必需的关键字段，如gas价格。
                         // (Import the `SimEpoch` struct from the `simulator` crate (package/library).
                         //  `SimEpoch` is likely a simplified version or a representation of Sui epoch information specifically designed for the transaction simulator.
                         //  It might only contain key fields necessary for simulating arbitrage calculations, such as the gas price.)
use sui_sdk::SuiClient; // 从 `sui_sdk` crate 中引入 `SuiClient` 结构体。
                        // `SuiClient` 是Sui官方软件开发工具包（SDK）提供的核心组件，
                        // 它封装了与Sui区块链RPC（远程过程调用）节点进行通信的逻辑，
                        // 使得程序可以查询链上状态、发送交易、订阅事件等。
                        // (Import the `SuiClient` struct from the `sui_sdk` crate.
                        //  `SuiClient` is a core component provided by the Sui official Software Development Kit (SDK).
                        //  It encapsulates the logic for communicating with a Sui blockchain RPC (Remote Procedure Call) node,
                        //  allowing the program to query on-chain state, send transactions, subscribe to events, etc.)

/// `get_latest_epoch` 是一个异步公共函数，其主要功能是：
/// ( `get_latest_epoch` is an asynchronous public function whose main functionalities are:)
/// 1. 连接到Sui网络（通过传入的 `SuiClient`）。
///    (Connect to the Sui network (via the passed `SuiClient`).)
/// 2. 查询当前Sui网络最新的“系统状态”（Sui System State）。
///    (Query the latest "Sui System State" of the current Sui network.)
/// 3. 从系统状态中提取出纪元相关的信息。
///    (Extract epoch-related information from the system state.)
/// 4. 将这些信息转换为模拟器或应用程序内部使用的 `SimEpoch` 格式。
///    (Convert this information into the `SimEpoch` format used internally by the simulator or application.)
///
/// **纪元 (Epoch) 在Sui网络中的意义 (Significance of Epoch in the Sui Network)**:
/// Sui网络像其他一些PoS（Proof-of-Stake，权益证明）区块链一样，其运行是以“纪元”为单位周期性进行的。
/// (The Sui network, like some other PoS (Proof-of-Stake) blockchains, operates in periodic units called "epochs".)
/// 一个纪元通常持续一段固定的时间（例如24小时）。在每个纪元结束和新纪元开始时，会发生一些重要的网络事件，例如：
/// (An epoch usually lasts for a fixed period (e.g., 24 hours). At the end of each epoch and the beginning of a new one, some important network events occur, such as:)
/// -   验证者集合的变更（新的验证者加入，不合格的验证者退出）。
///     (Changes in the validator set (new validators join, unqualified ones exit).)
/// -   重新计算和调整网络交易的gas价格基准。
///     (Recalculation and adjustment of the network's base gas price for transactions.)
/// -   分配和发放质押SUI代币的奖励。
///     (Distribution and issuance of staking rewards for SUI tokens.)
/// 对于套利机器人或任何需要与Sui链进行精确交互的程序来说，获取当前纪元的最新信息至关重要，
/// 特别是当前的 `reference_gas_price` (参考gas价格)，因为它直接影响交易成本的估算。
/// (For an arbitrage bot or any program that needs to interact precisely with the Sui chain, obtaining the latest information of the current epoch is crucial,
/// especially the current `reference_gas_price`, as it directly affects the estimation of transaction costs.)
///
/// **参数 (Parameters)**:
/// - `sui`: 一个对 `SuiClient` 实例的不可变引用 (`&SuiClient`)。
///          (`sui`: An immutable reference (`&SuiClient`) to a `SuiClient` instance.)
///          `SuiClient` 提供了与Sui RPC节点通信的方法。通过传递引用，
///          调用者仍然保留 `SuiClient` 的所有权，可以在调用此函数后继续使用它。
///          (`SuiClient` provides methods for communicating with a Sui RPC node. By passing a reference,
///           the caller retains ownership of the `SuiClient` and can continue to use it after calling this function.)
///
/// **返回 (Returns)**:
/// - `Result<SimEpoch>`: 这是一个异步操作的结果，其类型是 `eyre::Result`。
///                       (This is the result of an asynchronous operation, typed as `eyre::Result`.)
///   -   如果函数成功执行（即成功从Sui节点获取信息并完成转换），则返回 `Ok(SimEpoch)`，
///       其中 `SimEpoch` 实例包含了最新的纪元数据。
///       (If the function executes successfully (i.e., successfully retrieves information from the Sui node and completes the conversion), it returns `Ok(SimEpoch)`,
///        where the `SimEpoch` instance contains the latest epoch data.)
///   -   如果在执行过程中发生任何错误（例如，网络连接问题、RPC节点返回错误、数据反序列化失败等），
///       则返回 `Err(eyre::Report)`，其中 `eyre::Report` 对象会包含详细的错误信息和上下文，
///       便于调试和问题定位。
///       (If any error occurs during execution (e.g., network connection issues, errors returned by the RPC node, data deserialization failure, etc.),
///        it returns `Err(eyre::Report)`, where the `eyre::Report` object will contain detailed error information and context,
///        facilitating debugging and problem localization.)
pub async fn get_latest_epoch(sui: &SuiClient) -> Result<SimEpoch> {
    // 步骤1: 调用 `sui.governance_api()` 来获取一个可以访问Sui网络“治理”相关RPC接口的客户端。
    //         Sui的系统状态信息（包含了纪元数据）通常被认为是治理层面的一部分。
    //         然后，调用 `.get_latest_sui_system_state()` 方法。这是一个异步方法（返回一个Future），
    //         所以我们需要使用 `.await` 来等待它完成并返回结果。
    //         这个方法会向Sui RPC节点请求最新的 `SuiSystemState` 对象。
    //         `?` 操作符是Rust中用于错误传播的语法糖。如果 `get_latest_sui_system_state().await`
    //         的结果是一个 `Err(...)`，那么 `?` 会使 `get_latest_epoch` 函数立即返回这个错误。
    //         如果结果是 `Ok(value)`，那么 `?` 会提取出 `value` 并将其赋值给 `sys_state`。
    // (Step 1: Call `sui.governance_api()` to get a client that can access Sui network "governance" related RPC interfaces.)
    // (Sui's system state information (which includes epoch data) is usually considered part of the governance layer.)
    // (Then, call the `.get_latest_sui_system_state()` method. This is an asynchronous method (returns a Future),
    //  so we need to use `.await` to wait for it to complete and return the result.)
    // (This method requests the latest `SuiSystemState` object from the Sui RPC node.)
    // (The `?` operator is syntactic sugar in Rust for error propagation. If the result of `get_latest_sui_system_state().await`
    //  is an `Err(...)`, then `?` will cause the `get_latest_epoch` function to immediately return this error.)
    // (If the result is `Ok(value)`, then `?` will extract `value` and assign it to `sys_state`.)
    let sys_state = sui.governance_api().get_latest_sui_system_state().await?;

    // 步骤2: 将从Sui SDK获取到的原始 `SuiSystemState` 类型的 `sys_state` 对象，
    //         转换为我们程序内部（特别是模拟器部分）使用的 `SimEpoch` 类型。
    //         `SimEpoch::from(sys_state)` 这行代码能够工作的前提是 `SimEpoch` 类型已经实现了
    //         `From<SuiSystemState>` 这个trait (或者 `Into<SimEpoch>` for `SuiSystemState`)。
    //         这个trait定义了如何从一个 `SuiSystemState` 对象构造出一个 `SimEpoch` 对象。
    //         这种转换通常是为了从复杂的原始数据中提取出应用程序关心的关键信息，形成一个更简洁或更适合内部使用的数据结构。
    //         例如，`SimEpoch` 可能只包含纪元号和gas价格。
    // (Step 2: Convert the raw `SuiSystemState` type `sys_state` object obtained from the Sui SDK
    //  into the `SimEpoch` type used internally by our program (especially the simulator part).)
    // (The line `SimEpoch::from(sys_state)` works on the premise that the `SimEpoch` type has implemented
    //  the `From<SuiSystemState>` trait (or `Into<SimEpoch>` for `SuiSystemState`).)
    // (This trait defines how to construct a `SimEpoch` object from a `SuiSystemState` object.)
    // (This conversion is usually to extract key information that the application cares about from complex raw data,
    //  forming a more concise or more suitable data structure for internal use.)
    // (For example, `SimEpoch` might only contain the epoch number and gas price.)
    Ok(SimEpoch::from(sys_state)) // 将转换后的 `SimEpoch` 实例包装在 `Ok` 中并返回。
                                  // (Wrap the converted `SimEpoch` instance in `Ok` and return it.)
}

[end of bin/arb/src/common/mod.rs]
