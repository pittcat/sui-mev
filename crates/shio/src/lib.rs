// 该文件 `lib.rs` 是 `shio` crate (库) 的根文件。
// `shio` crate 封装了与 Shio MEV (Miner Extractable Value) 协议交互的逻辑。
// Shio 似乎是一个为Sui区块链设计的MEV解决方案，允许机器人（搜索者）
// 监听MEV机会并通过提交竞价来尝试捕获这些机会。
//
// **文件概览 (File Overview)**:
// 这个文件是 `shio` 库的“大门”。它定义了与Shio协议通信所需的一些核心组件和常量。
// 主要功能包括：
// -   声明和重新导出子模块中定义的类型（如 `ShioCollector`, `ShioExecutor`, `ShioItem` 等）。
// -   定义与Shio协议相关的常量，如全局状态对象ID列表 (`SHIO_GLOBAL_STATES`) 和API URL (`SHIO_FEED_URL`, `SHIO_JSON_RPC_URL`)。
// -   提供一个工厂函数 `new_shio_collector_and_executor()` 来方便地创建Shio事件收集器和执行器实例。
//
// **核心组件和概念 (Core Components and Concepts)**:
//
// 1.  **子模块 (Submodules)**:
//     -   `shio_collector`: 可能定义了 `ShioCollector`，负责从Shio的事件源（如WebSocket feed）接收MEV机会信息 (`ShioItem`)。
//     -   `shio_conn`: 可能处理与Shio服务器的底层连接（例如WebSocket连接的建立和管理）。
//     -   `shio_executor`: 可能定义了 `ShioExecutor`，负责将机器人的竞价（bid）提交回Shio协议。
//     -   `shio_rpc_executor`: 可能提供了另一种通过RPC方式提交竞价的执行器。
//     -   `types`: 可能定义了Shio协议特定的数据结构，如 `ShioItem` (代表一个MEV机会) 和 `BidInfo` (代表一个竞价)。
//
// 2.  **`SHIO_GLOBAL_STATES` 常量**:
//     -   这是一个包含32个元组的数组，每个元组由一个字符串（表示Shio全局状态对象的ObjectID）和一个u64（表示该对象的初始共享版本号）组成。
//     -   Shio协议可能将其核心状态分布在多个共享对象上，以提高并发处理能力或可扩展性。
//         当机器人需要与Shio协议交互时（例如提交竞价），它可能需要轮流使用这些全局状态对象中的一个。
//         这种设计在 `bin/arb/src/defi/shio.rs` 中的 `Shio::next_state()` 方法里有所体现。
//
// 3.  **API URL常量**:
//     -   `SHIO_FEED_URL`: Shio提供MEV机会事件流的WebSocket服务器地址。`ShioCollector` 会连接这个地址。
//     -   `SHIO_JSON_RPC_URL`: Shio可能提供的JSON RPC服务地址，用于其他类型的交互或查询。
//
// 4.  **`pub use` 语句**:
//     -   这些语句将子模块中定义的关键类型（如 `ShioCollector`, `ShioExecutor`, `ShioRPCExecutor`, 以及 `types` 模块中的所有类型）
//         重新导出到 `shio` crate的顶层命名空间。
//     -   这样做使得 `shio` crate的使用者可以直接通过 `shio::ShioCollector` 这样的路径来访问这些类型，
//         而不需要知道它们具体是在哪个子模块中定义的，简化了外部调用。
//
// 5.  **`new_shio_collector_and_executor()` 异步函数**:
//     -   这是一个工厂函数，用于方便地创建一对匹配的 `ShioCollector` 和 `ShioExecutor` 实例。
//     -   **内部逻辑**:
//         1.  调用 `shio_conn::new_shio_conn()` 来建立与Shio WebSocket feed的连接。
//             这个函数会返回两个东西：
//             -   `bid_sender`: 一个异步通道的发送端，用于将竞价信息 (`BidInfo`) 发送回Shio连接管理模块，再由后者提交给Shio服务器。
//             -   `shio_item_receiver`: 一个异步通道的接收端，用于从Shio连接管理模块接收新的MEV机会 (`ShioItem`)。
//         2.  使用 `bid_sender` 和传入的 `keypair` (用于签名竞价) 创建 `ShioExecutor`。
//         3.  使用 `shio_item_receiver` 创建 `ShioCollector`。
//         4.  返回创建好的 `(ShioCollector, ShioExecutor)` 元组。
//     -   **参数**:
//         -   `keypair`: 用于签名竞价交易的Sui密钥对。
//         -   `shio_feed_url`: (可选) Shio WebSocket feed的URL。如果为 `None`，则使用默认的 `SHIO_FEED_URL`。
//         -   `num_retries`: (可选) 连接失败时的重试次数。如果为 `None`，则使用默认的重试次数 (例如3次)。
//
// **MEV (Miner Extractable Value / Maximal Extractable Value) 在此上下文中的意义**:
// -   Shio协议本身就是一个为MEV设计的系统。它充当了MEV机会的来源（通过 `ShioCollector` 广播 `ShioItem`）
//     和MEV捕获的执行渠道（通过 `ShioExecutor` 提交竞价）。
// -   套利机器人作为MEV“搜索者”（Searcher），会监听来自Shio的机会，分析它们，
//     如果发现有利可图，就会构建一个包含套利逻辑和支付给验证者“小费”（即竞价金额）的交易，
//     然后通过ShioExecutor提交这个交易和竞价。
// -   使用多个全局状态对象 (`SHIO_GLOBAL_STATES`) 可能是一种应对高并发竞价请求的机制，
//     通过分散请求到不同的状态对象来减轻单个对象的负载压力。

// 声明子模块。这些模块包含了Shio客户端的具体实现。
mod shio_collector;    // 定义了从Shio feed收集MEV机会的 ShioCollector
mod shio_conn;         // 处理与Shio服务器的底层WebSocket连接
mod shio_executor;     // 定义了通过Shio连接提交竞价的 ShioExecutor
mod shio_rpc_executor; // 可能定义了通过RPC方式提交竞价的 ShioRPCExecutor
mod types;             // 定义了Shio协议相关的数据类型，如 ShioItem, BidInfo等

// `SHIO_GLOBAL_STATES` 常量数组
//
// 这个数组存储了Shio协议使用的多个全局状态对象的ObjectID字符串及其对应的初始共享版本号。
// Shio协议可能将其核心状态分布在这些共享对象上，以提高并发处理能力或实现某种分片/轮询机制。
// 套利机器人在与Shio协议交互（例如提交竞价）时，可能需要轮流选择这些状态对象中的一个作为参数。
// 每个元组的结构是: (对象ID的十六进制字符串, 对象的初始共享版本号 u64)
pub const SHIO_GLOBAL_STATES: [(&str, u64); 32] = [
    // 示例条目 (第一个): 对象ID "0xc32..." 的初始共享版本是 72869622
    (
        "0xc32ce42eac951759666cbc993646b72387ec2708a2917c2c6fb7d21f00108c18",
        72869622,
    ),
    // ... (其他31个全局状态对象的ID和版本号) ...
    (
        "0x0289acae0edcdf1fe3aedc2e886bc23064d41c359e0179a18152a64d1c1c2b3e",
        327637282,
    ),
    (
        "0x03132160e8c2c45208abf3ccf165e82edcc42fee2e614afe54582f9740a808b8",
        327637282,
    ),
    (
        "0x072ae7307459e535379f422995a0d10132f12a3450298f8cf0cc07bd164f9999",
        327637282,
    ),
    (
        "0x1c1a96a2f4a34ea09ab15b8ff98f4b6b4338ce89f4158eb7d3eb2cd4dcbd6d86",
        327637282,
    ),
    (
        "0x20d76f37ad9f2421a9e6afaf3bb204250b1c2241c50e8a955e86a1a48767d06f",
        327637282,
    ),
    (
        "0x213ed368233cc7480fcb6336e70c5ae7ee106b2317ba02ccb5d0478e45bcc046",
        327637282,
    ),
    (
        "0x22ce1e80937354eba5549fed2937dc6e2b24026d03505bb51a3e4a64aa4142f6",
        327637282,
    ),
    (
        "0x26188cb7ce5ae633279f440f66081cb65cc585e428de18e194f8843e866f799f",
        327637282,
    ),
    (
        "0x38642f01422480128388d3e2948d3dc1b2680f9914077edb6bd3451ae1c5bcf0",
        327637282,
    ),
    (
        "0x3dd85b6424aea1cae9eff6e55456ca783e056226325f1362106eca8b3ed04ca0",
        327637282,
    ),
    (
        "0x42f8adc490542369d9c3b95e9f6eb70b2583102900feb7e103072ed49ba7fc3d",
        327637282,
    ),
    (
        "0x46b8158c82fa6bda7230d31a127d934c7295a0042083b4900f3096e9191f6f3f",
        327637282,
    ),
    (
        "0x6ebac88a8c3f7a4a9fb05ea49d188a1fe8520ae59ee736e0473004d3033512a4",
        327637282,
    ),
    (
        "0x6f55ad6cb40cfc124c11b11c19be0a80237b104acd955e7b52ccb7bf9046fe33",
        327637282,
    ),
    (
        "0x71aafb8bac986e82e5f78846bf3b36c2a82505585625207324140227a27ff279",
        327637282,
    ),
    (
        "0x7fe9b08680d4179de5672f213b863525b21f10604ca161538075e9338d1d2324",
        327637282,
    ),
    (
        "0x81538ef2909a3e0dd3d7f38bcbee191509bae4e8666272938ced295672e2ee8d",
        327637282,
    ),
    (
        "0x828eb6b3354ad68a23dd792313a16a0d888b7ea4fdb884bb22bd569f8e61319e",
        327637282,
    ),
    (
        "0x9705a332b8c1650dd7fe687ef9f9a9638afb51c30c0b34db150d60b920bc07eb",
        327637282,
    ),
    (
        "0x9918f73797a9390e9888b55454f2b31bc01de1a4634acab08f80641c4248e8a5",
        327637282,
    ),
    (
        "0x9cd4c08bdf2e132ec2cc77b0f03be60a94951e046d8e82ed5494f44e609edd2f",
        327637282,
    ),
    (
        "0xac8ce2033571140509788337c8a1f3aa8941a320ecd7047acda310d39cad9e03",
        327637282,
    ),
    (
        "0xbcd4527035265461a9a7b4f1e57c63ea7a6bdf0dc223c66033c218d880f928b1",
        327637282,
    ),
    (
        "0xbfdb691b8cc0b3c3a3b7a654f6682f3e53b164d9ee00b9582cdb4d0a353440a9",
        327637282,
    ),
    (
        "0xc2559d5c52ae04837ddf943a8c2cd53a5a0b512cee615d30d3abe25aa339465e",
        327637282,
    ),
    (
        "0xc56db634d02511e66d7ca1254312b71c60d64dc44bf67ea46b922c52d8aebba6",
        327637282,
    ),
    (
        "0xc84545cbff1b36b874ab2b69d11a3d108f23562e87550588c0bda335b27101e0",
        327637282,
    ),
    (
        "0xcc141659b5885043f9bfcfe470064819ab9ac667953bcedd1000e0652e90ee76",
        327637282,
    ),
    (
        "0xef6bf4952968d25d3e79f7e4db1dc38f2e9d99d61ad38f3829acb4100fe6383a",
        327637282,
    ),
    (
        "0xf2ed8d00ef829de5c4a3c5adf2d6b0f41f7fec005fb9c88e5616b98173b2fd66",
        327637282,
    ),
    (
        "0xfce73f3c32c3f56ddb924a04cabd44dd870b72954bbe7c3d7767c3b8c25c4326",
        327637282,
    ),
];

/// `SHIO_FEED_URL` 常量
///
/// Shio协议提供MEV机会事件流的WebSocket服务器的默认URL。
pub const SHIO_FEED_URL: &str = "wss://rpc.getshio.com/feed";
/// `SHIO_JSON_RPC_URL` 常量
///
/// Shio协议可能提供的JSON RPC服务的URL，用于其他类型的API交互（例如，查询状态或提交非竞价类请求）。
pub const SHIO_JSON_RPC_URL: &str = "https://rpc.getshio.com";

// --- 重新导出子模块中的公共类型 ---
// `pub use` 语句将子模块中定义的类型提升到当前 `shio` crate 的顶层命名空间，
// 使得外部使用者可以直接通过 `shio::TypeName` 来访问它们，而无需关心内部模块结构。
pub use shio_collector::ShioCollector;     // 从 shio_collector.rs 导出 ShioCollector
pub use shio_executor::ShioExecutor;       // 从 shio_executor.rs 导出 ShioExecutor
pub use shio_rpc_executor::ShioRPCExecutor; // 从 shio_rpc_executor.rs 导出 ShioRPCExecutor
pub use types::*;                           // 从 types.rs 导出所有公共类型 (例如 ShioItem, BidInfo)

/// `new_shio_collector_and_executor` 异步函数 (工厂函数)
///
/// 创建并返回一对匹配的 `ShioCollector` 和 `ShioExecutor` 实例。
/// 这个函数封装了与Shio服务器建立连接、创建通道以及初始化收集器和执行器的逻辑。
///
/// 参数:
/// - `keypair`: 用户的 `sui_types::crypto::SuiKeyPair`，用于对提交给Shio的竞价进行签名。
/// - `shio_feed_url`: (可选) Shio WebSocket feed的URL。如果为 `None`，则使用默认的 `SHIO_FEED_URL`。
/// - `num_retries`: (可选) `shio_conn` 在尝试连接WebSocket失败时的重试次数。如果为 `None`，则使用默认值 (例如3次)。
///
/// 返回:
/// - `(ShioCollector, ShioExecutor)`: 一个包含新创建的收集器和执行器的元组。
pub async fn new_shio_collector_and_executor(
    keypair: sui_types::crypto::SuiKeyPair, // 用于签名竞价的密钥对
    shio_feed_url: Option<String>,         // 可选的Shio feed URL
    num_retries: Option<u32>,              // 可选的连接重试次数
) -> (ShioCollector, ShioExecutor) {
    // 调用 `shio_conn::new_shio_conn` 来建立与Shio服务器的WebSocket连接，并设置双向通信通道。
    // `bid_sender` 用于将本地产生的竞价信息发送到连接管理器，再由其发送给Shio服务器。
    // `shio_item_receiver` 用于从连接管理器接收来自Shio服务器的MEV机会 (`ShioItem`)。
    let (bid_sender, shio_item_receiver) = shio_conn::new_shio_conn(
        shio_feed_url.unwrap_or_else(|| SHIO_FEED_URL.to_string()), // 如果未提供URL，则使用默认值
        num_retries.unwrap_or(3), // 如果未提供重试次数，则默认为3次
    )
    .await; // 等待连接和通道设置完成

    // 创建 `ShioExecutor` 实例，它需要密钥对 (用于签名) 和竞价发送通道。
    let executor = ShioExecutor::new(keypair, bid_sender).await;
    // 创建 `ShioCollector` 实例，它需要MEV机会接收通道。
    let collector = ShioCollector::new(shio_item_receiver);

    // 返回创建好的收集器和执行器。
    (collector, executor)
}

[end of crates/shio/src/lib.rs]
