// 该文件 `types.rs` (位于 `shio` crate中) 定义了与 Shio MEV 协议交互时使用的数据结构。
// 这些结构体主要用于反序列化从 Shio WebSocket feed 接收到的JSON消息，
// 将其转换为Rust程序可以方便处理的类型。
//
// **文件概览 (File Overview)**:
// 这个文件是 `shio` 库的“数据字典”或“信息蓝图”。它详细描述了从Shio服务器收到的各种消息
// （特别是MEV机会信息）应该是什么样子，以及我们程序内部如何表示这些信息。
//
// **核心数据结构 (Core Data Structures)**:
//
// 1.  **`ShioItem` 枚举**:
//     -   这是最顶层的枚举，代表从Shio feed接收到的不同类型的消息/事件。
//     -   **`AuctionStarted`**: 当Shio协议检测到一个新的MEV拍卖开始时，会发送这种类型的消息。它包含：
//         -   `tx_digest`: 触发拍卖的原始“机会交易”的摘要。
//         -   `gas_price`: 该机会交易的Gas价格。
//         -   `deadline_timestamp_ms`: 此拍卖的竞价截止时间戳（毫秒）。
//         -   `side_effects`: 一个 `SideEffects` 结构，包含了机会交易执行后产生的“副作用”，如创建/修改的对象和发出的事件。
//         -   `_other`: 一个 `()` (空元组)，用 `#[serde(skip)]` 标记，意味着在反序列化时会忽略JSON中任何未在此结构中明确定义的其他字段。
//             这是一种使反序列化更具鲁棒性的常见做法，即使API在未来添加了新字段，旧代码也不会因此中断。
//     -   **`AuctionEnded`**: 当一个MEV拍卖结束时发送的消息。它包含：
//         -   `tx_digest`: 对应的机会交易摘要。
//         -   `winning_bid_amount`: 此次拍卖的获胜竞价金额。
//     -   **`Dummy(Value)`**: 一个“备用”或“未知类型”的成员。如果从WebSocket收到的JSON消息无法被成功反序列化为
//         `AuctionStarted` 或 `AuctionEnded`，则会尝试将其捕获为 `ShioItem::Dummy(Value)`，
//         其中 `Value` 是原始的 `serde_json::Value` 对象。这可以防止程序因遇到未知消息类型而崩溃，并允许后续检查或记录这些未知消息。
//         `From<Value> for ShioItem` 的实现就利用了这个 `Dummy` 成员。
//     -   `ShioItem` 还实现了一些辅助方法，如 `tx_digest()`, `gas_price()`, `deadline_timestamp_ms()`, `events()`, `created_mutated_objects()`，
//         这些方法提供了方便的方式来从不同类型的 `ShioItem` 中提取通用或特定的信息。
//         例如，`tx_digest()` 会根据 `ShioItem` 是 `AuctionStarted` 还是其他类型返回相应的值。
//
// 2.  **`SideEffects` 结构体**:
//     -   用于封装一个交易（特指Shio中的“机会交易”）执行后产生的各种链上状态变更的详细信息。
//     -   `created_objects: Vec<ShioObject>`: 交易执行后新创建的对象列表。
//     -   `mutated_objects: Vec<ShioObject>`: 交易执行后状态被修改的对象列表。
//     -   `gas_usage: u64`: 交易执行消耗的Gas数量。
//     -   `events: Vec<ShioEvent>`: 交易执行过程中发出的Sui事件列表 (这里是 `ShioEvent` 格式)。
//     -   `#[serde(default)]` 属性用于那些在JSON中可能是可选的字段（例如 `created_objects`）。如果JSON中缺少这些字段，
//         它们会被初始化为其类型的默认值（例如 `Vec::new()`）。
//
// 3.  **`ShioObject` 结构体**:
//     -   用于表示在 `SideEffects` 中列出的、被创建或修改的Sui对象的详细信息。
//     -   `id: String`: 对象的ObjectID字符串。
//     -   `object_type: String`: 对象的Move类型字符串。
//     -   `owner: Value`: 对象的所有者信息，以原始 `serde_json::Value` 形式存储，可能需要进一步解析。
//     -   `content: ShioObjectContent`: 包含对象内容的一些元数据，如 `dataType` 和 `hasPublicTransfer`。
//     -   `object_bcs: String`: 对象内容的BCS序列化字节流，经过Base64编码后的字符串。
//         套利机器人可能需要解码这个BCS数据来获取对象的具体字段值，以便进行模拟或分析。
//     -   它也提供了 `data_type()` 和 `has_public_transfer()` 两个辅助方法。
//
// 4.  **`ShioObjectContent` 结构体**:
//     -   存储 `ShioObject` 内容的元数据。
//     -   `data_type: String`: 通常是 "moveObject" 或 "package"。
//     -   `has_public_transfer: bool`: 指示该对象是否具有公共转移能力。
//
// 5.  **`ShioEvent` 结构体**:
//     -   用于表示包含在 `SideEffects` 中的Sui事件。它与Sui SDK的 `SuiEvent` 结构类似，但字段名和某些表示可能特定于Shio API的格式。
//     -   `event_type: String`: 事件的完整类型字符串。
//     -   `bcs: String`: 事件内容的BCS序列化字节流，经过Base64编码。
//     -   `event_id: ShioEventId`: 事件的唯一ID，包含交易摘要和事件序列号。
//     -   `package_id: String`: 发出此事件的Move包的ID。
//     -   `parsed_json: Option<Value>`: (可选) 事件内容的JSON表示形式。
//     -   `sender: String`: 发出此事件的Sui地址。
//     -   `transaction_module: String`: 发出此事件的Move模块的名称。
//
// 6.  **`ShioEventId` 结构体**:
//     -   用于唯一标识一个Sui事件，由产生该事件的交易摘要 (`txDigest`) 和该事件在该交易中的序列号 (`eventSeq`) 组成。
//
// 7.  **`From<Value> for ShioItem` 实现**:
//     -   这个实现使得可以将一个通用的 `serde_json::Value` 对象（通常是从JSON文本解析而来）尝试转换为一个 `ShioItem` 枚举实例。
//     -   它首先尝试将 `Value` 反序列化为 `ShioItem` 的某个已知变体（如 `AuctionStarted`）。
//     -   如果所有已知的反序列化尝试都失败了（即 `Value` 的结构不匹配任何预期的 `ShioItem` 变体），
//         它会捕获这个 `Value` 并将其包装在 `ShioItem::Dummy(value)` 中，而不是直接panic。
//         这使得程序可以优雅地处理来自Shio feed的、未预期的或格式不正确的消息。
//
// **用途 (Purpose in Shio Crate)**:
// 这些类型定义是 `shio` crate 与Shio MEV协议服务器进行有效通信和数据交换的基础。
// 当 `shio_conn` 模块从WebSocket接收到JSON消息时，它会使用这些类型（特别是 `ShioItem::from(value)`)
// 来解析消息内容。然后，解析得到的 `ShioItem` 对象会被发送给 `ShioCollector`，
// 再由后者提供给套利策略模块进行分析和决策。

// 引入 serde 库的 Deserialize trait，用于从如JSON这样的格式反序列化数据。
use serde::Deserialize;
// 引入 serde_json 的 Value 类型，用于表示通用的JSON值。
use serde_json::Value;

/// `ShioItem` 枚举
///
/// 代表从Shio MEV协议的事件流中接收到的不同类型的消息或事件。
/// `#[derive(Debug, Clone, Deserialize)]` 自动为枚举实现Debug, Clone, 和 Deserialize trait。
/// - `Debug`: 允许使用 `{:?}` 格式化打印枚举实例，方便调试。
/// - `Clone`: 允许创建枚举实例的副本。
/// - `Deserialize`: 允许从序列化格式（如JSON）中反序列化填充此枚举的成员。
#[derive(Debug, Clone, Deserialize)]
pub enum ShioItem {
    /// `AuctionStarted` 成员：表示一个MEV拍卖已开始。
    /// `#[serde(rename = "auctionStarted")]` 属性指定在JSON反序列化时，
    /// 如果JSON中的类型字段是 "auctionStarted"，则应映射到此枚举成员。
    #[serde(rename = "auctionStarted")]
    AuctionStarted {
        /// `tx_digest`: 触发此次拍卖的原始“机会交易”的摘要（哈希）字符串。
        #[serde(rename = "txDigest")]
        tx_digest: String,
        /// `gas_price`: 机会交易的Gas价格。
        #[serde(rename = "gasPrice")]
        gas_price: u64,
        /// `deadline_timestamp_ms`: 此拍卖的竞价截止时间戳（毫秒精度的Unix时间戳）。
        #[serde(rename = "deadlineTimestampMs")]
        deadline_timestamp_ms: u64,
        /// `side_effects`: 一个 `SideEffects` 结构，包含了机会交易执行后产生的链上状态变更。
        #[serde(rename = "sideEffects")]
        side_effects: SideEffects,
        /// `_other`: 用于捕获JSON中任何未在此结构中明确定义的额外字段。
        /// `#[serde(skip)]` 表示在序列化和反序列化时忽略此字段。
        /// 这里类型是 `()` (空元组)，意味着它不存储任何额外数据，只是一个占位符，
        /// 以允许 `serde` 在遇到未知字段时不会报错（如果 `serde` 配置为允许未知字段的话）。
        /// （更常见的做法可能是使用 `#[serde(flatten)] other: HashMap<String, Value>` 来捕获未知字段）。
        /// 当前的 `#[serde(skip)] _other: ()` 实际上不会捕获任何未知字段，
        /// 如果JSON中有未知字段且没有全局配置 `#[serde(deny_unknown_fields)]`，它们会被默认忽略。
        #[serde(skip)]
        _other: (),
    },

    /// `AuctionEnded` 成员：表示一个MEV拍卖已结束。
    #[serde(rename = "auctionEnded")]
    AuctionEnded {
        /// `tx_digest`: 对应的机会交易摘要字符串。
        #[serde(rename = "txDigest")]
        tx_digest: String,
        /// `winning_bid_amount`: 此次拍卖的获胜竞价金额。
        #[serde(rename = "winningBidAmount")]
        winning_bid_amount: u64,
    },

    /// `Dummy(Value)` 成员：用于处理无法被解析为其他已知 `ShioItem` 类型的消息。
    /// `#[serde(skip)]` 表示这个成员不会直接从一个名为 "Dummy" 的JSON字段反序列化。
    /// 它的填充逻辑在下面的 `From<Value> for ShioItem` 实现中定义：
    /// 如果一个JSON值无法匹配 `AuctionStarted` 或 `AuctionEnded`，则会被包装成 `Dummy(value)`。
    #[serde(skip)]
    Dummy(Value), // 包含原始的 serde_json::Value
}

impl ShioItem {
    /// `tx_digest` 方法
    ///
    /// 返回与此 `ShioItem` 相关联的交易摘要字符串的引用。
    /// - 对于 `AuctionStarted`，返回其 `tx_digest` 字段。
    /// - 对于 `AuctionEnded`，返回一个固定的字符串 "auctionEnded" (这可能是一个设计选择，或者表示其摘要不重要/不可用)。
    /// - 对于 `Dummy`，返回固定的字符串 "dummy"。
    pub fn tx_digest(&self) -> &str {
        match self {
            ShioItem::AuctionStarted { tx_digest, .. } => tx_digest,
            ShioItem::AuctionEnded { .. } => "auctionEnded", // 或者也应该有 tx_digest 字段？
            ShioItem::Dummy(_) => "dummy",
        }
    }

    /// `gas_price` 方法
    ///
    /// 返回与此 `ShioItem` 相关联的Gas价格。
    /// - 对于 `AuctionStarted`，返回其 `gas_price` 字段。
    /// - 其他类型返回0。
    pub fn gas_price(&self) -> u64 {
        match self {
            ShioItem::AuctionStarted { gas_price, .. } => *gas_price,
            ShioItem::AuctionEnded { .. } => 0, // 拍卖结束事件可能不直接携带原始Gas价格
            ShioItem::Dummy(_) => 0,
        }
    }

    /// `deadline_timestamp_ms` 方法
    ///
    /// 返回与此 `ShioItem` 相关联的截止时间戳。
    /// - 对于 `AuctionStarted`，返回其 `deadline_timestamp_ms` 字段。
    /// - 其他类型返回0。
    pub fn deadline_timestamp_ms(&self) -> u64 {
        match self {
            ShioItem::AuctionStarted {
                deadline_timestamp_ms, ..
            } => *deadline_timestamp_ms,
            ShioItem::AuctionEnded { .. } => 0, // 拍卖结束事件通常没有“未来”的截止时间
            ShioItem::Dummy(_) => 0,
        }
    }

    /// `events` 方法
    ///
    /// 返回与此 `ShioItem` (特指 `AuctionStarted`) 相关的Sui事件列表 (`Vec<ShioEvent>`) 的克隆。
    /// - 对于 `AuctionStarted`，返回其 `side_effects.events` 字段的克隆。
    /// - 其他类型返回一个空向量。
    pub fn events(&self) -> Vec<ShioEvent> {
        match self {
            ShioItem::AuctionStarted { side_effects, .. } => side_effects.events.clone(),
            ShioItem::AuctionEnded { .. } => vec![],
            ShioItem::Dummy(_) => vec![],
        }
    }

    /// `created_mutated_objects` 方法
    ///
    /// 返回一个包含了在 `AuctionStarted` 事件的 `side_effects` 中所有被创建或修改的对象的引用列表 (`Vec<&ShioObject>`)。
    /// - 它将 `side_effects.created_objects` 和 `side_effects.mutated_objects` 两个列表连接起来。
    /// - 其他类型返回一个空向量。
    /// 这个方法对于获取MEV机会交易影响的链上状态非常重要，这些状态可能需要在模拟时被“覆盖”。
    pub fn created_mutated_objects(&self) -> Vec<&ShioObject> {
        match self {
            ShioItem::AuctionStarted { side_effects, .. } => side_effects
                .created_objects // 获取创建对象列表的迭代器
                .iter()
                .chain(&side_effects.mutated_objects) // 连接上修改对象列表的迭代器
                .collect(), // 收集为一个新的 Vec<&ShioObject>
            ShioItem::AuctionEnded { .. } => vec![],
            ShioItem::Dummy(_) => vec![],
        }
    }
}

/// `SideEffects` 结构体
///
/// 描述一个交易（特指Shio中的“机会交易”）执行后产生的各种链上状态变更。
#[derive(Debug, Clone, Deserialize)]
pub struct SideEffects {
    /// `created_objects`: 交易执行后新创建的对象列表。
    /// `#[serde(default)]` 表示如果JSON中缺少此字段，则使用 `Vec::default()` (即空Vec) 初始化。
    #[serde(rename = "createdObjects", default)]
    pub created_objects: Vec<ShioObject>,
    /// `mutated_objects`: 交易执行后状态被修改的对象列表。
    #[serde(rename = "mutatedObjects", default)]
    pub mutated_objects: Vec<ShioObject>,
    /// `gas_usage`: 交易执行消耗的Gas数量。
    #[serde(rename = "gasUsage")]
    pub gas_usage: u64,
    /// `events`: 交易执行过程中发出的Sui事件列表 (这里是 `ShioEvent` 格式)。
    #[serde(rename = "events", default)]
    pub events: Vec<ShioEvent>,
}

/// `ShioObject` 结构体
///
/// 表示一个Sui对象，包含了从Shio API获取的关于该对象的详细信息。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")] // 指示serde在序列化/反序列化时使用驼峰命名法 (e.g., "objectType")
pub struct ShioObject {
    pub id: String,                 // 对象的ObjectID字符串
    pub object_type: String,        // 对象的Move类型字符串
    pub owner: Value,               // 对象的所有者信息 (以原始 `serde_json::Value` 形式存储，需要进一步解析)
    pub content: ShioObjectContent, // 包含对象内容的一些元数据
    pub object_bcs: String,         // 对象内容的BCS序列化字节流，经过Base64编码后的字符串
}

impl ShioObject {
    /// `data_type` 方法
    ///
    /// 返回对象内容的 `dataType` (例如 "moveObject" 或 "package")。
    pub fn data_type(&self) -> &str {
        &self.content.data_type
    }

    /// `has_public_transfer` 方法
    ///
    /// 返回该对象是否具有公共转移能力。
    pub fn has_public_transfer(&self) -> bool {
        self.content.has_public_transfer
    }
}

/// `ShioObjectContent` 结构体
///
/// 存储 `ShioObject` 内容的元数据。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShioObjectContent {
    #[serde(rename = "dataType")] // JSON中的字段名是 "dataType"
    pub data_type: String,        // 数据类型，例如 "moveObject"
    pub has_public_transfer: bool, // 是否具有公共转移能力
}

/// `ShioEvent` 结构体
///
/// 表示一个Sui事件，字段名和格式可能特定于Shio API的返回。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShioEvent {
    /// `event_type`: 事件的完整类型字符串 (例如 "0xPKG::module::EventName<TypeParam>")。
    #[serde(rename = "type")] // JSON中的字段名是 "type"
    pub event_type: String,
    /// `bcs`: 事件内容的BCS序列化字节流，经过Base64编码。
    #[serde(rename = "bcs")]
    pub bcs: String,
    /// `event_id`: 事件的唯一ID。
    #[serde(rename = "id")]
    pub event_id: ShioEventId,
    /// `package_id`: 发出此事件的Move包的ID字符串。
    #[serde(rename = "packageId")]
    pub package_id: String,
    /// `parsed_json`: (可选) 事件内容的JSON表示形式。
    /// `#[serde(default)]` 表示如果JSON中缺少此字段，则使用 `Option::default()` (即 `None`) 初始化。
    #[serde(rename = "parsedJson", default)]
    pub parsed_json: Option<Value>,
    /// `sender`: 发出此事件的Sui地址字符串。
    #[serde(rename = "sender")]
    pub sender: String,
    /// `transaction_module`: 发出此事件的Move模块的名称字符串。
    #[serde(rename = "transactionModule")]
    pub transaction_module: String,
}

/// `ShioEventId` 结构体
///
/// 用于唯一标识一个Sui事件，由产生该事件的交易摘要和事件在该交易中的序列号组成。
#[derive(Debug, Clone, Deserialize)]
pub struct ShioEventId {
    /// `event_seq`: 事件在该交易中的序列号 (字符串形式的数字)。
    #[serde(rename = "eventSeq")]
    pub event_seq: String,
    /// `tx_digest`: 产生该事件的交易的摘要字符串。
    #[serde(rename = "txDigest")]
    pub tx_digest: String,
}

/// 为 `ShioItem` 实现 `From<Value>` trait。
/// 这允许将一个通用的 `serde_json::Value` 对象尝试转换为一个 `ShioItem`。
/// 如果 `serde_json::from_value` 能够成功地将 `value` 反序列化为 `ShioItem` 的某个已知变体
/// (如 `AuctionStarted` 或 `AuctionEnded`，这依赖于 `value` 的内部结构和 `ShioItem` 的 `#[serde(rename = ...)]` 属性)，
/// 则返回该变体。
/// 如果反序列化失败（例如 `value` 的结构不匹配任何已知变体），则捕获原始的 `value` 并将其包装在 `ShioItem::Dummy(value)` 中返回。
/// 这样可以避免程序因遇到未知或格式错误的消息而panic。
impl From<Value> for ShioItem {
    fn from(value: Value) -> Self {
        // 尝试将 value 反序列化为 ShioItem 的某个具体变体。
        // `value.clone()` 是因为 `from_value` 需要所有权，但原始 `value` 可能还需用于 Dummy。
        serde_json::from_value(value.clone()).unwrap_or(ShioItem::Dummy(value))
    }
}

impl ShioItem {
    /// `type_name` 方法
    ///
    /// 返回一个表示 `ShioItem` 具体变体类型的字符串字面量。
    /// 主要用于日志记录或调试。
    pub fn type_name(&self) -> &str {
        match self {
            ShioItem::AuctionStarted { .. } => "auctionStarted", // 如果是拍卖开始事件
            ShioItem::AuctionEnded { .. } => "auctionEnded",   // 如果是拍卖结束事件
            ShioItem::Dummy(_) => "dummy",                     // 如果是未知或Dummy事件
        }
    }
}

[end of crates/shio/src/types.rs]
