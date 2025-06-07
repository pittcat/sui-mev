// 该文件 `cetus.rs` 实现了与 Cetus 协议进行交互的逻辑。
// Cetus 是 Sui 区块链上一个知名的去中心化交易所 (DEX) 和流动性协议。
// Cetus 的一个显著特点是它采用了“集中流动性做市商”（CLMM, Concentrated Liquidity Market Maker）模型，
// 并且它还提供了“闪电贷”（Flashloan）功能，这对于执行某些高级DeFi策略（如套利）非常有用。
//
// **文件概览 (File Overview)**:
// 这个 `cetus.rs` 文件是专门用来和Sui区块链上的Cetus这个DeFi协议“对话”的代码。
// Cetus是一个“去中心化交易所”（DEX），就像一个不需要人工干预的货币兑换处。
// 它有两个特别之处：
// 1.  **集中流动性做市商 (CLMM)**：传统的DEX（比如早期的Uniswap V2）要求你提供流动性（比如SUI和USDC）时，资金会在从0到无穷大的所有可能价格区间内平均分配。
//     而CLMM允许你把资金“集中”在你认为最常发生交易的价格范围，这样可以更有效地利用资金，获得更高的手续费收益，并为交易者提供更好的价格（更小的滑点）。
// 2.  **闪电贷 (Flashloan)**：这是一种神奇的贷款，你可以在一笔交易（一个原子操作）中借到一大笔钱，但前提是你必须在同一笔交易结束前把钱（加上一点手续费）还回去。
//     如果还不回去，整个交易（包括借钱那一步）都会失败，就像什么都没发生过一样。这对于套利来说非常有用，因为套利者往往需要大量本金才能从微小的价格差中获利，
//     但他们可能自己没有那么多钱。有了闪电贷，他们可以临时借钱来完成套利操作。
//
// **这个文件里主要有哪些东西 (What's in this file)?**
//
// 1.  **常量定义 (Constant Definitions)**:
//     -   `CETUS_DEX`: Cetus核心智能合约的“门牌号”（Package ID）。
//     -   `CONFIG`: Cetus协议全局配置对象的“身份证号”（ObjectID）。这个对象里存着整个Cetus协议的一些通用设置。
//     -   `PARTNER`: Cetus合作伙伴对象的“身份证号”（ObjectID）。这个对象可能用来记录是谁推荐了这笔交易，或者给合作伙伴一些特殊待遇。
//
// 2.  **`ObjectArgs` 结构体与 `OBJ_CACHE` (ObjectArgs Struct and OBJ_CACHE)**:
//     -   `ObjectArgs`: 用来把上面 `CONFIG`、`PARTNER` 以及Sui系统时钟（`SUI_CLOCK_OBJECT_ID`，ObjectID是`0x6`，链上所有合约都可以读取它来获取当前时间）
//         这些经常要用到的对象的引用信息（`ObjectArg`格式）打包存起来。
//     -   `OBJ_CACHE`: 这是一个“一次性初始化并全局共享的缓存”。意思是，程序第一次需要这些对象信息时，会去链上查询一次，然后把结果存到这个`OBJ_CACHE`里。
//         以后再需要时，就直接从缓存里拿，不用每次都去麻烦Sui网络了，这样可以提高效率。
//
// 3.  **`Cetus` 结构体 (The `Cetus` Struct)**:
//     -   这个结构体代表了Cetus协议里的一个具体的“交易池”（Pool）。比如，一个SUI-USDC的交易池。
//     -   它里面存着与这个池子互动所需要的所有信息，比如池子的基本信息（从`dex_indexer`服务获取）、池子对象的引用（`pool_arg`）、
//         池子里有多少钱（流动性）、池子里是哪两种代币、调用合约时需要的特定“类型参数”（`type_params`，告诉合约具体操作的是哪两种代币），
//         以及从`OBJ_CACHE`里拿到的上面说的那些共享对象的引用。
//     -   最重要的是，这个 `Cetus` 结构体实现了我们项目里定义的 `Dex` 这个“通用DEX接口”（trait）。这意味着，无论是什么DEX（Cetus, Turbos, Kriya等），
//         只要它实现了 `Dex` 接口，我们的套利机器人就能用同样的方式去操作它。
//
// 4.  **`Cetus::new()` 构造函数 (The `Cetus::new()` Constructor)**:
//     -   这是一个异步函数（意味着它在等待链上数据时不会卡住整个程序），用来创建一个新的 `Cetus` 交易池实例。
//     -   它需要从`dex_indexer`（一个专门收集DEX信息的服务）那里拿到原始的池子信息，并知道我们要用哪种代币作为“输入代币”。
//     -   它会去链上读取这个池子对象的详细数据，检查池子是不是被暂停了，并提取出池子里的流动性有多少。
//     -   因为它假设这是一个两种代币的池子，所以它会自动推断出另一种代币（“输出代币”）是什么。
//
// 5.  **交换相关方法 (Swap-related Methods)**:
//     -   `build_swap_tx()` 和 `build_swap_args()`：这两个是内部辅助函数，用来准备进行普通代币交换时需要发送给Sui区块链的指令和参数。
//     -   Cetus的交换操作也区分方向，比如从代币A换到代币B（`swap_a2b`）和从代币B换到代币A（`swap_b2a`）可能是调用不同的合约函数。
//
// 6.  **闪电贷相关方法 (Flashloan-related Methods)**:
//     -   `build_flashloan_args()`: 准备调用Cetus发起闪电贷的合约函数（比如 `flash_swap_a2b`）时需要的参数。
//     -   `build_repay_args()`: 准备调用Cetus偿还闪电贷的合约函数（比如 `repay_flash_swap_a2b`）时需要的参数。
//     -   `extend_flashloan_tx()`: 实现了 `Dex` 接口里的方法。当我们的套利交易需要从Cetus借钱（闪电贷）时，这个函数会把借钱的指令添加到我们正在构建的Sui交易包里。
//     -   `extend_repay_tx()`: 同样实现了 `Dex` 接口。当套利操作完成，需要还钱给Cetus时，这个函数会把还钱的指令添加到交易包里。
//     -   `support_flashloan()`: 实现了 `Dex` 接口。它明确告诉大家：“是的，我（Cetus）支持闪电贷！”
//
// 7.  **`Dex` trait 实现 (Implementation of the `Dex` Trait)**:
//     -   除了上面提到的闪电贷方法，`Cetus` 结构体还实现了 `Dex` 接口要求的其他所有方法，比如：
//         -   `extend_trade_tx`: 用于普通的代币交换。
//         -   `coin_in_type()`, `coin_out_type()`: 获取当前配置的输入/输出代币类型。
//         -   `protocol()`: 返回 `Protocol::Cetus`，表明这是Cetus协议。
//         -   `liquidity()`: 返回池子的流动性。
//         -   `object_id()`: 返回池子对象的ID。
//         -   `flip()`: 翻转交易方向（比如从 SUI->USDC 变成 USDC->SUI）。
//         -   `is_a2b()`: 判断当前交易方向是不是从池子里的“代币A”到“代币B”。
//
// **相关的Sui区块链和DeFi概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **集中流动性做市商 (CLMM / Concentrated Liquidity Market Maker)**:
//     这是一种更高级的自动做市商模型。传统的AMM（比如Uniswap V2）要求流动性提供者（LP）将其资金分散在从0到无穷大的整个价格曲线上。
//     而CLMM允许LP将其资金集中在他们认为最有可能发生交易的特定价格区间内。
//     这样做的好处是：
//     1.  **资本效率更高**：在活跃的交易价格附近，可以提供更“厚”的流动性，从而减少大额交易的价格滑点。
//     2.  **LP收益可能更高**：因为他们的资金更集中在交易发生的地方，所以能赚取更多的交易手续费。
//     Cetus 就是 Sui 上采用 CLMM 模型的 DEX 之一。
//
// -   **闪电贷 (Flashloan)**:
//     一种特殊的无抵押贷款，但它有一个非常严格的规则：你必须在借款的**同一笔交易内**把本金和利息（如果有的话）全部还清。
//     如果在交易结束时你没有还钱，那么整笔交易（包括你借钱的那一步）都会失败回滚，就好像什么都没发生过一样。
//     这保证了借出方不会损失资金。
//     闪电贷对于套利者来说是一个强大的工具，因为：
//     -   **无需自有大额资金**：套利机会可能需要大量本金才能获得可观利润，闪电贷解决了这个问题。
//     -   **原子性操作**：借款、执行套利、还款都在一笔交易内完成，要么全部成功，要么全部失败，风险相对可控。
//     Cetus 提供了闪电贷功能，这意味着套利机器人可以利用它来执行需要大量初始资金的套利策略。
//
// -   **Sui 时钟对象 (`0x6` / Sui Clock Object)**:
//     这是Sui系统内置的一个特殊的、全局共享的、只读的系统对象，它的ObjectID固定为`0x6`。
//     任何智能合约都可以读取这个Clock对象来获取当前网络的Unix时间戳（以毫秒为单位）等与时间相关的信息。
//     许多DeFi协议（包括Cetus）在执行某些操作时（比如检查订单是否超时、计算随时间变化的费率、确定某些操作的有效期限等）可能需要引用这个Clock对象作为参数。

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::sync::Arc; // `Arc` (Atomic Reference Counting) 用于在多线程/异步环境中安全地共享数据。
                    // `Arc` (Atomic Reference Counting) is used for safely sharing data in multi-threaded/asynchronous environments.

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` crate 引入 `Pool` 和 `Protocol` 类型。
                                        // Import `Pool` and `Protocol` types from the `dex_indexer` crate.
use eyre::{ensure, eyre, OptionExt, Result}; // `eyre` 错误处理库。
                                             // `eyre` error handling library.
use move_core_types::annotated_value::MoveStruct; // 用于解析Move对象的结构。
                                                 // Used for parsing the structure of Move objects.
use simulator::Simulator; // 交易模拟器接口。 (Transaction simulator interface.)
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui核心基本类型。 (Sui core basic types.)
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建相关类型。
                                                                                        // Sui transaction building related types.
    Identifier, TypeTag, SUI_CLOCK_OBJECT_ID, // `Identifier` (Move标识符), `TypeTag` (Move类型标签), `SUI_CLOCK_OBJECT_ID` (Sui时钟对象的固定ID, "0x6")。
                                              // `Identifier` (Move identifier), `TypeTag` (Move type tag), `SUI_CLOCK_OBJECT_ID` (Fixed ID of Sui clock object, "0x6").
};
use tokio::sync::OnceCell; // Tokio提供的异步单次初始化单元。
                           // Tokio's asynchronous single initialization cell.
use utils::{coin, new_test_sui_client, object::*}; // 项目内部的工具库：`coin` (代币操作), `new_test_sui_client` (创建测试客户端), `object` (Sui对象处理)。
                                                   // Internal utility library of the project: `coin` (token operations), `new_test_sui_client` (create test client), `object` (Sui object handling).

use super::{trade::FlashResult, TradeCtx}; // 从父模块 (`defi`) 引入 `FlashResult` (闪电贷操作结果的封装) 和 `TradeCtx` (交易上下文)。
                                         // Import `FlashResult` (encapsulation of flash loan operation result) and `TradeCtx` (transaction context) from the parent module (`defi`).
use crate::{config::*, defi::Dex}; // 从当前crate的根作用域引入 `config` 模块所有项和 `defi::Dex` trait。
                                  // Import all items from the `config` module and the `defi::Dex` trait from the current crate's root scope.

// --- Cetus协议相关的常量定义 ---
// (Constant definitions related to the Cetus protocol)

// `CETUS_DEX`: Cetus Swap 核心合约的Sui Package ID (包ID) 字符串。
// (Sui Package ID string of the Cetus Swap core contract.)
const CETUS_DEX: &str = "0xeffc8ae61f439bb34c9b905ff8f29ec56873dcedf81c7123ff2f1f67c45ec302";
// `CONFIG`: Cetus 协议的全局配置对象 (`GlobalConfig Object`) 的 `ObjectID` 字符串。
// (ObjectID string of the Cetus protocol's global configuration object (`GlobalConfig Object`).)
// 这个对象存储了协议级别的设置，如默认费率、协议开关等。
// (This object stores protocol-level settings, such as default fees, protocol switches, etc.)
const CONFIG: &str = "0xdaa46292632c3c4d8f31f23ea0f9b36a28ff3677e9684980e4438403a67a3d8f";
// `PARTNER`: Cetus 的合作伙伴对象 (`Partner Object`) 的 `ObjectID` 字符串。
// (ObjectID string of Cetus's partner object (`Partner Object`).)
// 这个对象可能用于记录推荐关系、为特定合作伙伴分配不同的手续费结构或启用某些合作伙伴特有的功能。
// (This object might be used for recording referral relationships, assigning different fee structures for specific partners, or enabling certain partner-specific features.)
const PARTNER: &str = "0x639b5e433da31739e800cd085f356e64cae222966d0f1b11bd9dc76b322ff58b";

/// `ObjectArgs` 结构体 (对象参数结构体 / Object Arguments Struct)
///
/// 用于缓存与Cetus协议交互时所需的关键共享对象的 `ObjectArg` 形式。
/// (Used to cache the `ObjectArg` form of key shared objects required when interacting with the Cetus protocol.)
/// `ObjectArg` 是在构建Sui可编程交易块 (PTB) 时实际使用的对象引用类型。
/// (`ObjectArg` is the object reference type actually used when building Sui Programmable Transaction Blocks (PTBs).)
/// 通过 `OnceCell` 实现这些对象参数的异步单次初始化和全局缓存，可以提高效率。
/// (Using `OnceCell` for asynchronous single initialization and global caching of these object parameters can improve efficiency.)
#[derive(Clone)] // 允许结构体实例被克隆 (Allows struct instances to be cloned)
pub struct ObjectArgs {
    config: ObjectArg,  // Cetus全局配置对象的 `ObjectArg`。(Cetus global config object's `ObjectArg`.)
    partner: ObjectArg, // Cetus合作伙伴对象的 `ObjectArg`。(Cetus partner object's `ObjectArg`.)
    clock: ObjectArg,   // Sui系统时钟对象 (`0x6`) 的 `ObjectArg`。(Sui system clock object (`0x6`)'s `ObjectArg`.)
}

// `OBJ_CACHE`: 一个静态的、线程安全的 `OnceCell<ObjectArgs>` 实例。
// (`OBJ_CACHE`: A static, thread-safe `OnceCell<ObjectArgs>` instance.)
// 用于全局缓存 `ObjectArgs` 结构体。
// (Used for globally caching the `ObjectArgs` struct.)
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数 (获取对象参数函数 / Get Object Arguments Function)
///
/// 负责获取并缓存 `ObjectArgs` 结构体（包含Cetus的 `config`、`partner` 对象以及Sui的 `clock` 对象）。
/// (Responsible for fetching and caching the `ObjectArgs` struct (containing Cetus's `config`, `partner` objects, and Sui's `clock` object).)
/// 如果 `OBJ_CACHE` 尚未初始化，它会异步地：
/// (If `OBJ_CACHE` has not been initialized, it will asynchronously:)
/// 1. 从常量字符串解析出 `config` 和 `partner` 对象的 `ObjectID`。
///    (Parse `ObjectID`s for `config` and `partner` objects from constant strings.)
/// 2. 使用传入的 `simulator` 从Sui网络获取这三个对象（`config`, `partner`, `SUI_CLOCK_OBJECT_ID`）的链上数据。
///    (Use the passed `simulator` to fetch on-chain data for these three objects (`config`, `partner`, `SUI_CLOCK_OBJECT_ID`) from the Sui network.)
/// 3. 将获取到的对象数据转换为构建PTB时所需的 `ObjectArg` 类型。
///    (Convert the fetched object data into the `ObjectArg` type required for building PTBs.)
/// 4. 用这些 `ObjectArg` 创建 `ObjectArgs` 实例，并将其存入 `OBJ_CACHE`。
///    (Create an `ObjectArgs` instance with these `ObjectArg`s and store it in `OBJ_CACHE`.)
/// 后续调用此函数会直接从缓存中获取 `ObjectArgs` 的克隆副本。
/// (Subsequent calls to this function will directly fetch a cloned copy of `ObjectArgs` from the cache.)
///
/// **参数 (Parameters)**:
/// - `simulator`: 一个共享的模拟器实例 (`Arc<Box<dyn Simulator>>`)。(A shared simulator instance.)
///
/// **返回 (Returns)**:
/// - `ObjectArgs`: 包含所需对象参数的 `ObjectArgs` 结构体的克隆副本。(A cloned copy of the `ObjectArgs` struct containing the required object parameters.)
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async { // 如果未初始化，则执行异步闭包 (If not initialized, execute the async closure)
            let config_id = ObjectID::from_hex_literal(CONFIG).unwrap(); // 解析CONFIG的ObjectID (Parse CONFIG's ObjectID)
            let partner_id = ObjectID::from_hex_literal(PARTNER).unwrap(); // 解析PARTNER的ObjectID (Parse PARTNER's ObjectID)

            // 通过模拟器异步获取链上对象数据 (Asynchronously fetch on-chain object data via simulator)
            let config_obj = simulator.get_object(&config_id).await.unwrap(); // 获取config对象 (Get config object)
            let partner_obj = simulator.get_object(&partner_id).await.unwrap(); // 获取partner对象 (Get partner object)
            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap(); // 获取Sui时钟对象 (ID 0x6) (Get Sui clock object (ID 0x6))

            // 将获取的 `SuiObject` 转换为 `ObjectArg` (Convert the fetched `SuiObject` to `ObjectArg`)
            ObjectArgs {
                config: shared_obj_arg(&config_obj, false), // `config` 对象通常是不可变的共享对象 (`config` object is usually an immutable shared object)
                partner: shared_obj_arg(&partner_obj, true),  // `partner` 对象在交易中可能是可变的（例如，更新推荐计数） (`partner` object might be mutable in a transaction (e.g., updating referral count))
                clock: shared_obj_arg(&clock_obj, false),   // Sui时钟对象是不可变的共享对象 (Sui clock object is an immutable shared object)
            }
        })
        .await // 等待初始化完成 (Wait for initialization to complete)
        .clone() // 克隆缓存中的值返回 (Clone the cached value and return)
}

/// `Cetus` 结构体 (Cetus Struct)
///
/// 代表一个Cetus协议的特定交易池的实例。
/// (Represents an instance of a specific trading pool of the Cetus protocol.)
/// 它封装了与该池进行交互所需的所有状态信息和参数。
/// (It encapsulates all state information and parameters required for interacting with this pool.)
///
/// `#[derive(Clone)]` 允许 `Cetus` 实例被克隆。
/// (`#[derive(Clone)]` allows `Cetus` instances to be cloned.)
#[derive(Clone)]
pub struct Cetus {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息 (`Pool` 类型)。(Original pool information (`Pool` type) obtained from `dex_indexer`.)
    pool_arg: ObjectArg,     // 当前交易池对象本身的 `ObjectArg` 表示，用于在PTB中引用这个池。(The `ObjectArg` representation of the current trading pool object itself, used for referencing this pool in a PTB.)
    liquidity: u128,         // 池的流动性估算值。在 `Cetus::new` 中，它被设置为从池对象结构中提取的 `liquidity` 字段。(Estimated liquidity value of the pool. In `Cetus::new`, it's set to the `liquidity` field extracted from the pool object's structure.)
    coin_in_type: String,    // 当前配置的交易方向下，输入代币的Sui类型字符串。(Sui type string of the input coin for the currently configured trading direction.)
    coin_out_type: String,   // 当前配置的交易方向下，输出代币的Sui类型字符串。(Sui type string of the output coin for the currently configured trading direction.)
    type_params: Vec<TypeTag>,// 调用Cetus Swap合约的泛型函数时，所需要的泛型类型参数列表。(List of generic type parameters required when calling Cetus Swap contract's generic functions.)
                              // 对于双币池，这通常是 `[CoinTypeA, CoinTypeB]`，其中A和B是池中的两种代币。(For a two-coin pool, this is usually `[CoinTypeA, CoinTypeB]`, where A and B are the two coins in the pool.)
    // --- 从 `OBJ_CACHE` 获取的共享对象参数 --- (Shared object parameters obtained from `OBJ_CACHE`)
    config: ObjectArg,  // Cetus全局配置对象的 `ObjectArg`。(Cetus global config object's `ObjectArg`.)
    partner: ObjectArg, // Cetus合作伙伴对象的 `ObjectArg`。(Cetus partner object's `ObjectArg`.)
    clock: ObjectArg,   // Sui系统时钟对象的 `ObjectArg`。(Sui system clock object's `ObjectArg`.)
}

impl Cetus {
    /// `new` 构造函数 (异步) (new constructor (asynchronous))
    ///
    /// 根据从 `dex_indexer` 获取到的原始 `Pool` 信息和用户指定的输入代币类型 (`coin_in_type`)，
    /// 来创建一个 `Cetus` DEX实例。
    /// (Creates a `Cetus` DEX instance based on the original `Pool` information obtained from `dex_indexer` and the user-specified input coin type (`coin_in_type`).)
    /// 这个构造函数假设Cetus的池是双币池，因此输出代币类型会根据输入代币类型自动推断出来。
    /// (This constructor assumes Cetus pools are two-coin pools, so the output coin type is automatically inferred based on the input coin type.)
    ///
    /// **参数 (Parameters)**:
    /// - `simulator`: 一个共享的模拟器实例 (`Arc<Box<dyn Simulator>>`)。(A shared simulator instance.)
    /// - `pool_info`: 一个对从 `dex_indexer` 获取的 `Pool` 结构体的引用。(A reference to the `Pool` struct obtained from `dex_indexer`.)
    /// - `coin_in_type`: 输入代币的Sui类型字符串。(Sui type string of the input coin.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Self>`: 如果成功初始化，返回一个 `Cetus` 实例；否则返回错误。(Returns a `Cetus` instance if initialization is successful; otherwise, returns an error.)
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 1. 确保传入的 `pool_info` 标记的协议是 `Protocol::Cetus`。
        //    (Ensure the protocol marked in the passed `pool_info` is `Protocol::Cetus`.)
        ensure!(pool_info.protocol == Protocol::Cetus, "提供的池信息与Cetus协议不匹配 (期望: Cetus, 实际: {}) (Provided pool info does not match Cetus protocol (Expected: Cetus, Actual: {}))", pool_info.protocol);

        // 2. 获取并解析池对象的Move结构体内容。
        //    (Get and parse the Move struct content of the pool object.)
        let pool_obj = simulator
            .get_object(&pool_info.pool) // `pool_info.pool` 是池的ObjectID (`pool_info.pool` is the pool's ObjectID)
            .await
            .ok_or_else(|| eyre!("Cetus池对象未在链上找到，ID: {} (Cetus pool object not found on-chain, ID: {})", pool_info.pool))?;

        let parsed_pool_struct = { // 块表达式限制作用域 (Block expression limits scope)
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre(format!("Cetus池对象 {} 的Move布局信息未找到 (Move layout info not found for Cetus pool object {})", pool_info.pool))?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre(format!("对象 {} 不是一个有效的Move对象 (Object {} is not a valid Move object)", pool_info.pool))?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!("反序列化Cetus池对象 {} 的Move结构失败: {} (Failed to deserialize Move struct of Cetus pool object {}: {})", pool_info.pool, e))?
        };

        // 从解析后的池结构中提取 `is_pause` 字段，检查池是否已被暂停交易。
        // (Extract the `is_pause` field from the parsed pool structure to check if the pool has been paused for trading.)
        let is_pause = extract_bool_from_move_struct(&parsed_pool_struct, "is_pause")?;
        ensure!(!is_pause, "Cetus池 {} 已被暂停，当前无法进行交易 (Cetus pool {} is paused, trading currently unavailable)", pool_info.pool); // 如果暂停，则返回错误 (If paused, return an error)

        // 提取池的流动性。这里假设流动性由池结构中名为 `liquidity` 的字段直接表示。
        // (Extract the pool's liquidity. It's assumed here that liquidity is directly represented by a field named `liquidity` in the pool structure.)
        let liquidity = extract_u128_from_move_struct(&parsed_pool_struct, "liquidity")?;

        // 3. 根据输入代币类型推断输出代币类型。
        //    (Infer the output coin type based on the input coin type.)
        //    假设池是双币池，`pool_info` 中应该有 `token0_type()` 和 `token1_type()` 方法。
        //    (Assuming it's a two-coin pool, `pool_info` should have `token0_type()` and `token1_type()` methods.)
        let coin_out_type = if pool_info.token0_type() == coin_in_type { // 如果输入是token0 (If input is token0)
            pool_info.token1_type().to_string() // 则输出是token1 (Then output is token1)
        } else { // 否则（输入是token1） (Otherwise (input is token1))
            pool_info.token0_type().to_string() // 则输出是token0 (Then output is token0)
        };

        // 4. 获取池本身的泛型类型参数 (`type_params`)。
        //    (Get the generic type parameters (`type_params`) of the pool itself.)
        //    这通常是池中包含的两种代币的 `TypeTag` 列表，例如 `[CoinTypeA, CoinTypeB]`。
        //    (This is usually a list of `TypeTag`s for the two coins contained in the pool, e.g., `[CoinTypeA, CoinTypeB]`.)
        //    这些类型参数在调用Swap或Flashloan合约的泛型函数时需要提供。
        //    (These type parameters need to be provided when calling generic functions of Swap or Flashloan contracts.)
        let type_params = parsed_pool_struct.type_.type_params.clone();

        // 5. 将池对象本身转换为 `ObjectArg` (在交易中通常是可变的，因为其状态会改变)。
        //    (Convert the pool object itself to `ObjectArg` (usually mutable in transactions as its state changes).)
        let pool_arg = shared_obj_arg(&pool_obj, true);
        // 6. 获取Cetus协议级别的共享对象参数 (config, partner, clock) (通过缓存机制)。
        //    (Get Cetus protocol-level shared object parameters (config, partner, clock) (via caching mechanism).)
        let ObjectArgs { config, partner, clock } = get_object_args(simulator).await; // 解构 `ObjectArgs` (Destructure `ObjectArgs`)

        // 7. 创建并返回 `Cetus` 实例。
        //    (Create and return the `Cetus` instance.)
        Ok(Self {
            pool: pool_info.clone(), // 存储原始池信息 (Store original pool information)
            liquidity, // 存储提取的流动性 (Store extracted liquidity)
            coin_in_type: coin_in_type.to_string(), // 存储输入代币类型 (Store input coin type)
            coin_out_type, // 存储推断的输出代币类型 (Store inferred output coin type)
            type_params, // 存储池的泛型类型参数 (Store pool's generic type parameters)
            pool_arg,    // 存储池对象的 `ObjectArg` (Store pool object's `ObjectArg`)
            config,      // 存储config对象的 `ObjectArg` (Store config object's `ObjectArg`)
            partner,     // 存储partner对象的 `ObjectArg` (Store partner object's `ObjectArg`)
            clock,       // 存储clock对象的 `ObjectArg` (Store clock object's `ObjectArg`)
        })
    }

    /// `build_swap_tx` (私有辅助函数，构建完整的常规交换交易PTB / Private helper function, builds a complete regular swap transaction PTB)
    ///
    /// `#[allow(dead_code)]` 表示即使此函数未被（非测试）代码直接调用，也不要产生警告。
    /// (`#[allow(dead_code)]` means do not warn if this function is not directly called by (non-test) code.)
    #[allow(dead_code)]
    async fn build_swap_tx(
        &self,
        sender: SuiAddress,
        recipient: SuiAddress,
        coin_in_ref: ObjectRef, // 输入代币的对象引用 (Object reference of the input coin)
        amount_in: u64,         // 输入代币的数量 (Amount of the input coin)
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default(); // 创建交易上下文 (Create transaction context)

        // 步骤1: 处理输入代币（如果需要，分割出精确数量）。
        // (Step 1: Process input coin (split out exact amount if needed).)
        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;

        // 步骤2: 将Cetus的常规交换操作指令添加到PTB。
        // (Step 2: Add Cetus's regular swap operation instruction to PTB.)
        // `None`作为`_amount_in`参数传递给`extend_trade_tx`，
        // 因为Cetus的`swap_a2b`/`swap_b2a`函数通常直接消耗传入的`Coin<T>`对象的全部余额作为输入数量。
        // (`None` is passed as the `_amount_in` argument to `extend_trade_tx`
        //  because Cetus's `swap_a2b`/`swap_b2a` functions usually consume the entire balance of the passed `Coin<T>` object as input amount.)
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;

        // 步骤3: 将交换得到的输出代币转移给接收者。
        // (Step 3: Transfer the output coins obtained from the swap to the recipient.)
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish()) // 完成并返回PTB (Finish and return PTB)
    }

    /// `build_swap_args` (私有辅助函数，构建调用Cetus常规交换函数所需的参数列表 / Private helper, builds arg list for Cetus regular swap function)
    ///
    /// 根据Cetus `swap_a2b` 或 `swap_b2a` 函数的签名，准备参数。
    /// (Prepares arguments based on the signature of Cetus `swap_a2b` or `swap_b2a` functions.)
    /// **推断的函数签名 (示例) (Inferred function signature (example))**:
    /// `public fun swap_a2b<CoinA, CoinB>(config: &GlobalConfig, pool: &mut Pool<CoinA, CoinB>, partner: &mut Partner, coin_a: Coin<CoinA>, clock: &Clock, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数顺序为: `config`, `pool`, `partner`, 输入代币 (`coin_a` 或 `coin_b`), `clock`。
    /// (Argument order: `config`, `pool`, `partner`, input coin (`coin_a` or `coin_b`), `clock`.)
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        // 将缓存的 `ObjectArg` 转换为PTB中实际使用的 `Argument`。
        // (Convert cached `ObjectArg`s to `Argument`s actually used in PTB.)
        let config_arg = ctx.obj(self.config).map_err(|e| eyre!("无法将Cetus config转换为PTB Argument: {} (Cannot convert Cetus config to PTB Argument: {})", e))?;
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!("无法将Cetus pool_arg转换为PTB Argument: {} (Cannot convert Cetus pool_arg to PTB Argument: {})", e))?;
        let partner_arg = ctx.obj(self.partner).map_err(|e| eyre!("无法将Cetus partner转换为PTB Argument: {} (Cannot convert Cetus partner to PTB Argument: {})", e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!("无法将Sui clock转换为PTB Argument: {} (Cannot convert Sui clock to PTB Argument: {})", e))?;

        // 返回参数列表，顺序必须与目标Move合约函数的参数顺序严格一致。
        // (Return the argument list; the order must strictly match the parameter order of the target Move contract function.)
        Ok(vec![config_arg, pool_arg, partner_arg, coin_in_arg, clock_arg])
    }

    /// `build_flashloan_args` (私有辅助函数，构建调用Cetus发起闪电贷函数所需的参数列表 / Private helper, builds arg list for Cetus flash loan initiation function)
    ///
    /// 根据Cetus `flash_swap_a2b` 或 `flash_swap_b2a` 函数的签名，准备参数。
    /// (Prepares arguments based on the signature of Cetus `flash_swap_a2b` or `flash_swap_b2a` functions.)
    /// **推断的函数签名 (示例 for `flash_swap_a2b`) (Inferred function signature (example for `flash_swap_a2b`))**:
    /// `public fun flash_swap_a2b<CoinA, CoinB>(config: &GlobalConfig, pool: &mut Pool<CoinA, CoinB>, partner: &mut Partner, amount: u64, by_amount_in: bool, clock: &Clock, ctx: &mut TxContext): (Coin<CoinB>, FlashSwapReceipt<CoinA, CoinB>, u64)`
    /// 参数顺序为: `config`, `pool`, `partner`, 借贷数量 (`amount`), 是否按输入数量计算 (`by_amount_in`), `clock`。
    /// (Argument order: `config`, `pool`, `partner`, loan amount (`amount`), whether to calculate by input amount (`by_amount_in`), `clock`.)
    fn build_flashloan_args(&self, ctx: &mut TradeCtx, amount_to_flashloan: u64) -> Result<Vec<Argument>> {
        let config_arg = ctx.obj(self.config).map_err(|e| eyre!(e))?;
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let partner_arg = ctx.obj(self.partner).map_err(|e| eyre!(e))?;

        let amount_arg = ctx.pure(amount_to_flashloan).map_err(|e| eyre!(e))?; // 要借入的代币数量 (Amount of coin to borrow)
        // `by_amount_in`: 一个布尔值，指示 `amount_to_flashloan` 是指希望借入的输入代币数量（`true`），
        // 还是指希望通过用这些借入的代币交换后得到的输出代币数量（`false`）。
        // (`by_amount_in`: A boolean value indicating whether `amount_to_flashloan` refers to the desired amount of input coin to borrow (`true`),
        //  or the desired amount of output coin after swapping these borrowed coins (`false`).)
        // 对于“借入X，得到Y，然后用Y换回X来偿还”这种典型套利场景，我们通常是指定借入的X的数量，所以 `by_amount_in` 为 `true`。
        // (For typical arbitrage scenarios like "borrow X, get Y, then swap Y back to X for repayment", we usually specify the amount of X to borrow, so `by_amount_in` is `true`.)
        let by_amount_in_arg = ctx.pure(true).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        Ok(vec![config_arg, pool_arg, partner_arg, amount_arg, by_amount_in_arg, clock_arg])
    }

    /// `build_repay_args` (私有辅助函数，构建调用Cetus偿还闪电贷函数所需的参数列表 / Private helper, builds arg list for Cetus flash loan repayment function)
    ///
    /// 根据Cetus `repay_flash_swap_a2b` 或 `repay_flash_swap_b2a` 函数的签名，准备参数。
    /// (Prepares arguments based on the signature of Cetus `repay_flash_swap_a2b` or `repay_flash_swap_b2a` functions.)
    /// **推断的函数签名 (示例 for `repay_flash_swap_a2b`) (Inferred function signature (example for `repay_flash_swap_a2b`))**:
    /// `public fun repay_flash_swap_a2b<CoinA, CoinB>(config: &GlobalConfig, pool: &mut Pool<CoinA, CoinB>, partner: &mut Partner, coin_a_to_repay: Coin<CoinA>, receipt: FlashSwapReceipt<CoinA, CoinB>, ctx: &mut TxContext): Coin<CoinA>;`
    /// 参数顺序为: `config`, `pool`, `partner`, 用于偿还的代币 (`coin_a_to_repay`)，闪电贷回执 (`receipt`)。
    /// (Argument order: `config`, `pool`, `partner`, coin for repayment (`coin_a_to_repay`), flash loan receipt (`receipt`).)
    fn build_repay_args(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, receipt_arg: Argument) -> Result<Vec<Argument>> {
        let config_arg = ctx.obj(self.config).map_err(|e| eyre!(e))?;
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let partner_arg = ctx.obj(self.partner).map_err(|e| eyre!(e))?;

        Ok(vec![config_arg, pool_arg, partner_arg, coin_to_repay_arg, receipt_arg])
    }
}

/// 为 `Cetus` 结构体实现 `Dex` trait。
/// (Implement `Dex` trait for `Cetus` struct.)
#[async_trait::async_trait]
impl Dex for Cetus {
    /// `support_flashloan` 方法
    /// (support_flashloan method)
    ///
    /// 表明该DEX（Cetus）是否支持闪电贷功能。Cetus是明确支持闪电贷的。
    /// (Indicates whether this DEX (Cetus) supports flash loan functionality. Cetus explicitly supports flash loans.)
    fn support_flashloan(&self) -> bool {
        true
    }

    /// `extend_flashloan_tx` 方法 (将发起Cetus闪电贷的操作添加到PTB中 / Add Cetus flash loan initiation op to PTB method)
    ///
    /// **核心逻辑 (Core Logic)**: (详情见上方中文总览 / See Chinese overview above for details)
    /// 1.  确定Cetus闪电贷函数名 (`flash_swap_a2b` / `flash_swap_b2a`)。
    ///     (Determine Cetus flash loan function name (`flash_swap_a2b` / `flash_swap_b2a`).)
    /// 2.  准备包ID、模块名、函数名。(Prepare package ID, module name, function name.)
    /// 3.  准备泛型类型参数 (根据交易方向调整)。(Prepare generic type arguments (adjust based on trade direction).)
    /// 4.  构建参数列表。(Build argument list.)
    /// 5.  添加 `Command::move_call` 指令到PTB。(Add `Command::move_call` instruction to PTB.)
    /// 6.  从返回的元组中提取 `Coin<Output>` 和 `FlashSwapReceipt` 作为PTB参数。
    ///     (Extract `Coin<Output>` and `FlashSwapReceipt` from the returned tuple as PTB arguments.)
    ///
    /// **参数 (Parameters)**:
    /// - `ctx`: 可变的交易上下文。(Mutable transaction context.)
    /// - `amount_in`: 希望作为闪电贷起始操作的输入代币数量。
    ///                (Amount of input coin desired for the initial flash loan operation.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<FlashResult>`: 包含“借到的输出代币”和“回执”的PTB参数。
    ///                          (PTB arguments containing "borrowed output coin" and "receipt".)
    async fn extend_flashloan_tx(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<FlashResult> {
        let function_name_str = if self.is_a2b() {
            "flash_swap_a2b"
        } else {
            "flash_swap_b2a"
        };

        let package_id = ObjectID::from_hex_literal(CETUS_DEX)?;
        let module_name = Identifier::new("cetus").map_err(|e| eyre!("创建Move模块标识符'cetus'失败: {} (Failed to create Move module identifier 'cetus': {})", e))?;
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!("创建Move函数标识符'{}'失败: {} (Failed to create Move function identifier '{}': {})", function_name_str, e))?;

        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() {
            type_arguments.swap(0, 1);
        }

        let call_arguments = self.build_flashloan_args(ctx, amount_in)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_command_idx = ctx.last_command_idx();
        Ok(FlashResult {
            coin_out: Argument::NestedResult(last_command_idx, 0), // 元组第0个元素是 Coin<Output>
            receipt: Argument::NestedResult(last_command_idx, 1),  // 元组第1个元素是 FlashSwapReceipt
            pool: None,
        })
    }

    /// `extend_repay_tx` 方法 (将偿还Cetus闪电贷的操作添加到PTB中 / Add Cetus flash loan repayment op to PTB method)
    ///
    /// **核心逻辑 (Core Logic)**: (详情见上方中文总览 / See Chinese overview above for details)
    /// 1.  确定Cetus偿还函数名 (`repay_flash_swap_a2b` / `repay_flash_swap_b2a`)。
    ///     (Determine Cetus repayment function name.)
    /// 2.  准备包ID、模块名、函数名。(Prepare package ID, module name, function name.)
    /// 3.  准备泛型类型参数。(Prepare generic type arguments.)
    /// 4.  构建参数列表 (包括要偿还的代币和回执)。(Build argument list (including coin to repay and receipt).)
    /// 5.  添加 `Command::move_call` 指令到PTB。(Add `Command::move_call` instruction to PTB.)
    /// 6.  返回代表找零代币的PTB参数。(Return PTB argument representing change coins.)
    ///
    /// **参数 (Parameters)**:
    /// - `coin_to_repay_arg`: 用于偿还闪电贷的代币 (PTB参数)。(Coin for repaying flash loan (PTB argument).)
    /// - `flash_res`: `extend_flashloan_tx` 返回的 `FlashResult` (主要用其回执)。
    ///                (`FlashResult` returned by `extend_flashloan_tx` (mainly its receipt is used).)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Argument>`: 代表找零代币的PTB参数。(PTB argument representing change coins.)
    async fn extend_repay_tx(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, flash_res: FlashResult) -> Result<Argument> {
        let function_name_str = if self.is_a2b() {
            "repay_flash_swap_a2b"
        } else {
            "repay_flash_swap_b2a"
        };

        let package_id = ObjectID::from_hex_literal(CETUS_DEX)?;
        let module_name = Identifier::new("cetus").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() {
            type_arguments.swap(0, 1);
        }

        let call_arguments = self.build_repay_args(ctx, coin_to_repay_arg, flash_res.receipt)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx)) // 返回的是偿还后可能剩余的找零币 (Returns potential change coins after repayment)
    }

    /// `extend_trade_tx` 方法 (将Cetus的常规交换操作添加到PTB中 / Add Cetus regular swap op to PTB method)
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress, // Cetus的swap函数签名不需要sender，但Dex trait可能要求
                             // (Cetus's swap function signature doesn't need sender, but Dex trait might require it)
        coin_in_arg: Argument,
        _amount_in: Option<u64>, // Cetus的swap函数通常消耗整个传入的Coin对象余额
                                 // (Cetus's swap functions usually consume the entire balance of the passed Coin object)
    ) -> Result<Argument> {
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        let package_id = ObjectID::from_hex_literal(CETUS_DEX)?;
        let module_name = Identifier::new("cetus").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() {
            type_arguments.swap(0, 1);
        }

        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx)) // 返回交换得到的输出代币 (Returns the output coin from the swap)
    }

    // --- 实现 `Dex` trait 的其他 getter 和 setter 方法 ---
    // (Implement other getter and setter methods of the `Dex` trait)
    fn coin_in_type(&self) -> String {
        self.coin_in_type.clone()
    }

    fn coin_out_type(&self) -> String {
        self.coin_out_type.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::Cetus // 指明这是Cetus协议 (Specify this is Cetus protocol)
    }

    fn liquidity(&self) -> u128 {
        self.liquidity
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 返回原始池信息的ObjectID (Return ObjectID from original pool info)
    }

    /// `flip` 方法 (翻转DEX的交易方向 / Flip DEX's trading direction method)
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type); // 交换输入和输出代币类型 (Swap input and output coin types)
        // `type_params` 的顺序在实际调用时根据 `is_a2b()` 动态调整，所以这里不需要修改 `self.type_params`。
        // (The order of `type_params` is dynamically adjusted based on `is_a2b()` during actual calls, so no need to modify `self.type_params` here.)
    }

    /// `is_a2b` 方法 (判断当前方向是否为 A 到 B / Is current direction A to B? method)
    ///
    /// 判断当前的 `self.coin_in_type` 是否是 `self.pool` (原始池信息) 中定义的“第一个”代币（token0）。
    /// (Checks if the current `self.coin_in_type` is the "first" token (token0) defined in `self.pool` (original pool info).)
    /// Cetus的 `swap_a2b` / `flash_swap_a2b` 函数通常指的是从池中的代币A（token0）到代币B（token1）的转换。
    /// (Cetus's `swap_a2b` / `flash_swap_a2b` functions usually refer to conversion from token A (token0) to token B (token1) in the pool.)
    fn is_a2b(&self) -> bool {
        self.pool.token_index(&self.coin_in_type) == Some(0) // 如果输入代币是池子里的第0个代币，则认为是A->B
                                                             // (If input coin is the 0th coin in the pool, it's considered A->B)
    }

    /// `swap_tx` 方法 (构建一个完整的、独立的Cetus常规交换交易，主要用于测试 / Build a complete, independent Cetus regular swap transaction, mainly for testing method)
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await; // 创建测试Sui客户端 (Create a test Sui client)

        // 准备输入的代币对象 (Prepare input coin object)
        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;

        // 调用内部的 `build_swap_tx` 方法来创建包含交换操作的PTB。
        // (Call the internal `build_swap_tx` method to create a PTB containing the swap operation.)
        let programmable_tx_block = self
            .build_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in)
            .await?;

        // 准备Gas币并构建最终的 `TransactionData`。
        // (Prepare gas coins and build the final `TransactionData`.)
        let gas_coins = coin::get_gas_coin_refs(&sui_client, sender, Some(coin_in_obj.coin_object_id)).await?;
        let gas_price = sui_client.read_api().get_reference_gas_price().await?;
        let tx_data = TransactionData::new_programmable(sender, gas_coins, programmable_tx_block, GAS_BUDGET, gas_price);

        Ok(tx_data)
    }
}

// --- 测试模块 (`tests`) ---
// (Test module (`tests`))
#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::Instant}; // `FromStr` 用于字符串转换, `Instant` 用于计时。
                                            // (`FromStr` for string conversion, `Instant` for timing.)
    use itertools::Itertools; // 引入 `itertools` crate 的额外迭代器方法。
                              // Import additional iterator methods from the `itertools` crate.
    use object_pool::ObjectPool; // 对象池。(Object pool.)
    use simulator::{DBSimulator, SimulateCtx, Simulator}; // 模拟器相关类型。(Simulator related types.)
    use sui_sdk::SuiClientBuilder; // Sui客户端构建器。(Sui client builder.)
    use tracing::info; // 日志记录。(Logging.)

    use super::*; // 导入外部模块 (当前 `cetus.rs` 文件) 的所有公共成员。
                  // (Import all public members from the outer module (current `cetus.rs` file).)
    use crate::{ // 从当前crate的根作用域引入：
                  // (Import from the current crate's root scope:)
        common::get_latest_epoch, // 获取最新纪元信息的函数。(Function to get the latest epoch information.)
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL}, // 测试配置常量。(Test configuration constants.)
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher}, // DEX搜索器实现和trait。(DEX searcher implementation and trait.)
    };

    /// `test_cetus_swap_tx` 测试函数
    /// (test_cetus_swap_tx test function)
    ///
    /// 这个异步测试函数旨在对Cetus DEX的常规交换功能进行集成测试。
    /// (This asynchronous test function aims to perform integration testing on Cetus DEX's regular swap functionality.)
    /// **测试命令示例 (Example test command)**:
    /// `cargo test --package arb --bin arb --all-features -- defi::cetus::tests::test_cetus_swap_tx --exact --show-output`
    #[tokio::test]
    async fn test_cetus_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]); // 初始化日志 (Initialize logger)

        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap();
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI";
        let token_out_type = "0xdeeb7a4662eec9f2f3def03fb937a663dddaa2e215b8078a284d026b7946c270::deep::DEEP"; // 一个示例代币 (An example token)
        let amount_in = 10000; // 0.00001 SUI (非常小的量用于测试) (A very small amount for testing)

        let simulator_pool_for_searcher = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(DBSimulator::new_test(true).await) as Box<dyn Simulator> })
        }));

        let searcher = IndexerDexSearcher::new(TEST_HTTP_URL, simulator_pool_for_searcher).await.unwrap();
        let dexes = searcher
            .find_dexes(token_in_type, Some(token_out_type.into()))
            .await
            .unwrap();
        info!("🧀 (测试信息) 找到的DEX总数量 (Total DEXs found): {}", dexes.len());

        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::Cetus)
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity())) // 按流动性升序，取最高的 (Sort by liquidity ascending, take the highest)
            .last()
            .expect("测试环境中未能找到任何Cetus协议的池来进行SUI->DEEP的交换 (Failed to find any Cetus pool for SUI->DEEP swap in test environment)");

        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 (测试信息) 构建的Cetus交换交易数据 (Constructed Cetus swap transaction data): {:?}", tx_data);

        let start_time = Instant::now();
        // **重要**: 下面的数据库路径和全节点配置文件路径是硬编码的，生产或CI环境应使用配置。
        // (**IMPORTANT**: The database path and fullnode config path below are hardcoded; production or CI environments should use configuration.)
        let db_sim = DBSimulator::new_slow(
            "/home/ubuntu/sui-nick/db/live/store", // 示例路径 (Example path)
            "/home/ubuntu/sui-nick/fullnode.yaml", // 示例路径 (Example path)
            None,
            None,
        )
        .await;
        info!("DBSimulator::new_slow 初始化耗时 (Initialization time): {:?}", start_time.elapsed());

        let sui_client = SuiClientBuilder::default().build(TEST_HTTP_URL).await.unwrap();
        let epoch = get_latest_epoch(&sui_client).await.unwrap();
        let sim_ctx = SimulateCtx::new(epoch, vec![]);

        let sim_start_time = Instant::now();
        let db_res = db_sim.simulate(tx_data, sim_ctx).await.unwrap();
        info!("🧀 (测试信息) Cetus数据库模拟耗时 {:?}, 结果 (Cetus DB simulation time {:?}, Result): {:?}", sim_start_time.elapsed(), db_res);

        assert!(db_res.is_ok(), "Cetus交换交易的数据库模拟应该成功执行 (Cetus swap transaction DB simulation should execute successfully)");
    }
}

[end of bin/arb/src/defi/cetus.rs]
