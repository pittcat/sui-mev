// 该文件 `utils.rs` 位于 `defi` 模块下，提供了一些与DeFi操作相关的辅助工具函数。
// 在这个特定的文件中，主要功能是封装了从Sui区块链获取对象数据的操作，并增加了一层缓存机制以提高性能。
//
// 文件概览:
// 1. `get_object_cache()`: 一个带缓存的异步函数，用于获取Sui对象数据。它使用 `cached` 宏来实现缓存功能，
//    避免对同一个对象ID进行重复的RPC查询。
// 2. `get_object()`: 一个基础的异步函数，用于根据对象ID从Sui RPC节点获取完整的对象数据。
//    `get_object_cache` 内部会调用这个函数来实际获取数据（当缓存未命中时）。
//
// Sui概念:
// - SuiObjectData: 代表Sui链上一个对象的完整数据，包括其内容、所有者、版本、类型等。
// - ObjectID: Sui链上对象的唯一标识符，通常是一个十六进制字符串。
// - SuiClient: Sui SDK提供的客户端，用于与Sui RPC节点进行通信，执行读取链上数据或发送交易等操作。
// - RPC (Remote Procedure Call): 远程过程调用，是客户端与Sui节点通信的标准方式。
// - Caching: 缓存是一种优化技术，通过将频繁访问或获取成本较高的数据存储在临时位置（内存中），
//   使得后续访问可以直接从缓存中快速获取，而无需再次执行高成本的操作（如RPC调用）。

// 引入所需的库和模块
use cached::proc_macro::cached; // `cached` crate提供的过程宏，用于轻松地为函数添加缓存功能。
use eyre::Result; // `eyre`库，用于更方便的错误处理。`Result`是其核心类型。
use sui_sdk::{
    rpc_types::{SuiObjectData, SuiObjectDataOptions}, // 从Sui SDK引入对象数据和对象数据选项的类型。
    SuiClient,                                     // Sui RPC客户端。
};
use sui_types::base_types::ObjectID; // Sui核心类型中的ObjectID。

/// `get_object_cache` 异步函数 (带缓存的对象获取)
///
/// 这个函数用于根据对象ID (`obj_id`) 从Sui网络获取对象数据 (`SuiObjectData`)。
/// 它使用 `#[cached]` 宏来实现缓存：
/// - 第一次调用此函数获取某个 `obj_id` 的数据时，它会实际调用 `get_object()` 函数执行RPC请求，
///   并将结果缓存起来。
/// - 后续再次使用相同的 `obj_id` 调用此函数时，它会直接从缓存中返回之前的结果，而不会再次发起RPC请求。
/// 这对于频繁请求相同对象数据的场景（例如，某些全局配置对象或常用的池对象）可以显著提高性能并减少RPC负载。
///
/// `#[cached]` 宏的参数说明:
/// - `key = "String"`: 指定用于缓存的键的类型是 `String`。
/// - `convert = r##"{ obj_id.to_string() }"##`: 指定如何将函数的输入参数转换为缓存键。
///   这里是将 `obj_id` (类型为 `&str`) 转换为 `String` 作为键。
///   `r##"{...}"##` 是Rust的原始字符串字面量，允许在字符串中直接使用花括号等特殊字符。
/// - `result = true`: 表示缓存的是函数的 `Result` 的成功 (`Ok`) 值。如果函数返回 `Err`，则不会缓存。
///
/// 参数:
/// - `sui`: 一个对 `SuiClient` 的引用，用于执行RPC调用 (当缓存未命中时)。
/// - `obj_id`: 要获取的对象的ID，作为字符串切片 (`&str`)。
///
/// 返回:
/// - `Result<SuiObjectData>`: 成功则返回获取到的对象数据，否则返回错误。
#[cached(key = "String", convert = r##"{ obj_id.to_string() }"##, result = true)]
pub async fn get_object_cache(sui: &SuiClient, obj_id: &str) -> Result<SuiObjectData> {
    // 当缓存未命中时，调用内部的 `get_object` 函数实际获取数据。
    get_object(sui, obj_id).await
}

/// `get_object` 异步函数 (基础的对象获取)
///
/// 这个函数根据对象ID (`obj_id`) 从Sui网络获取完整的对象数据。
/// 它不包含缓存逻辑，每次调用都会发起一次RPC请求。
///
/// 参数:
/// - `sui`: 一个对 `SuiClient` 的引用，用于执行RPC调用。
/// - `obj_id`: 要获取的对象的ID，作为字符串切片 (`&str`)。
///
/// 返回:
/// - `Result<SuiObjectData>`: 成功则返回获取到的对象数据，否则返回错误。
pub async fn get_object(sui: &SuiClient, obj_id: &str) -> Result<SuiObjectData> {
    // 步骤 1: 将字符串形式的对象ID转换为 `ObjectID` 类型。
    // `ObjectID::from_hex_literal()` 用于从十六进制字符串解析。`?` 用于错误传播。
    let parsed_obj_id = ObjectID::from_hex_literal(obj_id)?;

    // 步骤 2: 设置获取对象数据的选项。
    // `SuiObjectDataOptions::full_content()` 表示我们希望获取对象的全部内容，
    // 包括其Move结构体数据、所有者信息、版本号、摘要等。
    let options = SuiObjectDataOptions::full_content();

    // 步骤 3: 通过Sui客户端的 `read_api()` 调用 `get_object_with_options()` 方法。
    // 这是一个异步调用，所以使用 `.await`。
    // `?` 同样用于错误传播，如果RPC调用失败，错误会向上传播。
    let object_response = sui
        .read_api()
        .get_object_with_options(parsed_obj_id, options)
        .await?;

    // `get_object_with_options` 返回的是 `RpcResult<SuiPastObjectResponse>` 或类似的类型，
    // 它可能包含对象存在或不存在或已删除等多种状态。
    // `.into_object()?` 是一个便捷方法 (可能在Sui SDK的 `RpcResult` 或 `SuiObjectResponse` 上实现)，
    // 用于提取 `SuiObjectData`。如果对象不存在或获取失败，它会返回一个错误。
    let sui_object_data = object_response.into_object()?;

    // 返回成功获取的对象数据。
    Ok(sui_object_data)
}
