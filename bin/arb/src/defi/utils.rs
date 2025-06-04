// 该文件 `utils.rs` 位于 `defi` 模块下，提供了一些与DeFi操作相关的辅助工具函数。
// 在这个特定的文件中，主要功能是封装了从Sui区块链获取对象数据的操作，并增加了一层缓存机制以提高性能。
//
// **文件概览 (File Overview)**:
// 这个 `utils.rs` 文件是 `defi`（去中心化金融）模块的一个“工具箱”。
// 它包含了一些帮助处理DeFi相关任务的小工具函数。
// 目前，这里最主要的功能是优化从Sui区块链上读取“对象”（Object）数据的过程。
// (This `utils.rs` file is a "toolbox" for the `defi` (Decentralized Finance) module.
// It contains utility functions that help with DeFi-related tasks.
// Currently, its main function is to optimize the process of reading "Object" data from the Sui blockchain.)
//
// **核心功能 (Core Functionality)**:
// 1.  **`get_object_cache()` (带缓存的对象获取)**:
//     -   这是一个“聪明”的函数，用来获取Sui链上的对象数据。
//     -   “聪明”之处在于它使用了“缓存”（Caching）。想象一下，你第一次问它要某个对象（比如某个交易池）的信息，它会真的去Sui网络上查询，然后把结果告诉你，并且自己偷偷记下来（存到缓存里）。
//     -   下次你再问它要同一个对象的信息，它就不会再去麻烦Sui网络了，而是直接从它的小本子（缓存）里把上次的结果拿出来给你。这样速度快很多，也减少了对Sui网络的访问压力。
//     -   这种缓存功能是通过一个叫做 `#[cached]` 的“魔法咒语”（宏）来实现的。
//         (This is a "smart" function for fetching object data from the Sui chain.
//          Its "smartness" lies in its use of "Caching". Imagine you ask it for information about an object (e.g., a trading pool) for the first time. It will actually query the Sui network, give you the result, and secretly note it down (store it in cache).
//          The next time you ask for information about the same object, it won't bother the Sui network again but will directly retrieve the previous result from its notepad (cache) for you. This is much faster and reduces the load on the Sui network.
//          This caching functionality is implemented using a "magic spell" (macro) called `#[cached]`.)
//
// 2.  **`get_object()` (基础的对象获取)**:
//     -   这是一个比较“朴实”的函数，也用来获取Sui链上的对象数据。
//     -   它没有缓存功能，每次你调用它，它都会老老实实地去Sui网络上查询一次。
//     -   上面的 `get_object_cache()` 函数在发现自己没有缓存某个对象信息的时候，就会调用这个 `get_object()` 函数去真正地获取数据。
//         (This is a more "basic" function, also used for fetching object data from the Sui chain.
//          It doesn't have caching. Every time you call it, it will honestly query the Sui network once.
//          The `get_object_cache()` function above calls this `get_object()` function to actually fetch data when it finds it doesn't have the information cached for an object.)
//
// **Sui区块链相关的概念解释 (Sui Blockchain-related Concepts)**:
//
// -   **`SuiObjectData` (Sui对象数据)**:
//     在Sui区块链上，所有东西（比如代币、NFT、智能合约、交易池）都是以“对象”（Object）的形式存在的。
//     `SuiObjectData` 就是一个用来描述这些对象详细信息的数据结构。它里面包含了对象的内容是什么、对象属于谁、对象的版本号等等。
//     (On the Sui blockchain, everything (like tokens, NFTs, smart contracts, trading pools) exists in the form of "Objects".
//      `SuiObjectData` is a data structure used to describe the detailed information of these objects. It contains what the object's content is, who owns the object, the object's version number, etc.)
//
// -   **`ObjectID` (对象ID)**:
//     每个Sui对象都有一个独一无二的“身份证号码”，就是 `ObjectID`。它通常显示为一个十六进制的字符串（比如 `0x123abc...`）。
//     通过这个ID，就可以准确地找到并引用链上的任何一个对象。
//     (Every Sui object has a unique "ID card number", which is the `ObjectID`. It's usually displayed as a hexadecimal string (e.g., `0x123abc...`).
//      With this ID, one can accurately find and reference any object on the chain.)
//
// -   **`SuiClient` (Sui客户端)**:
//     这是Sui官方提供的一个工具（SDK的一部分），让你的程序可以和Sui区块链网络进行“对话”。
//     通过 `SuiClient`，你的程序可以查询链上数据（比如获取对象信息）、发送交易、监听事件等。
//     (This is a tool provided by Sui officially (part of the SDK) that allows your program to "talk" with the Sui blockchain network.
//      Through `SuiClient`, your program can query on-chain data (like fetching object information), send transactions, listen to events, etc.)
//
// -   **RPC (Remote Procedure Call / 远程过程调用)**:
//     这是一种计算机之间互相请求服务的方式。当你的程序（客户端）想要从Sui节点（服务器）获取信息时，它就会发起一个RPC请求。
//     比如，`get_object()` 函数内部实际上就是通过 `SuiClient` 发起了一个RPC请求到Sui节点，去获取特定对象的数据。
//     (This is a way for computers to request services from each other. When your program (client) wants to get information from a Sui node (server), it makes an RPC request.
//      For example, the `get_object()` function internally makes an RPC request via `SuiClient` to a Sui node to fetch data for a specific object.)
//
// -   **缓存 (Caching)**:
//     这是一种常见的计算机技术，用来提高程序性能。基本思想是：把那些经常要用到的、或者获取起来比较慢（比如需要通过网络RPC请求）的数据，
//     临时存放在一个更容易访问的地方（比如内存里）。下次再需要这些数据时，就可以直接从这个临时存放点拿，而不用再费力去重新获取。
//     `get_object_cache()` 函数就是利用了这个原理。
//     (This is a common computer technique used to improve program performance. The basic idea is: for data that is frequently used or slow to obtain (e.g., requiring a network RPC request),
//      store it temporarily in a more easily accessible place (like memory). The next time this data is needed, it can be retrieved directly from this temporary storage instead of going through the effort of re-fetching it.
//      The `get_object_cache()` function utilizes this principle.)

// 引入所需的库和模块 (Import necessary libraries and modules)
use cached::proc_macro::cached; // `cached` crate提供的过程宏，用于轻松地为函数添加缓存功能。
                                // Procedural macro from the `cached` crate, used to easily add caching functionality to functions.
use eyre::Result; // `eyre`库，用于更方便的错误处理。`Result`是其核心类型。
                  // `eyre` library, for more convenient error handling. `Result` is its core type.
use sui_sdk::{
    rpc_types::{SuiObjectData, SuiObjectDataOptions}, // 从Sui SDK引入对象数据和对象数据选项的类型。
                                                     // Import object data and object data options types from the Sui SDK.
    SuiClient,                                     // Sui RPC客户端。(Sui RPC client.)
};
use sui_types::base_types::ObjectID; // Sui核心类型中的ObjectID。(ObjectID from Sui core types.)

/// `get_object_cache` 异步函数 (带缓存的对象获取 / Asynchronous function `get_object_cache` (Object fetching with cache))
///
/// 这个函数用于根据对象ID (`obj_id`) 从Sui网络获取对象数据 (`SuiObjectData`)。
/// (This function is used to fetch object data (`SuiObjectData`) from the Sui network based on the object ID (`obj_id`).)
/// 它使用 `#[cached]` 宏来实现缓存：
/// (It uses the `#[cached]` macro to implement caching:)
/// - 第一次调用此函数获取某个 `obj_id` 的数据时，它会实际调用 `get_object()` 函数执行RPC请求，
///   并将结果缓存起来。
///   (When this function is called for the first time to fetch data for a specific `obj_id`, it will actually call the `get_object()` function to perform an RPC request,
///    and then cache the result.)
/// - 后续再次使用相同的 `obj_id` 调用此函数时，它会直接从缓存中返回之前的结果，而不会再次发起RPC请求。
///   (Subsequent calls to this function with the same `obj_id` will directly return the previous result from the cache without making another RPC request.)
/// 这对于频繁请求相同对象数据的场景（例如，某些全局配置对象或常用的池对象）可以显著提高性能并减少RPC负载。
/// (This can significantly improve performance and reduce RPC load for scenarios that frequently request the same object data (e.g., certain global configuration objects or commonly used pool objects).)
///
/// `#[cached]` 宏的参数说明 (Explanation of `#[cached]` macro parameters):
/// - `key = "String"`: 指定用于缓存的键的类型是 `String`。
///                     (Specifies that the type of the key used for caching is `String`.)
/// - `convert = r##"{ obj_id.to_string() }"##`: 指定如何将函数的输入参数转换为缓存键。
///   这里是将 `obj_id` (类型为 `&str`) 转换为 `String` 作为键。
///   `r##"{...}"##` 是Rust的原始字符串字面量，允许在字符串中直接使用花括号等特殊字符。
///   (Specifies how to convert the function's input parameters into a cache key.
///    Here, `obj_id` (of type `&str`) is converted to a `String` to be used as the key.
///    `r##"{...}"##` is a Rust raw string literal, allowing special characters like curly braces to be used directly within the string.)
/// - `result = true`: 表示缓存的是函数的 `Result` 的成功 (`Ok`) 值。如果函数返回 `Err`，则不会缓存。
///                    (Indicates that what's cached is the successful (`Ok`) value of the function's `Result`. If the function returns `Err`, it will not be cached.)
///
/// 参数 (Parameters):
/// - `sui`: 一个对 `SuiClient` 的引用，用于执行RPC调用 (当缓存未命中时)。
///          (A reference to `SuiClient`, used for executing RPC calls (when cache misses).)
/// - `obj_id`: 要获取的对象的ID，作为字符串切片 (`&str`)。
///             (The ID of the object to fetch, as a string slice (`&str`).)
///
/// 返回 (Returns):
/// - `Result<SuiObjectData>`: 成功则返回获取到的对象数据，否则返回错误。
///                            (Returns the fetched object data if successful, otherwise returns an error.)
#[cached(key = "String", convert = r##"{ obj_id.to_string() }"##, result = true)]
pub async fn get_object_cache(sui: &SuiClient, obj_id: &str) -> Result<SuiObjectData> {
    // 当缓存未命中时，调用内部的 `get_object` 函数实际获取数据。
    // (When a cache miss occurs, call the internal `get_object` function to actually fetch the data.)
    get_object(sui, obj_id).await
}

/// `get_object` 异步函数 (基础的对象获取 / Asynchronous function `get_object` (Basic object fetching))
///
/// 这个函数根据对象ID (`obj_id`) 从Sui网络获取完整的对象数据。
/// (This function fetches the complete object data from the Sui network based on the object ID (`obj_id`).)
/// 它不包含缓存逻辑，每次调用都会发起一次RPC请求。
/// (It does not include caching logic; each call will initiate an RPC request.)
///
/// 参数 (Parameters):
/// - `sui`: 一个对 `SuiClient` 的引用，用于执行RPC调用。
///          (A reference to `SuiClient`, used for executing RPC calls.)
/// - `obj_id`: 要获取的对象的ID，作为字符串切片 (`&str`)。
///             (The ID of the object to fetch, as a string slice (`&str`).)
///
/// 返回 (Returns):
/// - `Result<SuiObjectData>`: 成功则返回获取到的对象数据，否则返回错误。
///                            (Returns the fetched object data if successful, otherwise returns an error.)
pub async fn get_object(sui: &SuiClient, obj_id: &str) -> Result<SuiObjectData> {
    // 步骤 1: 将字符串形式的对象ID转换为 `ObjectID` 类型。
    // (Step 1: Convert the string form of the object ID to the `ObjectID` type.)
    // `ObjectID::from_hex_literal()` 用于从十六进制字符串解析。`?` 用于错误传播。
    // (`ObjectID::from_hex_literal()` is used for parsing from a hexadecimal string. `?` is for error propagation.)
    let parsed_obj_id = ObjectID::from_hex_literal(obj_id)?;

    // 步骤 2: 设置获取对象数据的选项。
    // (Step 2: Set options for fetching object data.)
    // `SuiObjectDataOptions::full_content()` 表示我们希望获取对象的全部内容，
    // 包括其Move结构体数据、所有者信息、版本号、摘要等。
    // (`SuiObjectDataOptions::full_content()` indicates that we want to fetch the full content of the object,
    //  including its Move struct data, owner information, version number, digest, etc.)
    let options = SuiObjectDataOptions::full_content();

    // 步骤 3: 通过Sui客户端的 `read_api()` 调用 `get_object_with_options()` 方法。
    // (Step 3: Call the `get_object_with_options()` method via the Sui client's `read_api()`.)
    // 这是一个异步调用，所以使用 `.await`。
    // (This is an asynchronous call, so `.await` is used.)
    // `?` 同样用于错误传播，如果RPC调用失败，错误会向上传播。
    // (`?` is also used for error propagation; if the RPC call fails, the error will propagate upwards.)
    let object_response = sui
        .read_api()
        .get_object_with_options(parsed_obj_id, options)
        .await?;

    // `get_object_with_options` 返回的是 `RpcResult<SuiPastObjectResponse>` 或类似的类型，
    // 它可能包含对象存在或不存在或已删除等多种状态。
    // (`get_object_with_options` returns `RpcResult<SuiPastObjectResponse>` or a similar type,
    //  which may indicate various states such as the object existing, not existing, or being deleted.)
    // `.into_object()?` 是一个便捷方法 (可能在Sui SDK的 `RpcResult` 或 `SuiObjectResponse` 上实现)，
    // 用于提取 `SuiObjectData`。如果对象不存在或获取失败，它会返回一个错误。
    // (`.into_object()?` is a convenience method (possibly implemented on `RpcResult` or `SuiObjectResponse` in the Sui SDK)
    //  used to extract `SuiObjectData`. If the object doesn't exist or fetching fails, it will return an error.)
    let sui_object_data = object_response.into_object()?;

    // 返回成功获取的对象数据。
    // (Return the successfully fetched object data.)
    Ok(sui_object_data)
}

[end of bin/arb/src/defi/utils.rs]
