// 该文件 `aftermath.rs` 实现了与 Aftermath Finance 协议进行交互的逻辑。
// Aftermath Finance 是 Sui 区块链上的一个去中心化交易所 (DEX)。
// 从代码结构和常量来看，Aftermath 可能是一个支持多代币池（multi-token pools）和加权池（weighted pools）的协议，
// 类似于以太坊上的 Balancer协议，或者它有自己独特的池类型和数学模型。
//
// **文件概览 (File Overview)**:
// 这个 `aftermath.rs` 文件是专门用来和Sui区块链上的Aftermath Finance这个DeFi协议“对话”的代码。
// Aftermath Finance是一个“去中心化交易所”（DEX）。
// 从代码里的一些常量和结构来看，Aftermath可能比较特别，它可能支持：
// -   **多代币池**：一个池子里不止两种代币，可能有三种、四种或更多。
// -   **加权池**：池子里的不同代币可以有不同的“权重”或“重要性”，这会影响它们之间的兑换价格。
// 这有点像以太坊上的Balancer协议，但也可能有Aftermath自己独特的设计。
// (This `aftermath.rs` file contains code specifically for interacting with the Aftermath Finance DeFi protocol on the Sui blockchain.
//  Aftermath Finance is a Decentralized Exchange (DEX).
//  Judging from some constants and structures in the code, Aftermath might be special, possibly supporting:
//  - **Multi-token pools**: A pool can contain more than two types of tokens, maybe three, four, or more.
//  - **Weighted pools**: Different tokens in a pool can have different "weights" or "importance," which affects their exchange prices.
//  This is somewhat similar to the Balancer protocol on Ethereum, but Aftermath might also have its own unique design.)
//
// **这个文件里主要有哪些东西 (What's in this file)?**
//
// 1.  **常量定义 (Constant Definitions)**:
//     -   `AFTERMATH_DEX`: Aftermath核心智能合约的“门牌号”（Package ID）。
//     -   一系列重要的 `ObjectID` 常量，比如 `POOL_REGISTRY`（池子注册表，可能记录了所有Aftermath池子）、`PROTOCOL_FEE_VAULT`（协议手续费金库）、`TREASURY`（国库）、`INSURANCE_FUND`（保险基金）、`REFERRAL_VAULT`（推荐人金库）。这些都是Aftermath协议在链上部署的关键共享对象，交易时可能需要用到它们。
//     -   `SLIPPAGE`: 一个用于“滑点保护”的常量。滑点是指你交易时，最终成交价格和你看到的价格不一样。这个常量的值（0.9 * 10^18）暗示它可能代表你期望至少能拿到理论输出的90%（即允许10%的向你不利方向的滑点）。
//     -   `ONE`: 代表数字 `1.0` 在Aftermath协议内部进行数学计算时所用的整数形式（这里是10^18）。因为电脑直接算小数容易有误差，所以很多DeFi协议会把所有数字都乘以一个很大的整数（比如10^18）然后用整数来算，最后再除回去。
//
// 2.  **`ObjectArgs` 结构体与 `OBJ_CACHE`**:
//     -   和Cetus、Kriya文件里类似，`ObjectArgs` 用来打包缓存一些常用的Aftermath协议全局对象的引用信息。
//     -   `OBJ_CACHE` 是一个一次性初始化并全局共享的缓存，提高效率。
//
// 3.  **`Aftermath` 结构体**:
//     -   核心部分，代表Aftermath协议里的一个具体的交易池实例。
//     -   包含了与这个池子互动所需的所有信息（比如池子对象的引用、流动性、池子里的各种代币类型、每种代币的余额和权重、交易手续费率、代币在池内列表中的索引位置等）。
//     -   它也实现了项目内部定义的 `Dex` 通用接口。
//
// 4.  **`Aftermath::new()` 构造函数**:
//     -   异步方法，根据从`dex_indexer`获取的池信息以及用户指定的输入代币（和可选的输出代币）来创建`Aftermath`实例。
//     -   它会去链上读取池子对象的详细数据，解析出余额、权重、手续费等重要参数。
//     -   如果只指定了输入代币，它会尝试为这个输入代币与池中所有其他代币的组合都创建一个`Aftermath`实例，每个实例代表一个特定的交易方向。
//
// 5.  **交易构建逻辑 (Transaction Building Logic)**:
//     -   `build_swap_tx()` 和 `build_swap_args()`：内部辅助函数，用来准备在Aftermath池子进行“精确输入代币交换”（你指定输入多少，换出尽可能多的另一种代币）时需要发送给Sui区块链的指令和参数。
//
// 6.  **价格计算 (Price Calculation)**:
//     -   `expect_amount_out()`: 根据当前池子的状态（各种代币的余额、权重、手续费）和你打算输入的代币数量，来计算理论上能换回多少输出代币。这个计算会调用更底层的、特定于Aftermath定价公式的数学函数。
//
// 7.  **`Dex` trait 实现**:
//     -   `Aftermath` 结构体实现了 `Dex` 接口要求的所有方法，比如：
//         -   `extend_trade_tx()`: 把Aftermath的交换操作指令添加到正在构建的Sui交易包（PTB）中。
//         -   `swap_tx()`: 构建一个完整的、独立的Sui交易，只包含一次Aftermath交换（主要用于测试）。
//         -   其他如 `coin_in_type()`, `coin_out_type()`, `protocol()`, `liquidity()`, `object_id()`, `flip()` 等，提供DEX实例的基本信息和操作。
//
// 8.  **数学辅助函数 (Mathematical Helper Functions)**:
//     -   文件底部包含了一系列数学函数，如 `calculate_expected_out`, `calc_spot_price_fixed_with_fees`, `convert_int_to_fixed`, `div_down` 等。
//     -   这些函数是专门用来处理Aftermath池子特有的数学运算的，特别是那些涉及到 `U256` 这种超大整数类型和“固定点数算术”（用整数模拟小数运算）的计算，以及精确计算手续费等。
//         Aftermath协议在这些计算中似乎也使用10^18 (`ONE` 常量) 作为固定点数的小数精度基准。
//
// **相关的Sui区块链和DeFi概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **Package ID (包ID)**: Sui智能合约是以“Move包”的形式部署的，Package ID是这个包的唯一地址。
// -   **Object ID (对象ID)**: Sui上一切皆对象，每个对象都有唯一的ID。
// -   **ObjectArg**: 在构建Sui的PTB（可编程交易块）时，引用链上对象需要用到的参数类型。
// -   **Programmable Transaction Block (PTB / 可编程交易块)**: Sui的一种强大交易构建方式，可以把多个操作（如分割代币、调用不同合约的函数）打包成一个原子交易。
// -   **TypeTag (类型标签)**: 在Move语言中，用来在运行时表示一个具体的类型，比如代币类型。调用泛型合约函数时常需要。
// -   **Liquidity Pool (流动性池)**: DEX的核心，用户存入代币对提供流动性，交易者与池子交换代币。
// -   **Weighted Pool (加权池 / Weighted Pool)**:
//     一种特殊的流动性池，池内的不同代币可以被赋予不同的“权重”。这些权重会影响池子的价格发现机制和交易时的滑点特性。
//     例如，在一个包含代币A（权重80%）和代币B（权重20%）的加权池中，代币A的相对价值和数量在池中占主导地位。
//     Balancer协议是以太坊上著名的加权池例子。Aftermath协议似乎也采用了类似的模型，允许池中的代币有不同的权重，而不仅仅是传统的50/50权重。
//     这对于创建包含多种资产（比如一个指数基金组合）的池子，或者对某些代币给予价格倾斜时非常有用。
//
// -   **Fixed-Point Arithmetic (固定点数算术 / Fixed-Point Arithmetic)**:
//     在计算机中用整数来模拟小数运算的一种方法。因为计算机直接用浮点数（比如 `float` 或 `double` 类型）进行金融计算时，可能会因为精度问题导致微小的误差，
//     而这些误差在金融领域（尤其是智能合约中）是不可接受的，因为它们必须是完全确定和可复现的。
//     固定点数算术通过选取一个很大的整数作为基数（比如这里的 `ONE = 10^18`），然后把所有小数都乘以这个基数转换成整数来运算。
//     例如，如果真实值是 `V`，那么在固定点数系统中它被表示为 `V * 10^18`。
//     所有的数学运算（加减乘除、开方、幂运算等）都基于这些大整数进行，并在适当的时候进行调整（比如乘法后除以基数，除法前乘以基数）来保持正确的数量级和精度。
//     Aftermath协议中的数学函数（如 `calculate_expected_out`, `div_down`）就是这种固定点数运算的体现。

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::{str::FromStr, sync::Arc, vec::Vec}; // `FromStr` trait 用于从字符串解析成特定类型。
                                             // (`FromStr` trait is used for parsing a string into a specific type.)
                                             // `Arc` (Atomic Reference Counting) 用于在多线程/异步环境中安全地共享数据。
                                             // (`Arc` (Atomic Reference Counting) is used for safely sharing data in multi-threaded/asynchronous environments.)
                                             // `Vec` 是Rust的动态数组（向量）类型。
                                             // (`Vec` is Rust's dynamic array (vector) type.)

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` crate (可能是用于从外部服务获取DEX数据的库) 的 `types` 模块引入 `Pool` (代表一个DEX池的原始信息) 和 `Protocol` (枚举，如Aftermath, Cetus等) 类型。
                                        // (Import `Pool` (representing raw info of a DEX pool) and `Protocol` (enum, e.g., Aftermath, Cetus) types from the `types` module of the `dex_indexer` crate (possibly a library for fetching DEX data from external services).)
use eyre::{ensure, eyre, Result}; // 从 `eyre` 库引入错误处理相关的工具：
                                  // (Import error handling related tools from the `eyre` library:)
                                  // `ensure!` 宏：检查一个条件，如果条件为false，则返回一个错误。
                                  // (`ensure!` macro: Checks a condition, if false, returns an error.)
                                  // `eyre!` 宏：方便地创建一个新的 `eyre::Report` 错误实例。
                                  // (`eyre!` macro: Conveniently creates a new `eyre::Report` error instance.)
                                  // `Result` 类型：通常是 `std::result::Result<T, eyre::Report>` 的别名。
                                  // (`Result` type: Usually an alias for `std::result::Result<T, eyre::Report>`.)

use move_core_types::annotated_value::MoveStruct; // 从 `move_core_types` 库引入 `MoveStruct`。Move是Sui和Aptos等区块链使用的智能合约语言。
                                                 // (Import `MoveStruct` from the `move_core_types` library. Move is the smart contract language used by blockchains like Sui and Aptos.)
                                                 // `MoveStruct` 可能用于表示从链上获取的Move对象的反序列化后的结构化数据。
                                                 // (`MoveStruct` might be used to represent structured data deserialized from Move objects fetched from the chain.)
use primitive_types::U256; // 引入 `U256` 类型，这是一个256位的无符号大整数类型。在处理加密货币余额（特别是那些具有很多小数位的代币）
                           // 或进行高精度固定点数运算时，经常需要用到这种大整数类型，以避免溢出和保持精度。
                           // (Import `U256` type, a 256-bit unsigned large integer type. This type is often needed when handling cryptocurrency balances (especially for tokens with many decimal places)
                           //  or performing high-precision fixed-point arithmetic, to avoid overflow and maintain precision.)
use simulator::Simulator; // 从 `simulator` crate 引入 `Simulator` trait，定义了交易模拟器的通用接口。
                         // (Import `Simulator` trait from the `simulator` crate, defining a common interface for transaction simulators.)
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // 从 `sui_types` 核心库引入Sui区块链的基本类型：
                                                 // (Import basic Sui blockchain types from the `sui_types` core library:)
                                                 // `ObjectID`: 对象的唯一ID。(Unique ID of an object.)
                                                 // `ObjectRef`: 对象的引用 (ID, 版本号, 摘要)，用于在交易中指定一个特定版本的对象。
                                                 //              (Object reference (ID, version, digest), used to specify a particular version of an object in a transaction.)
                                                 // `SuiAddress`: Sui网络上的账户地址。(Account address on the Sui network.)
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // 同样从 `sui_types` 引入与构建Sui交易（特别是可编程交易块PTB）相关的类型：
                                                                                         // (Also import types related to building Sui transactions (especially Programmable Transaction Blocks, PTBs) from `sui_types`:)
                                                                                         // `Argument`: PTB中的一个参数，可以是对象、纯值或另一个命令的结果。
                                                                                         //             (An argument in a PTB, can be an object, a pure value, or the result of another command.)
                                                                                         // `Command`: PTB中的一个操作指令，如调用Move函数、转移对象等。
                                                                                         //            (An operation instruction in a PTB, such as calling a Move function, transferring an object, etc.)
                                                                                         // `ObjectArg`: 在PTB中用于引用链上对象的特定参数类型。
                                                                                         //              (A specific argument type used in PTBs to reference on-chain objects.)
                                                                                         // `ProgrammableTransaction`: 代表一个PTB的结构。
                                                                                         //                            (Represents the structure of a PTB.)
                                                                                         // `TransactionData`: 代表一笔完整的、可被签名和提交的Sui交易的数据结构。
                                                                                         //                    (Represents the data structure of a complete Sui transaction that can be signed and submitted.)
    Identifier, TypeTag, // `Identifier`: Move语言中的标识符（如模块名、函数名、结构名）。
                         //               (Identifier in Move language (e.g., module name, function name, struct name).)
                         // `TypeTag`: 在运行时表示一个Move类型，例如代币类型 "0x2::sui::SUI"。
                         //            (Represents a Move type at runtime, e.g., coin type "0x2::sui::SUI".)
};
use tokio::sync::OnceCell; // 从 `tokio` 库引入 `OnceCell`，这是一个异步环境下用于实现“一次性初始化”的单元。
                           // (Import `OnceCell` from the `tokio` library, an asynchronous cell for "initialize-once" semantics.)
                           // 它确保某个值只被计算或初始化一次，即使在并发的异步调用中也是如此，常用于创建全局缓存或延迟初始化的静态变量。
                           // (It ensures a value is computed or initialized only once, even across concurrent async calls, often used for global caches or lazily initialized static variables.)
use utils::{coin, new_test_sui_client, object::*}; // 从项目内部的 `utils` 工具库引入：
                                                   // (Import from the project's internal `utils` utility library:)
                                                   // `coin` 模块：可能包含与代币操作相关的辅助函数（如获取余额、分割代币等）。
                                                   // (`coin` module: Might contain helper functions related to coin operations (e.g., getting balance, splitting coins).)
                                                   // `new_test_sui_client` 函数：可能是一个用于快速创建Sui客户端实例以便测试的辅助函数。
                                                   // (`new_test_sui_client` function: Might be a helper function for quickly creating a Sui client instance for testing.)
                                                   // `object::*`：可能导入了 `utils::object` 模块下的所有公共项，这些项可能与处理Sui对象数据相关（如从MoveStruct中提取字段）。
                                                   // (`object::*`: Might import all public items under the `utils::object` module, which could be related to handling Sui object data (e.g., extracting fields from MoveStruct).)

use super::TradeCtx; // 从父模块 (`defi`) 引入 `TradeCtx` (交易上下文) 类型。`TradeCtx` 用于在构建PTB时跟踪状态和参数。
                   // (Import `TradeCtx` (transaction context) type from the parent module (`defi`). `TradeCtx` is used for tracking state and arguments when building PTBs.)
use crate::{config::*, defi::Dex}; // 从当前crate的根作用域引入：
                                  // (Import from the current crate's root scope:)
                                  // `config::*`: 导入 `config` 模块下的所有公共项（如 `GAS_BUDGET` 等常量）。
                                  //              (Import all public items under the `config` module (e.g., constants like `GAS_BUDGET`).)
                                  // `defi::Dex`: 导入在父模块 `defi` 中定义的 `Dex` trait。
                                  //              (Import the `Dex` trait defined in the parent `defi` module.)

// --- Aftermath协议相关的常量定义 ---
// (Constant definitions related to Aftermath protocol)

// `AFTERMATH_DEX`: Aftermath Swap 合约的Sui Package ID (包ID) 字符串。
// (Sui Package ID string for the Aftermath Swap contract.)
// 这是在Sui区块链上唯一标识Aftermath Swap智能合约代码的地址。
// (This is the address that uniquely identifies the Aftermath Swap smart contract code on the Sui blockchain.)
const AFTERMATH_DEX: &str = "0xc4049b2d1cc0f6e017fda8260e4377cecd236bd7f56a54fee120816e72e2e0dd";

// Aftermath协议中一些关键的、全局共享的对象的ID字符串。
// (ID strings of some key, globally shared objects in the Aftermath protocol.)
// 这些对象通常是在协议部署时创建的，并在许多协议操作中被引用。
// (These objects are typically created when the protocol is deployed and are referenced in many protocol operations.)
const POOL_REGISTRY: &str = "0xfcc774493db2c45c79f688f88d28023a3e7d98e4ee9f48bbf5c7990f651577ae"; // 池注册表对象ID，可能用于查找和管理所有Aftermath池。
                                                                                                  // (Pool Registry Object ID, possibly used for finding and managing all Aftermath pools.)
const PROTOCOL_FEE_VAULT: &str = "0xf194d9b1bcad972e45a7dd67dd49b3ee1e3357a00a50850c52cd51bb450e13b4"; // 协议手续费库对象ID，用于存储协议收取的手续费。
                                                                                                        // (Protocol Fee Vault Object ID, used for storing fees collected by the protocol.)
const TREASURY: &str = "0x28e499dff5e864a2eafe476269a4f5035f1c16f338da7be18b103499abf271ce"; // 国库对象ID，可能用于存储协议的储备金或利润。
                                                                                              // (Treasury Object ID, possibly used for storing the protocol's reserves or profits.)
const INSURANCE_FUND: &str = "0xf0c40d67b078000e18032334c3325c47b9ec9f3d9ae4128be820d54663d14e3b"; // 保险基金对象ID，用于在发生意外损失时提供补偿。
                                                                                                    // (Insurance Fund Object ID, used to provide compensation in case of unexpected losses.)
const REFERRAL_VAULT: &str = "0x35d35b0e5b177593d8c3a801462485572fc30861e6ce96a55af6dc4730709278"; // 推荐人系统相关的库对象ID。
                                                                                                    // (Vault Object ID related to the referral system.)

// `SLIPPAGE` 常量：用于滑点保护。 (SLIPPAGE constant: for slippage protection.)
// (滑点是指在去中心化交易所（DEX）上交易时，由于从你看到价格到交易实际执行之间市场价格可能发生变动，
//  或者由于你的交易本身对池子价格产生影响，导致最终成交价格与预期价格有所偏差的现象。)
// (Slippage refers to the phenomenon in DEX trading where the final execution price differs from the expected price
//  due to market price changes between when you see the price and when the trade actually executes,
//  or due to the impact of your trade itself on the pool's price.)
//
// 这个 `SLIPPAGE` 值 `900_000_000_000_000_000` 代表 `0.9 * 10^18`。
// (This `SLIPPAGE` value `900_000_000_000_000_000` represents `0.9 * 10^18`.)
// 在Aftermath的固定点数数学体系中，`ONE` (1.0) 被定义为 `10^18` (见下文)。
// (In Aftermath's fixed-point math system, `ONE` (1.0) is defined as `10^18` (see below).)
// 所以，这个值实际上是 `0.9 * ONE`。 (So, this value is actually `0.9 * ONE`.)
//
// 它通常用于计算在执行交易时，你能接受的“最小输出代币数量”。
// (It's typically used to calculate the "minimum output token amount" you can accept when executing a trade.)
// 例如，如果理论上你应该得到 `X` 个输出代币，那么设置了这个滑点后，
// 你可能会要求至少得到 `X * SLIPPAGE / ONE` (即 `X * 0.9`) 个代币。
// (For example, if theoretically you should receive `X` output tokens, after setting this slippage,
//  you might require at least `X * SLIPPAGE / ONE` (i.e., `X * 0.9`) tokens.)
// 这相当于设置了10%的滑点容忍度（即价格最多可以对你不利10%）。
// (This is equivalent to setting a 10% slippage tolerance (i.e., the price can move against you by at most 10%).)
//
// 注意：这个滑点容忍度 (10%) 相对较高。在实际套利中，滑点通常需要设置得非常小（例如0.1%到0.5%），
// 因为套利利润往往也很微薄。高滑点可能会导致交易在不利的价格下执行，从而损失利润甚至本金。
// (Note: This slippage tolerance (10%) is relatively high. In actual arbitrage, slippage usually needs to be set very small (e.g., 0.1% to 0.5%),
//  as arbitrage profits are often very slim. High slippage can lead to trades executing at unfavorable prices, resulting in loss of profit or even principal.)
// 这个值可能需要根据具体策略和市场情况进行调整。
// (This value might need adjustment based on specific strategies and market conditions.)
const SLIPPAGE: u128 = 900_000_000_000_000_000; //  0.9 * 10^18

// `ONE` 常量：代表数值 `1.0` 在Aftermath协议的固定点数数学运算中所使用的整数表示。
// (`ONE` constant: Represents the integer representation of the value `1.0` used in Aftermath protocol's fixed-point math operations.)
// Aftermath协议（像许多DeFi协议一样）使用固定点数算术来进行精确计算，以避免浮点数带来的不确定性。
// (The Aftermath protocol (like many DeFi protocols) uses fixed-point arithmetic for precise calculations to avoid uncertainties associated with floating-point numbers.)
// 在这种体系下，一个非常大的整数被用来表示小数。`ONE` 就是这个体系的“单位1”。
// (In this system, a very large integer is used to represent decimals. `ONE` is the "unit 1" of this system.)
// 这里，`ONE` 被定义为 `10^18`，这意味着所有涉及到比例、费率或价格的计算，
// 如果其真实值是 `V`，那么在合约内部或计算中它会被表示为 `V * 10^18`。
// (Here, `ONE` is defined as `10^18`, meaning that for all calculations involving ratios, fees, or prices,
//  if their true value is `V`, it will be represented as `V * 10^18` internally in the contract or calculations.)
// 例如，一个0.3%的手续费率 (0.003) 会被表示为 `0.003 * 10^18 = 3 * 10^15`。
// (For example, a 0.3% fee rate (0.003) would be represented as `0.003 * 10^18 = 3 * 10^15`.)
// `U256([low, mid_low, mid_high, high])` 是 `primitive_types::U256` 类型的构造方式，
// 它接收一个包含四个 `u64` 值的数组，分别代表256位整数的从低到高的四个64位部分。
// (`U256([low, mid_low, mid_high, high])` is the construction method for the `primitive_types::U256` type,
//  which takes an array of four `u64` values representing the four 64-bit parts of a 256-bit integer, from low to high.)
// 这里 `1_000_000_000_000_000_000` (10^18) 存储在最低的64位部分，其他部分为0。
// (Here, `1_000_000_000_000_000_000` (10^18) is stored in the lowest 64-bit part, with other parts being 0.)
const ONE: U256 = U256([1_000_000_000_000_000_000, 0, 0, 0]);


/// `ObjectArgs` 结构体 (对象参数结构体 / Object Arguments Struct)
///
/// 这个结构体用于缓存一些从链上获取的、常用的Aftermath协议共享对象的参数信息，
/// 并将它们转换为 `ObjectArg` 类型，以便在构建Sui可编程交易块 (PTB) 时直接使用。
/// (This struct is used to cache parameter information for commonly used Aftermath protocol shared objects fetched from the chain,
///  and converts them to `ObjectArg` type for direct use when building Sui Programmable Transaction Blocks (PTBs).)
/// 通过使用 `OnceCell` 进行异步单次初始化（见下面的 `OBJ_CACHE` 和 `get_object_args`），
/// 可以确保这些对象信息在程序生命周期内只从链上查询一次，后续使用则直接从缓存中快速获取，
/// 从而避免了不必要的网络请求和开销。
/// (By using `OnceCell` for asynchronous single initialization (see `OBJ_CACHE` and `get_object_args` below),
///  it ensures that this object information is queried from the chain only once during the program's lifecycle. Subsequent uses directly fetch from the cache quickly,
///  thus avoiding unnecessary network requests and overhead.)
///
/// `#[derive(Clone)]` 使得 `ObjectArgs` 实例可以被克隆。由于其内部字段 `ObjectArg` 通常是轻量级的（可能是ID和元数据的组合），
/// 克隆操作的成本较低。
/// (`#[derive(Clone)]` allows `ObjectArgs` instances to be cloned. Since its internal field `ObjectArg` is usually lightweight (possibly a combination of ID and metadata),
///  the cost of cloning is low.)
#[derive(Clone)]
pub struct ObjectArgs {
    pool_registry: ObjectArg,       // 池注册表对象的 `ObjectArg` 表示。(Pool Registry object's `ObjectArg` representation.)
    protocol_fee_vault: ObjectArg,  // 协议手续费库对象的 `ObjectArg` 表示。(Protocol Fee Vault object's `ObjectArg` representation.)
    treasury: ObjectArg,            // 国库对象的 `ObjectArg` 表示。(Treasury object's `ObjectArg` representation.)
    insurance_fund: ObjectArg,      // 保险基金对象的 `ObjectArg` 表示。(Insurance Fund object's `ObjectArg` representation.)
    referral_vault: ObjectArg,      // 推荐人库对象的 `ObjectArg` 表示。(Referral Vault object's `ObjectArg` representation.)
}

// `OBJ_CACHE`: 一个静态的、线程安全的 `OnceCell<ObjectArgs>` 实例。
// (`OBJ_CACHE`: A static, thread-safe `OnceCell<ObjectArgs>` instance.)
// `OnceCell` 是一种同步原语，它允许多个线程或异步任务尝试初始化一个值，但确保只有第一个成功完成初始化的会设置该值，
// 其他的则会等待初始化完成并获取相同的值，或者如果初始化失败则也得到失败结果。
// (`OnceCell` is a synchronization primitive that allows multiple threads or async tasks to attempt to initialize a value, but ensures only the first one to successfully complete initialization sets the value.
//  Others will wait for initialization to complete and get the same value, or get a failure result if initialization fails.)
// `const_new()` 创建一个空的 `OnceCell`。(creates an empty `OnceCell`.)
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数 (获取对象参数函数 / Get Object Arguments Function)
///
/// 这个函数负责获取并缓存 `ObjectArgs` 结构体。
/// (This function is responsible for fetching and caching the `ObjectArgs` struct.)
/// 它的核心逻辑是利用 `OBJ_CACHE.get_or_init()` 方法：
/// (Its core logic utilizes the `OBJ_CACHE.get_or_init()` method:)
/// -   如果 `OBJ_CACHE` 之前从未被初始化过，那么传递给 `get_or_init` 的异步闭包（`async { ... }`）会被执行。
///     (If `OBJ_CACHE` has never been initialized before, the async closure (`async { ... }`) passed to `get_or_init` will be executed.)
///     这个闭包会 (This closure will)：
///     1.  从上面定义的字符串常量（如 `POOL_REGISTRY`）解析出 `ObjectID`。
///         (Parse `ObjectID`s from the string constants defined above (e.g., `POOL_REGISTRY`).)
///     2.  使用传入的 `simulator` (交易模拟器) 从Sui网络（或模拟环境）异步获取这些 `ObjectID` 对应的链上对象数据 (`SuiObject`)。
///         (Use the passed `simulator` (transaction simulator) to asynchronously fetch on-chain object data (`SuiObject`) corresponding to these `ObjectID`s from the Sui network (or simulation environment).)
///     3.  将获取到的 `SuiObject` 转换为构建PTB时所需的 `ObjectArg` 类型（通过 `shared_obj_arg` 辅助函数）。
///         (Convert the fetched `SuiObject`s into the `ObjectArg` type required for building PTBs (via the `shared_obj_arg` helper function).)
///     4.  用这些 `ObjectArg` 填充一个新的 `ObjectArgs` 结构体实例。
///         (Populate a new `ObjectArgs` struct instance with these `ObjectArg`s.)
///     5.  这个新的 `ObjectArgs` 实例会被存入 `OBJ_CACHE` 中。
///         (This new `ObjectArgs` instance will be stored in `OBJ_CACHE`.)
/// -   如果 `OBJ_CACHE` 已经被初始化过了，那么 `get_or_init` 会直接返回缓存中 `ObjectArgs` 实例的一个引用，
///     而不会再次执行异步闭包。
///     (If `OBJ_CACHE` has already been initialized, `get_or_init` will directly return a reference to the cached `ObjectArgs` instance
///      without executing the async closure again.)
///
/// 最终，函数会等待初始化（如果是首次调用）完成，并返回缓存中 `ObjectArgs` 的一个克隆副本。
/// (Finally, the function waits for initialization (if it's the first call) to complete and returns a cloned copy of `ObjectArgs` from the cache.)
///
/// **参数 (Parameters)**:
/// - `simulator`: 一个共享的模拟器实例 (`Arc<Box<dyn Simulator>>`)，用于从链上（或模拟环境）获取对象数据。
///                使用 `Arc` 是因为 `simulator` 可能需要在多个地方共享。
///                (A shared simulator instance (`Arc<Box<dyn Simulator>>`) used to fetch object data from the chain (or simulation environment).
///                 `Arc` is used because `simulator` might need to be shared in multiple places.)
///
/// **返回 (Returns)**:
/// - `ObjectArgs`: 一个包含了所有常用Aftermath协议对象参数的 `ObjectArgs` 结构体的克隆副本。
///                 (A cloned copy of the `ObjectArgs` struct containing all commonly used Aftermath protocol object parameters.)
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async { // 如果 `OBJ_CACHE` 为空，则执行这个异步闭包来初始化它。(If `OBJ_CACHE` is empty, execute this async closure to initialize it.)
            // 从字符串常量解析ObjectID，然后通过模拟器异步获取对应的链上对象 (`SuiObject`)。
            // (Parse ObjectID from string constants, then asynchronously fetch the corresponding on-chain object (`SuiObject`) via the simulator.)
            // `.unwrap()` 在这里用于简化代码，表示我们假设这些对象一定存在且获取成功。
            // (`.unwrap()` is used here for simplicity, assuming these objects definitely exist and are fetched successfully.)
            // 在生产代码中，应该对这些 `.unwrap()` 进行更健壮的错误处理（例如使用 `?` 或 `match`）。
            // (In production code, these `.unwrap()` calls should have more robust error handling (e.g., using `?` or `match`).)
            let pool_registry_obj = simulator
                .get_object(&ObjectID::from_hex_literal(POOL_REGISTRY).unwrap()) // 将十六进制字符串转为ObjectID (Convert hex string to ObjectID)
                .await // 等待异步获取完成 (Wait for async fetch to complete)
                .unwrap(); // 假设对象获取成功 (Assume object fetch is successful)
            let protocol_fee_vault_obj = simulator
                .get_object(&ObjectID::from_hex_literal(PROTOCOL_FEE_VAULT).unwrap())
                .await
                .unwrap();
            let treasury_obj = simulator
                .get_object(&ObjectID::from_hex_literal(TREASURY).unwrap())
                .await
                .unwrap();
            let insurance_fund_obj = simulator
                .get_object(&ObjectID::from_hex_literal(INSURANCE_FUND).unwrap())
                .await
                .unwrap();
            let referral_vault_obj = simulator
                .get_object(&ObjectID::from_hex_literal(REFERRAL_VAULT).unwrap())
                .await
                .unwrap();

            // 将获取到的 `SuiObject` 转换为 PTB 中实际使用的 `ObjectArg` 类型。
            // (Convert the fetched `SuiObject` to the `ObjectArg` type actually used in PTBs.)
            // `shared_obj_arg` 是一个辅助函数 (可能定义在 `utils::object` 模块中)，
            // 它会根据传入的 `SuiObject` 的特性（例如，它是否是可变的共享对象）
            // 来创建合适的 `ObjectArg` 枚举成员 (例如 `ObjectArg::SharedObject { id, initial_shared_version, mutable }`)。
            // (`shared_obj_arg` is a helper function (possibly defined in the `utils::object` module)
            //  that creates the appropriate `ObjectArg` enum member (e.g., `ObjectArg::SharedObject { id, initial_shared_version, mutable }`)
            //  based on the characteristics of the passed `SuiObject` (e.g., whether it's a mutable shared object).)
            // 第二个布尔参数 `mutable` 指示该共享对象在交易中是否需要以可变方式引用。
            // (The second boolean parameter `mutable` indicates whether the shared object needs to be referenced mutably in the transaction.)
            ObjectArgs {
                pool_registry: shared_obj_arg(&pool_registry_obj, false), // 池注册表通常是不可变的共享对象。(Pool registry is usually an immutable shared object.)
                protocol_fee_vault: shared_obj_arg(&protocol_fee_vault_obj, false), // 手续费库通常也是不可变的。(Fee vault is usually also immutable.)
                treasury: shared_obj_arg(&treasury_obj, true), // 国库对象可能是可变的（例如，当协议向其存入资金时）。(Treasury object might be mutable (e.g., when the protocol deposits funds into it).)
                insurance_fund: shared_obj_arg(&insurance_fund_obj, true), // 保险基金也可能是可变的。(Insurance fund might also be mutable.)
                referral_vault: shared_obj_arg(&referral_vault_obj, false), // 推荐人库通常是不可变的。(Referral vault is usually immutable.)
            }
        })
        .await // 等待 `get_or_init` 的异步闭包（如果执行了的话）完成。(Wait for the async closure of `get_or_init` (if executed) to complete.)
        .clone() // `get_or_init` 返回的是对缓存值的引用，所以需要克隆一份以获得所有权。( `get_or_init` returns a reference to the cached value, so clone it to get ownership.)
}


/// `Aftermath` 结构体 (Aftermath Struct)
///
/// 代表一个Aftermath协议的特定交易池的实例。
/// (Represents an instance of a specific trading pool of the Aftermath protocol.)
/// 它封装了与该池进行交互所需的所有状态信息和参数。
/// (It encapsulates all state information and parameters required for interacting with this pool.)
///
/// `#[derive(Clone)]` 允许 `Aftermath` 实例被克隆。这在需要在不同地方（例如，不同的交易路径或模拟任务中）
/// 使用同一个池的配置信息时非常有用。
/// (`#[derive(Clone)]` allows `Aftermath` instances to be cloned. This is very useful when the configuration information of the same pool
/// needs to be used in different places (e.g., different trading paths or simulation tasks).)
#[derive(Clone)]
pub struct Aftermath {
    // --- 从链上或索引器获取的、用于PTB构建的参数 ---
    // (Parameters fetched from on-chain or indexer, used for PTB construction)
    pool_arg: ObjectArg,      // 当前交易池对象本身的 `ObjectArg` 表示。(The `ObjectArg` representation of the current trading pool object itself.)
    liquidity: u128,          // 池的流动性总量。(Total liquidity of the pool.)

    // --- 当前交易方向和类型参数 ---
    // (Current trading direction and type parameters)
    coin_in_type: String,     // 输入代币的Sui类型字符串。(Sui type string of the input coin.)
    coin_out_type: String,    // 输出代币的Sui类型字符串。(Sui type string of the output coin.)
    type_params: Vec<TypeTag>,// 调用Aftermath Swap合约的泛型函数时所需的泛型类型参数列表。
                              // (List of generic type parameters required when calling Aftermath Swap contract's generic functions.)

    // --- 从 `OBJ_CACHE` (get_object_args()) 获取的共享对象参数 ---
    // (Shared object parameters obtained from `OBJ_CACHE` (get_object_args()))
    pool_registry: ObjectArg,
    protocol_fee_vault: ObjectArg,
    treasury: ObjectArg,
    insurance_fund: ObjectArg,
    referral_vault: ObjectArg,

    // --- 从特定池对象的链上状态中解析出来的具体参数 ---
    // (Specific parameters parsed from the on-chain state of the particular pool object)
    balances: Vec<u128>,      // 池中各个代币的“标准化余额”。(Normalized balances of various coins in the pool.)
    weights: Vec<u64>,        // 池中各个代币的权重。(Weights of various coins in the pool.)
    swap_fee_in: u64,         // 输入方向的交换手续费率。(Swap fee rate for the input direction.)
    swap_fee_out: u64,        // 输出方向的交换手续费率。(Swap fee rate for the output direction.)
    index_in: usize,          // 输入代币在池代币列表中的索引。(Index of the input coin in the pool's coin list.)
    index_out: usize,         // 输出代币在池代币列表中的索引。(Index of the output coin in the pool's coin list.)
}

impl Aftermath {
    /// `new` 构造函数 (异步) (new constructor (asynchronous))
    ///
    /// 根据从 `dex_indexer` (DEX索引服务) 获取到的原始 `Pool` 信息，以及用户指定的输入代币类型 (`coin_in_type`)
    /// 和可选的输出代币类型 (`coin_out_type`)，来创建一个或多个 `Aftermath` DEX实例。
    /// (Creates one or more `Aftermath` DEX instances based on original `Pool` information obtained from `dex_indexer` (DEX indexing service),
    ///  the user-specified input coin type (`coin_in_type`), and an optional output coin type (`coin_out_type`).)
    ///
    /// **主要逻辑 (Main Logic)**: (详情见上方中文总览 / See Chinese overview above for details)
    ///
    /// **参数 (Parameters)**:
    /// - `simulator`: 共享的模拟器实例。(Shared simulator instance.)
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息。(Pool information from `dex_indexer`.)
    /// - `coin_in_type`: 输入代币类型。(Input coin type.)
    /// - `coin_out_type`: 可选的输出代币类型。(Optional output coin type.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Vec<Self>>`: 包含一个或多个 `Aftermath` 实例的向量，或错误。
    ///                       (A vector containing one or more `Aftermath` instances, or an error.)
    pub async fn new(
        simulator: Arc<Box<dyn Simulator>>,
        pool_info: &Pool,
        coin_in_type: &str,
        coin_out_type: Option<String>,
    ) -> Result<Vec<Self>> {
        ensure!(pool_info.protocol == Protocol::Aftermath, "池协议不是Aftermath (Pool protocol is not Aftermath)");

        let pool_obj = simulator
            .get_object(&pool_info.pool)
            .await
            .ok_or_else(|| eyre!("Aftermath池对象 {} 未找到 (Aftermath pool object {} not found)", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_else(|| eyre!("Aftermath池 {} 布局未找到 (Aftermath pool {} layout not found)", pool_info.pool))?;
            let move_obj = pool_obj.data.try_as_move().ok_or_else(|| eyre!("对象 {} 非Move对象 (Object {} is not a Move object)", pool_info.pool))?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!("反序列化Aftermath池 {} 失败: {} (Failed to deserialize Aftermath pool {}: {})", pool_info.pool, e))?
        };

        let liquidity = {
            let lp_supply_struct = extract_struct_from_move_struct(&parsed_pool_struct, "lp_supply")?;
            extract_u64_from_move_struct(&lp_supply_struct, "value")? as u128
        };

        let balances = extract_u128_vec_from_move_struct(&parsed_pool_struct, "normalized_balances")?;
        let weights = extract_u64_vec_from_move_struct(&parsed_pool_struct, "weights")?;
        let fees_swap_in = extract_u64_vec_from_move_struct(&parsed_pool_struct, "fees_swap_in")?;
        let fees_swap_out = extract_u64_vec_from_move_struct(&parsed_pool_struct, "fees_swap_out")?;

        let index_in = pool_info.token_index(coin_in_type).ok_or_else(|| eyre!("输入代币 {} 在池 {} 中无索引 (Input coin {} not indexed in pool {})", coin_in_type, pool_info.pool))?;

        let mut base_type_params = parsed_pool_struct.type_.type_params.clone(); // 池的所有代币类型 (All coin types in the pool)
        let coin_in_type_tag = TypeTag::from_str(coin_in_type).map_err(|e| eyre!("解析输入代币类型Tag '{}' 失败: {} (Failed to parse input coin TypeTag '{}': {})", coin_in_type, e))?;

        let pool_arg = shared_obj_arg(&pool_obj, true);
        let object_args_cache = get_object_args(Arc::clone(&simulator)).await;

        if let Some(specific_coin_out_type) = coin_out_type {
            let coin_out_type_tag = TypeTag::from_str(&specific_coin_out_type).map_err(|e| eyre!("解析输出代币类型Tag '{}' 失败: {} (Failed to parse output coin TypeTag '{}': {})", specific_coin_out_type, e))?;
            let mut final_type_params = base_type_params;
            final_type_params.push(coin_in_type_tag.clone());
            final_type_params.push(coin_out_type_tag);

            let index_out = pool_info.token_index(&specific_coin_out_type).ok_or_else(|| eyre!("输出代币 {} 在池 {} 中无索引 (Output coin {} not indexed in pool {})", specific_coin_out_type, pool_info.pool))?;

            return Ok(vec![Self {
                pool_arg, liquidity,
                coin_in_type: coin_in_type.to_string(),
                coin_out_type: specific_coin_out_type,
                type_params: final_type_params, // 完整的泛型列表，用于 swap_exact_in (Complete generic list for swap_exact_in)
                pool_registry: object_args_cache.pool_registry,
                protocol_fee_vault: object_args_cache.protocol_fee_vault,
                treasury: object_args_cache.treasury,
                insurance_fund: object_args_cache.insurance_fund,
                referral_vault: object_args_cache.referral_vault,
                balances: balances.clone(),
                weights: weights.clone(),
                swap_fee_in: fees_swap_in[index_in],
                swap_fee_out: fees_swap_out[index_out],
                index_in, index_out,
            }]);
        }

        let mut result_dex_instances = Vec::new();
        for (idx_out_candidate, coin_out_token_info) in pool_info.tokens.iter().enumerate() {
            if coin_out_token_info.token_type == coin_in_type {
                continue;
            }

            let coin_out_type_tag_candidate = TypeTag::from_str(&coin_out_token_info.token_type).map_err(|e| eyre!("解析候选输出代币类型 '{}' 失败: {} (Failed to parse candidate output coin type '{}': {})", coin_out_token_info.token_type, e))?;
            let mut final_type_params = base_type_params.clone();
            final_type_params.push(coin_in_type_tag.clone());
            final_type_params.push(coin_out_type_tag_candidate);

            result_dex_instances.push(Self {
                pool_arg: pool_arg.clone(), liquidity,
                coin_in_type: coin_in_type.to_string(),
                coin_out_type: coin_out_token_info.token_type.clone(),
                type_params: final_type_params,
                pool_registry: object_args_cache.pool_registry.clone(),
                protocol_fee_vault: object_args_cache.protocol_fee_vault.clone(),
                treasury: object_args_cache.treasury.clone(),
                insurance_fund: object_args_cache.insurance_fund.clone(),
                referral_vault: object_args_cache.referral_vault.clone(),
                balances: balances.clone(), weights: weights.clone(),
                swap_fee_in: fees_swap_in[index_in],
                swap_fee_out: fees_swap_out[idx_out_candidate],
                index_in, index_out: idx_out_candidate,
            });
        }
        Ok(result_dex_instances)
    }

    /// `build_swap_tx` (私有辅助函数，构建完整PTB / Private helper, builds full PTB)
    #[allow(dead_code)]
    async fn build_swap_tx(
        &self,
        sender: SuiAddress,
        recipient: SuiAddress,
        coin_in_ref: ObjectRef,
        amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default();
        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, Some(amount_in)).await?;
        ctx.transfer_arg(recipient, coin_out_arg);
        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数，构建调用合约参数 / Private helper, builds contract call arguments)
    async fn build_swap_args(
        &self,
        ctx: &mut TradeCtx,
        coin_in_arg: Argument,
        amount_in: u64, // 在这里 `amount_in` 用于计算 `min_amount_out`
    ) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!("转换pool_arg失败: {} (Failed to convert pool_arg: {})", e))?;
        let pool_registry_arg = ctx.obj(self.pool_registry).map_err(|e| eyre!("转换pool_registry失败: {} (Failed to convert pool_registry: {})", e))?;
        let protocol_fee_vault_arg = ctx.obj(self.protocol_fee_vault).map_err(|e| eyre!("转换protocol_fee_vault失败: {} (Failed to convert protocol_fee_vault: {})", e))?;
        let treasury_arg = ctx.obj(self.treasury).map_err(|e| eyre!("转换treasury失败: {} (Failed to convert treasury: {})", e))?;
        let insurance_fund_arg = ctx.obj(self.insurance_fund).map_err(|e| eyre!("转换insurance_fund失败: {} (Failed to convert insurance_fund: {})", e))?;
        let referral_vault_arg = ctx.obj(self.referral_vault).map_err(|e| eyre!("转换referral_vault失败: {} (Failed to convert referral_vault: {})", e))?;

        // 计算考虑滑点的最小预期输出
        // (Calculate min expected output considering slippage)
        let min_amount_out = (U256::from(self.expect_amount_out(amount_in)?) * U256::from(SLIPPAGE) / ONE).low_u64();
        let min_amount_out_arg = ctx.pure(min_amount_out).map_err(|e| eyre!("转换min_amount_out失败: {} (Failed to convert min_amount_out: {})", e))?;

        // Aftermath `swap_exact_in` 参数顺序:
        // pool_registry, protocol_fee_vault, treasury, insurance_fund, referral_vault, pool, coins_in_object, min_amount_out
        Ok(vec![
            pool_registry_arg,
            protocol_fee_vault_arg,
            treasury_arg,
            insurance_fund_arg,
            referral_vault_arg,
            pool_arg,
            coin_in_arg,
            min_amount_out_arg,
        ])
    }

    /// `expect_amount_out` (内联辅助函数，计算预期输出 / Inline helper, calculates expected output)
    #[inline]
    fn expect_amount_out(&self, amount_in: u64) -> Result<u64> {
        calculate_expected_out(
            self.balances[self.index_in],
            self.balances[self.index_out],
            self.weights[self.index_in],
            self.weights[self.index_out],
            self.swap_fee_in,
            self.swap_fee_out, // 这个参数在 calculate_expected_out 中实际如何使用需要注意
                               // (Need to be careful how this param is actually used in calculate_expected_out)
            amount_in,
        )
    }
}

/// 为 `Aftermath` 结构体实现 `Dex` trait。(Implement `Dex` trait for `Aftermath` struct.)
#[async_trait::async_trait]
impl Dex for Aftermath {
    /// `extend_trade_tx` 方法 (将Aftermath交换操作添加到PTB / Add Aftermath swap op to PTB method)
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress, // 在此实现中未使用 (Unused in this implementation)
        coin_in_arg: Argument,
        amount_in: Option<u64>,
    ) -> Result<Argument> {
        let amount_in_val = amount_in.ok_or_else(|| eyre!("Aftermath交易需提供精确输入金额 (Aftermath trade requires exact input amount)"))?;

        let package_id = ObjectID::from_hex_literal(AFTERMATH_DEX)?;
        let module_name = Identifier::new("router").map_err(|e| eyre!("创建模块名'router'失败: {} (Failed to create module name 'router': {})", e))?; // 假设路由模块是 "router"
        let function_name = Identifier::new("swap_exact_in").map_err(|e| eyre!("创建函数名'swap_exact_in'失败: {} (Failed to create function name 'swap_exact_in': {})", e))?;

        // `self.type_params` 应该已包含所有池代币类型，以及输入和输出代币类型作为最后两个。
        // (`self.type_params` should already contain all pool coin types, plus input and output coin types as the last two.)
        let type_arguments = self.type_params.clone();
        let call_arguments = self.build_swap_args(ctx, coin_in_arg, amount_in_val).await?;

        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx)) // `swap_exact_in` 返回输出的Coin对象 (returns the output Coin object)
    }

    /// `swap_tx` 方法 (构建完整独立的Aftermath交换交易 / Build full independent Aftermath swap tx method)
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await;
        let coin_in_sui_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;
        let programmable_tx_block = self
            .build_swap_tx(sender, recipient, coin_in_sui_obj.object_ref(), amount_in)
            .await?;
        let gas_coins = coin::get_gas_coin_refs(&sui_client, sender, Some(coin_in_sui_obj.coin_object_id)).await?;
        let gas_price = sui_client.read_api().get_reference_gas_price().await?;
        let tx_data = TransactionData::new_programmable(sender, gas_coins, programmable_tx_block, GAS_BUDGET, gas_price);
        Ok(tx_data)
    }

    // --- Dex trait 的其他 getter 和 setter 方法 ---
    // (Other getter and setter methods for Dex trait)
    fn coin_in_type(&self) -> String { self.coin_in_type.clone() }
    fn coin_out_type(&self) -> String { self.coin_out_type.clone() }
    fn protocol(&self) -> Protocol { Protocol::Aftermath }
    fn liquidity(&self) -> u128 { self.liquidity }
    fn object_id(&self) -> ObjectID { self.pool_arg.id() }

    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
        std::mem::swap(&mut self.index_in, &mut self.index_out);
        std::mem::swap(&mut self.swap_fee_in, &mut self.swap_fee_out);
        if self.type_params.len() >= 2 {
            let len = self.type_params.len();
            self.type_params.swap(len - 2, len - 1); // 交换最后两个类型参数 (Swap the last two type parameters)
        }
    }
    fn is_a2b(&self) -> bool { false } // Aftermath的 `swap_exact_in` 可能不区分a2b/b2a，通过代币类型和索引隐式处理
                                      // (Aftermath's `swap_exact_in` might not distinguish a2b/b2a, handling it implicitly via coin types and indices)
}

// --- Aftermath 定价和数学计算相关的辅助函数 ---
// (Helper functions for Aftermath pricing and mathematical calculations)

/// `calculate_expected_out` 函数 (计算预期输出金额 / Calculate Expected Output Amount function)
/// (详情见上方中文总览 / See Chinese overview above for details)
pub fn calculate_expected_out(
    balance_in: u128, balance_out: u128,
    weight_in: u64, weight_out: u64,
    swap_fee_in: u64, swap_fee_out: u64,
    amount_in: u64,
) -> Result<u64> {
    let spot_price_with_fees = calc_spot_price_fixed_with_fees(
        U256::from(balance_in), U256::from(balance_out),
        U256::from(weight_in), U256::from(weight_out),
        U256::from(swap_fee_in), U256::from(swap_fee_out),
    )?;
    Ok(convert_fixed_to_int(div_down(convert_int_to_fixed(amount_in), spot_price_with_fees)?))
}

// --- 固定点数数学运算辅助函数 ---
// (Fixed-point math helper functions)
fn convert_int_to_fixed(a: u64) -> U256 { U256::from(a) * ONE }
fn convert_fixed_to_int(a: U256) -> u64 { (a / ONE).low_u64() }

fn div_down(a: U256, b: U256) -> Result<U256> {
    if b.is_zero() { return Err(eyre!("定点数除法除数为零 (Fixed-point division by zero)")); }
    Ok((a * ONE) / b)
}

#[allow(dead_code)]
fn mul_down(a: U256, b: U256) -> Result<U256> { Ok((a * b) / ONE) }

fn complement(x: U256) -> U256 {
    if x < ONE { ONE - x } else { U256::zero() }
}

/// `calc_spot_price_fixed_with_fees` 函数 (计算含费用的现货价 / Calculate Spot Price with Fees function)
/// (详情见上方中文总览 / See Chinese overview above for details)
fn calc_spot_price_fixed_with_fees(
    balance_in: U256, balance_out: U256,
    weight_in: U256, weight_out: U256,
    swap_fee_in: U256, swap_fee_out: U256,
) -> Result<U256> {
    let spot_price_no_fees = calc_spot_price(balance_in, balance_out, weight_in, weight_out)?;
    // Aftermath的费用模型似乎是 (1 - fee_in) * (1 - fee_out) 作为分母，或者等效地，价格乘以 (1 / ((1-fee_in)(1-fee_out)))
    // 这里假设 swap_fee_in 和 swap_fee_out 都是交易中会产生的费用，需要从保留比例中扣除。
    // 实际有效的费用调整因子是 (1 - swap_fee_in)。如果swap_fee_out也适用，则需要确认 Aftermath 的精确费用模型。
    // 通常，对于单次swap_exact_in, 使用一个综合的swap_fee。如果 `swap_fee_in` 和 `swap_fee_out` 代表不同场景，
    // 这里的 `fees_scalar` 可能需要调整。假设这里 `swap_fee_in` 是实际应用的费率。
    // let fees_scalar = complement(swap_fee_in); // 使用 complement(swap_fee_in) 即 (ONE - swap_fee_in)
    // 如果费用是从输出中扣除，那么价格应该除以 (1-fee)。如果从输入中扣除，价格应该乘以 (1-fee)。
    // Balancer公式是 P_effective = P_spot / (1 - fee)。所以这里用 `div_down` 是合理的。
    // 但 `complement(swap_fee_in)` 和 `complement(swap_fee_out)` 相乘可能不正确，
    // 通常是只应用一个方向的费率，或者一个综合费率。
    // 假设 Aftermath 的文档或实现表明 `swap_fee_in` 是主要的交易费。
    let fees_scalar = complement(swap_fee_in); // 假设只使用输入费率
    if fees_scalar.is_zero() { return Err(eyre!("计算Aftermath现货价格时费用调整因子为零 (Fee scalar is zero in Aftermath spot price calc)")); }
    div_down(spot_price_no_fees, fees_scalar)
}

/// `calc_spot_price` 函数 (计算不含费用的原始现货价 / Calculate Raw Spot Price without Fees function)
/// (详情见上方中文总览 / See Chinese overview above for details)
fn calc_spot_price(balance_in: U256, balance_out: U256, weight_in: U256, weight_out: U256) -> Result<U256> {
    // (BalanceIn / WeightIn) 的定点表示
    // (Fixed-point representation of (BalanceIn / WeightIn))
    let term_in_fixed = div_down(balance_in, weight_in)?;
    // (BalanceOut / WeightOut) 的定点表示
    // (Fixed-point representation of (BalanceOut / WeightOut))
    let term_out_fixed = div_down(balance_out, weight_out)?;
    // 现货价格 SP = term_in_fixed / term_out_fixed
    // (Spot Price SP = term_in_fixed / term_out_fixed)
    div_down(term_in_fixed, term_out_fixed)
}

// --- 测试模块 ---
// (Test module)
#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use object_pool::ObjectPool;
    use simulator::{DBSimulator, Simulator};
    use tracing::info;
    use super::*;
    use crate::{
        config::tests::{TEST_ATTACKER, TEST_HTTP_URL},
        defi::{indexer_searcher::IndexerDexSearcher, DexSearcher},
    };

    /// `test_aftermath_swap_tx` 测试函数 (test_aftermath_swap_tx test function)
    #[tokio::test]
    async fn test_aftermath_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug"]);
        let simulator_pool = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(DBSimulator::new_test(true).await) as Box<dyn Simulator> })
        }));

        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap();
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI";
        let token_out_type = "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN"; // Wormhole USDC
        let amount_in = 1_000_000_000; // 1 SUI

        let searcher = IndexerDexSearcher::new(TEST_HTTP_URL, Arc::clone(&simulator_pool))
            .await
            .unwrap();
        let dexes = searcher
            .find_dexes(token_in_type, Some(token_out_type.into()))
            .await
            .unwrap();
        info!("🧀 (测试信息) 找到的DEX总数量 (Total DEXs found): {}", dexes.len());

        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::Aftermath)
            .max_by_key(|dex| dex.liquidity()) // 选择流动性最高的Aftermath池 (Select Aftermath pool with highest liquidity)
            .expect("测试环境中未能找到Aftermath协议的池 (Aftermath pool not found in test environment)");

        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 (测试信息) 构建的Aftermath交换交易数据 (Constructed Aftermath swap tx data): {:?}", tx_data);

        let simulator_instance = simulator_pool.get();
        let response = simulator_instance.simulate(tx_data, Default::default()).await.unwrap();
        info!("🧀 (测试信息) Aftermath交换交易的模拟结果 (Aftermath swap tx simulation result): {:?}", response);

        assert!(response.is_ok(), "Aftermath交换交易的模拟应该成功执行 (Aftermath swap tx simulation should succeed)");
    }
}

[end of bin/arb/src/defi/aftermath.rs]
