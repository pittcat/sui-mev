// 该文件 `types.rs` 的主要作用是定义整个套利机器人应用程序中会用到的多种核心数据类型。
// 特别地，它定义了与 `burberry` 事件驱动引擎（这可能是一个内部框架或库）紧密相关的 `Event` (事件) 和 `Action` (动作) 枚举。
// 这些枚举类型是机器人不同组件之间（例如数据收集器、策略分析器、交易执行器）进行通信和数据传递的契约。
// 此外，该文件还定义了 `Source` (来源) 枚举，用于追踪和管理套利机会的来源及其特定属性，
// 例如，一个机会是来自公开市场还是来自像Shio这样的MEV（矿工可提取价值）协议，以及相关的竞价信息和时间戳。
//
// **文件概览 (File Overview)**:
// 这个 `types.rs` 文件就像是套利机器人的“字典”或“词汇表”。它定义了机器人内部交流时使用的各种“词语”（数据类型）的含义和结构。
// 这非常重要，因为它确保了机器人的不同部分（比如负责听消息的“耳朵”、负责做决定的“大脑”、负责行动的“手脚”）能够准确地理解对方在说什么。
//
// 主要内容 (Main Contents):
//
// -   **`Action` 枚举 (动作 / Action Enum)**:
//     这个枚举定义了当机器人的“大脑”（套利策略模块）分析完情况并决定要做某件事情时，它可以下达哪些类型的“指令”。
//     (This enum defines the types of "commands" that the bot's "brain" (arbitrage strategy module) can issue after analyzing a situation and deciding to do something.)
//     例如 (For example):
//     -   `NotifyViaTelegram`: “发个Telegram消息！”（比如报告赚了多少钱，或者哪里出错了）。
//         ("Send a Telegram message!" (e.g., to report profits or errors).)
//     -   `ExecutePublicTx`: “去Sui链上执行这笔公开交易！”（这通常是实际的套利操作）。
//         ("Execute this public transaction on the Sui chain!" (This is usually the actual arbitrage operation).)
//     -   `ShioSubmitBid`: “针对这个Shio MEV机会，提交一个竞价！”（MEV是指“矿工可提取价值”，Shio是一个帮助机器人参与MEV竞价的协议）。
//         ("Submit a bid for this Shio MEV opportunity!" (MEV means "Miner Extractable Value"; Shio is a protocol that helps bots participate in MEV bidding).)
//     为了让代码写起来更方便，文件还为 `Action` 实现了一些叫做 `From` trait 的东西。
//     这就像是提供了“快速转换模板”，可以很容易地把一个具体的数据（比如一笔交易 `TransactionData`）直接变成一个 `Action` 指令。
//     (To make coding more convenient, the file also implements something called `From` traits for `Action`. This is like providing "quick conversion templates" to easily turn specific data (like a `TransactionData`) directly into an `Action` command.)
//
// -   **`Event` 枚举 (事件 / Event Enum)**:
//     这个枚举定义了机器人可以从外部世界接收到的、各种不同类型的“消息”或“状况报告”。
//     (This enum defines the various types of "messages" or "situation reports" that the bot can receive from the outside world.)
//     这些“事件”通常由机器人的“耳朵”（数据收集器模块）捕捉到。例如 (These "events" are usually captured by the bot's "ears" (data collector modules). For example):
//     -   `PublicTx`: “Sui链上刚发生了一笔公开交易，这是它的详细结果！”
//         ("A public transaction just occurred on the Sui chain, here are its details!")
//     -   `PrivateTx`: “我通过秘密渠道收到了一笔还没公开的交易信息！”（这通常和MEV有关）。
//         ("I received information about a transaction through a secret channel that isn't public yet!" (This is often related to MEV).)
//     -   `Shio`: “Shio MEV协议那边发来了一个新的套利机会！”
//         ("The Shio MEV protocol sent over a new arbitrage opportunity!")
//     代码里有一个 `#[allow(clippy::large_enum_variant)]` 的注解，它是写给一个叫做Clippy的“代码风格警察”看的。
//     意思是：“我知道这个 `Event` 枚举里，有些类型的事件可能比其他类型的事件占内存大很多，但这是业务需要，请不要警告我。”
//     (There's an annotation `#[allow(clippy::large_enum_variant)]` in the code, which is for a "code style police" tool called Clippy. It means: "I know some event types in this `Event` enum might take up much more memory than others, but it's necessary for the business logic, so please don't warn me.")
//
// -   **`Source` 枚举 (来源 / Source Enum)**:
//     这个枚举用来详细记录一个套利机会是从哪里来的，以及和这个来源相关的特定信息。
//     (This enum is used to meticulously record where an arbitrage opportunity came from and specific information related to that source.)
//     这对于机器人做后续决定（比如要不要尝试这个机会？如果是MEV机会，什么时候截止？出多少价合适？）非常重要。
//     (This is crucial for the bot's subsequent decision-making (e.g., should I try this opportunity? If it's an MEV opportunity, when is the deadline? How much should I bid?).)
//     例如 (For example):
//     -   `Public`: 机会来自公开市场数据。 (The opportunity comes from public market data.)
//     -   `Shio`: 机会来自Shio MEV协议。这种情况下，它会额外记录：
//         (The opportunity comes from the Shio MEV protocol. In this case, it additionally records:)
//         -   原始机会交易的“指纹”（`opp_tx_digest`）。 (The "fingerprint" (digest) of the original opportunity transaction.)
//         -   机器人打算出的MEV竞价金额（`bid_amount`）。 (The MEV bid amount the bot intends to offer.)
//         -   一系列重要时间点：机器人开始处理机会的时间（`start`）、找到套利方案的时间（`arb_found`）、以及这个机会的竞价截止时间（`deadline`）。
//             (A series of important timestamps: when the bot started processing the opportunity (`start`), when it found an arbitrage solution (`arb_found`), and the bidding deadline for this opportunity (`deadline`).)
//     -   `ShioDeadlineMissed`: 一个特殊状态，表示虽然从Shio发现了一个机会，但机器人算得太慢，错过了竞价截止时间。
//         (A special state indicating that although an opportunity was found via Shio, the bot was too slow in its calculations and missed the bidding deadline.)
//     `Source` 枚举还有一些辅助“小工具”函数，方便检查它是什么类型的来源、获取里面的数据，或者根据新算出来的竞价金额或发现时间来更新它自己。
//     (The `Source` enum also has some auxiliary "helper" functions to conveniently check its type, access its data, or update itself based on newly calculated bid amounts or discovery times.)
//     它还能把自己格式化成一段方便人阅读的文字，这样在看日志的时候就能清楚地知道每个机会的来龙去脉。
//     (It can also format itself into human-readable text, so when looking at logs, one can clearly understand the context of each opportunity.)
//
// 总之，这个文件定义的这些“词汇”，是套利机器人内部各个部件高效、准确沟通的基础。
// (In summary, these "vocabulary" items defined in this file are fundamental for efficient and accurate communication between the various components of the arbitrage bot.)

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::fmt; // `fmt` 模块用于格式化输出，例如实现 `Display` trait 使自定义类型能被友好地打印。
              // The `fmt` module is used for formatted output, e.g., implementing the `Display` trait to allow custom types to be printed nicely.

use burberry::executor::telegram_message::Message as BurberryTelegramMessage; // 从 `burberry` 框架（可能是公司内部的）的 `executor::telegram_message` 模块引入 `Message` 类型，
                                                                            // 并使用 `as BurberryTelegramMessage` 将其重命名，以避免与当前或其他库中可能存在的同名 `Message` 类型发生冲突。
                                                                            // Import the `Message` type from the `burberry` framework's (possibly internal) `executor::telegram_message` module,
                                                                            // and rename it using `as BurberryTelegramMessage` to avoid naming conflicts with other `Message` types.
use shio::ShioItem; // 从 `shio` crate (外部库或子模块) 引入 `ShioItem` 类型。
                    // `ShioItem` 可能代表了从Shio MEV协议获取到的一个具体套利机会的原始数据结构。
                    // Import the `ShioItem` type from the `shio` crate (external library or submodule).
                    // `ShioItem` likely represents the raw data structure of a specific arbitrage opportunity obtained from the Shio MEV protocol.
use sui_json_rpc_types::{SuiEvent, SuiTransactionBlockEffects}; // 从 `sui_json_rpc_types` 库引入Sui RPC（远程过程调用）接口定义的相关类型：
                                                              // `SuiEvent` 代表Sui区块链上发生的一个标准事件。
                                                              // `SuiTransactionBlockEffects` 代表一笔交易在链上执行后产生的详细效果（状态变更、对象创建/删除等），这是RPC通常返回的格式。
                                                              // Import types related to the Sui RPC (Remote Procedure Call) interface from the `sui_json_rpc_types` library:
                                                              // `SuiEvent` represents a standard event occurring on the Sui blockchain.
                                                              // `SuiTransactionBlockEffects` represents the detailed effects produced after a transaction is executed on-chain (state changes, object creation/deletion, etc.), which is the format RPC usually returns.
use sui_types::{
    digests::TransactionDigest, // 从 `sui_types` 核心库引入 `TransactionDigest`，它是一个交易的唯一哈希标识符。
                                // Import `TransactionDigest` from the `sui_types` core library; it's a unique hash identifier for a transaction.
    transaction::TransactionData, // 同样从 `sui_types` 引入 `TransactionData`，它代表了一笔Sui交易的完整内容和结构（发送者、交易指令、gas信息等）。
                                  // Also import `TransactionData` from `sui_types`; it represents the complete content and structure of a Sui transaction (sender, transaction instructions, gas information, etc.).
};

/// `Action` 枚举 (动作 / Action Enum)
///
/// 这个枚举定义了套利策略在分析完输入事件（`Event`）后，可能会决定执行的各种不同类型的输出操作。
/// (This enum defines the various types of output operations that the arbitrage strategy might decide to execute after analyzing input events (`Event`).)
/// 这些 `Action` 对象随后会被发送给相应的“执行器”（Executor）模块，由执行器负责与外部世界（如Sui链、Telegram API、MEV中继）进行实际的交互。
/// (These `Action` objects are then sent to the corresponding "Executor" modules, which are responsible for actual interaction with the external world (like the Sui chain, Telegram API, MEV relays).)
/// 把它看作是策略模块发出的“指令单”。
/// (Think of it as a "command sheet" issued by the strategy module.)
#[derive(Debug, Clone)] // `derive` 属性宏自动为 `Action` 实现一些有用的trait：
                         // (`derive` attribute macro automatically implements some useful traits for `Action`:)
                         // `Debug`: 允许使用 `{:?}` 格式化打印 `Action` 实例，方便调试。
                         //          (Allows printing `Action` instances using `{:?}` formatting for debugging.)
                         // `Clone`: 允许创建 `Action` 实例的深拷贝副本。
                         //          (Allows creating deep copy replicas of `Action` instances.)
pub enum Action {
    /// **动作类型：通过Telegram发送一条通知消息。**
    /// (Action Type: Send a notification message via Telegram.)
    /// 当机器人需要向用户报告状态、成功套利、发生错误或其他重要信息时，可能会产生这个动作。
    /// (This action might be produced when the bot needs to report status, successful arbitrage, errors, or other important information to the user.)
    /// 它包含一个 `BurberryTelegramMessage` 类型的实例，这个实例内部应该封装了要发送的消息内容、接收者等信息。
    /// (It contains an instance of `BurberryTelegramMessage`, which should encapsulate the message content, recipient, etc.)
    NotifyViaTelegram(BurberryTelegramMessage),

    /// **动作类型：在Sui区块链上执行一笔公开的交易。**
    /// (Action Type: Execute a public transaction on the Sui blockchain.)
    /// 这通常是执行实际套利操作的动作。
    /// (This is usually the action for performing the actual arbitrage operation.)
    /// 它包含一个 `TransactionData` 类型的实例，这个 `TransactionData` 应该是一笔已经构建好、并且通常是已经签过名的Sui交易，可以直接提交到链上。
    /// (It contains an instance of `TransactionData`, which should be a fully constructed, and usually already signed, Sui transaction ready for submission to the chain.)
    ExecutePublicTx(TransactionData),

    /// **动作类型：向Shio MEV协议提交一个MEV竞价。**
    /// (Action Type: Submit an MEV bid to the Shio MEV protocol.)
    /// 当套利机会来源于Shio协议，并且策略决定参与竞价时，会产生这个动作。
    /// (This action is produced when an arbitrage opportunity originates from the Shio protocol and the strategy decides to participate in bidding.)
    /// 它包含一个元组 `(TransactionData, u64, TransactionDigest)`，其中：
    /// (It contains a tuple `(TransactionData, u64, TransactionDigest)`, where:)
    ///   - `TransactionData`: 这是套利者构建的“套利交易”。这笔交易不仅会执行套利操作，
    ///                        通常还会包含支付给区块生产者的MEV竞价金额（“小费”）。
    ///                        (This is the "arbitrage transaction" constructed by the arbitrageur. This transaction not only performs the arbitrage but also usually includes the MEV bid amount ("tip") paid to the block producer.)
    ///   - `u64`: 明确的竞价金额（以SUI的最小单位MIST计算）。这部分金额会作为小费给区块生产者。
    ///            (The explicit bid amount (calculated in MIST, SUI's smallest unit). This amount is given as a tip to the block producer.)
    ///   - `TransactionDigest`: 这是Shio机会所对应的原始“机会交易”的摘要（哈希）。
    ///                          提交竞价时通常需要指明是针对哪个原始机会的。
    ///                          (This is the digest (hash) of the original "opportunity transaction" corresponding to the Shio opportunity. When submitting a bid, it's usually necessary to specify which original opportunity it targets.)
    ShioSubmitBid((TransactionData, u64, TransactionDigest)),
}

// --- 为 Action 枚举实现 From trait ---
// (Implement From trait for Action enum)
// `From<T> for U` 是Rust中的一个标准trait，它允许类型T的值被转换成类型U的值。
// (`From<T> for U` is a standard trait in Rust that allows a value of type T to be converted into a value of type U.)
// 在这里，我们为 `Action` 枚举实现多个 `From` trait，
// 目的是让创建 `Action` 枚举实例的过程更加简洁和符合人体工程学。
// (Here, we implement multiple `From` traits for the `Action` enum
// to make the process of creating `Action` enum instances more concise and ergonomic.)
// 例如，如果你有一个 `BurberryTelegramMessage` 对象 `msg`，你可以直接写 `Action::from(msg)`
// 或者在类型推断允许的情况下更简单地写 `msg.into()` 来得到一个 `Action::NotifyViaTelegram(msg)`。
// (For example, if you have a `BurberryTelegramMessage` object `msg`, you can directly write `Action::from(msg)`
// or, where type inference allows, more simply `msg.into()` to get an `Action::NotifyViaTelegram(msg)`.)

/// 将 `BurberryTelegramMessage` 转换为 `Action`。
/// (Convert `BurberryTelegramMessage` to `Action`.)
impl From<BurberryTelegramMessage> for Action {
    fn from(msg: BurberryTelegramMessage) -> Self {
        // 将传入的 `msg` (一个 `BurberryTelegramMessage` 实例)
        // 包装进 `Action::NotifyViaTelegram` 枚举成员中。
        // (Wrap the incoming `msg` (an instance of `BurberryTelegramMessage`)
        // into the `Action::NotifyViaTelegram` enum member.)
        Self::NotifyViaTelegram(msg)
    }
}

/// 将 `TransactionData` 转换为 `Action`。
/// (Convert `TransactionData` to `Action`.)
impl From<TransactionData> for Action {
    fn from(tx_data: TransactionData) -> Self {
        // 将传入的 `tx_data` (一个 `TransactionData` 实例)
        // 包装进 `Action::ExecutePublicTx` 枚举成员中。
        // (Wrap the incoming `tx_data` (an instance of `TransactionData`)
        // into the `Action::ExecutePublicTx` enum member.)
        Self::ExecutePublicTx(tx_data)
    }
}

/// 将元组 `(TransactionData, u64, TransactionDigest)` 转换为 `Action`。
/// (Convert tuple `(TransactionData, u64, TransactionDigest)` to `Action`.)
impl From<(TransactionData, u64, TransactionDigest)> for Action {
    fn from(data: (TransactionData, u64, TransactionDigest)) -> Self {
        // 将传入的整个元组 `data`
        // 包装进 `Action::ShioSubmitBid` 枚举成员中。
        // (Wrap the entire incoming tuple `data`
        // into the `Action::ShioSubmitBid` enum member.)
        Self::ShioSubmitBid(data)
    }
}


/// `Event` 枚举 (事件 / Event Enum)
///
/// 这个枚举定义了套利策略模块可以从外部（通常是“收集器”模块）接收和处理的各种不同类型的输入信息。
/// (This enum defines the various types of input information that the arbitrage strategy module can receive and process from the outside (usually from "collector" modules).)
/// 每种事件都代表了外部世界发生的一些值得策略关注的变化。
/// (Each type of event represents some change in the external world that is worthy of the strategy's attention.)
///
/// `#[allow(clippy::large_enum_variant)]` 属性:
/// (Attribute `#[allow(clippy::large_enum_variant)]`:)
/// 这是对Clippy（Rust的静态代码分析工具）发出的一个警告的忽略。
/// (This is to ignore a warning issued by Clippy (Rust's static code analysis tool).)
/// `large_enum_variant` 警告会在一个枚举类型中，如果某些成员（变体）在内存中占用的空间
/// 显著大于其他成员时触发。例如，如果 `PublicTx` 成员因为内含 `SuiTransactionBlockEffects`
/// 和 `Vec<SuiEvent>` 而占用比如1KB内存，而 `PrivateTx` 因为只含一个 `TransactionData` （可能较小）
/// 只占用100B内存，Clippy就会警告。
/// (The `large_enum_variant` warning is triggered when some members (variants) in an enum type occupy
/// significantly more memory space than others. For example, if the `PublicTx` member, containing `SuiTransactionBlockEffects`
/// and `Vec<SuiEvent>`, takes up 1KB of memory, while `PrivateTx`, containing only a `TransactionData` (possibly smaller),
/// takes up only 100B, Clippy will issue a warning.)
/// 这是因为Rust枚举的大小由其最大的成员决定。如果大小差异悬殊，可能会导致内存使用的浪费（当枚举实例是较小成员时）。
/// (This is because the size of a Rust enum is determined by its largest member. If the size difference is substantial, it might lead to wasted memory usage (when the enum instance is one of the smaller members).)
/// 然而，对于像 `Event` 这样需要聚合不同来源、不同大小数据的类型来说，这种大小差异是很常见的，
/// 并且通常是可以接受的设计。因此，这里使用 `#[allow(...)]` 来显式告诉编译器和Clippy，我们已经意识到了这个问题并且接受它。
/// (However, for types like `Event` that need to aggregate data of different origins and sizes, such size differences are common
/// and usually an acceptable design choice. Therefore, `#[allow(...)]` is used here to explicitly tell the compiler and Clippy that we are aware of this issue and accept it.)
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)] // 自动实现 `Clone` (用于创建副本) 和 `Debug` (用于调试打印) trait。
                         // (Automatically implement `Clone` (for creating copies) and `Debug` (for debug printing) traits.)
pub enum Event {
    /// **事件类型：观察到一笔新的公开交易及其执行效果。**
    /// (Event Type: A new public transaction and its execution effects were observed.)
    /// 当一个收集器（例如 `PublicTxCollector`）监听到链上有一笔新的交易被确认，
    /// 并且获得了它的执行结果时，就会产生这种类型的事件。
    /// (This type of event is produced when a collector (e.g., `PublicTxCollector`) listens to a new transaction being confirmed on-chain
    /// and obtains its execution results.)
    /// 它包含两个部分 (It contains two parts)：
    ///   - `SuiTransactionBlockEffects`: 这笔交易的详细执行效果，例如哪些对象被创建/修改/删除，gas消耗等。
    ///                                   (Detailed execution effects of this transaction, e.g., which objects were created/modified/deleted, gas consumption, etc.)
    ///   - `Vec<SuiEvent>`: 一个列表，包含了这笔交易在执行过程中发出的所有Sui原生事件（由智能合约定义和触发的）。
    ///                      (A list containing all Sui native events emitted during the execution of this transaction (defined and triggered by smart contracts).)
    PublicTx(SuiTransactionBlockEffects, Vec<SuiEvent>),

    /// **事件类型：接收到一笔新的私有交易数据。**
    /// (Event Type: New private transaction data was received.)
    /// 当一个收集器（例如 `PrivateTxCollector`）从MEV中继或其他私有渠道接收到一笔尚未公开的交易时，
    /// 会产生这种类型的事件。
    /// (This type of event is produced when a collector (e.g., `PrivateTxCollector`) receives a yet-to-be-publicized transaction from an MEV relay or other private channels.)
    /// 它包含一个 `TransactionData` 实例。这通常是交易的原始、未签名或已签名但未广播的数据。
    /// (It contains a `TransactionData` instance. This is usually the raw, unsigned, or signed-but-not-broadcast data of the transaction.)
    /// 套利机器人可能会分析这些私有交易，看是否有可以利用的MEV机会（例如抢跑或跟随）。
    /// (The arbitrage bot might analyze these private transactions to see if there are exploitable MEV opportunities (e.g., front-running or back-running).)
    PrivateTx(TransactionData),

    /// **事件类型：从Shio MEV协议获得一个新的潜在套利机会。**
    /// (Event Type: A new potential arbitrage opportunity was obtained from the Shio MEV protocol.)
    /// 当一个专门的收集器监听到Shio协议发布了新的MEV机会时，会产生这种事件。
    /// (This event is produced when a dedicated collector listens to the Shio protocol publishing new MEV opportunities.)
    /// 它包含一个 `ShioItem` 实例，该实例应该封装了Shio机会的所有相关信息，
    /// 例如机会交易的内容、预期的利润、相关的交易池等。
    /// (It contains a `ShioItem` instance, which should encapsulate all relevant information about the Shio opportunity,
    /// such as the content of the opportunity transaction, expected profit, related trading pools, etc.)
    Shio(ShioItem),
}

/// `Source` 枚举 (套利机会来源 / Source Enum for Arbitrage Opportunity)
///
/// 这个枚举用于精确地描述一个被发现的套利机会是从哪里来的，并且携带与该来源相关的特定上下文信息。
/// (This enum is used to precisely describe where a discovered arbitrage opportunity originates from and carries specific contextual information related to that source.)
/// 这对于后续的决策（例如，这个机会是否值得尝试？如果是MEV机会，截止时间是什么？竞价应该多少？）至关重要。
/// (This is crucial for subsequent decision-making (e.g., is this opportunity worth trying? If it's an MEV opportunity, what's the deadline? How much should be bid?).)
///
/// `#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]` 自动实现了一系列有用的trait:
/// (`#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]` automatically implements a series of useful traits:)
/// - `Debug`: 用于调试打印。 (For debug printing.)
/// - `Clone`: 用于创建副本。 (For creating copies.)
/// - `Copy`: 表示这个枚举类型是“可复制”的。这意味着当它被赋值或传递给函数时，会进行按位复制，而不是移动所有权。
///           只有当类型的所有成员都是 `Copy` 的时候，这个类型才能是 `Copy` 的。`TransactionDigest` 和 `u64` 都是 `Copy` 的。
///           这使得处理 `Source` 对象更方便，因为不需要担心所有权问题。
///           (Indicates this enum type is "copyable." This means when it's assigned or passed to a function, a bitwise copy occurs instead of moving ownership.
///            A type can only be `Copy` if all its members are `Copy`. Both `TransactionDigest` and `u64` are `Copy`.
///            This makes handling `Source` objects more convenient as ownership issues are alleviated.)
/// - `Hash`: 允许 `Source` 实例被用作哈希表（如 `HashMap`）的键。
///           (Allows `Source` instances to be used as keys in hash tables (like `HashMap`).)
/// - `PartialEq`: 允许比较两个 `Source` 实例是否相等。
///                (Allows comparison of two `Source` instances for equality.)
/// - `Eq`: `PartialEq` 的一个更严格版本，表示相等关系是自反、对称和传递的。
///         (A stricter version of `PartialEq`, indicating that the equality relation is reflexive, symmetric, and transitive.)
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Source {
    /// **来源：公开市场活动 (Source: Public Market Activity)**
    /// 表示这个套利机会是通过分析公开的链上数据（例如DEX的公开交易池状态、公开的交易流）发现的。
    /// (Indicates this arbitrage opportunity was discovered by analyzing public on-chain data (e.g., public DEX pool states, public transaction streams).)
    /// 这种来源通常没有特殊的截止时间或竞价要求，套利者可以直接尝试执行。
    /// (This type of source usually has no special deadlines or bidding requirements; arbitrageurs can try to execute directly.)
    /// 它不携带额外的特定信息。
    /// (It carries no additional specific information.)
    Public,

    /// **来源：Shio MEV协议 (Source: Shio MEV Protocol)**
    /// 表示这个套利机会是通过Shio MEV协议发现的。Shio是一个专门用于Sui生态的MEV解决方案。
    /// (Indicates this arbitrage opportunity was discovered via the Shio MEV protocol. Shio is an MEV solution specifically for the Sui ecosystem.)
    /// 这种来源的套利机会通常与MEV竞价相关，并且有严格的时间限制。
    /// (Arbitrage opportunities from this source are usually related to MEV bidding and have strict time limits.)
    /// 它包含以下与MEV竞价相关的详细信息 (It contains the following details related to MEV bidding)：
    Shio {
        /// `opp_tx_digest` (Opportunity Transaction Digest / 机会交易的摘要):
        /// Shio机会所对应的原始“机会发起交易”的摘要（哈希）。
        /// (The digest (hash) of the original "opportunity-initiating transaction" corresponding to the Shio opportunity.)
        /// MEV搜索者通常会监听链上的交易，当发现某个交易（机会发起交易）创造了一个潜在的MEV机会时（例如，一个大的DEX交易导致价格显著波动），
        /// Shio协议可能会将这个机会广播出来。这个摘要就是那个原始交易的ID。
        /// (MEV searchers typically monitor on-chain transactions. When a transaction (opportunity-initiating transaction) is found to create a potential MEV opportunity (e.g., a large DEX trade causing significant price volatility),
        /// the Shio protocol might broadcast this opportunity. This digest is the ID of that original transaction.)
        opp_tx_digest: TransactionDigest,

        /// `bid_amount` (Bid Amount / 竞价金额):
        /// 当前机器人为这个Shio机会计算出的或已经提交的MEV竞价金额（通常以SUI的最小单位MIST表示）。
        /// (The MEV bid amount (usually in MIST, SUI's smallest unit) that the bot has calculated or already submitted for this Shio opportunity.)
        /// 这是套利者愿意支付给区块生产者的“小费”，以换取自己的套利交易被优先打包并成功执行。
        /// (This is the "tip" the arbitrageur is willing to pay the block producer in exchange for their arbitrage transaction being prioritized for packaging and successful execution.)
        /// 初始时可能为0，在套利计算完成后会被更新。
        /// (May be initially 0 and updated after arbitrage calculation is complete.)
        bid_amount: u64,

        /// `start` (Start Timestamp / 开始时间戳):
        /// 机器人开始处理这个Shio机会的时间戳（通常是毫秒精度的Unix时间戳）。
        /// (Timestamp (usually millisecond-precision Unix timestamp) when the bot started processing this Shio opportunity.)
        /// 用于追踪处理延迟和机会的生命周期。
        /// (Used for tracking processing delays and the lifecycle of the opportunity.)
        start: u64,

        /// `arb_found` (Arbitrage Found Timestamp / 套利发现时间戳):
        /// 机器人实际找到一个可行的套利方案（并计算出利润和交易路径）的时间戳。
        /// (Timestamp when the bot actually found a viable arbitrage solution (and calculated profit and transaction path).)
        /// 这个时间戳与 `start` 和 `deadline` 一起，可以用来评估机器ンの反应速度和机会的紧迫性。
        /// (This timestamp, along with `start` and `deadline`, can be used to evaluate the bot's reaction speed and the urgency of the opportunity.)
        /// 初始时可能为0或某个哨兵值，在找到套利后被更新。
        /// (May be initially 0 or some sentinel value, updated after finding arbitrage.)
        arb_found: u64,

        /// `deadline` (Deadline Timestamp / 截止时间戳):
        /// 此Shio机会的竞价截止时间戳。
        /// (Bidding deadline timestamp for this Shio opportunity.)
        /// 套利者的竞价交易必须在这个时间点之前被区块生产者接收并考虑，否则就会失效。
        /// (The arbitrageur's bidding transaction must be received and considered by the block producer before this time, otherwise it becomes invalid.)
        /// 这是一个非常关键的时间点，机器人必须在此之前完成所有计算和提交操作。
        /// (This is a very critical time point; the bot must complete all calculations and submission operations before this.)
        deadline: u64,
    },

    /// **来源：Shio MEV机会已错过截止时间 (Source: Shio MEV Opportunity Deadline Missed)**
    /// 这是一个特殊的状态，表示虽然机器人从Shio协议收到了一个机会，并可能进行了套利计算，
    /// 但是当计算完成（即 `arb_found` 时间确定）时，已经晚于该机会的 `deadline`。
    /// (This is a special state indicating that although the bot received an opportunity from the Shio protocol and may have performed arbitrage calculations,
    /// by the time the calculation was completed (i.e., `arb_found` time was determined), it was already past the opportunity's `deadline`.)
    /// 这种状态主要用于内部记录、统计和分析错过的机会，以便优化机器人的性能或策略。
    /// (This state is mainly used for internal recording, statistics, and analysis of missed opportunities to optimize the bot's performance or strategy.)
    /// 它包含以下时间戳信息 (It contains the following timestamp information)：
    ShioDeadlineMissed {
        start: u64,     // 开始处理这个（最终被错过的）机会的时间戳。(Timestamp when processing of this (eventually missed) opportunity began.)
        arb_found: u64, // 机器人发现套利方案的时间戳（但此时 `arb_found >= deadline`）。(Timestamp when the bot found the arbitrage solution (but at this time `arb_found >= deadline`).)
        deadline: u64,  // 已经过去了的竞价截止时间戳。(The bidding deadline timestamp that has already passed.)
    },
}

/// 为 `Source` 枚举实现 `fmt::Display` trait。
/// (Implement `fmt::Display` trait for the `Source` enum.)
/// 这使得 `Source` 对象可以通过标准的格式化宏（如 `println!("{}", source_instance);`）
/// 输出为人类可读的字符串，这对于日志记录和调试非常有用。
/// (This allows `Source` objects to be output as human-readable strings via standard formatting macros (e.g., `println!("{}", source_instance);`), which is very useful for logging and debugging.)
impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // 如果来源是 Public (If the source is Public)
            Source::Public => write!(f, "来源: 公开市场 (Source: Public Market)"), // 直接输出 "来源: 公开市场" (Directly output "Source: Public Market")

            // 如果来源是 Shio (且未错过截止时间) (If the source is Shio (and deadline not missed))
            Source::Shio {
                start,       // 开始处理时间 (Start processing time)
                arb_found,   // 套利发现时间 (Arbitrage found time)
                deadline,    // 竞价截止时间 (Bidding deadline)
                bid_amount,  // 当前竞价金额 (Current bid amount)
                opp_tx_digest, // 机会交易摘要 (这里没有直接打印，但可以通过Debug格式查看)
                               // (Opportunity transaction digest (not printed directly here, but can be viewed via Debug format))
            } => write!( // 使用 `write!` 宏构建格式化的字符串 (Use the `write!` macro to build a formatted string)
                f,
                // 输出详细信息，包括各个时间戳、计算出的时间窗口、距离截止时间的提前量以及竞价金额。
                // (Output detailed information, including various timestamps, calculated time window, lead time before deadline, and bid amount.)
                "来源: Shio MEV (机会摘要前缀: {}, 开始时间戳={}, 截止时间戳={}, 总时间窗口={}ms, 套利发现于戳={}, 距截止提前={}ms, 当前出价={}) \
                (Source: Shio MEV (Opp Digest Prefix: {}, StartTs={}, DeadlineTs={}, TotalWindow={}ms, ArbFoundTs={}, LeadTime={}ms, CurrentBid={}))",
                opp_tx_digest.to_string().chars().take(8).collect::<String>(), // 显示机会交易摘要的前8个字符作为标识 (Display the first 8 chars of opp tx digest as identifier)
                *start,
                *deadline,
                (*deadline).saturating_sub(*start), // 计算总时间窗口 (deadline - start)，使用 saturating_sub 防止时间戳顺序问题导致的下溢变成一个巨大的正数。
                                                    // (Calculate total time window (deadline - start), using saturating_sub to prevent underflow to a large positive number due to timestamp order issues.)
                *arb_found,
                (*deadline).saturating_sub(*arb_found), // 计算从发现套利到截止时间的剩余提前量。
                                                        // (Calculate remaining lead time from arbitrage discovery to deadline.)
                *bid_amount // 显示当前的竞价金额。(Display the current bid amount.)
            ),

            // 如果来源是 ShioDeadlineMissed (已错过截止时间) (If the source is ShioDeadlineMissed (deadline missed))
            Source::ShioDeadlineMissed {
                start,
                arb_found,
                deadline,
            } => write!(
                f,
                // 输出类似的信息，但强调机会已错过，并计算已逾期的时间。
                // (Output similar information, but emphasize that the opportunity was missed and calculate the overdue time.)
                "来源: Shio MEV机会已错过 (开始时间戳={}, 截止时间戳={}, 总时间窗口={}ms, 套利发现于戳={}, 已逾期={}ms) \
                (Source: Shio MEV Opportunity Missed (StartTs={}, DeadlineTs={}, TotalWindow={}ms, ArbFoundTs={}, OverdueBy={}ms))",
                *start,
                *deadline,
                (*deadline).saturating_sub(*start), // 总时间窗口 (Total time window)
                *arb_found, // 套利发现时间 (此时已晚于deadline) (Arbitrage found time (at this point, already past deadline))
                (*arb_found).saturating_sub(*deadline) // 计算从截止时间到发现套利时已过去多久（逾期时间）。
                                                       // (Calculate how much time has passed from the deadline to when arbitrage was found (overdue time).)
            ),
        }
    }
}

// 为 `Source` 枚举实现一些辅助方法。
// (Implement some helper methods for the `Source` enum.)
impl Source {
    /// `is_shio` 方法
    /// (is_shio method)
    ///
    /// 用于判断当前的 `Source` 实例是否是 `Source::Shio` 类型。
    /// (Used to determine if the current `Source` instance is of type `Source::Shio`.)
    /// 注意：这个方法只在 `Source` 是活跃的 `Shio` 机会时返回 `true`，
    /// 对于 `ShioDeadlineMissed` 类型，它会返回 `false`。
    /// (Note: This method only returns `true` if `Source` is an active `Shio` opportunity.
    /// For `ShioDeadlineMissed` type, it will return `false`.)
    ///
    /// 返回 (Returns):
    /// - `bool`: 如果是 `Source::Shio` 则为 `true`，否则为 `false`。
    ///           (`true` if it is `Source::Shio`, otherwise `false`.)
    pub fn is_shio(&self) -> bool {
        // `matches!` 是一个方便的宏，用于判断一个表达式是否匹配某个模式。
        // (`matches!` is a convenient macro for checking if an expression matches a certain pattern.)
        // `Source::Shio { .. }` 中的 `..` 表示我们不关心 `Shio` 成员内部字段的具体值，只关心它是不是 `Shio` 这个变体。
        // (In `Source::Shio { .. }`, `..` means we don't care about the specific values of the fields inside the `Shio` member, only whether it's the `Shio` variant.)
        matches!(self, Source::Shio { .. })
    }

    /// `opp_tx_digest` 方法
    /// (opp_tx_digest method)
    ///
    /// 如果当前的 `Source` 实例是 `Source::Shio` 类型，则返回其内部存储的
    /// 原始机会交易的摘要 (`TransactionDigest`)。
    /// (If the current `Source` instance is of type `Source::Shio`, it returns the internally stored
    /// digest (`TransactionDigest`) of the original opportunity transaction.)
    /// 对于其他类型（`Public` 或 `ShioDeadlineMissed`），则返回 `None`，因为它们没有这个字段。
    /// (For other types (`Public` or `ShioDeadlineMissed`), it returns `None` as they don't have this field.)
    ///
    /// 返回 (Returns):
    /// - `Option<TransactionDigest>`: 如果是 `Shio` 则为 `Some(digest)`，否则为 `None`。
    ///                               (`Some(digest)` if it is `Shio`, otherwise `None`.)
    pub fn opp_tx_digest(&self) -> Option<TransactionDigest> {
        match self {
            // 如果 `self` 匹配 `Source::Shio` 模式，解构出 `opp_tx_digest` 字段，
            // 并将其包装在 `Some()` 中返回。`*opp_tx_digest` 是因为 `TransactionDigest` 是 `Copy` 类型，我们返回它的一个副本。
            // (If `self` matches the `Source::Shio` pattern, destructure the `opp_tx_digest` field,
            // and return it wrapped in `Some()`. `*opp_tx_digest` is used because `TransactionDigest` is a `Copy` type, and we return a copy of it.)
            Source::Shio { opp_tx_digest, .. } => Some(*opp_tx_digest),
            // 对于任何其他模式 (即 `Public` 或 `ShioDeadlineMissed`)，返回 `None`。
            // (For any other pattern (i.e., `Public` or `ShioDeadlineMissed`), return `None`.)
            _ => None,
        }
    }

    /// `deadline` 方法
    /// (deadline method)
    ///
    /// 如果当前的 `Source` 实例是 `Source::Shio` 类型（活跃的Shio机会），
    /// 则返回其竞价截止时间戳 (`u64`)。
    /// (If the current `Source` instance is of type `Source::Shio` (an active Shio opportunity),
    /// it returns its bidding deadline timestamp (`u64`).)
    /// 对于 `Public` 类型，没有截止时间的概念。
    /// (For the `Public` type, there's no concept of a deadline.)
    /// 对于 `ShioDeadlineMissed` 类型，虽然它有一个 `deadline` 字段，但那个是“已过的”截止时间，
    /// 此方法可能旨在返回“将来的”或“有效的”截止时间，因此对 `ShioDeadlineMissed` 也返回 `None` 可能是一个设计选择。
    /// （或者，也可以为 `ShioDeadlineMissed` 返回其存储的 `deadline`，具体取决于方法的语义设计）。
    /// (For `ShioDeadlineMissed` type, although it has a `deadline` field, that's a "past" deadline.
    /// This method might be intended to return a "future" or "valid" deadline, so returning `None` for `ShioDeadlineMissed`
    /// could be a design choice. (Alternatively, it could return the stored `deadline` for `ShioDeadlineMissed`, depending on the method's semantic design).)
    ///
    /// 返回 (Returns):
    /// - `Option<u64>`: 如果是 `Source::Shio` 则为 `Some(timestamp)`，否则为 `None`。
    ///                   (`Some(timestamp)` if it is `Source::Shio`, otherwise `None`.)
    pub fn deadline(&self) -> Option<u64> {
        match self {
            Source::Shio { deadline, .. } => Some(*deadline), // 返回 Shio 机会的截止时间 (Return the deadline of the Shio opportunity)
            _ => None, // Public 和 ShioDeadlineMissed 在此上下文中不提供“有效”截止时间 (Public and ShioDeadlineMissed do not provide a "valid" deadline in this context)
        }
    }

    /// `bid_amount` 方法
    /// (bid_amount method)
    ///
    /// 如果当前的 `Source` 实例是 `Source::Shio` 类型，则返回其当前记录的竞价金额。
    /// (If the current `Source` instance is of type `Source::Shio`, it returns its currently recorded bid amount.)
    /// 对于其他类型（`Public` 或 `ShioDeadlineMissed`），竞价金额的概念不适用或已无意义，因此返回0。
    /// (For other types (`Public` or `ShioDeadlineMissed`), the concept of a bid amount is not applicable or is meaningless, so it returns 0.)
    ///
    /// 返回 (Returns):
    /// - `u64`: Shio机会的竞价金额，或其他情况下的0。
    ///          (The bid amount for a Shio opportunity, or 0 in other cases.)
    pub fn bid_amount(&self) -> u64 {
        match self {
            Source::Shio { bid_amount, .. } => *bid_amount, // 返回 Shio 机会的竞价金额 (Return the bid amount of the Shio opportunity)
            _ => 0, // 其他类型没有竞价金额，或默认为0 (Other types have no bid amount, or default to 0)
        }
    }

    /// `with_bid_amount` 方法 (构建器模式风格 / Builder pattern style)
    /// (with_bid_amount method)
    ///
    /// 这个方法用于更新 `Source::Shio` 实例中的 `bid_amount` 字段。
    /// (This method is used to update the `bid_amount` field in a `Source::Shio` instance.)
    /// 如果当前的 `Source` 实例是 `Source::Shio`，它会创建一个新的 `Source::Shio` 实例，
    /// 其中包含所有旧的字段值，但 `bid_amount` 被更新为 `new_bid_amount`。
    /// (If the current `Source` instance is `Source::Shio`, it creates a new `Source::Shio` instance
    /// containing all old field values, but with `bid_amount` updated to `new_bid_amount`.)
    /// 如果当前的 `Source` 实例是其他类型（`Public` 或 `ShioDeadlineMissed`），则此方法原样返回 `self`，不做任何修改。
    /// (If the current `Source` instance is of another type (`Public` or `ShioDeadlineMissed`), this method returns `self` as is, without any modification.)
    /// 这种返回新实例而不是修改原实例的方式，使得 `Source` 可以保持 `Copy` 特性，并且更符合函数式编程风格。
    /// (This approach of returning a new instance instead of modifying the original allows `Source` to maintain its `Copy` trait and aligns better with functional programming style.)
    ///
    /// 参数 (Parameters):
    /// - `self`: 获取 `Source` 实例的所有权（因为它是 `Copy` 类型，所以实际上是复制）。
    ///           (Takes ownership of the `Source` instance (actually a copy, as it's a `Copy` type).)
    /// - `new_bid_amount`: 新的竞价金额 (`u64`)。
    ///                     (The new bid amount (`u64`).)
    ///
    /// 返回 (Returns):
    /// - 一个新的 `Source` 实例（如果原先是 `Shio` 且被更新了），或者是原始 `Source` 实例的副本（如果未被修改）。
    ///   (A new `Source` instance (if it was originally `Shio` and was updated), or a copy of the original `Source` instance (if not modified).)
    pub fn with_bid_amount(self, new_bid_amount: u64) -> Self {
        match self {
            Source::Shio {
                opp_tx_digest, // 保留原始的机会摘要 (Retain original opportunity digest)
                start,         // 保留原始的开始时间 (Retain original start time)
                deadline,      // 保留原始的截止时间 (Retain original deadline)
                arb_found,     // 保留原始的套利发现时间 (Retain original arbitrage found time)
                // 注意：旧的 `bid_amount` 字段在这里被忽略，不从 `self` 中解构出来并再次使用
                // (Note: The old `bid_amount` field is ignored here, not destructured from `self` and reused)
            } => Source::Shio { // 创建一个新的 `Source::Shio` 实例 (Create a new `Source::Shio` instance)
                opp_tx_digest,
                bid_amount: new_bid_amount, // 使用传入的 `new_bid_amount` (Use the passed `new_bid_amount`)
                start,
                deadline,
                arb_found,
            },
            // 如果 `self` 是 `Source::Public` 或 `Source::ShioDeadlineMissed`，则直接返回 `self` 的副本，不做修改。
            // (If `self` is `Source::Public` or `Source::ShioDeadlineMissed`, return a copy of `self` directly, without modification.)
            _ => self,
        }
    }

    /// `with_arb_found_time` 方法 (构建器模式风格 / Builder pattern style)
    /// (with_arb_found_time method)
    ///
    /// 这个方法用于更新 `Source::Shio` 实例中的 `arb_found` (套利发现时间) 字段。
    /// (This method is used to update the `arb_found` (arbitrage found time) field in a `Source::Shio` instance.)
    /// 它有一个重要的额外逻辑：在更新 `arb_found` 时间后，它会检查这个新的发现时间是否仍然在该Shio机会的 `deadline` (截止时间) 之前。
    /// (It has an important additional logic: after updating the `arb_found` time, it checks if this new discovery time is still before the Shio opportunity's `deadline`.)
    ///
    /// -   如果新的 `new_arb_found_time` 仍然早于 `deadline`：
    ///     (If the new `new_arb_found_time` is still earlier than `deadline`:)
    ///     则返回一个更新了 `arb_found` 字段的新的 `Source::Shio` 实例。
    ///     (Then return a new `Source::Shio` instance with the `arb_found` field updated.)
    /// -   如果新的 `new_arb_found_time` 等于或晚于 `deadline`：
    ///     (If the new `new_arb_found_time` is equal to or later than `deadline`:)
    ///     则意味着当机器人找到套利方案时，机会已经错过了。此时，方法会将来源转换为 `Source::ShioDeadlineMissed` 类型，
    ///     并使用相关的 `start`, `new_arb_found_time`, 和 `deadline` 时间戳来构造它。
    ///     (Then it means the opportunity was missed when the bot found the arbitrage solution. In this case, the method converts the source to `Source::ShioDeadlineMissed` type,
    ///      and constructs it using the relevant `start`, `new_arb_found_time`, and `deadline` timestamps.)
    ///
    /// 如果当前的 `Source` 实例是其他类型（`Public` 或 `ShioDeadlineMissed`），则此方法原样返回 `self`，不做任何修改。
    /// (If the current `Source` instance is of another type (`Public` or `ShioDeadlineMissed`), this method returns `self` as is, without any modification.)
    ///
    /// 参数 (Parameters):
    /// - `self`: 获取 `Source` 实例的所有权 (副本)。
    ///           (Takes ownership of the `Source` instance (a copy).)
    /// - `new_arb_found_time`: 新的套利发现时间戳 (`u64`)。
    ///                         (The new arbitrage found timestamp (`u64`).)
    ///
    /// 返回 (Returns):
    /// - 一个新的 `Source` 实例，可能是更新后的 `Source::Shio`，或者转换后的 `Source::ShioDeadlineMissed`，
    ///   或者是原始 `Source` 实例的副本（如果未被修改）。
    ///   (A new `Source` instance, which could be an updated `Source::Shio`, a converted `Source::ShioDeadlineMissed`,
    ///    or a copy of the original `Source` instance (if not modified).)
    pub fn with_arb_found_time(self, new_arb_found_time: u64) -> Self {
        match self {
            Source::Shio {
                opp_tx_digest, // 保留这些字段 (Retain these fields)
                start,
                deadline,
                bid_amount,
                // 旧的 `arb_found` 被忽略 (Old `arb_found` is ignored)
            } => {
                // 检查新的套利发现时间是否在截止时间之前
                // (Check if the new arbitrage found time is before the deadline)
                if new_arb_found_time < deadline {
                    // 如果仍在截止时间之前，则更新 `arb_found` 并保持为 `Source::Shio`
                    // (If still before the deadline, update `arb_found` and keep as `Source::Shio`)
                    Source::Shio {
                        opp_tx_digest,
                        bid_amount,
                        start,
                        arb_found: new_arb_found_time, // 使用新的套利发现时间 (Use the new arbitrage found time)
                        deadline,
                    }
                } else {
                    // 如果等于或晚于截止时间，则将来源转换为 `Source::ShioDeadlineMissed`
                    // (If equal to or later than the deadline, convert the source to `Source::ShioDeadlineMissed`)
                    Source::ShioDeadlineMissed {
                        start,                         // 保留原始的开始时间 (Retain original start time)
                        arb_found: new_arb_found_time, // 使用新的（但已晚的）套利发现时间 (Use the new (but late) arbitrage found time)
                        deadline,                      // 保留原始的截止时间 (Retain original deadline)
                    }
                }
            }
            // 如果 `self` 不是活跃的 `Shio` 机会，则不做任何修改。
            // (If `self` is not an active `Shio` opportunity, no modifications are made.)
            _ => self,
        }
    }
}

[end of bin/arb/src/types.rs]
