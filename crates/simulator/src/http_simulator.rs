// 该文件 `http_simulator.rs` (位于 `simulator` crate中) 定义了 `HttpSimulator` 结构体。
// `HttpSimulator` 是一种通过Sui JSON-RPC API与远程Sui节点交互来进行交易模拟的模拟器。
// 与 `DBSimulator` 不同，它不直接访问本地数据库，而是依赖网络请求来获取链上状态和执行模拟。
// 文件顶部的注释提到了 `HttpSimulator` 已被弃用 (deprecated)，这可能意味着它主要用于测试或作为备选方案，
// 而 `DBSimulator` 因其性能优势成为首选。
//
// **文件概览 (File Overview)**:
// 这个文件定义了一个叫做 `HttpSimulator` 的“远程模拟器”。
// 它的工作方式是：当需要模拟一笔交易时，它会通过互联网（HTTP或IPC）连接到一个正在运行的Sui节点，
// 把交易数据发送给那个节点，让节点帮忙模拟，然后把模拟结果拿回来。
//
// **核心组件和概念 (Core Components and Concepts)**:
//
// 1.  **`HttpSimulator` 结构体**:
//     -   `client: SuiClient`: 一个 `SuiClient` 实例。这是 `HttpSimulator` 用来与Sui RPC节点进行所有网络通信的工具。
//
// 2.  **`HttpSimulator::new()` 异步构造函数**:
//     -   创建一个新的 `HttpSimulator` 实例。
//     -   **参数**:
//         -   `url: impl AsRef<str>`: Sui RPC节点的URL (例如 "https://fullnode.mainnet.sui.io:443")。
//         -   `ipc_path: &Option<String>`: (可选的) IPC (Inter-Process Communication) 套接字路径。
//             如果提供了IPC路径，`SuiClientBuilder` 可能会优先尝试通过IPC连接到本地节点，这通常比HTTP更快。
//     -   **实现**:
//         -   记录一条警告日志，提示 `HttpSimulator` 已被弃用。
//         -   使用 `SuiClientBuilder` 来构建 `SuiClient`。
//             -   `max_concurrent_requests(2000)`: 设置客户端并发请求的上限。
//             -   如果提供了 `ipc_path`，则配置IPC路径和IPC连接池大小。
//         -   调用 `builder.build(url).await.unwrap()` 来实际创建 `SuiClient`。
//             `.unwrap()` 假设客户端构建总是成功的。
//
// 3.  **`HttpSimulator::max_budget()` 异步方法**:
//     -   **功能**: 从Sui网络获取当前协议配置中允许的最大交易Gas预算 (`max_tx_gas`)。
//     -   **实现**:
//         -   调用 `self.client.read_api().get_protocol_config(None).await` 获取最新的协议配置。
//         -   从配置的属性中查找 "max_tx_gas" 字段，并期望其值为 `SuiProtocolConfigValue::U64`。
//         -   如果找不到或类型不匹配，则 `panic!`。
//     -   **用途**: 这个值可以用来设置模拟或实际交易的Gas预算上限，以确保不超过网络允许的最大值。
//
// 4.  **`Simulator` trait 实现 for `HttpSimulator`**:
//     -   **`simulate()`**:
//         -   **核心逻辑**: 调用 `self.client.read_api().dry_run_transaction_block_override(tx, override_objects).await`。
//             这是Sui RPC API提供的一个端点，允许用户提交一个交易数据 (`tx: TransactionData`) 和一组覆盖对象 (`override_objects`)，
//             然后在不实际执行或提交到共识的情况下，模拟该交易的执行结果。
//         -   **参数处理**:
//             -   它从 `SimulateCtx` 中提取 `override_objects`。
//                 `filter_map(|o| o.as_object().map(|obj| (obj.id(), obj.clone())))`
//                 将 `Vec<ObjectReadResult>` 转换为 `Vec<(ObjectID, Object)>`，这是 `dry_run_transaction_block_override` API期望的格式。
//                 它只包含实际的对象，忽略了删除标记等。
//         -   **结果处理**:
//             -   RPC调用返回一个 `SuiTransactionBlockResponse` (`resp`)。
//             -   `SimulateResult` 使用 `resp.effects` (交易效果), `resp.events` (交易事件), `resp.balance_changes` (余额变更) 来填充。
//             -   **注意**: `object_changes` 字段被硬编码为空向量 `vec![]`。
//                 这意味着 `HttpSimulator` (通过当前的 `dry_run_transaction_block_override` API)
//                 **不返回或不处理**详细的对象状态变更列表（即哪些对象的哪些字段变成了什么值）。
//                 这与 `DBSimulator` 不同，后者可以从其内部状态中计算出这些详细的对象变更。
//             -   `cache_misses` 被硬编码为0，因为 `HttpSimulator` 不涉及像 `DBSimulator` 那样的多层对象缓存机制。
//     -   `name()`: 返回 "HttpSimulator"。
//     -   `get_object()`:
//         -   调用 `self.client.read_api().get_object_with_options(*obj_id, SuiObjectDataOptions::bcs_lossless()).await`。
//         -   `bcs_lossless()` 选项请求对象的完整BCS序列化内容，以便可以无损地恢复为 `Object` 类型。
//         -   对结果进行处理，如果成功获取并能转换为 `Object`，则返回 `Some(Object)`。
//     -   `get_object_layout()`: (在此文件中未实现，但 `Simulator` trait 可能要求)。
//         如果需要，它应该调用 `self.client.read_api().get_move_object_layout(...)` 或类似方法。
//
// **HttpSimulator的特点**:
// -   **简单性**: 相对于 `DBSimulator`，它的实现更简单，因为它不管理本地数据库状态。
// -   **依赖RPC节点**: 其性能和准确性完全依赖于所连接的Sui RPC节点的性能和状态。
// -   **网络延迟**: 每次模拟都需要网络往返，因此通常比 `DBSimulator` 慢。
// -   **状态一致性**: 它总是基于RPC节点报告的“当前”链状态进行模拟（除非使用了 `dry_run_transaction_block_override` 中的覆盖对象）。
// -   **信息限制**: 正如注释所指出的，通过 `dry_run_transaction_block_override` 可能无法获取到与本地模拟（如 `DBSimulator`）一样详细的执行后状态信息 (例如 `object_changes`)。
// -   **弃用状态**: 文件明确指出此模拟器已被弃用，建议主要用于测试或在无法使用 `DBSimulator` 的情况下作为备选。

// 引入 async_trait 宏，用于在trait中定义异步方法。
use async_trait::async_trait;
// 引入 Sui JSON RPC 类型中的 SuiObjectDataOptions，用于指定获取对象时需要哪些数据。
use sui_json_rpc_types::SuiObjectDataOptions;
// 引入 Sui SDK 中的 SuiProtocolConfigValue (协议配置值的枚举), SuiClient (RPC客户端), SuiClientBuilder (客户端构建器)。
use sui_sdk::{rpc_types::SuiProtocolConfigValue, SuiClient, SuiClientBuilder};
// 引入 Sui 核心类型库中的 ObjectID, Object, TransactionData。
use sui_types::{base_types::ObjectID, object::Object, transaction::TransactionData};

// 从当前crate的根模块引入 SimulateCtx, SimulateResult, Simulator trait。
use super::{SimulateCtx, SimulateResult, Simulator};

/// `HttpSimulator` 结构体
///
/// 通过Sui JSON-RPC API与远程Sui节点交互来进行交易模拟。
/// 注意：此模拟器已被标记为已弃用。
#[derive(Clone)] // 允许克隆 HttpSimulator 实例 (内部的 SuiClient 通常是 Arc<...>，克隆成本低)
pub struct HttpSimulator {
    pub client: SuiClient, // Sui RPC客户端实例
}

impl HttpSimulator {
    /// `new` 异步构造函数
    ///
    /// 创建一个新的 `HttpSimulator` 实例。
    ///
    /// 参数:
    /// - `url`: Sui RPC节点的URL字符串。`impl AsRef<str>` 表示可以是 `String`, `&str` 等。
    /// - `ipc_path`: (可选的) IPC套接字路径字符串的引用。如果提供，会尝试使用IPC连接。
    ///
    /// 返回:
    /// - `Self`: 新创建的 `HttpSimulator` 实例。
    pub async fn new(url: impl AsRef<str>, ipc_path: &Option<String>) -> Self {
        // 记录一条警告日志，提示此模拟器已弃用。
        tracing::warn!("HttpSimulator 已被弃用，请考虑使用 DBSimulator (http simulator is deprecated)");

        // 使用 SuiClientBuilder 构建 SuiClient。
        let mut builder = SuiClientBuilder::default()
            .max_concurrent_requests(2000); // 设置最大并发请求数为2000 (可配置)
        if let Some(path_str) = ipc_path { // 如果提供了IPC路径
            builder = builder.ipc_path(path_str).ipc_pool_size(100); // 配置IPC路径和连接池大小
        }
        // 构建SuiClient，连接到指定的URL。
        // `.unwrap()` 假设客户端构建总是成功的，在生产代码中应进行错误处理。
        let rpc_client = builder.build(url).await.unwrap();

        Self { client: rpc_client }
    }

    /// `max_budget` 异步方法
    ///
    /// 从Sui网络获取当前协议配置中允许的最大交易Gas预算 (`max_tx_gas`)。
    ///
    /// 返回:
    /// - `u64`: 最大Gas预算值。如果获取失败或配置中不存在该值，则会panic。
    pub async fn max_budget(&self) -> u64 {
        // 调用Sui RPC API的 `get_protocol_config` 方法获取协议配置。
        // `None` 作为参数表示获取当前版本的协议配置。
        let protocol_config = self
            .client
            .read_api()
            .get_protocol_config(None) // 获取当前版本的协议配置
            .await
            .expect("获取Sui协议配置失败 (failed to get Sui protocol config)"); // RPC调用失败则panic

        // 从协议配置的属性中查找 "max_tx_gas" 字段。
        // `cfg.attributes` 是一个存储配置属性的映射。
        // `get("max_tx_gas")` 返回 `Option<&SuiProtocolConfigValueContainer>`。
        let Some(Some(SuiProtocolConfigValue::U64(max_gas_value))) = protocol_config.attributes.get("max_tx_gas") else {
            // 如果找不到 "max_tx_gas" 字段，或者其值的类型不是期望的 U64，则panic。
            panic!("从协议配置中获取max_tx_gas失败或类型不匹配 (failed to get max_tx_gas from protocol config or type mismatch)");
        };

        *max_gas_value // 解引用并返回u64值
    }
}

/// 为 `HttpSimulator` 实现 `Simulator` trait。
#[async_trait]
impl Simulator for HttpSimulator {
    /// `simulate` 异步方法 (核心模拟逻辑)
    ///
    /// 使用Sui节点的 `dry_run_transaction_block_override` RPC API来模拟交易。
    /// **注意**: 此方法在其返回的 `SimulateResult` 中不填充 `object_changes` 字段。
    ///
    /// 参数:
    /// - `tx_data_to_simulate`: 要模拟的 `TransactionData`。
    /// - `simulation_context`: `SimulateCtx`，其中 `override_objects` 字段用于覆盖模拟中的对象状态。
    ///
    /// 返回:
    /// - `eyre::Result<SimulateResult>`: 包含模拟结果或错误。
    async fn simulate(&self, tx_data_to_simulate: TransactionData, simulation_context: SimulateCtx) -> eyre::Result<SimulateResult> {
        // 从 SimulateCtx 中提取需要在模拟中覆盖的对象列表。
        // `dry_run_transaction_block_override` API期望的覆盖对象格式是 `Vec<(ObjectID, Object)>`。
        // `ObjectReadResult::as_object()` 用于从 `ObjectReadResult` 中安全地提取 `Object` 数据
        // (如果 `ObjectReadResult` 代表的是一个实际的对象而不是删除标记等)。
        let objects_to_override_for_rpc = simulation_context
            .override_objects // Vec<ObjectReadResult>
            .into_iter()
            .filter_map(|object_read_result| { // 过滤掉那些不是实际对象的条目
                object_read_result.as_object().map(|sui_object| (sui_object.id(), sui_object.clone())) // 转换为 (ObjectID, Object) 元组
            })
            .collect::<Vec<_>>();

        // 调用Sui RPC的 `dry_run_transaction_block_override` 方法。
        // 这个API允许在不实际执行交易的情况下，模拟交易在特定对象状态下的执行结果。
        let dry_run_response = self
            .client
            .read_api()
            .dry_run_transaction_block_override(tx_data_to_simulate, objects_to_override_for_rpc)
            .await?; // 处理RPC调用可能返回的错误

        // 从RPC响应构建 `SimulateResult`。
        Ok(SimulateResult {
            effects: dry_run_response.effects,   // 交易效果
            events: dry_run_response.events,     // 交易事件
            object_changes: vec![],              // 注意：HttpSimulator不返回详细的对象变更
            balance_changes: dry_run_response.balance_changes, // 余额变更
            cache_misses: 0,                     // HttpSimulator不使用像DBSimulator那样的多层缓存，所以未命中数为0
        })
    }

    /// `name` 方法 (来自 `Simulator` trait)
    /// 返回模拟器的名称。
    fn name(&self) -> &str {
        "HttpSimulator"
    }

    /// `get_object` 异步方法 (来自 `Simulator` trait)
    ///
    /// 通过Sui RPC从链上获取指定ObjectID的对象数据。
    ///
    /// 参数:
    /// - `obj_id`: 要获取的对象的ID。
    ///
    /// 返回:
    /// - `Option<Object>`: 如果成功获取并能转换为 `Object` 类型，则返回 `Some(Object)`，否则 `None`。
    async fn get_object(&self, obj_id: &ObjectID) -> Option<Object> {
        self.client
            .read_api()
            .get_object_with_options(*obj_id, SuiObjectDataOptions::bcs_lossless()) // 请求对象的无损BCS内容
            .await // 等待异步RPC调用
            .ok()? // 将 Result<SuiObjectResponse, Error> 转换为 Option<SuiObjectResponse>, 忽略错误
            .data? // 从 SuiObjectResponse 中获取 Option<SuiObjectData>
            .try_into() // 尝试将 SuiObjectData 转换为 Object (SuiObjectData 实现了 TryInto<Object>)
            .ok() // 将 Result<Object, Error> 转换为 Option<Object>, 忽略转换错误
    }

    // `get_object_layout` 方法没有在此实现。
    // 如果 `Simulator` trait 要求，需要添加并调用类似 `client.read_api().get_move_object_layout()` 的方法。
}

[end of crates/simulator/src/http_simulator.rs]
