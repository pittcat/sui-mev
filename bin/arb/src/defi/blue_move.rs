// 该文件 `blue_move.rs` 实现了与 BlueMove 协议进行交互的相关逻辑。
// BlueMove 最初是 Sui 区块链上一个知名的 NFT (非同质化代币) 市场。
// 然而，它也可能提供或集成了某些去中心化交易所 (DEX) 的功能，允许用户进行代币交换。
// 从这个文件的实现来看，特别是 `extend_trade_tx` 方法中对 `CETUS_AGGREGATOR` 的使用，
// 强烈暗示了这里的 BlueMove 交互很可能是通过 Cetus DEX 的聚合器（Aggregator）智能合约来完成的。
// 这意味着，当程序要执行一个涉及BlueMove的交易时，它实际上是调用Cetus聚合器的某个函数，
// 而聚合器合约内部再负责将这个交易路由到BlueMove的相应资金池（如果BlueMove是当时的最优选择之一）。
//
// **文件概览 (File Overview)**:
// 这个 `blue_move.rs` 文件是用来和Sui上的BlueMove协议“沟通”的代码。
// BlueMove一开始主要是做NFT（就是那些独一无二的数字藏品，比如数字艺术画）交易的市场。
// 但是，它也可能自己做了一些或者整合了别人的DEX功能，让大家可以换不同种类的代币。
// 从代码里看，尤其是它用到了一个叫做 `CETUS_AGGREGATOR` 的东西，这很可能说明：
// 当我们的机器人想通过BlueMove换代币时，它并不是直接和BlueMove的合约打交道，
// 而是通过Cetus（另一个DEX）提供的一个“聚合器”合约来完成的。
// 这个聚合器就像一个中介，它知道很多DEX（包括BlueMove）的池子，会帮你找到最划算的路径去换。
// (This `blue_move.rs` file contains code for "communicating" with the BlueMove protocol on Sui.
//  BlueMove initially started as a well-known marketplace for NFTs (Non-Fungible Tokens, those unique digital collectibles like digital art).
//  However, it might also have developed its own or integrated others' DEX functionalities, allowing users to swap different kinds of tokens.
//  Looking at the code, especially its use of something called `CETUS_AGGREGATOR`, it strongly suggests that:
//  When our bot wants to swap tokens via BlueMove, it's not directly interacting with BlueMove's contracts.
//  Instead, it's done through an "Aggregator" contract provided by Cetus (another DEX).
//  This aggregator acts like an intermediary; it knows about pools on many DEXs (including BlueMove) and helps find the most cost-effective path for your swap.)
//
// **主要内容 (Main Contents)**:
//
// 1.  **常量定义 (Constant Definitions)**:
//     -   `DEX_INFO`: 这是一个关键的“身份证号”（ObjectID），指向BlueMove（或者它在Cetus聚合器里注册信息）的一个叫做 `Dex_Info` 的对象。
//         这个 `Dex_Info` 对象里可能存着BlueMove DEX功能相关的设置、状态或者用来指路的信息。
//         (This is a key "ID card number" (ObjectID) pointing to an object called `Dex_Info` for BlueMove (or its registration information within the Cetus aggregator).
//          This `Dex_Info` object might store settings, status, or routing information related to BlueMove's DEX functionality.)
//
// 2.  **`ObjectArgs` 结构体与 `OBJ_CACHE`**:
//     -   `ObjectArgs`: 用来把上面 `Dex_Info` 对象的引用信息（`ObjectArg`格式）打包存起来。
//     -   `OBJ_CACHE`: 一个一次性初始化并全局共享的缓存，用来提高获取 `Dex_Info` 对象引用的效率。
//         (Similar to other files, `ObjectArgs` is used to package and cache reference information for the `Dex_Info` object.
//          `OBJ_CACHE` is a globally shared cache initialized once to improve efficiency in fetching this object reference.)
//
// 3.  **`BlueMove` 结构体**:
//     -   代表BlueMove上的一个交易池实例，或者更准确地说，是通过Cetus聚合器可以访问到的、可能属于BlueMove（或其他协议）的某个特定代币交易对。
//     -   包含了与这个交易对互动所需的信息，如原始池信息（从`dex_indexer`服务获取）、流动性估算、两种代币的类型、调用合约时需要的特定“类型参数”（`type_params`），以及缓存的 `dex_info` 对象参数。
//     -   最重要的是，它也实现了项目里定义的 `Dex` 通用接口。
//         (Represents a trading pool instance on BlueMove, or more accurately, a specific token trading pair accessible via the Cetus aggregator that might belong to BlueMove (or another protocol).
//          It contains information needed to interact with this trading pair, such as original pool info (from `dex_indexer`), liquidity estimation, types of the two tokens, specific "type parameters" (`type_params`) for contract calls, and the cached `dex_info` object parameter.
//          Most importantly, it also implements the project's defined `Dex` common interface.)
//
// 4.  **`BlueMove::new()` 构造函数**:
//     -   异步方法，根据从`dex_indexer`获取的池信息和指定的输入代币类型来初始化一个 `BlueMove` 实例。
//     -   它可能会去链上读取这个池子对象的详细数据（比如检查池子是否被“冻结”了，或者获取LP代币供应量来估算流动性）。
//     -   它假设是两种代币的池子，所以会自动推断出另一种代币（“输出代币”）是什么。
//     -   它会准备好调用合约时需要的泛型类型参数（通常就是交易对的两种代币类型）。
//         (Asynchronous method to initialize a `BlueMove` instance based on pool information from `dex_indexer` and a specified input coin type.
//          It might read detailed data of the pool object from the chain (e.g., to check if the pool is "frozen", or to get LP token supply for liquidity estimation).
//          It assumes a two-coin pool, so it automatically infers the other token (the "output coin").
//          It prepares the generic type parameters needed for contract calls (usually the two coin types of the trading pair).)
//
// 5.  **交易构建逻辑 (Transaction Building Logic)**:
//     -   `build_swap_tx()` 和 `build_swap_args()`：内部辅助函数，用来准备在BlueMove上（通过Cetus聚合器）进行代币交换时需要发送给Sui区块链的指令和参数。
//     -   调用的合约函数名是 `swap_a2b` 和 `swap_b2a`，这说明它区分了交易方向（比如从币A到币B，或从币B到币A）。
//         (Internal helper functions for preparing instructions and parameters to be sent to the Sui blockchain when performing a token swap on BlueMove (via Cetus aggregator).
//          The contract function names called are `swap_a2b` and `swap_b2a`, indicating it distinguishes trading directions (e.g., from coin A to coin B, or vice versa).)
//
// 6.  **`Dex` trait 实现**:
//     -   `BlueMove` 结构体实现了 `Dex` 接口要求的所有方法，比如：
//         -   `extend_trade_tx()`: 把BlueMove的交换操作指令（通过调用Cetus聚合器合约）添加到正在构建的Sui交易包（PTB）中。
//         -   其他如 `coin_in_type()`, `coin_out_type()`, `protocol()`, `liquidity()`, `object_id()`, `flip()`, `is_a2b()` 等，提供DEX实例的基本信息和操作。
//         (The `BlueMove` struct implements all methods required by the `Dex` interface, such as:
//          `extend_trade_tx()`: Adds BlueMove's swap operation instruction (by calling the Cetus aggregator contract) to the Sui transaction package (PTB) being built.
//          Others like `coin_in_type()`, `coin_out_type()`, `protocol()`, `liquidity()`, `object_id()`, `flip()`, `is_a2b()`, etc., provide basic information and operations for the DEX instance.)
//
// **相关的Sui区块链和DeFi概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **NFT Marketplace (NFT市场 / NFT Marketplace)**:
//     一个允许用户购买、出售、交易非同质化代币（NFT）的平台。BlueMove是Sui生态中一个主要的NFT市场。
//     (A platform that allows users to buy, sell, and trade Non-Fungible Tokens (NFTs). BlueMove is a major NFT marketplace in the Sui ecosystem.)
//
// -   **DEX Aggregator (DEX聚合器 / DEX Aggregator)**:
//     DEX聚合器是一种服务或智能合约，它旨在为用户找到代币交换的最优价格。
//     (A DEX aggregator is a service or smart contract designed to find the optimal price for users' token swaps.)
//     它通过连接到多个不同的DEX协议，查询它们各自的报价和流动性，然后智能地将用户的交易请求进行分割或路由到单个或多个DEX上执行，
//     以期达到整体上最好的成交结果（例如，换到最多的目标代币或支付最少的输入代币）。
//     (It connects to multiple different DEX protocols, queries their respective quotes and liquidity, and then intelligently splits or routes the user's trade request to one or more DEXs for execution,
//      aiming to achieve the best overall result (e.g., getting the most target tokens or paying the least input tokens).)
//     Cetus协议提供了一个聚合器功能，此文件中的BlueMove交互似乎就是利用了这一点。
//     (The Cetus protocol provides an aggregator function, and the BlueMove interaction in this file seems to leverage that.)
//     这意味着，即使我们认为是在和“BlueMove”交易，实际的执行路径可能是 Cetus聚合器 -> BlueMove池（或其他DEX池）。
//     (This means that even if we think we are trading with "BlueMove", the actual execution path might be Cetus Aggregator -> BlueMove Pool (or other DEX pools).)
//
// -   **`Dex_Info` Object (`Dex_Info` 对象)**:
//     这可能是一个由BlueMove协议或其使用的聚合器（如Cetus）在链上部署和维护的中心化对象。
//     (This might be a centralized object deployed and maintained on-chain by the BlueMove protocol or its aggregator (like Cetus).)
//     它可能存储了关于BlueMove DEX功能的重要状态信息、配置参数（如费率）、或者用于路由交易到不同池子的逻辑和数据。
//     (It might store important state information about BlueMove's DEX functionality, configuration parameters (like fees), or logic and data for routing trades to different pools.)
//     在执行与BlueMove相关的交易时，合约可能需要引用这个 `Dex_Info` 对象。
//     (When executing BlueMove-related trades, the contract might need to reference this `Dex_Info` object.)

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::sync::Arc; // `Arc` (Atomic Reference Counting) 用于在多线程/异步环境中安全地共享数据。
                    // `Arc` (Atomic Reference Counting) is used for safely sharing data in multi-threaded/asynchronous environments.

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` crate 引入 `Pool` (代表DEX池的原始信息) 和 `Protocol` (DEX协议枚举) 类型。
                                        // Import `Pool` (representing raw info of a DEX pool) and `Protocol` (DEX protocol enum) types from the `dex_indexer` crate.
use eyre::{ensure, eyre, OptionExt, Result}; // 从 `eyre` 库引入错误处理工具：
                                             // Import error handling tools from the `eyre` library:
                                             // `ensure!` 宏：检查条件，若为false则返回错误。 (`ensure!` macro: checks a condition, returns an error if false.)
                                             // `eyre!` 宏：创建新的错误实例。 (`eyre!` macro: creates a new error instance.)
                                             // `OptionExt` trait：为 `Option` 类型提供额外的便捷方法，如 `ok_or_eyre` (将None转为错误)。
                                             // (`OptionExt` trait: provides additional convenience methods for `Option` type, like `ok_or_eyre` (converts None to an error).)
                                             // `Result` 类型：`eyre`库的通用结果类型。 (`Result` type: `eyre` library's generic result type.)
use move_core_types::annotated_value::MoveStruct; // 从 `move_core_types` 库引入 `MoveStruct`，用于表示从链上获取的Move对象的反序列化结构。
                                                 // Import `MoveStruct` from `move_core_types` library, used to represent deserialized structure of Move objects fetched from on-chain.
use simulator::Simulator; // 从 `simulator` crate 引入 `Simulator` trait，定义了交易模拟器的通用接口。
                         // Import `Simulator` trait from `simulator` crate, defining a common interface for transaction simulators.
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui核心类型：对象ID, 对象引用, Sui地址。 (Sui core types: Object ID, Object Reference, Sui Address.)
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建相关类型：PTB参数、指令、对象参数、PTB结构、完整交易数据。
                                                                                         // Sui transaction building related types: PTB argument, command, object argument, PTB structure, full transaction data.
    Identifier, TypeTag, // `Identifier`: Move语言中的标识符（如模块名、函数名）。 (`Identifier`: Identifier in Move language (e.g., module name, function name).)
                         // `TypeTag`: 运行时表示Move类型（如代币类型）。 (`TypeTag`: Represents a Move type at runtime (e.g., coin type).)
};
use tokio::sync::OnceCell; // 从 `tokio` 库引入 `OnceCell`，用于异步环境下的单次初始化。
                           // Import `OnceCell` from `tokio` library, for single initialization in asynchronous environments.
use utils::{coin, new_test_sui_client, object::*}; // 从项目内部的 `utils` 工具库引入：
                                                   // Import from the project's internal `utils` utility library:
                                                   // `coin` 模块：代币操作辅助函数。( `coin` module: helper functions for coin operations.)
                                                   // `new_test_sui_client` 函数：创建Sui客户端实例（主要用于测试）。
                                                   // (`new_test_sui_client` function: creates a Sui client instance (mainly for testing).)
                                                   // `object::*`：导入 `utils::object` 模块所有公共项，用于处理Sui对象数据。
                                                   // (`object::*`: imports all public items from `utils::object` module, for handling Sui object data.)

use super::{TradeCtx, CETUS_AGGREGATOR}; // 从父模块 (`defi`) 引入 `TradeCtx` (交易上下文) 和 `CETUS_AGGREGATOR` (Cetus聚合器包ID常量)。
                                         // Import `TradeCtx` (transaction context) and `CETUS_AGGREGATOR` (Cetus aggregator package ID constant) from the parent module (`defi`).
use crate::{config::*, defi::Dex}; // 从当前crate的根作用域引入 `config` 模块所有项和 `defi::Dex` trait。
                                  // Import all items from the `config` module and the `defi::Dex` trait from the current crate's root scope.

// `DEX_INFO`: BlueMove 的 `Dex_Info` 对象的全局唯一 `ObjectID` 字符串。
// (`DEX_INFO`: Globally unique `ObjectID` string for BlueMove's `Dex_Info` object.)
// 这个对象被认为是BlueMove DEX功能（或者其在Cetus聚合器中的表示）的一个关键配置或状态对象。
// (This object is considered a key configuration or state object for BlueMove's DEX functionality (or its representation in the Cetus aggregator).)
// 在与BlueMove相关的交易中，合约可能需要引用这个对象。
// (Contracts might need to reference this object in BlueMove-related transactions.)
const DEX_INFO: &str = "0x3f2d9f724f4a1ce5e71676448dc452be9a6243dac9c5b975a588c8c867066e92";

// `OBJ_CACHE`: 一个静态的、线程安全的 `OnceCell<ObjectArgs>` 实例。
// (`OBJ_CACHE`: A static, thread-safe `OnceCell<ObjectArgs>` instance.)
// 用于全局缓存 `ObjectArgs` 结构体（这里面只包含了 `dex_info`）。
// (Used for globally caching the `ObjectArgs` struct (which only contains `dex_info` here).)
// 目的是确保 `dex_info` 对象只从链上获取和处理一次。
// (The purpose is to ensure the `dex_info` object is fetched and processed from the chain only once.)
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数 (获取对象参数函数 / Get Object Arguments Function)
///
/// 负责获取并缓存 `ObjectArgs` 结构体（当前只包含 `dex_info`）。
/// (Responsible for fetching and caching the `ObjectArgs` struct (currently only contains `dex_info`).)
/// 如果 `OBJ_CACHE` 尚未初始化，它会异步地：
/// (If `OBJ_CACHE` has not been initialized, it will asynchronously:)
/// 1. 从 `DEX_INFO` 常量字符串解析出 `ObjectID`。
///    (Parse `ObjectID` from the `DEX_INFO` constant string.)
/// 2. 使用传入的 `simulator` 从Sui网络获取该 `ObjectID` 对应的链上对象数据。
///    (Use the passed `simulator` to fetch on-chain object data corresponding to this `ObjectID` from the Sui network.)
/// 3. 将获取到的对象数据转换为构建PTB时所需的 `ObjectArg` 类型。
///    (Convert the fetched object data into the `ObjectArg` type required for building PTBs.)
/// 4. 用这个 `ObjectArg` 创建 `ObjectArgs` 实例，并将其存入 `OBJ_CACHE`。
///    (Create an `ObjectArgs` instance with this `ObjectArg` and store it in `OBJ_CACHE`.)
/// 后续调用此函数会直接从缓存中获取 `ObjectArgs` 的克隆副本。
/// (Subsequent calls to this function will directly fetch a cloned copy of `ObjectArgs` from the cache.)
///
/// **参数 (Parameters)**:
/// - `simulator`: 一个共享的模拟器实例 (`Arc<Box<dyn Simulator>>`)。(A shared simulator instance.)
///
/// **返回 (Returns)**:
/// - `ObjectArgs`: 包含 `dex_info` 的 `ObjectArgs` 结构体的克隆副本。(A cloned copy of the `ObjectArgs` struct containing `dex_info`.)
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async { // 如果未初始化，则执行异步闭包 (If not initialized, execute the async closure)
            let id = ObjectID::from_hex_literal(DEX_INFO).unwrap(); // 将DEX_INFO字符串转为ObjectID，unwrap假设总是成功 (Convert DEX_INFO string to ObjectID, unwrap assumes success)
            let dex_info_obj = simulator.get_object(&id).await.unwrap(); // 异步获取对象，unwrap假设成功 (Async fetch object, unwrap assumes success)

            ObjectArgs {
                // `shared_obj_arg` 是一个辅助函数，用于将 `SuiObject` 转换为 `ObjectArg`。
                // (`shared_obj_arg` is a helper function to convert `SuiObject` to `ObjectArg`.)
                // 第二个参数 `true` 表示这个 `dex_info` 对象在交易中预期是可变的（mutable）。
                // (The second argument `true` indicates this `dex_info` object is expected to be mutable in transactions.)
                dex_info: shared_obj_arg(&dex_info_obj, true),
            }
        })
        .await // 等待初始化完成 (Wait for initialization to complete)
        .clone() // 克隆缓存中的值返回 (Clone the cached value and return)
}

/// `ObjectArgs` 结构体 (对象参数结构体 / Object Arguments Struct)
///
/// 用于缓存与BlueMove交互时所需的关键对象的 `ObjectArg` 形式。
/// (Used to cache the `ObjectArg` form of key objects required for BlueMove interaction.)
/// 目前，它只包含 `dex_info`。
/// (Currently, it only contains `dex_info`.)
/// `#[derive(Clone)]` 允许此结构体实例被克隆。
/// (`#[derive(Clone)]` allows instances of this struct to be cloned.)
#[derive(Clone)]
pub struct ObjectArgs {
    dex_info: ObjectArg, // BlueMove 的 Dex_Info 对象的 `ObjectArg` 表示。(BlueMove's Dex_Info object's `ObjectArg` representation.)
}

/// `BlueMove` 结构体 (BlueMove Struct)
///
/// 代表一个BlueMove的交易池实例，或者更准确地说是通过Cetus聚合器可以访问到的、
/// 可能属于BlueMove或其他协议的某个特定交易对。
/// (Represents a BlueMove trading pool instance, or more accurately, a specific trading pair
///  accessible via the Cetus aggregator, which might belong to BlueMove or other protocols.)
/// 它封装了与这个交易对进行交互所需的状态信息和参数。
/// (It encapsulates the state information and parameters required for interacting with this trading pair.)
///
/// `#[derive(Clone)]` 允许 `BlueMove` 实例被克隆。
/// (`#[derive(Clone)]` allows `BlueMove` instances to be cloned.)
#[derive(Clone)]
pub struct BlueMove {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息 (`Pool` 类型包含了池ID、代币类型等)。
                             // (Original pool information (`Pool` type includes pool ID, coin types, etc.) from `dex_indexer`.)
    liquidity: u128,         // 池的流动性估算值。在 `BlueMove::new` 中，它被设置为池的LP代币供应量。
                             // (Estimated liquidity of the pool. In `BlueMove::new`, it's set to the pool's LP token supply.)
    coin_in_type: String,    // 当前配置的交易方向下，输入代币的Sui类型字符串。
                             // (Sui type string of the input coin for the currently configured trading direction.)
    coin_out_type: String,   // 当前配置的交易方向下，输出代币的Sui类型字符串。
                             // (Sui type string of the output coin for the currently configured trading direction.)
    type_params: Vec<TypeTag>,// 调用Swap合约函数时所需的泛型类型参数列表。
                              // (List of generic type parameters required when calling the Swap contract function.)
                              // 对于双币池，这通常是 `[CoinTypeA, CoinTypeB]`，其中A和B是池中的两种代币。
                              // (For a two-coin pool, this is usually `[CoinTypeA, CoinTypeB]`, where A and B are the two coins in the pool.)
    dex_info: ObjectArg,     // 从 `OBJ_CACHE` 获取的、BlueMove（或其聚合器）的 `Dex_Info` 对象的 `ObjectArg` 表示。
                             // (`ObjectArg` representation of BlueMove's (or its aggregator's) `Dex_Info` object, obtained from `OBJ_CACHE`.)
}

impl BlueMove {
    /// `new` 构造函数 (异步) (new constructor (asynchronous))
    ///
    /// 根据从 `dex_indexer` 获取到的原始 `Pool` 信息和用户指定的输入代币类型 (`coin_in_type`)，
    /// 来创建一个 `BlueMove` DEX实例。
    /// (Creates a `BlueMove` DEX instance based on original `Pool` information from `dex_indexer` and user-specified input coin type (`coin_in_type`).)
    /// 这个构造函数假设BlueMove的池（或通过聚合器访问的池）是双币池，因此输出代币类型会根据输入代币类型自动推断出来。
    /// (This constructor assumes BlueMove's pools (or pools accessed via aggregator) are two-coin pools, so the output coin type is automatically inferred from the input coin type.)
    ///
    /// **参数 (Parameters)**:
    /// - `simulator`: 一个共享的模拟器实例 (`Arc<Box<dyn Simulator>>`)。(A shared simulator instance.)
    /// - `pool_info`: 一个对从 `dex_indexer` 获取的 `Pool` 结构体的引用。(A reference to the `Pool` struct from `dex_indexer`.)
    /// - `coin_in_type`: 输入代币的Sui类型字符串。(Sui type string of the input coin.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Self>`: 如果成功初始化，返回一个 `BlueMove` 实例；否则返回错误。(Returns a `BlueMove` instance if successfully initialized; otherwise, returns an error.)
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        ensure!(pool_info.protocol == Protocol::BlueMove, "池协议非BlueMove (Pool protocol is not BlueMove)");

        let parsed_pool_struct = {
            let pool_obj = simulator.get_object(&pool_info.pool).await
                .ok_or_else(|| eyre!("BlueMove池对象 {} 未找到 (BlueMove pool object {} not found)", pool_info.pool))?;
            let layout = simulator.get_object_layout(&pool_info.pool)
                .ok_or_eyre(format!("BlueMove池 {} 布局未找到 (Layout for BlueMove pool {} not found)", pool_info.pool))?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre(format!("对象 {} 非Move对象 (Object {} is not a Move object)", pool_info.pool))?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!("反序列化BlueMove池 {} 失败: {} (Failed to deserialize BlueMove pool {}: {})", pool_info.pool, e))?
        };

        let is_freeze = extract_bool_from_move_struct(&parsed_pool_struct, "is_freeze")?;
        ensure!(!is_freeze, "BlueMove池 {} 已冻结 (BlueMove pool {} is frozen)", pool_info.pool);

        let liquidity = {
            let lsp_supply_struct = extract_struct_from_move_struct(&parsed_pool_struct, "lsp_supply")?;
            extract_u64_from_move_struct(&lsp_supply_struct, "value")? as u128
        };

        let coin_out_type = if let Some(0) = pool_info.token_index(coin_in_type) {
            pool_info.token1_type()
        } else {
            pool_info.token0_type()
        };

        let type_params = parsed_pool_struct.type_.type_params.clone(); // 通常是 [CoinA, CoinB]
        let ObjectArgs { dex_info } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(), liquidity,
            coin_in_type: coin_in_type.to_string(), coin_out_type,
            type_params, dex_info,
        })
    }

    /// `build_swap_tx` (私有辅助函数，构建完整PTB / Private helper, builds full PTB)
    #[allow(dead_code)]
    async fn build_swap_tx(
        &self, sender: SuiAddress, recipient: SuiAddress,
        coin_in_ref: ObjectRef, amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default();
        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?; // `None` for amount_in as aggregator usually takes full coin
        ctx.transfer_arg(recipient, coin_out_arg);
        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数，构建调用合约参数 / Private helper, builds contract call arguments)
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let dex_info_arg = ctx.obj(self.dex_info).map_err(|e| eyre!("转换dex_info失败: {} (Failed to convert dex_info: {})", e))?;
        Ok(vec![dex_info_arg, coin_in_arg]) // 参数顺序：dex_info, coin_in (Argument order: dex_info, coin_in)
    }
}

/// 为 `BlueMove` 结构体实现 `Dex` trait。(Implement `Dex` trait for `BlueMove` struct.)
#[async_trait::async_trait]
impl Dex for BlueMove {
    /// `extend_trade_tx` 方法 (将BlueMove交换操作添加到PTB / Add BlueMove swap op to PTB method)
    async fn extend_trade_tx(
        &self, ctx: &mut TradeCtx, _sender: SuiAddress, // sender 未使用 (sender is unused)
        coin_in_arg: Argument, _amount_in: Option<u64>, // amount_in 未使用 (amount_in is unused)
    ) -> Result<Argument> {
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        // (**IMPORTANT**: Package ID uses `CETUS_AGGREGATOR`.)
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("bluemove").map_err(|e| eyre!("创建模块名'bluemove'失败: {} (Failed to create module name 'bluemove': {})", e))?;
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!("创建函数名'{}'失败: {} (Failed to create function name '{}': {})", function_name_str, e))?;

        let mut type_arguments = self.type_params.clone(); // [PoolToken0, PoolToken1]
        if !self.is_a2b() { // 如果是 B->A (coin_in is PoolToken1) (If B->A (coin_in is PoolToken1))
            type_arguments.swap(0, 1); // 交换为 [PoolToken1, PoolToken0] (Swap to [PoolToken1, PoolToken0])
        }

        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        Ok(Argument::Result(ctx.last_command_idx())) // 返回输出代币 (Return the output coin)
    }

    // --- Dex trait 的其他 getter 和 setter 方法 ---
    // (Other getter and setter methods for Dex trait)
    fn coin_in_type(&self) -> String { self.coin_in_type.clone() }
    fn coin_out_type(&self) -> String { self.coin_out_type.clone() }
    fn protocol(&self) -> Protocol { Protocol::BlueMove } // 协议类型为BlueMove (Protocol type is BlueMove)
    fn liquidity(&self) -> u128 { self.liquidity }
    fn object_id(&self) -> ObjectID { self.pool.pool } // 池的ObjectID (Pool's ObjectID)

    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        // type_params 在 extend_trade_tx 中根据 is_a2b() 动态调整，这里无需修改原始的池代币顺序。
        // (type_params are dynamically adjusted in extend_trade_tx based on is_a2b(), no need to modify original pool coin order here.)
    }

    fn is_a2b(&self) -> bool { // 判断 coin_in_type 是否是池的 token0 (Check if coin_in_type is pool's token0)
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试 / Mainly for testing)
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await;
        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;
        let programmable_tx_block = self.build_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in).await?;
        let gas_coins = coin::get_gas_coin_refs(&sui_client, sender, Some(coin_in_obj.coin_object_id)).await?;
        let gas_price = sui_client.read_api().get_reference_gas_price().await?;
        Ok(TransactionData::new_programmable(sender, gas_coins, programmable_tx_block, GAS_BUDGET, gas_price))
    }
}

// --- 测试模块 ---
// (Test module)
#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use itertools::Itertools;
    use object_pool::ObjectPool;
    use simulator::{DBSimulator, HttpSimulator, Simulator};
    use tracing::info;
    use super::*;
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL},
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher},
    };

    /// `test_bluemove_swap_tx` 测试函数 (test_bluemove_swap_tx test function)
    #[tokio::test]
    async fn test_bluemove_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);
        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap();
        let recipient = SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI";
        let token_out_type = "0x0bffc4f0333fb1256431156395a93fc252432152b0ff732197e8459a365e5a9f::suicat::SUICAT"; // 示例代币 (Example coin)
        let amount_in = 10000; // 0.00001 SUI

        let simulator_pool_for_searcher = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new().unwrap().block_on(async { Box::new(DBSimulator::new_test(true).await) as Box<dyn Simulator> })
        }));

        let searcher = IndexerDexSearcher::new(TEST_HTTP_URL, simulator_pool_for_searcher).await.unwrap();
        let dexes = searcher.find_dexes(token_in_type, Some(token_out_type.into())).await.unwrap();
        info!("🧀 (测试信息) 找到的DEX总数量 (Total DEXs found): {}", dexes.len());

        let dex_to_test = dexes.into_iter()
            .filter(|dex| dex.protocol() == Protocol::BlueMove)
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity()))
            .last()
            .expect("测试中未找到BlueMove池 (BlueMove pool not found in test)");

        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 (测试信息) 构建的BlueMove交换交易数据 (Constructed BlueMove swap tx data): {:?}", tx_data);

        let response = http_simulator.simulate(tx_data, Default::default()).await.unwrap();
        info!("🧀 (测试信息) BlueMove交换交易的模拟结果 (BlueMove swap tx simulation result): {:?}", response);

        assert!(response.is_ok(), "BlueMove交换交易的模拟应成功执行 (BlueMove swap tx simulation should succeed)");
    }
}

[end of bin/arb/src/defi/blue_move.rs]
