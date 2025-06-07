// 该文件 `config.rs` 的主要作用是存放整个套利机器人应用程序的全局配置参数和一些共享的常量值。
// 将这些配置信息集中存放在一个专门的文件中，有几个好处：
// 1.  **易于管理和修改**：当需要调整某些参数（比如Gas预算、价格范围）时，可以直接在这个文件中找到并修改，而不需要在代码的多个地方搜索。
// 2.  **提高代码可读性**：通过使用有意义的常量名（如 `GAS_BUDGET`），代码中引用这些值的地方会更清晰易懂。
// 3.  **方便共享**：项目中的其他模块如果需要这些配置值，可以直接导入并使用它们。
//
// **文件概览 (File Overview)**:
// 这个配置文件主要包含以下几个部分：
//
// 1.  **`GAS_BUDGET` 常量**:
//     定义了在Sui区块链上发送交易时，愿意支付的Gas单位数量的上限。Gas是Sui网络中用于支付交易执行和存储的“燃料费”。
//     (Defines the maximum amount of Gas units willing to be paid when sending a transaction on the Sui blockchain. Gas is the "fuel fee" for transaction execution and storage on the Sui network.)
//
// 2.  **`MAX_SQRT_PRICE_X64` 和 `MIN_SQRT_PRICE_X64` 常量**:
//     这两个常量定义了“价格平方根”（sqrt_price）的允许范围。这种价格表示方式常见于某些先进的去中心化交易所（DeFi）协议，
//     特别是那些采用“集中流动性做市商”（CLMM，Concentrated Liquidity Market Maker）模型的交易所，例如Uniswap V3及其在Sui上的类似实现。
//     (These constants define the allowable range for "square root of price" (sqrt_price). This price representation is common in some advanced Decentralized Finance (DeFi) protocols,
//      especially those using the "Concentrated Liquidity Market Maker" (CLMM) model, such as Uniswap V3 and its similar implementations on Sui.)
//     -   `X64` 后缀通常表示这个值是使用Q64.64定点数格式来表示的。这意味着一个128位的整数被分为两部分：
//         高64位代表整数部分，低64位代表小数部分（即小数点后有64位二进制精度）。这种方式可以在整数运算的基础上实现高精度的小数计算。
//         (The `X64` suffix usually indicates that this value is represented using the Q64.64 fixed-point number format. This means a 128-bit integer is divided into two parts:
//          the upper 64 bits represent the integer part, and the lower 64 bits represent the fractional part (i.e., 64 bits of binary precision after the decimal point). This method allows for high-precision decimal calculations using integer arithmetic.)
//
// 3.  **`pegged_coin_types()` 函数**:
//     这个函数返回一个包含多种“锚定币”（pegged coins）或主流币种的类型字符串的集合（`HashSet`）。
//     (This function returns a `HashSet` containing type strings of various "pegged coins" or major cryptocurrencies.)
//     -   **锚定币 (Pegged Coins)**：指其价值与某种法定货币（如美元USD）或其他真实世界资产（如黄金）保持相对稳定（锚定）的加密货币。常见的例子有USDC、USDT等。
//         (Pegged Coins: Cryptocurrencies whose value is kept relatively stable (pegged) to a fiat currency (like USD) or other real-world assets (like gold). Common examples include USDC, USDT.)
//     -   **主流币种 (Major Coins)**：也可能包含一些非锚定但非常流行、流动性好的加密货币，如Wrapped ETH (WETH)、或者Sui的原生代币SUI本身。
//         (Major Coins: May also include non-pegged but very popular, highly liquid cryptocurrencies like Wrapped ETH (WETH), or Sui's native token SUI itself.)
//     在套利逻辑中，这些币种可能因为其稳定性、高流动性或作为常用报价货币而被特殊对待，或者作为套利交易对的基础币种。
//     (In arbitrage logic, these coins might be treated specially due to their stability, high liquidity, or use as common quote currencies, or as base currencies for arbitrage pairs.)
//
// 4.  **`tests` 模块 (仅在测试(`#[cfg(test)]`)时编译)**:
//     这个子模块专门用于存放测试环境下使用的特定常量。例如：
//     (This submodule is dedicated to storing specific constants used in the testing environment. For example:)
//     -   测试时连接的Sui RPC节点的URL地址。 (The URL address of the Sui RPC node to connect to during testing.)
//     -   测试时使用的Sui账户地址（通常需要预先充值测试代币）。(The Sui account address used for testing (usually needs to be pre-funded with test tokens).)
//     将测试配置与主配置分开，可以确保主配置的整洁，并方便在测试时使用固定的、可控的环境参数。
//     (Separating test configuration from the main configuration ensures the main configuration remains clean and facilitates the use of fixed, controllable environment parameters during testing.)
//     *(注意：在提供的原始代码中，这些测试常量的值被设置为空字符串。在实际运行测试之前，开发者需要将它们替换为有效的测试网络地址和账户。)*
//     *(Note: In the provided original code, the values of these test constants are set to empty strings. Developers need to replace them with valid test network addresses and accounts before actually running the tests.)*

use std::collections::HashSet; // 引入标准库中的 `HashSet` 数据结构。
                               // `HashSet` 用于存储一组唯一的元素，并且能够快速地检查某个元素是否存在于集合中。
                               // 在这里，它被用来存储一组代币的类型字符串。
                               // Import `HashSet` from the standard library.
                               // `HashSet` is used to store a unique set of elements and allows for fast checking of an element's existence.
                               // Here, it's used to store a set of coin type strings.

use sui_sdk::SUI_COIN_TYPE; // 从 `sui_sdk` (Sui官方软件开发工具包) 中引入 `SUI_COIN_TYPE` 常量。
                           // `SUI_COIN_TYPE` 是一个字符串常量，代表了Sui区块链原生代币 (SUI) 的官方、标准类型标识符，
                           // 通常是 "0x2::sui::SUI"。
                           // Import `SUI_COIN_TYPE` constant from the `sui_sdk` (Sui official Software Development Kit).
                           // `SUI_COIN_TYPE` is a string constant representing the official, standard type identifier for Sui's native token (SUI),
                           // usually "0x2::sui::SUI".

/// `GAS_BUDGET` 常量 (Gas预算)
///
/// 这个常量定义了在Sui区块链上执行一笔交易时，我们愿意设置的Gas预算的上限值。
/// Gas是Sui网络中用于支付交易执行、数据存储等操作所需消耗的“燃料”的计量单位。
/// 每笔交易都需要指定一个Gas预算，表示你最多愿意为这笔交易支付多少Gas。
/// (This constant defines the maximum gas budget we are willing to set when executing a transaction on the Sui blockchain.)
/// (Gas is the unit used to measure the "fuel" consumed for operations like transaction execution and data storage in the Sui network.)
/// (Each transaction needs a specified gas budget, indicating the maximum gas you are willing to pay for it.)
///
/// 设置一个合理的Gas预算非常重要：
/// (Setting a reasonable gas budget is very important:)
/// -   **如果预算太低 (If the budget is too low)**：交易可能会因为Gas不足而执行失败，已经消耗的Gas通常不会退还。
///     (The transaction might fail due to insufficient gas, and the gas already consumed is usually not refunded.)
/// -   **如果预算太高 (If the budget is too high)**：交易会正常执行。如果实际消耗的Gas远低于预算，未使用的部分会被退还给发送者。
///     (The transaction will execute normally. If the actual gas consumed is much lower than the budget, the unused portion is refunded to the sender.)
///     然而，设置过高的预算也意味着你的账户中必须有足够的SUI余额来暂时覆盖这个预算额度。
///     (However, setting an overly high budget also means your account must have enough SUI balance to temporarily cover this budget amount.)
///
/// 这里的 `10_000_000_000` (即100亿 MIST) 是一个相对较大的Gas预算。
/// MIST是SUI代币的最小单位，1 SUI = 10^9 MIST。所以这个预算相当于 10 SUI。
/// (The value `10_000_000_000` (i.e., 10 billion MIST) here is a relatively large gas budget.)
/// (MIST is the smallest unit of the SUI token, where 1 SUI = 10^9 MIST. So this budget is equivalent to 10 SUI.)
/// 这么大的预算可能是为了确保那些涉及多个步骤、与多个智能合约交互的复杂套利交易能够成功执行，
/// 因为这类交易通常会消耗更多的Gas。
/// (Such a large budget might be to ensure that complex arbitrage transactions involving multiple steps and interactions with multiple smart contracts can execute successfully,
/// as these types of transactions typically consume more gas.)
pub const GAS_BUDGET: u64 = 10_000_000_000;

/// `MAX_SQRT_PRICE_X64` 常量 (最大价格平方根，X64表示法)
/// (Maximum square root of price, X64 representation)
///
/// 这个常量定义了“价格平方根”（sqrt_price）所能允许的最大值，并且这个值是使用X64表示法（也常称为Q64.64定点数格式）存储的。
/// (This constant defines the maximum allowed value for "square root of price" (sqrt_price), stored using X64 notation (often called Q64.64 fixed-point format).)
///
/// **背景知识：价格平方根与集中流动性AMM (Background: Sqrt Price and Concentrated Liquidity AMMs)**
/// 在一些先进的去中心化金融（DeFi）协议中，特别是采用“集中流动性做市商”（CLMM）模型的自动做市商（AMM）（例如Uniswap V3及其在Sui上的诸多仿制品），
/// 为了进行某些数学运算（如计算流动性、交易滑点等）并提高计算精度，价格并不总是直接存储的。
/// 相反，它们可能会存储价格的平方根 (`sqrt(P)`)。
/// (In some advanced DeFi protocols, especially Automated Market Makers (AMMs) using the "Concentrated Liquidity Market Maker" (CLMM) model (e.g., Uniswap V3 and its Sui counterparts),
/// price is not always stored directly. Instead, the square root of the price (`sqrt(P)`) might be stored to perform certain mathematical operations (like calculating liquidity, slippage) and improve precision.)
///
/// **X64 (Q64.64) 定点数表示法 (X64 (Q64.64) Fixed-Point Notation)**:
/// `X64` 格式意味着这个 `u128` 类型的无符号128位整数实际上代表一个小数。
/// (The `X64` format means this `u128` (unsigned 128-bit integer) actually represents a decimal number.)
/// 它的位被划分为两部分 (Its bits are divided into two parts)：
/// -   较高的64位 (Higher 64 bits)：代表数字的整数部分 (Represent the integer part of the number)。
/// -   较低的64位 (Lower 64 bits)：代表数字的小数部分 (Represent the fractional part of the number)。
/// 这种表示方法允许在只有整数运算能力的计算机硬件上高效地处理高精度的小数。
/// (This representation allows for efficient handling of high-precision decimals on hardware with only integer arithmetic capabilities.)
///
/// 这个 `MAX_SQRT_PRICE_X64` 值通常对应于一个代币相对于另一个代币的价格达到一个极高的理论上限时的平方根。
/// (This `MAX_SQRT_PRICE_X64` value usually corresponds to the square root of a price when one token reaches an extremely high theoretical upper limit relative to another.)
/// 它用于定义价格区间的边界或进行计算时的溢出检查。
/// (It is used to define the boundaries of price ranges or for overflow checks during calculations.)
pub const MAX_SQRT_PRICE_X64: u128 = 79226673515401279992447579055; // 这个具体数值可能来源于某个特定AMM协议的规范 (This specific value might come from a particular AMM protocol's specification)

/// `MIN_SQRT_PRICE_X64` 常量 (最小价格平方根，X64表示法)
/// (Minimum square root of price, X64 representation)
///
/// 与 `MAX_SQRT_PRICE_X64` 对应，这个常量定义了价格平方根所能允许的最小值（但通常仍大于0），同样采用X64表示法。
/// (Corresponding to `MAX_SQRT_PRICE_X64`, this constant defines the minimum allowed value for the square root of price (though typically still greater than 0), also using X64 notation.)
/// 这通常对应于一个代币相对于另一个代币的价格达到一个极低的理论下限（接近于零但不完全是零）时的平方根。
/// (This usually corresponds to the square root of a price when one token reaches an extremely low theoretical lower limit (close to zero but not exactly zero) relative to another.)
/// `MAX_SQRT_PRICE_X64` 和 `MIN_SQRT_PRICE_X64` 共同定义了在相关DeFi协议中价格（或其平方根）的有效表示范围。
/// (`MAX_SQRT_PRICE_X64` and `MIN_SQRT_PRICE_X64` together define the valid representation range for price (or its square root) in relevant DeFi protocols.)
/// 超出这个范围的价格可能被认为是无效的或不切实际的。
/// (Prices outside this range might be considered invalid or impractical.)
pub const MIN_SQRT_PRICE_X64: u128 = 4295048016; // 这个具体数值同样可能来源于协议规范 (This specific value might also come from a protocol specification)

/// `pegged_coin_types` 函数 (获取锚定币/主流币种类型列表)
/// (Function to get a list of pegged/major coin types)
///
/// 这个函数的作用是返回一个 `HashSet<&'static str>`，其中包含了多种被认为是“锚定币”（pegged coins）
/// 或主流币种的代币类型字符串。
/// (The purpose of this function is to return a `HashSet<&'static str>` containing type strings of various tokens considered "pegged coins" or major cryptocurrencies.)
///
/// **什么是锚定币/主流币种? (What are Pegged/Major Coins?)**
/// -   **锚定币 (Pegged Coins)**：指那些其价值试图与某种法定货币（例如美元USD、欧元EUR）或其他相对稳定的资产（例如黄金）
///     保持1:1或其他固定比例锚定的加密货币。最常见的例子是各种美元稳定币，如USDC (USD Coin)、USDT (Tether)。
///     (Pegged Coins: Cryptocurrencies whose value attempts to remain pegged at a 1:1 or other fixed ratio to a fiat currency (e.g., USD, EUR) or other relatively stable assets (e.g., gold). The most common examples are various USD stablecoins like USDC and USDT.)
/// -   **主流币种 (Major Coins)**：除了严格意义上的锚定币，这个列表也可能包含一些虽然价格会波动，但在加密货币生态系统中被广泛接受、
///     交易量大、流动性好的主要加密货币，例如Wrapped Ether (WETH) 或特定链的原生代币（如SUI）。
///     (Major Coins: Besides strictly pegged coins, this list might also include major cryptocurrencies that, while their prices fluctuate, are widely accepted, heavily traded, and highly liquid within the crypto ecosystem, such as Wrapped Ether (WETH) or a chain's native token (like SUI).)
///
/// **在套利逻辑中的作用 (Role in Arbitrage Logic)**:
/// 在套利机器人的逻辑中，这些被列出的币种可能因为以下原因而受到特殊关注或处理：
/// (In an arbitrage bot's logic, these listed coins might receive special attention or handling for the following reasons:)
/// -   **作为报价货币或基础货币 (As Quote or Base Currencies)**：许多去中心化交易所（DEX）的交易对都是以这些稳定币或主流币（如SUI、USDC）来计价的。
///     (Many trading pairs on DEXs are quoted in these stablecoins or major coins (like SUI, USDC).)
/// -   **流动性 (Liquidity)**：这些币种通常具有较好的流动性，这意味着可以进行较大金额的交易而不会造成过大的价格滑点，这对于套利至关重要。
///     (These coins usually have good liquidity, meaning large trades can be made without causing excessive price slippage, which is crucial for arbitrage.)
/// -   **策略相关性 (Strategy Relevance)**：某些套利策略可能专门围绕这些币种设计，例如，寻找不同稳定币之间微小的价格偏差进行套利。
///     (Some arbitrage strategies might be specifically designed around these coins, e.g., arbitraging tiny price differences between different stablecoins.)
/// -   **风险管理 (Risk Management)**：有时，套利者可能更倾向于最终持有这些相对稳定的币种以降低风险。
///     (Sometimes, arbitrageurs might prefer to end up holding these relatively stable coins to reduce risk.)
///
/// **`HashSet<&'static str>`**:
/// -   `HashSet`：使用哈希集合来存储这些类型字符串，可以确保列表中的每个代币类型都是唯一的，并且能够非常快速地查询某个特定的代币类型是否存在于此集合中。
///     (Using `HashSet` to store these type strings ensures each coin type in the list is unique and allows for very fast lookup of whether a specific coin type exists in the set.)
/// -   `&'static str`：表示存储在集合中的是字符串字面量（literal strings）的引用。`'static` 生命周期说明这些字符串是在程序编译时就已知的，
///     并且在整个程序的运行期间都有效（它们通常存储在程序二进制文件的只读数据段中）。
///     (Indicates that references to string literals are stored in the set. The `'static` lifetime means these strings are known at compile time and are valid for the entire duration of the program (they are usually stored in the read-only data segment of the program's binary).)
///
/// **返回 (Returns)**:
/// - `HashSet<&'static str>`: 一个包含多种锚定币或主流币种的Sui代币类型字符串的集合。
///   (A `HashSet` containing Sui coin type strings for various pegged or major coins.)
pub fn pegged_coin_types() -> HashSet<&'static str> {
    // `HashSet::from_iter([...])` 从一个包含元素的数组（这里是字符串字面量数组）创建一个 `HashSet`。
    // `iter()` 方法（或隐式调用）会遍历数组中的每个元素。
    // `HashSet::from_iter([...])` creates a `HashSet` from an array of elements (here, string literals).
    // The `iter()` method (or implicit call) iterates over each element in the array.
    HashSet::from_iter([
        // **SUI 原生代币 (SUI Native Token)**
        SUI_COIN_TYPE, // "0x2::sui::SUI"，这是Sui平台的官方原生代币。 (This is the official native token of the Sui platform.)

        // --- 各种版本的稳定币 (主要是USDC, USDT) ---
        // (Various versions of stablecoins, mainly USDC, USDT)
        // **重要提示 (Important Note)**：在像Sui这样的多链、多桥接生态系统中，同一种逻辑上的稳定币（例如“美元稳定币USDC”）
        // 可能会有多种不同的链上表示形式。这些不同的版本可能来自于：
        // (In a multi-chain, multi-bridge ecosystem like Sui, the same logical stablecoin (e.g., "USD stablecoin USDC")
        // can have multiple different on-chain representations. These different versions might originate from:)
        //   - 不同的发行机构。 (Different issuers.)
        //   - 通过不同的跨链桥（bridge）从其他区块链（如以太坊）转移到Sui上的。 (Being transferred to Sui from other blockchains (like Ethereum) via different cross-chain bridges.)
        //   - 不同的包装（wrapping）方式。 (Different wrapping methods.)
        // 因此，它们在Sui区块链上会拥有各自不同的对象类型地址（即这里的类型字符串）。
        // (Therefore, they will have their own distinct object type addresses (i.e., the type strings here) on the Sui blockchain.)
        // 对于套利机器人来说，区分这些不同版本的同种稳定币非常重要，因为它们之间可能存在微小的价格差异，从而产生套利机会。
        // (For an arbitrage bot, distinguishing between these different versions of the same stablecoin is very important, as minute price differences between them can create arbitrage opportunities.)
        // 同时，它们的流动性、安全性、以及被DEX支持的程度也可能不同。
        // (Additionally, their liquidity, security, and the extent to which they are supported by DEXs can also differ.)
        // 下面列出的是一些在Sui生态中已知的或常用的USDC和USDT等代币的类型字符串：
        // (Listed below are some type strings for USDC, USDT, etc., known or commonly used in the Sui ecosystem:)

        // **Wormhole USDC (来自以太坊的USDC，通过Wormhole桥接)**
        // (Wormhole USDC (USDC from Ethereum, bridged via Wormhole))
        // Wormhole是一种流行的跨链桥协议，允许资产在不同区块链之间转移。
        // (Wormhole is a popular cross-chain bridge protocol that allows assets to be transferred between different blockchains.)
        // 这个地址代表了通过Wormhole从以太坊等其他链桥接过来的USDC。
        // (This address represents USDC bridged over from Ethereum or other chains via Wormhole.)
        // 格式通常是 `<BRIDGE_CONTRACT_ADDRESS>::coin::COIN`，其中 `coin::COIN` 是一个通用表示。
        // (The format is usually `<BRIDGE_CONTRACT_ADDRESS>::coin::COIN`, where `coin::COIN` is a generic representation.)
        "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN", // USDCet (Wormhole Ethereum USDC)

        // **Wormhole USDT (来自以太坊的USDT，通过Wormhole桥接)**
        // (Wormhole USDT (USDT from Ethereum, bridged via Wormhole))
        // 类似于上面的USDC，这是通过Wormhole桥接过来的USDT。
        // (Similar to the USDC above, this is USDT bridged over via Wormhole.)
        "0xc060006111016b8a020ad5b33834984a437aaa7d3c74c18e09a95d48aceab08c::coin::COIN", // USDTet (Wormhole Ethereum USDT)

        // **Wormhole WETH (来自以太坊的Wrapped ETH，通过Wormhole桥接)**
        // (Wormhole WETH (Wrapped ETH from Ethereum, bridged via Wormhole))
        // Wrapped ETH (WETH) 是以太坊原生币ETH的ERC-20代币版本。这是桥接过来的WETH。
        // (Wrapped ETH (WETH) is the ERC-20 token version of Ethereum's native coin ETH. This is bridged WETH.)
        "0xaf8cd5edc19c4512f4259f0bee101a40d41ebed738ade5874359610ef8eeced5::coin::COIN", // WETHet (Wormhole Ethereum WETH)

        // **Celer USDC (另一种通过Celer cBridge桥接的USDC)**
        // (Celer USDC (another USDC bridged via Celer cBridge))
        // Celer cBridge是另一种跨链桥方案。
        // (Celer cBridge is another cross-chain bridge solution.)
        // "0xb231fcda8bbddb31f2ef02e6161444aec64a514e2c89279584ac9806ce9cf037::coin::COIN", // 这个地址之前在注释中被提及，但可能不常用或已被取代。(This address was previously mentioned in comments but might be less common or deprecated.)
        // 下面这个地址看起来更像是一个由特定项目（可能与Celer合作）发行的、更具体的USDC类型，
        // 它使用了自定义的模块名 `usdc` 和结构名 `USDC`，而不是通用的 `coin::COIN`。
        // (The address below looks more like a specific USDC type issued by a particular project (possibly in collaboration with Celer),
        // using custom module name `usdc` and struct name `USDC` instead of the generic `coin::COIN`.)
        "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC", // ceUSDC (Celer Ethereum USDC)

        // **Bucket Protocol的稳定币 BUCK (BUCK Stablecoin from Bucket Protocol)**
        // Bucket Protocol 是Sui生态系统中的一个原生去中心化借贷协议，它也发行自己的稳定币BUCK。
        // (Bucket Protocol is a native decentralized lending protocol in the Sui ecosystem that also issues its own stablecoin, BUCK.)
        // BUCK通过超额抵押SUI等资产来生成，并试图将其价值锚定在1美元。
        // (BUCK is generated by over-collateralizing assets like SUI and attempts to peg its value to $1.)
        "0xce7ff77a83ea0cb6fd39bd8748e2ec89a3f41e8efdc3f4eb123e0ca37b184db2::buck::BUCK",
    ])
}

// --- 测试模块 (`tests`) ---
// (Test Module (`tests`))
// `#[cfg(test)]` 是一个条件编译属性宏。它告诉Rust编译器，只有在执行 `cargo test` 命令
// （即编译项目用于测试目的）时，才需要编译和包含这个 `tests` 模块。
// (`#[cfg(test)]` is a conditional compilation attribute macro. It tells the Rust compiler to compile and include this `tests` module only when the `cargo test` command is executed (i.e., when compiling the project for testing purposes).)
// 在正常的 `cargo build` 或 `cargo run`（用于构建和运行应用程序）时，这个模块会被忽略。
// (During normal `cargo build` or `cargo run` (for building and running the application), this module is ignored.)
// 这种做法的好处是 (The advantages of this approach are)：
// 1.  测试代码（包括测试用的常量、辅助函数等）不会被包含在最终的生产版本二进制文件中，减小了文件大小。
//     (Test code (including test constants, helper functions, etc.) will not be included in the final production binary, reducing file size.)
// 2.  测试相关的依赖和逻辑与主应用程序逻辑分离，使代码结构更清晰。
//     (Test-related dependencies and logic are separated from the main application logic, making the code structure clearer.)
#[cfg(test)]
pub mod tests {
    // `TEST_HTTP_URL` 常量
    // (TEST_HTTP_URL Constant)
    //
    // 这个常量定义了在运行自动化测试时，Sui客户端应该连接的Sui RPC节点的HTTP URL地址。
    // (This constant defines the HTTP URL address of the Sui RPC node that the Sui client should connect to when running automated tests.)
    // 通常，测试会在一个专门的测试网络（testnet）、开发网络（devnet）或者本地运行的Sui节点上进行，
    // 以避免影响主网络（mainnet）或产生真实费用。
    // (Usually, tests are conducted on a dedicated test network (testnet), development network (devnet), or a locally running Sui node to avoid affecting the mainnet or incurring real costs.)
    //
    // **重要 (IMPORTANT)**: 在提供的代码中，这个值被设置为空字符串 `""`。
    // (In the provided code, this value is set to an empty string `""`.)
    // 这意味着如果直接运行依赖此常量的测试，它们很可能会因为无法连接到有效的RPC节点而失败。
    // (This means that if tests depending on this constant are run directly, they will likely fail due to inability to connect to a valid RPC node.)
    // 开发者在实际运行测试前，需要将这个空字符串替换为一个有效的测试RPC节点URL。
    // (Developers need to replace this empty string with a valid test RPC node URL before actually running tests.)
    // 例如 (For example): `"https://fullnode.testnet.sui.io:443"` (连接到Sui官方测试网 (connect to Sui official testnet))
    // 或者 (or) `"http://127.0.0.1:9000"` (连接到本地运行的Sui节点，默认端口9000 (connect to a locally running Sui node, default port 9000))
    pub const TEST_HTTP_URL: &str = ""; // 示例 (Example): "https://fullnode.devnet.sui.io:443"

    // `TEST_ATTACKER` 常量
    // (TEST_ATTACKER Constant)
    //
    // 这个常量定义了在运行测试时，用作交易发送者（有时在MEV或套利场景下也称为“攻击者”attacker）的Sui账户地址。
    // (This constant defines the Sui account address used as the transaction sender (sometimes referred to as an "attacker" in MEV or arbitrage scenarios) during testing.)
    // 这个账户在测试环境中需要 (This account, in the test environment, needs to)：
    // 1.  拥有足够的测试SUI代币，以支付执行测试交易所需的Gas费用。
    //     (Have enough test SUI tokens to pay for the gas fees required to execute test transactions.)
    // 2.  可能还需要拥有一些其他用于测试的特定代币（例如，上面 `pegged_coin_types` 中列出的某些代币的测试网版本）。
    //     (Possibly also have some other specific tokens for testing (e.g., testnet versions of some coins listed in `pegged_coin_types` above).)
    //
    // Sui地址通常是一个以 "0x" 开头的十六进制字符串。
    // (A Sui address is typically a hexadecimal string starting with "0x".)
    //
    // **重要 (IMPORTANT)**: 同样地，这个值在提供的代码中也是空字符串 `""`。
    // (Similarly, this value is also an empty string `""` in the provided code.)
    // 开发者需要将其替换为一个在所选测试环境 (`TEST_HTTP_URL` 指向的网络) 中实际存在的、
    // 并且拥有必要资金和代币的Sui账户地址。
    // (Developers need to replace it with a Sui account address that actually exists in the chosen test environment (the network pointed to by `TEST_HTTP_URL`) and has the necessary funds and tokens.)
    // 例如 (For example): `"0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"` (一个虚构的64字符十六进制地址 (a fictional 64-character hexadecimal address))
    pub const TEST_ATTACKER: &str = ""; // 示例 (Example): "0xyour_test_sui_address_hex_string_here"
}

[end of bin/arb/src/config.rs]
