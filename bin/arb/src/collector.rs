// 该文件定义了不同类型的“收集器”(Collectors)，它们用于从不同来源收集Sui区块链上的事件数据，
// 例如公开的交易效果 (Transaction Effects) 和私下广播的交易 (Private Transactions)。
// 这些收集器是套利机器人（或其他链上监控工具）获取实时信息的重要组成部分。
// 把它们想象成不同频道的新闻记者，有的专门报道官方发布会（公开交易），有的通过特殊渠道获取内幕消息（私有交易）。
// 套利机器人需要这些“新闻报道”来了解市场上正在发生什么，从而发现机会。
//
// **核心概念解释**:
// - **Collector (收集器)**:
//   这是一个抽象的名字，指的是一个能够持续不断地“收集”并“产生”事件流（一连串事件）的程序组件。
//   就像一个信息收集站，它会从特定的信息源（比如Sui网络）获取数据，然后把这些数据整理成一个个“事件”发送出来。
//
// - **Event (事件)**:
//   在这个程序里，“事件”通常指的是Sui区块链上发生的一笔交易及其相关信息。
//   比如，张三给李四转了10个SUI币，这就是一个事件；一个智能合约被调用了，也是一个事件。
//
// - **Transaction Effects (交易效果/交易结果)**:
//   当一笔交易在Sui区块链上被成功执行后，它会对链上的状态产生一系列影响和结果。
//   这些结果就被称为“交易效果”。它详细记录了这笔交易做了什么，比如：
//     - 创建了哪些新的数字资产（对象）？
//     - 修改了哪些已有的数字资产？
//     - 发出了哪些通知（事件日志）？
//     - 消耗了多少gas（手续费）？
//   套利机器人分析交易效果，可以了解市场价格变动、资金流动等情况。
//
// - **Public Transactions (公开交易)**:
//   这些交易是通过Sui网络中常规的、公开的渠道广播和处理的。
//   任何人都可以通过连接到一个Sui RPC节点来观察到这些交易及其效果。
//   就像新闻发布会上的公开消息，所有人都能看到。
//
// - **Private Transactions (私有交易)**:
//   这些交易不是通过公开渠道广播的，而是可能通过一些专门的、私密的通道（例如MEV中继器或特定服务商提供的接口）发送给区块生产者（验证者）。
//   这么做的目的通常是为了进行MEV（Miner Extractable Value，矿工可提取价值，现在泛指验证者可提取价值）操作，
//   比如避免自己的套利交易被其他机器人发现并抢先执行（即“抢跑”或“三明治攻击”）。
//   就像重要的内幕消息，只在小圈子里流通，直到被最终敲定（打包进区块）。
//
// - **IPC (Inter-Process Communication, 进程间通信)**:
//   这是一种允许在同一台计算机上运行的多个不同程序（进程）互相交换数据和信息的技术。
//   `PublicTxCollector` 使用了本地套接字（local socket，一种IPC方式）来接收数据。
//   可以想象成在同一栋大楼里的两个办公室之间铺设了专门的电话线，用于快速传递信息。
//   它可能连接到本地运行的一个Sui节点服务，这个服务会把公开交易数据通过这条“电话线”发送过来。
//
// - **WebSocket (WS)**:
//   一种先进的网络通信协议，它允许在客户端（比如这里的收集器）和服务器（比如MEV中继器）之间建立一个持久的、双向的连接。
//   一旦连接建立，双方可以随时互相发送消息，实现实时通信。
//   `PrivateTxCollector` 使用WebSocket连接到一个中继服务器，这个服务器会把私有交易实时推送过来。
//   就像一个专门的新闻直播频道，一旦有新消息，会立刻推送给订阅者。

// 引入所需的库和模块
use burberry::{async_trait, Collector, CollectorStream}; // `burberry` 可能是项目内或公司内部的一个自定义基础库，
                                                      // `Collector` 是一个 trait (接口)，定义了收集器应该具备哪些功能 (比如提供一个事件流)。
                                                      // `CollectorStream` 是这个库定义的事件流类型，可能是一个包装了异步迭代器的类型。
                                                      // `async_trait` 宏使得我们可以在 trait 中使用 `async fn` (异步函数)。
use eyre::Result; // `eyre` 是一个流行的Rust错误处理库，提供了更方便的错误上下文管理和报告。`Result` 是其核心类型，通常是 `Result<T, eyre::Report>`。
use fastcrypto::encoding::{Base64, Encoding}; // `fastcrypto` 是一个专注于密码学和编码的库。
                                              // `Base64` 是一种将二进制数据转换为文本字符串的编码方式，常用于在文本协议（如JSON）中传输二进制内容。
                                              // `Encoding` trait 定义了编码和解码的通用接口。私有交易数据在通过网络传输时，其原始字节码可能被Base64编码。
use futures::stream::StreamExt; // `futures` 库是Rust异步编程的核心。`StreamExt` trait 为异步流 (Stream) 提供了很多方便的扩展方法，
                                // 比如 `next()` (获取流中的下一个元素), `filter()`, `map()` 等。
use interprocess::local_socket::{ // `interprocess` 库用于进行进程间通信 (IPC)。
    tokio::{prelude::*, Stream as LocalSocketStream}, // `tokio::prelude::*` 导入了tokio常用的trait和类型。
                                                      // `Stream as LocalSocketStream` 指的是与tokio异步运行时集成的本地套接字流。
                                                      // 允许程序像处理网络流一样处理本地套接字上的数据。
    GenericNamespaced, // 用于创建具有命名空间的本地套接字名称。这有助于避免不同应用程序之间的套接字名称冲突。
};
use serde::Deserialize; // `serde` 是一个非常强大的Rust序列化和反序列化框架。
                       // `Deserialize` trait 用于将数据从某种格式（如JSON, Bincode, YAML等）转换为Rust结构体或枚举。
use sui_json_rpc_types::{SuiEvent, SuiTransactionBlockEffects}; // `sui_json_rpc_types` 包含了Sui RPC接口所使用的各种数据类型定义。
                                                              // `SuiEvent` 代表Sui区块链上的一个事件记录。
                                                              // `SuiTransactionBlockEffects` 代表一个交易区块执行后的效果，这是RPC接口通常返回的格式。
use sui_types::{
    effects::TransactionEffects, // `sui_types` 是Sui核心逻辑库，定义了Sui区块链内部使用的数据结构。
                                 // `TransactionEffects` 是Sui节点内部表示交易效果的结构。
    transaction::TransactionData, // `TransactionData` 是Sui节点内部表示一笔待执行或已执行交易的核心数据。
};
use tokio::{io::AsyncReadExt, pin, time}; // `tokio` 是Rust主要的异步运行时库。
                                        // `io::AsyncReadExt` 为异步读取操作（如从套接字读取）提供了扩展方法，如 `read_exact()`。
                                        // `pin!` 宏用于将一个值“固定”在内存中的某个位置，这对于某些异步操作是必需的，因为异步任务可能在不同时间点被移动。
                                        // `time` 模块提供了异步环境下的时间相关功能，如 `sleep()`。
use tracing::{debug, error, info}; // `tracing` 是一个用于日志记录和分布式追踪的框架。
                                  // `debug!` 用于输出调试级别的日志，通常在开发和排查问题时使用。
                                  // `error!` 用于输出错误级别的日志，表示发生了需要关注的问题。
                                  // `info!` 用于输出信息级别的日志，通常用于报告程序运行状态或重要事件。

use crate::types::Event; // 从当前项目 (crate) 的 `types` 模块中引入自定义的 `Event` 枚举类型。
                         // 这个 `Event` 枚举可能包含了 `PublicTx` 和 `PrivateTx` 等变体，用于统一表示不同来源的事件。

// --- PublicTxCollector ---
// `PublicTxCollector` 的作用是收集通过公开渠道广播的Sui交易及其执行结果（交易效果）。
// 它通过一个本地的IPC（进程间通信）套接字来接收这些数据。
// 想象一下，它就像一个专门监听本地“广播站”（可能是本地运行的Sui节点服务）的接收器，
// 这个广播站会实时播报所有公开的交易信息。
pub struct PublicTxCollector {
    path: String, // 这个字符串存储了本地IPC套接字的“地址”或“文件名”。
                  // 例如，在Linux或macOS上，它可能类似于 "/tmp/sui_public_transactions.sock"。
}

impl PublicTxCollector {
    /// 创建一个新的 `PublicTxCollector` 实例。
    ///
    /// 参数 (Parameters):
    /// - `path`: 一个字符串切片 (`&str`)，表示IPC套接字的路径。例如 "/tmp/my_socket.sock"。
    ///           This is like the "address" of a local mailbox.
    ///
    /// 返回 (Returns):
    /// - 一个 `PublicTxCollector` 实例。
    ///   A new instance of the collector, ready to be connected.
    pub fn new(path: &str) -> Self {
        info!("初始化 PublicTxCollector, IPC路径: {}", path); // 记录日志：公共交易收集器初始化，指明IPC路径
        Self { path: path.to_string() } // 将传入的路径字符串切片转换为 `String` 类型（拥有所有权）并存储起来。
                                        // We save the path so we can use it later to connect.
    }

    /// 异步地连接到指定的本地IPC套接字。
    /// 这个方法会尝试建立与数据源的连接。
    ///
    /// 返回 (Returns):
    /// - `Result<LocalSocketStream>`:
    ///   - 如果连接成功 (If connection succeeds)，返回 `Ok(LocalSocketStream)`，其中 `LocalSocketStream` 是一个可以从中异步读取数据的流。
    ///     This stream is like an open "pipe" to receive data.
    ///   - 如果连接失败 (If connection fails)（例如路径不存在、权限问题等），返回 `Err(...)`，包含错误信息。
    ///     This tells us something went wrong trying to connect.
    async fn connect(&self) -> Result<LocalSocketStream> {
        // `self.path.as_str()` 将 `String` 转换为 `&str`。
        // `.to_ns_name::<GenericNamespaced>()?` 将普通的路径字符串转换为特定于操作系统平台的、具有命名空间的套接字名称。
        //   `GenericNamespaced` 是一种通用的命名空间策略。 `?` 用于错误传播，如果转换失败则返回错误。
        //   This prepares the path string to be used by the operating system for local sockets.
        let name = self.path.as_str().to_ns_name::<GenericNamespaced>()?;
        debug!("PublicTxCollector 尝试连接到IPC: {:?}", name); // 记录日志：尝试连接到IPC
        // `LocalSocketStream::connect(name)` 异步地尝试连接到这个命名的本地套接字。
        // `await` 等待连接操作完成。
        let conn = LocalSocketStream::connect(name).await?;
        info!("PublicTxCollector 成功连接到IPC: {:?}", name); // 记录日志：成功连接到IPC
        Ok(conn) // 返回成功的连接流。
    }
}

// 为 `PublicTxCollector` 实现 `burberry::Collector` trait (接口)。
// 这意味着 `PublicTxCollector` 承诺提供 `Collector` trait 所定义的行为，
// 主要是能够产生一个事件流 (`CollectorStream`)。
#[async_trait] // 这个宏使得 trait 中的方法可以使用 `async` 关键字，成为异步方法。
impl Collector<Event> for PublicTxCollector {
    /// 返回收集器的名称。
    /// 这是一个简单的方法，用于标识这个收集器是什么类型的。
    ///
    /// 返回 (Returns):
    /// - 一个字符串切片 (`&str`)，代表收集器的名字。
    ///   A simple name for this type of collector.
    fn name(&self) -> &str {
        "PublicTxCollector" // 直接返回写死的名称。
    }

    /// 获取一个包含 `Event` 的异步流 (`CollectorStream`)。
    /// 这是 `Collector` trait 的核心方法。调用此方法后，收集器会开始工作，
    /// 并持续不断地从其数据源（这里是IPC套接字）接收数据，将数据转换成 `Event`，然后通过流发送出来。
    /// 调用者可以从这个流中异步地读取这些 `Event`。
    ///
    /// 返回 (Returns):
    /// - `Result<CollectorStream<'_, Event>>`:
    ///   - 如果成功初始化并开始监听 (If successful)，返回 `Ok(CollectorStream)`，其中 `CollectorStream` 是一个可以从中异步拉取 `Event` 的流。
    ///     `'_` 是一个生命周期参数，表示流的生命周期与 `self` (即 `PublicTxCollector` 实例) 相关联。
    ///     This stream will provide a sequence of `Event`s.
    ///   - 如果在连接或初始化过程中发生错误 (If an error occurs)，返回 `Err(...)`。
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Event>> {
        // 首次尝试连接到IPC套接字。
        let mut conn = self.connect().await?; // `mut` 因为后续连接失败时会尝试重连并赋新值给 `conn`。

        // 用于存储接下来要读取的数据块长度的缓冲区。
        // 数据源（如Sui节点）在通过IPC发送交易效果和事件数据时，通常会采用一种简单的协议：
        // 先发送4个字节表示后续实际数据的长度，然后再发送实际数据。
        // 这样做可以帮助接收方准确地知道需要读取多少字节。
        // Think of it like a message envelope: first you get the size, then the message itself.
        let mut effects_len_buf = [0u8; 4]; // 4字节的数组，用于存储交易效果数据的长度 (通常是一个u32整数)。
                                            // This buffer will hold the size of the transaction effects data.
        let mut events_len_buf = [0u8; 4];  // 4字节的数组，用于存储相关Sui事件数据的长度。
                                            // This buffer will hold the size of the associated events data.

        // 使用 `async_stream::stream!` 宏来创建一个异步流。
        // 这个宏极大地简化了手动实现异步 `Stream` trait 的复杂性。
        // 在 `stream! { ... }` 块内部，我们可以使用 `yield` 关键字来从流中“产生”一个值。
        // This creates a "generator" that will produce events one by one.
        let stream = async_stream::stream! {
            // 进入一个无限循环，表示收集器会持续不断地尝试从连接中读取数据并产生事件，
            // 直到发生不可恢复的错误或程序被终止。
            // This loop runs forever to keep collecting data.
            loop {
                // `tokio::select!` 宏用于同时等待多个异步操作，当其中任何一个操作完成时，`select!` 块就会执行相应的分支。
                // 这对于处理像网络IO这种可能需要等待的操作非常有用。
                // 在这里，它主要等待 `conn.read_exact` (从套接字读取数据) 的完成。
                // `biased;` (可选)可以指示select优先检查分支的顺序，但这里没有使用。
                // `select!` lets us wait for data to arrive on the connection.
                tokio::select! {
                    // **分支1: 尝试从连接中精确读取4个字节到 `effects_len_buf`**
                    // `conn.read_exact(&mut effects_len_buf)` 会一直尝试读取，直到填满 `effects_len_buf` (4个字节) 或者发生错误。
                    // `result` 会是 `std::io::Result<usize>` (读取的字节数，成功时总是4) 或 `std::io::Error`。
                    // We try to read the first 4 bytes, which tell us the length of the effects data.
                    result = conn.read_exact(&mut effects_len_buf) => {
                        // 检查读取操作是否成功。
                        if result.is_err() { // 如果 `read_exact` 返回错误 (例如连接被对方关闭、网络问题等)
                            error!("读取交易效果数据长度失败 (IPC: {}), 尝试重连... Error: {:?}", self.path, result.err()); // 记录一条错误信息，包含路径和具体错误。
                            // 尝试重新连接到IPC套接字。
                            // `.expect(...)` 在这里表示如果重连失败，则程序直接panic（崩溃）。
                            // 在生产环境中，可能需要更健壮的重连策略（例如带退避的重试几次）。
                            // If reading fails (e.g., connection closed), try to reconnect.
                            conn = self.connect().await.expect("无法重连到交易效果的IPC套接字");
                            info!("PublicTxCollector IPC ({}) 重连成功。", self.path); // 记录重连成功
                            continue; // 跳过当前循环的剩余部分，从下一次循环开始，尝试用新的连接重新读取。
                        }

                        // 如果读取长度成功，将 `effects_len_buf` 中的4个字节 (假设是大端序，Big Endian) 转换为一个u32整数。
                        // 这个整数就是接下来实际交易效果数据的长度。
                        // Convert the 4 bytes (length) into a number.
                        let effects_len = u32::from_be_bytes(effects_len_buf);
                        // 根据获取到的长度，创建一个动态大小的字节向量 (vector) `effects_buf` 来存储实际的交易效果数据。
                        // Create a buffer of the correct size to hold the effects data.
                        let mut effects_buf = vec![0u8; effects_len as usize]; // `effects_len` 转为 `usize` 作为长度。
                        // 再次调用 `conn.read_exact`，这次读取 `effects_len` 数量的字节到 `effects_buf`。
                        // Read the actual effects data.
                        if conn.read_exact(&mut effects_buf).await.is_err() {
                            error!("读取交易效果数据体失败 (IPC: {}), 尝试重连...", self.path);
                            conn = self.connect().await.expect("无法重连到交易效果的IPC套接字");
                            info!("PublicTxCollector IPC ({}) 重连成功。", self.path);
                            continue;
                        }
                        debug!("成功读取 {} 字节的交易效果数据。", effects_len);

                        // --- 以类似的方式读取关联的Sui事件数据 ---
                        // Now, do the same for the associated Sui events data.
                        // 1. 读取Sui事件数据的长度 (4字节)。
                        // Read the length of the events data.
                        if conn.read_exact(&mut events_len_buf).await.is_err() {
                            error!("读取Sui事件数据长度失败 (IPC: {}), 尝试重连...", self.path);
                            conn = self.connect().await.expect("无法重连到Sui事件的IPC套接字");
                            info!("PublicTxCollector IPC ({}) 重连成功。", self.path);
                            continue;
                        }
                        let events_len = u32::from_be_bytes(events_len_buf);
                        // 2. 根据长度创建缓冲区。
                        // Create a buffer for events data.
                        let mut events_buf = vec![0u8; events_len as usize];
                        // 3. 读取实际的Sui事件数据。
                        // Read the actual events data.
                        if conn.read_exact(&mut events_buf).await.is_err() {
                            error!("读取Sui事件数据体失败 (IPC: {}), 尝试重连...", self.path);
                            conn = self.connect().await.expect("无法重连到Sui事件的IPC套接字");
                            info!("PublicTxCollector IPC ({}) 重连成功。", self.path);
                            continue;
                        }
                        debug!("成功读取 {} 字节的Sui事件数据。", events_len);


                        // --- 数据反序列化 ---
                        // 现在我们已经从IPC套接字读取了原始的字节数据 (`effects_buf` 和 `events_buf`)。
                        // 下一步是将这些字节数据转换回程序可以理解的Rust数据结构。这个过程称为“反序列化”。
                        // "Deserialization" means turning raw bytes back into meaningful data structures.

                        // 将 `effects_buf` 中的字节数据反序列化为 `TransactionEffects` 类型。
                        // `bincode` 是一个用于二进制序列化和反序列化的库，它通常用于高效地传输结构化数据。
                        // 这里假设IPC数据源发送的是bincode编码的 `TransactionEffects` 数据。
                        // We expect `effects_buf` to contain `TransactionEffects` data, encoded using `bincode`.
                        let tx_effects: TransactionEffects = match bincode::deserialize(&effects_buf) {
                            Ok(deserialized_effects) => deserialized_effects, // 反序列化成功
                            Err(e) => { // 如果反序列化失败 (例如数据损坏、格式不匹配)
                                error!("无效的交易效果数据 (bincode反序列化失败): {:?}, 跳过该条目。原始数据长度: {}", e, effects_buf.len());
                                continue; // 记录错误，并跳过处理这个损坏的数据，继续下一次循环。
                            }
                        };

                        // 将 `events_buf` 中的字节数据反序列化为 `Vec<SuiEvent>` (一个包含多个Sui事件的列表)。
                        // 这里假设Sui事件数据是通过JSON格式进行编码和传输的。
                        // `serde_json::from_slice` 用于从字节切片反序列化JSON数据。
                        // We expect `events_buf` to contain a list of `SuiEvent`s, encoded as JSON.
                        let events: Vec<SuiEvent> = if events_len == 0 { // 检查长度是否为0
                            vec![] // 如果长度为0，意味着没有关联的Sui事件，返回一个空列表。
                                   // If length is 0, there are no events.
                        } else {
                            match serde_json::from_slice(&events_buf) {
                                Ok(deserialized_events) => deserialized_events, // JSON反序列化成功
                                Err(e) => {
                                    error!("无效的Sui事件数据 (JSON反序列化失败): {:?}, 跳过该条目。原始数据长度: {}", e, events_buf.len());
                                    continue; // 记录错误，跳过。
                                }
                            }
                        };
                        debug!("成功反序列化交易效果 (摘要: {:?}) 和 {} 个Sui事件。", tx_effects.digest(), events.len());

                        // 将Sui核心库的 `TransactionEffects` 类型转换为Sui RPC接口兼容的 `SuiTransactionBlockEffects` 类型。
                        // 这可能是因为下游的处理逻辑（例如套利机器人的分析模块）期望接收的是RPC标准格式的数据。
                        // `SuiTransactionBlockEffects::try_from(tx_effects)` 尝试进行这种转换。
                        // Convert the internal `TransactionEffects` type to the RPC-standard `SuiTransactionBlockEffects`.
                        if let Ok(sui_tx_effects) = SuiTransactionBlockEffects::try_from(tx_effects) {
                            // `yield` 关键字是 `async_stream::stream!` 宏的核心。
                            // 它从异步流中“产生”一个值。调用者在 `await` 流的 `next()` 方法时会接收到这个值。
                            // 这里产生一个 `Event::PublicTx` 类型的事件，它封装了转换后的交易效果 (`sui_tx_effects`)
                            // 和相关的Sui事件列表 (`events`)。
                            // `yield` sends this event out through the stream.
                            yield Event::PublicTx(sui_tx_effects, events);
                        } else {
                            // 如果 `TransactionEffects` -> `SuiTransactionBlockEffects` 转换失败，也记录一个错误。
                            error!("无法将TransactionEffects转换为SuiTransactionBlockEffects, 跳过。摘要: {:?}", tx_effects.digest());
                            // 这里没有 `continue`，因为主要数据已处理，只是转换rpc类型失败。
                            // 是否continue取决于业务逻辑，如果下游严格需要rpc类型，则应该continue。
                        }
                    }
                    // **分支2: `else` 分支**
                    // 在 `tokio::select!` 宏中，如果没有任何其他分支的异步操作立即准备好（即 `conn.read_exact` 需要阻塞等待数据），
                    // 那么 `else` 分支（如果提供）会被执行。
                    // 这里用它来在没有新数据可读时进行短暂的异步休眠，以避免CPU持续空转检查套接字，从而节省资源。
                    // If no data is ready to be read, pause for a short time (10ms) to avoid busy-waiting.
                    else => {
                        time::sleep(time::Duration::from_millis(10)).await; // 异步休眠10毫秒。
                    }
                }
            }
        };

        // 将创建的异步流 (`stream`) 固定 (pin) 到堆上，并将其类型擦除为 `CollectorStream<'_, Event>` 后返回。
        // `Box::pin` 是创建 `Pin<Box<dyn Stream + Send + 'a>>` （或者这里是 `CollectorStream`）的常用方法。
        // `Pin` 确保了流在内存中的位置不会改变，这对于某些异步操作是必需的。
        // Return the stream, ready for the caller to consume events from.
        Ok(Box::pin(stream))
    }
}

// --- PrivateTxCollector ---
// `PrivateTxCollector` 的作用是收集那些不通过公开Sui RPC节点广播，而是通过私有渠道（如MEV中继器）发送的交易。
// 这些通常是MEV搜索者或套利者为了避免被抢先交易而使用的“秘密通道”。
// 它通过WebSocket连接到一个中继服务器来接收这些私有交易数据。
// 把它想象成一个专门接收“加密电报”（私有交易）的接收器。
// This collector listens for "private" transactions, often used in MEV strategies.
// It connects to a relay server via WebSocket.

/// `TxMessage` 结构体用于表示从私有交易中继服务器接收到的原始交易消息的格式。
/// 通常，中继服务器会用JSON格式包装实际的交易数据，而交易数据本身（字节码）可能是Base64编码的字符串。
/// `#[derive(...)]` 是Rust的属性宏，用于自动为结构体生成一些常用的trait实现。
/// This struct defines the expected format of a message from the private transaction relay.
/// It's usually a JSON object containing the transaction bytes encoded in Base64.
#[derive(Debug, Clone, Deserialize, Default)] //
                                              // `Debug`: 允许使用 `{:?}` 格式化打印结构体实例，方便调试。
                                              // `Clone`: 允许创建结构体实例的深拷贝。
                                              // `Deserialize`: 来自 `serde` 库，使得这个结构体可以从序列化格式（如JSON）中反序列化出来。
                                              // `Default`: 允许创建结构体的默认实例（例如，`tx_bytes` 为空字符串）。
pub struct TxMessage {
    // 这个字段存储了从WebSocket消息中解析出来的、经过Base64编码的原始交易字节。
    // 例如，它可能看起来像这样："AQIAAAEAAAAAAAAAAAAA..."
    // The actual transaction data, encoded as a Base64 string.
    tx_bytes: String,
}

/// 为 `TransactionData` 实现 `TryFrom<TxMessage>` trait。
/// 这个 `impl` 块使得我们可以方便地将接收到的 `TxMessage` 对象尝试转换为Sui核心库能理解的 `TransactionData` 类型。
/// `TryFrom` 是一个标准的Rust trait，用于表示一个类型可以从另一个类型尝试转换而来，转换过程可能失败。
/// This allows us to convert a `TxMessage` (from the relay) into `TransactionData` (Sui's internal format).
impl TryFrom<TxMessage> for TransactionData {
    type Error = eyre::Error; // 定义了如果转换失败，返回的错误类型是 `eyre::Error`。
                              // If conversion fails, it will return an `eyre::Error`.

    /// 执行实际的转换操作。
    ///
    /// 参数 (Parameters):
    /// - `tx_message`: 一个 `TxMessage` 实例，包含了Base64编码的交易数据。
    ///                 The message received from the relay.
    ///
    /// 返回 (Returns):
    /// - `Result<Self, Self::Error>` (即 `Result<TransactionData, eyre::Error>`):
    ///   - 如果转换成功 (If successful)，返回 `Ok(TransactionData)`。
    ///   - 如果转换失败 (If it fails)（例如Base64解码错误、bcs反序列化错误），返回 `Err(eyre::Error)`。
    fn try_from(tx_message: TxMessage) -> Result<Self> {
        // 步骤1: 将Base64编码的字符串 `tx_message.tx_bytes` 解码为原始的字节序列 (`Vec<u8>`)。
        // `Base64::decode()` 是 `fastcrypto` 库提供的方法。
        // `?` 操作符用于错误传播：如果 `decode` 失败，整个 `try_from` 函数会立即返回解码错误。
        // First, decode the Base64 string into raw bytes.
        let tx_bytes_decoded = Base64::decode(&tx_message.tx_bytes)?;
        debug!("成功将私有交易消息解码Base64 ({} 字节)。", tx_bytes_decoded.len());

        // 步骤2: 将解码后的原始字节序列 (`tx_bytes_decoded`) 反序列化为 `TransactionData` 类型。
        // Sui交易在网络上传输或存储时，通常使用 BCS (Binary Canonical Serialization) 格式进行序列化。
        // `bcs::from_bytes()` 函数尝试从字节切片中反序列化出 `TransactionData` 结构。
        // `?` 再次用于错误传播：如果 `from_bytes` 失败（例如字节流格式不正确），则返回BCS反序列化错误。
        // Second, deserialize these raw bytes (expected to be in BCS format) into `TransactionData`.
        let tx_data: TransactionData = bcs::from_bytes(&tx_bytes_decoded)?;
        debug!("成功将私有交易字节反序列化为TransactionData (摘要: {:?})。", tx_data.digest());


        Ok(tx_data) // 如果两步都成功，返回转换得到的 `TransactionData`。
    }
}

/// `PrivateTxCollector` 结构体的定义。
pub struct PrivateTxCollector {
    ws_url: String, // 存储WebSocket中继服务器的URL地址。
                    // 例如："ws://localhost:8080/private-txs" 或 "wss://some-relay.com/stream"。
                    // The URL of the WebSocket server to connect to.
}

impl PrivateTxCollector {
    /// 创建一个新的 `PrivateTxCollector` 实例。
    ///
    /// 参数 (Parameters):
    /// - `ws_url`: 一个字符串切片 (`&str`)，表示WebSocket服务器的URL。
    ///             The WebSocket URL, e.g., "ws://example.com/transactions".
    ///
    /// 返回 (Returns):
    /// - 一个 `PrivateTxCollector` 实例。
    ///   A new collector, ready to connect.
    pub fn new(ws_url: &str) -> Self {
        info!("初始化 PrivateTxCollector, WebSocket URL: {}", ws_url); // 记录日志：私有交易收集器初始化，指明WebSocket URL
        Self {
            ws_url: ws_url.to_string(), // 将传入的URL字符串切片转换为 `String` 类型并存储。
        }
    }
}

// 为 `PrivateTxCollector` 实现 `burberry::Collector` trait。
#[async_trait]
impl Collector<Event> for PrivateTxCollector {
    /// 返回收集器的名称。
    fn name(&self) -> &str {
        "PrivateTxCollector"
    }

    /// 获取一个包含 `Event` 的异步流，用于接收和处理来自WebSocket的私有交易。
    /// 这个方法会建立WebSocket连接，并持续监听来自服务器的消息。
    /// 收到消息后，它会尝试将其解析为 `TransactionData`，然后作为 `Event::PrivateTx` 发送出去。
    /// This is the main method that connects to the WebSocket and provides a stream of private transaction events.
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Event>> {
        // 尝试异步连接到 `self.ws_url` 指定的WebSocket服务器。
        // `tokio_tungstenite::connect_async` 是一个常用的库函数，用于建立WebSocket连接。
        // 它返回一个元组 `(ws_stream, response)`:
        //   - `ws_stream`: 实际的WebSocket流对象，用于发送和接收消息。
        //   - `response`: 服务器对连接请求的HTTP响应信息 (这里用 `_` 忽略掉，因为通常不太关心)。
        // `.await` 等待连接完成。
        // `.expect(...)` 如果连接失败（例如URL无效、服务器不可达），程序会panic并显示错误信息。
        //   在生产代码中，这里应该使用更优雅的错误处理，比如返回 `Result` 或者进行重试。
        // Connect to the WebSocket server.
        info!("PrivateTxCollector 尝试连接到 WebSocket: {}", self.ws_url);
        let (ws_stream, response) = tokio_tungstenite::connect_async(&self.ws_url)
            .await
            .map_err(|e| {
                error!("PrivateTxCollector 无法连接到WebSocket ({}): {}", self.ws_url, e); // 记录连接失败的详细错误
                eyre::Report::new(e) // 将 tungstenite 错误转换为 eyre::Report
            })?; // 使用 eyre 错误传播
        info!("PrivateTxCollector 成功连接到 WebSocket: {}, 响应: {:?}", self.ws_url, response.status());


        // `ws_stream.split()` 方法将一个双向的WebSocket流 (`WebSocketStream`) 分割成两个独立的部分：
        //   - `sink` (发送端): 用于向服务器发送消息 (这里用 `_` 忽略，因为这个收集器只负责接收)。
        //   - `read` (接收端): 一个只读的流，用于从服务器接收消息。
        // Split the WebSocket stream into a sender and a receiver. We only need the receiver part.
        let (_, read) = ws_stream.split();

        // 使用 `async_stream::stream!` 宏创建异步流。
        // Create an async stream (generator) to produce events.
        let stream = async_stream::stream! {
            // `pin!(read);` 将 `read` 流固定在内存中。
            // 某些流操作（尤其是像 `StreamExt::next()` 这样的方法）要求被操作的流是 `Unpin` 的，
            // 或者被 `Pin` 包裹。`pin!` 宏是一种方便的写法。
            // Pin the `read` stream to allow `next()` to be called on it in a loop.
            pin!(read); // `read` 是分割出来的 `futures::stream::SplitStream<...>`

            // 无限循环，持续从 `read` 流中获取下一条WebSocket消息。
            // Loop forever, trying to get the next message from the WebSocket.
            while let Some(message_result) = read.next().await {
                // `read.next().await` 异步地获取流中的下一条消息。
                // `message_result` 的类型是 `Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>`。
                let message = match message_result {
                    Ok(msg) => msg, // 如果成功获取消息，`msg` 就是实际的WebSocket消息对象。
                                    // If we got a message successfully.
                    Err(e) => { // 如果在获取消息时发生错误 (例如连接中断)
                        error!("从私有交易中继WebSocket接收消息时发生错误: {:?}, 将继续尝试接收后续消息。", e);
                        // TODO: Consider if a reconnect strategy is needed here or if the stream should end.
                        // For now, it just skips this error and tries to get the next message.
                        continue; // 记录错误，并跳到下一次循环，尝试处理流中的下一条消息（如果可能的话）。
                    }
                };
                debug!("从WebSocket收到消息: {:?}", message.is_text()); // 记录收到消息的类型 (是否文本)

                // 将接收到的WebSocket消息 (`message`) 尝试转换为 `TxMessage` 结构体。
                // WebSocket消息可以是文本、二进制、Ping/Pong等多种类型。这里我们期望的是文本消息 (JSON格式)。
                // We expect the message to be in text format (JSON).
                let tx_message_parsed: TxMessage = match message {
                    tokio_tungstenite::tungstenite::Message::Text(text_payload) => {
                        // 如果消息是文本类型，`text_payload` 就是包含JSON内容的字符串。
                        // Attempt to parse the JSON string into our `TxMessage` struct.
                        match serde_json::from_str(&text_payload) {
                            Ok(tm) => tm, // JSON成功反序列化为 `TxMessage`
                            Err(e) => {
                                error!("从文本消息解析TxMessage失败 (JSON反序列化错误): {:?}, 原始文本: '{}'", e, text_payload);
                                continue; // 解析失败，记录错误并跳过这条消息。
                            }
                        }
                    },
                    // 如果消息不是文本类型 (例如是二进制数据、Ping/Pong、Close等)。
                    _ => {
                        // 记录一条调试信息，说明收到了非文本消息，然后跳过。
                        debug!("接收到非文本格式的WebSocket消息或控制帧，已跳过。消息详情: {:?}", message);
                        continue;
                    }
                };


                // 将解析得到的 `tx_message_parsed` (类型 `TxMessage`) 转换为Sui核心库的 `TransactionData` 类型。
                // 这个转换是通过我们之前为 `TransactionData` 实现的 `TryFrom<TxMessage>` trait 来完成的。
                // Convert the parsed `TxMessage` into Sui's `TransactionData`.
                let tx_data = match TransactionData::try_from(tx_message_parsed) {
                    Ok(converted_tx_data) => converted_tx_data, // 转换成功
                    Err(e) => { // 如果转换失败 (例如Base64解码错误或BCS反序列化错误)
                        error!("无效的私有交易消息 (TxMessage转换为TransactionData失败): {:?}, 跳过该消息。", e);
                        continue; // 记录错误，并跳过这条消息。
                    }
                };

                // 如果所有步骤都成功，就从流中 `yield` (产生) 一个 `Event::PrivateTx` 事件。
                // 这个事件封装了从私有渠道获取并成功解析的 `TransactionData`。
                // Send out the successfully processed private transaction as an event.
                yield Event::PrivateTx(tx_data);
            }
            info!("PrivateTxCollector 的 WebSocket 读取流结束。"); // 当 read.next() 返回 None 时，循环会结束
        };

        // 将创建的异步流固定到堆上并返回。
        Ok(Box::pin(stream))
    }
}

[end of bin/arb/src/collector.rs]
