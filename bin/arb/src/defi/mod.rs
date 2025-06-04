// 该文件 `mod.rs` 是 `defi` 模块的根文件，它扮演着该模块的组织核心和外部入口点的角色。
// `defi` 模块（DeFi 是 Decentralized Finance，即去中心化金融的缩写）封装了与各种去中心化金融协议进行交互的所有核心逻辑。
// 在这个套利机器人的背景下，它特别关注与各种“去中心化交易所”（DEX, Decentralized Exchange）的交互，
// 以及如何发现、评估和执行潜在的套利交易路径。
//
// **文件概览 (File Overview)**:
// 这个 `mod.rs` 文件是 `defi` 模块的“总指挥部”或者说“模块声明文件”。
// `defi` 这个名字是“去中心化金融”（Decentralized Finance）的缩写。顾名思义，这个模块里包含了所有与各种DeFi协议打交道的代码。
// 对于套利机器人来说，最重要的DeFi协议就是“去中心化交易所”（DEX），因为套利通常就是在不同的DEX之间，或者同一个DEX的不同交易对之间寻找价格差异来赚钱。
// (This `mod.rs` file is the "general headquarters" or "module declaration file" for the `defi` module.
//  The name `defi` is short for "Decentralized Finance". As the name suggests, this module contains all the code for interacting with various DeFi protocols.
//  For an arbitrage bot, the most important DeFi protocols are "Decentralized Exchanges" (DEXs), as arbitrage usually involves finding price differences between different DEXs or different trading pairs on the same DEX to make a profit.)
//
// **这个文件主要承担以下职责 (This file primarily handles the following responsibilities)**:
//
// 1.  **声明子模块 (Declaring Submodules)**:
//     -   它像一个目录一样，列出了 `defi` 模块下所有的“部门”（子模块）。每个子模块通常负责与一个特定的DEX协议（比如Aftermath, Cetus, Turbos等）进行交互，或者是提供一些通用的DeFi辅助功能（比如`trade`模块负责交易执行和模拟，`indexer_searcher`模块负责从外部服务发现DEX池子）。
//     -   例如，`mod aftermath;` 就表示项目中有一个叫做 `aftermath` 的子模块，它的代码可能在 `defi/aftermath.rs` 文件里。
//         (It acts like a directory, listing all the "departments" (submodules) under the `defi` module. Each submodule is typically responsible for interacting with a specific DEX protocol (e.g., Aftermath, Cetus, Turbos), or for providing general DeFi utility functions (e.g., the `trade` module for transaction execution and simulation, the `indexer_searcher` module for discovering DEX pools from external services).
//          For example, `mod aftermath;` indicates that there's a submodule named `aftermath` in the project, and its code might be in the `defi/aftermath.rs` file.)
//
// 2.  **定义核心 Traits (接口) (Defining Core Traits (Interfaces))**:
//     -   **`DexSearcher`**:
//         -   “Trait”在Rust里有点像其他语言中的“接口”或“规范”。`DexSearcher` 这个接口定义了“如何去寻找和发现可用的DEX”的一套标准方法。
//         -   任何想要提供“DEX发现服务”的组件（比如上面提到的 `indexer_searcher`）都必须按照这个接口的标准来实现功能。
//             (A "Trait" in Rust is somewhat like an "interface" or "specification" in other languages. The `DexSearcher` trait defines a standard set of methods for "how to find and discover available DEXs".
//              Any component that wants to provide a "DEX discovery service" (like the `indexer_searcher` mentioned above) must implement its functionality according to the standards of this trait.)
//     -   **`Dex`**:
//         -   这是另一个非常关键的接口，它定义了与“单个DEX的交易池”进行交互的通用方法。
//         -   比如，如何获取池子里的代币信息和流动性，如何进行代币交换（swap），以及（如果这个池子支持的话）如何执行“闪电贷”（flash loan）。
//         -   有了这个统一的接口，机器人程序就可以用同样的方式去操作不同类型的DEX（比如Cetus的池子和Turbos的池子），大大简化了代码。
//             (This is another very critical trait. It defines common methods for interacting with "a single DEX's trading pool".
//              For example, how to get information about the tokens and liquidity in a pool, how to perform a token swap, and (if the pool supports it) how to execute a "flash loan".
//              With this unified interface, the bot program can operate different types of DEXs (like Cetus pools and Turbos pools) in the same way, greatly simplifying the code.)
//
// 3.  **`Defi` 结构体 (The `Defi` Struct)**:
//     -   这个结构体可以看作是整个 `defi` 模块的“总负责人”或“高级控制器”。
//     -   它内部整合了一个 `DexSearcher`（用来找池子）和一个 `Trader`（来自 `trade` 子模块，用来执行和模拟交易）。
//     -   `Defi` 结构体对外提供了一些更高级、更方便的函数，供上层的套利策略模块（比如 `arb.rs` 文件里的 `Arb` 结构体）调用。例如，调用一个函数就能找到所有潜在的套利路径，或者构建出最终要发送到链上的交易数据。
//         (This struct can be seen as the "general manager" or "high-level controller" of the entire `defi` module.
//          It internally integrates a `DexSearcher` (for finding pools) and a `Trader` (from the `trade` submodule, for executing and simulating trades).
//          The `Defi` struct provides higher-level, more convenient functions for the upper-level arbitrage strategy module (like the `Arb` struct in `arb.rs`) to call. For example, one function call can find all potential arbitrage paths or construct the final transaction data to be sent to the chain.)
//
// 4.  **路径发现逻辑 (Pathfinding Logic)**:
//     -   这是套利机器人最核心的算法之一，目标是在一大堆DEX和代币之间，找出能赚钱的交易顺序（路径）。
//     -   `find_sell_paths()`: 这个函数尝试找出从某个你指定的代币开始，经过一系列DEX交换后，最终能换回SUI币（Sui的原生代币）的所有可能路径。
//     -   `find_buy_paths()`: 这个函数则相反，尝试找出从SUI币开始，最终能买到某个你指定的目标代币的所有可能路径。（目前的实现很聪明，它是通过调用 `find_sell_paths` 然后把结果路径“反转”一下得到的）。
//     -   路径搜索算法：代码里用了一种类似于“深度优先搜索”（DFS）的算法，在复杂的DEX网络里“摸索”所有可能的交易组合。同时，为了防止搜索范围无限扩大，还用了一些“剪枝”技巧，比如限制一条路径最多能经过多少个DEX（`MAX_HOP_COUNT`）。
//         (This is one of the core algorithms of an arbitrage bot. Its goal is to find profitable sequences of trades (paths) among a multitude of DEXs and tokens.
//          `find_sell_paths()`: This function tries to find all possible paths starting from a token you specify, going through a series of DEX swaps, and eventually swapping back to SUI coin (Sui's native token).
//          `find_buy_paths()`: This function does the opposite, trying to find all possible paths starting from SUI coin and eventually buying a target token you specify. (The current implementation is clever: it achieves this by calling `find_sell_paths` and then "reversing" the resulting paths).
//          Pathfinding Algorithm: The code uses an algorithm similar to "Depth-First Search" (DFS) to "explore" all possible trade combinations in the complex DEX network. At the same time, to prevent the search space from becoming infinitely large, some "pruning" techniques are used, such as limiting the maximum number of DEXs a path can go through (`MAX_HOP_COUNT`).)
//
// 5.  **路径评估和交易构建 (Path Evaluation and Transaction Building)**:
//     -   `find_best_path_exact_in()`: 当找到了很多条潜在的套利路径后，这个函数会针对一个确定的“本金”数额，通过模拟计算每条路径实际能赚多少钱，来找出最好（通常是利润最高）的那条路径。
//     -   `build_final_tx_data()`: 一旦确定了最佳的套利路径和投入的本金，这个函数就负责把这些信息组装成一个标准的Sui交易数据包（`TransactionData`），这个数据包可以直接发送到Sui区块链上执行。如果套利策略需要先借一笔钱（闪电贷）来作为启动资金，这个函数也会把借款和还款的操作包含进去。
//         (`find_best_path_exact_in()`: After many potential arbitrage paths are found, this function, for a specific "principal" amount, simulates how much money each path can actually make to find the best one (usually the most profitable).
//          `build_final_tx_data()`: Once the best arbitrage path and investment amount are determined, this function is responsible for assembling this information into a standard Sui transaction data package (`TransactionData`), which can be directly sent to the Sui blockchain for execution. If the arbitrage strategy requires borrowing money first (flash loan) as starting capital, this function will also include the borrowing and repayment operations.)
//
// 6.  **常量定义 (Constant Definitions)**:
//     -   文件里还定义了一些重要的“固定参数”（常量），用来控制路径搜索和选择DEX池子时的一些行为。比如：
//         -   `MAX_HOP_COUNT`: 一条套利路径最多能经过几个DEX。
//         -   `MAX_POOL_COUNT`: 从外部服务获取某个代币对的DEX池子列表时，最多看前面多少个（通常按流动性排序）。
//         -   `MIN_LIQUIDITY`: 一个DEX池子至少要有这么多“存货”（流动性）才会被考虑，太少的会被忽略（因为大单交易价格会滑得太厉害）。
//         -   `CETUS_AGGREGATOR`: Cetus这个DEX提供的一个“聚合器”智能合约的地址。有些操作可能需要通过这种聚合器来进行。
//             (The file also defines some important "fixed parameters" (constants) to control certain behaviors during path searching and DEX pool selection. For example:
//              `MAX_HOP_COUNT`: The maximum number of DEXs an arbitrage path can pass through.
//              `MAX_POOL_COUNT`: When fetching a list of DEX pools for a token pair from an external service, how many of the top ones (usually sorted by liquidity) to consider.
//              `MIN_LIQUIDITY`: A DEX pool must have at least this much "inventory" (liquidity) to be considered; pools with too little are ignored (because large trades would suffer too much price slippage).
//              `CETUS_AGGREGATOR`: The address of an "aggregator" smart contract provided by the Cetus DEX. Some operations might need to go through such an aggregator.)
//
// **相关的Sui区块链和DeFi概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **Trait (接口 / Interface)**:
//     在Rust语言中，Trait（特征）是一种定义共享行为的方式，非常类似于其他面向对象编程语言中的“接口”（Interface）或“抽象类”（Abstract Class）。
//     (In Rust, a Trait is a way to define shared behavior, very similar to "interfaces" or "abstract classes" in other object-oriented programming languages.)
//     它允许我们定义一组方法签名，然后不同的具体类型（如不同的DEX实现）可以实现这些方法。
//     (It allows us to define a set of method signatures, which different concrete types (like different DEX implementations) can then implement.)
//     这使得我们可以编写更通用、更灵活的代码，例如，可以用一个统一的 `Dex` trait变量来引用任何实现了该trait的DEX对象。
//     (This enables us to write more generic and flexible code. For example, we can use a unified `Dex` trait variable to refer to any DEX object that implements this trait.)
//
// -   **`Box<dyn Dex>` (特征对象 / Trait Object)**:
//     这是一种特殊的Rust类型，称为“特征对象”（trait object）。它允许我们在运行时持有一个实现了 `Dex` trait 的任何具体类型的实例，而无需在编译时知道其具体类型。
//     (This is a special Rust type called a "trait object". It allows us to hold an instance of any concrete type that implements the `Dex` trait at runtime, without needing to know its specific type at compile time.)
//     `Box` 表示这个实例被分配在堆内存上。`dyn Dex` 表示“任何实现了Dex trait的类型”。
//     (`Box` indicates that this instance is allocated on the heap. `dyn Dex` means "any type that implements the Dex trait".)
//     这对于处理一个包含多种不同DEX实现的列表非常有用，因为它们都可以被当作一个 `Dex` 来对待。
//     (This is very useful for handling a list containing various different DEX implementations, as they can all be treated as a `Dex`.)
//
// -   **Path (交易路径 / Trading Path)**:
//     一条交易路径指的是一系列连续的、通过不同DEX进行的代币交换操作。
//     (A trading path refers to a series of consecutive token swap operations performed through different DEXs.)
//     例如，一个路径可能是：用代币A在DEX1上换取代币B，然后用代币B在DEX2上换取代币C。
//     (For example, a path might be: swap token A for token B on DEX1, then swap token B for token C on DEX2.)
//
// -   **Hop (跳 / Hop)**:
//     路径中的一个单独步骤，即通过一个DEX（或其特定的资金池）进行一次代币交换。
//     (A single step in a path, i.e., one token swap through a DEX (or its specific liquidity pool).)
//     例如，上面路径中的 “代币A -> DEX1 -> 代币B” 就是一跳。
//     (For example, "token A -> DEX1 -> token B" in the path above is one hop.)
//
// -   **Liquidity (流动性 / Liquidity)**:
//     在DEX的上下文中，流动性通常指一个交易池中可供交易的代币数量。
//     (In the context of a DEX, liquidity usually refers to the amount of tokens available for trading in a liquidity pool.)
//     流动性是衡量一个DEX池健康状况和交易效率的重要指标。流动性越高，大额交易的价格滑点通常就越小。
//     (Liquidity is an important indicator of a DEX pool's health and trading efficiency. Higher liquidity generally means less price slippage for large trades.)
//
// -   **DFS (Depth-First Search, 深度优先搜索)**:
//     一种常用的图（或树）遍历算法。它从一个起始节点开始，尽可能深地探索图中的一个分支，直到到达末端或无法再深入，然后回溯到上一个节点，继续探索其他未访问的分支。
//     (A common graph (or tree) traversal algorithm. It starts from a starting node and explores as far as possible along each branch before backtracking.)
//     在这个文件中，DFS被用于从一个代币开始，通过连接的DEX网络，探索所有可能达到目标代币（如SUI）的交易路径。
//     (In this file, DFS is used to explore all possible trading paths from a starting token to a target token (like SUI) through the connected DEX network.)
//
// -   **Pegged Coins (锚定币 / Pegged Coins)**:
//     指其价值与某种法定货币（如美元USD）或其他相对稳定的资产（如黄金）保持固定比例（通常是1:1）的加密货币。例如USDC、USDT。
//     (Cryptocurrencies whose value is pegged at a fixed ratio (usually 1:1) to a fiat currency (like USD) or other relatively stable assets (like gold). Examples include USDC, USDT.)
//
// -   **Native Coin (原生代币 / Native Coin)**:
//     指特定区块链平台的内置、基础代币，例如Sui网络中的SUI代币。SUI用于支付gas费、参与质押和治理等。
//     (The built-in, fundamental token of a specific blockchain platform, e.g., the SUI token in the Sui network. SUI is used for paying gas fees, participating in staking and governance, etc.)

// --- 声明子模块 ---
// (Declare submodules)
// Rust中使用 `mod` 关键字来声明一个模块。
// (In Rust, the `mod` keyword is used to declare a module.)
// 如果模块内容在同级目录下的同名 `.rs` 文件中，则写 `mod module_name;`
// (If the module content is in a `.rs` file of the same name in the same directory, write `mod module_name;`)
// 如果模块内容在一个同名子目录下的 `mod.rs` 文件中，也写 `mod module_name;`
// (If the module content is in a `mod.rs` file within a subdirectory of the same name, also write `mod module_name;`)
// `pub mod` 表示这个子模块是公开的，其内部的公共项可以被 `defi` 模块之外的代码访问。
// (`pub mod` indicates that this submodule is public, and its public items can be accessed by code outside the `defi` module.)
mod aftermath;        // 对应 `aftermath.rs`，可能包含与Aftermath Finance DEX交互的逻辑。
                      // (Corresponds to `aftermath.rs`, likely containing logic for interacting with Aftermath Finance DEX.)
mod blue_move;        // 对应 `blue_move.rs`，BlueMove最初可能是一个NFT市场，但也可能聚合或提供了DEX功能。
                      // (Corresponds to `blue_move.rs`, BlueMove might have started as an NFT marketplace but could also aggregate or provide DEX functionality.)
mod cetus;            // 对应 `cetus.rs`，Cetus是一个在Sui和Aptos上知名的DEX协议。
                      // (Corresponds to `cetus.rs`, Cetus is a well-known DEX protocol on Sui and Aptos.)
mod deepbook_v2;      // 对应 `deepbook_v2.rs`，DeepBook是Sui官方推出的一个中央限价订单簿（CLOB）协议，V2是其升级版。它也可能被视为一种DEX。
                      // (Corresponds to `deepbook_v2.rs`, DeepBook is a Central Limit Order Book (CLOB) protocol officially launched by Sui, V2 is its upgraded version. It might also be considered a type of DEX.)
mod flowx_clmm;       // 对应 `flowx_clmm.rs`，FlowX可能是一个DEX，CLMM指集中流动性做市商模型。
                      // (Corresponds to `flowx_clmm.rs`, FlowX might be a DEX; CLMM refers to Concentrated Liquidity Market Maker model.)
mod indexer_searcher; // 对应 `indexer_searcher.rs`，这个模块可能负责从外部的DEX索引服务（如SuiVision的API）查询和发现DEX池信息。
                      // (Corresponds to `indexer_searcher.rs`, this module is likely responsible for querying and discovering DEX pool information from external DEX indexing services (like SuiVision's API).)
mod kriya_amm;        // 对应 `kriya_amm.rs`，KriyaDEX是Sui上的一个DeFi协议，AMM指自动做市商模型。
                      // (Corresponds to `kriya_amm.rs`, KriyaDEX is a DeFi protocol on Sui; AMM refers to Automated Market Maker model.)
mod kriya_clmm;       // 对应 `kriya_clmm.rs`，KriyaDEX也可能采用了CLMM模型。
                      // (Corresponds to `kriya_clmm.rs`, KriyaDEX might also use the CLMM model.)
mod navi;             // 对应 `navi.rs`，Navi是一个Sui上的借贷协议。虽然主要是借贷，但有时借贷利率或清算机制也可能与套利相关，或者它也提供代币获取途径。
                      // (Corresponds to `navi.rs`, Navi is a lending protocol on Sui. While primarily for lending, sometimes interest rates or liquidation mechanisms might be relevant for arbitrage, or it might provide ways to acquire tokens.)
mod shio;             // 对应 `shio.rs`，Shio可能是一个较新的或特定类型的DEX或DeFi服务。
                      // (Corresponds to `shio.rs`, Shio might be a newer or specific type of DEX or DeFi service.)
mod trade;            // 对应 `trade.rs`，这个模块非常核心，可能包含了通用的交易执行逻辑、交易模拟功能、交易路径的组合与表示、闪电贷处理等。
                      // (Corresponds to `trade.rs`, this module is very core and likely contains general transaction execution logic, transaction simulation functionality, combination and representation of trading paths, flash loan handling, etc.)
mod turbos;           // 对应 `turbos.rs`，Turbos Finance是Sui上的另一个DEX协议。
                      // (Corresponds to `turbos.rs`, Turbos Finance is another DEX protocol on Sui.)
mod utils;            // 对应 `utils.rs` (在 `defi` 目录下)，包含 `defi` 模块内部使用的一些辅助工具函数。
                      // (Corresponds to `utils.rs` (under the `defi` directory), containing some helper utility functions used within the `defi` module.)

// --- 引入标准库及第三方库的类型和功能 ---
// (Import types and functions from standard and third-party libraries)
use std::{
    collections::{HashMap, HashSet}, // `HashMap` 用于创建键值对映射（例如，用代币类型字符串作为键，存储一个包含多个DEX对象的列表）。
                                     // (`HashMap` is used for creating key-value mappings (e.g., using coin type strings as keys to store a list of multiple DEX objects).)
                                     // `HashSet` 用于存储一组唯一的元素（例如，存储已经访问过的DEX池ID，以避免重复处理）。
                                     // (`HashSet` is used for storing a unique set of elements (e.g., storing visited DEX pool IDs to avoid reprocessing).)
    fmt,                             // `fmt` 模块用于格式化输出，例如为自定义的结构体实现 `Debug` 或 `Display` trait，使其能被方便地打印。
                                     // (The `fmt` module is used for formatted output, e.g., implementing `Debug` or `Display` traits for custom structs to make them printable.)
    hash::Hash,                      // `Hash` trait 用于使一个类型可以被哈希计算，这是将其作为 `HashMap` 键或存入 `HashSet` 的前提。
                                     // (The `Hash` trait is used to enable a type to be hashed, which is a prerequisite for using it as a key in `HashMap` or storing it in `HashSet`.)
    sync::Arc,                       // `Arc` (Atomic Reference Counting，原子引用计数) 是一种智能指针，它允许多个所有者安全地共享同一个数据。
                                     // (`Arc` (Atomic Reference Counting) is a smart pointer that allows multiple owners to safely share the same data.)
                                     // 当需要在多个线程或异步任务之间共享对象（如 `DexSearcher`, `Trader`）时，`Arc` 非常有用。
                                     // (`Arc` is very useful when objects (like `DexSearcher`, `Trader`) need to be shared across multiple threads or asynchronous tasks.)
};

use ::utils::coin; // 从外部的 `utils` crate (注意 `::` 前缀表示从crate根开始查找，区别于当前模块的 `utils` 子模块) 引入 `coin` 模块。
                   // (Import the `coin` module from the external `utils` crate (note the `::` prefix indicates lookup from the crate root, distinct from the current module's `utils` submodule).)
                   // 这个外部的 `utils::coin` 可能包含更通用的代币相关工具函数，比如检查一个代币是否是原生SUI币。
                   // (This external `utils::coin` might contain more general coin-related utility functions, such as checking if a coin is the native SUI coin.)
use dex_indexer::types::Protocol; // 从 `dex_indexer` crate (可能是用于与DEX索引服务交互的库) 的 `types` 模块引入 `Protocol` 枚举。
                                // (`Protocol` enum imported from the `types` module of the `dex_indexer` crate (possibly a library for interacting with DEX indexing services).)
                                // `Protocol` 枚举可能用来标识一个DEX属于哪个已知的协议（如Cetus, Turbos等）。
                                // (The `Protocol` enum might be used to identify which known protocol a DEX belongs to (e.g., Cetus, Turbos).)
use eyre::{bail, ensure, Result}; // 从 `eyre` 错误处理库引入：
                                  // (Import from the `eyre` error handling library:)
                                  // `bail!` 宏：用于快速创建一个错误并立即从当前函数返回该错误 (类似于 `return Err(eyre!("..."));`)。
                                  // (`bail!` macro: Used to quickly create an error and immediately return it from the current function (similar to `return Err(eyre!("..."));`).)
                                  // `ensure!` 宏：用于检查一个条件是否为真，如果不为真，则创建一个错误并返回。
                                  // (`ensure!` macro: Used to check if a condition is true; if not, creates an error and returns it.)
                                  // `Result` 类型：通常是 `std::result::Result<T, eyre::Report>` 的别名，用于表示可能失败的操作。
                                  // (`Result` type: Usually an alias for `std::result::Result<T, eyre::Report>`, used to represent operations that might fail.)
pub use indexer_searcher::IndexerDexSearcher; // `pub use` 将 `indexer_searcher` 子模块中的 `IndexerDexSearcher` 类型重新导出到当前 `defi` 模块的公共API中。
                                            // (`pub use` re-exports the `IndexerDexSearcher` type from the `indexer_searcher` submodule into the public API of the current `defi` module.)
                                            // 这使得 `defi` 模块的使用者可以直接通过 `crate::defi::IndexerDexSearcher` 来访问它，而不需要写更长的路径。
                                            // (This allows users of the `defi` module to access it directly via `crate::defi::IndexerDexSearcher` without needing to write a longer path.)
use object_pool::ObjectPool; // 引入 `ObjectPool` 类型，用于创建和管理可复用对象池，这里主要用于管理交易模拟器 (`Simulator`) 实例。
                             // (Import `ObjectPool` type, used for creating and managing reusable object pools, here primarily for managing transaction simulator (`Simulator`) instances.)
use simulator::{SimulateCtx, Simulator}; // 从 `simulator` crate 引入：
                                        // (Import from the `simulator` crate:)
                                        // `SimulateCtx` (模拟上下文)：可能包含了模拟交易时需要的环境信息，如当前纪元、gas价格等。
                                        // (`SimulateCtx` (Simulation Context): Might contain environmental information needed for transaction simulation, such as current epoch, gas price, etc.)
                                        // `Simulator` trait：定义了交易模拟器应该具有的通用接口。
                                        // (`Simulator` trait: Defines the common interface that a transaction simulator should have.)
use sui_sdk::SUI_COIN_TYPE; // 从 `sui_sdk` 引入SUI原生代币的类型字符串常量 ("0x2::sui::SUI")。
                           // (Import the SUI native coin type string constant ("0x2::sui::SUI") from `sui_sdk`.)
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // 从 `sui_types` 核心库引入Sui区块链的基本类型：
                                                 // (Import basic Sui blockchain types from the `sui_types` core library:)
                                                 // `ObjectID`: 对象的唯一ID。(Unique ID of an object.)
                                                 // `ObjectRef`: 对象的引用，包含ID、版本和摘要，用于唯一指向一个特定版本的对象。
                                                 //              (Reference to an object, including ID, version, and digest, used to uniquely point to a specific version of an object.)
                                                 // `SuiAddress`: Sui网络上的账户地址。(Account address on the Sui network.)
    transaction::{Argument, TransactionData},      // 同样从 `sui_types` 引入与交易构建相关的类型：
                                                 // (Also import transaction building related types from `sui_types`:)
                                                 // `Argument`: 在构建可编程交易块（PTB）时，代表一个交易参数，可以是一个对象、一个纯值或另一个命令的结果。
                                                 //             (When building a Programmable Transaction Block (PTB), represents a transaction argument, which can be an object, a pure value, or the result of another command.)
                                                 // `TransactionData`: 代表一笔完整的、可以被签名和提交到链上的Sui交易的数据结构。
                                                 //                    (Represents the data structure of a complete Sui transaction that can be signed and submitted to the chain.)
};
use tokio::task::JoinSet; // 从 `tokio` 异步运行时库引入 `JoinSet`，它用于管理一组并发执行的异步任务，并可以方便地收集它们的结果。
                         // (Import `JoinSet` from the `tokio` asynchronous runtime library; it's used to manage a set of concurrently executing asynchronous tasks and conveniently collect their results.)
use tracing::Instrument; // 从 `tracing` 库引入 `Instrument` trait。这个trait提供了 `.instrument()` 方法，
                         // 可以为一个异步代码块或Future附加当前的追踪span，使得在异步执行上下文中也能正确地记录日志和追踪信息。
                         // (Import the `Instrument` trait from the `tracing` library. This trait provides the `.instrument()` method,
                         //  which can attach the current tracing span to an asynchronous code block or Future, enabling correct logging and tracing information in asynchronous execution contexts.)
use trade::{FlashResult, TradeResult}; // 从当前 `defi` 模块的子模块 `trade` 中引入 `FlashResult` (闪电贷结果) 和 `TradeResult` (交易模拟结果) 类型。
                                      // (Import `FlashResult` (flash loan result) and `TradeResult` (trade simulation result) types from the `trade` submodule of the current `defi` module.)
pub use trade::{Path, TradeCtx, TradeType, Trader}; // 同样从 `trade` 子模块中重新导出一些核心类型，使其在 `defi` 模块的公共API中可用：
                                                    // (Also re-export some core types from the `trade` submodule to make them available in the public API of the `defi` module:)
                                                    // `Path`: 代表一条交易路径（一系列DEX交换）。(Represents a trading path (a series of DEX swaps).)
                                                    // `TradeCtx`: 交易上下文，用于在构建复杂交易（如PTB）时跟踪状态和参数。
                                                    //             (Trade context, used for tracking state and arguments when building complex transactions (like PTBs).)
                                                    // `TradeType`: 交易类型枚举（例如普通交换、闪电贷）。(Trade type enum (e.g., normal swap, flash loan).)
                                                    // `Trader`: 负责执行和模拟交易的核心结构体。(Core struct responsible for executing and simulating trades.)

use crate::{config::pegged_coin_types, types::Source}; // 从当前crate的根作用域引入：
                                                      // (Import from the current crate's root scope:)
                                                      // `config::pegged_coin_types` 函数：用于获取锚定币/主流币种列表。
                                                      // (`config::pegged_coin_types` function: Used to get the list of pegged/major coin types.)
                                                      // `types::Source` 枚举：用于表示套利机会的来源。
                                                      // (`types::Source` enum: Used to represent the source of an arbitrage opportunity.)

// --- DeFi模块常量定义 ---
// (DeFi module constant definitions)

// `MAX_HOP_COUNT`: 路径中允许的最大“跳数”（即最多经过多少个DEX进行交换）。
// (Maximum number of "hops" allowed in a path (i.e., the maximum number of DEXs to swap through).)
// 这个限制有助于控制路径搜索算法的复杂度。如果允许无限跳，搜索空间会变得非常大，难以处理。
// (This limit helps control the complexity of the pathfinding algorithm. If infinite hops were allowed, the search space would become very large and difficult to handle.)
// 设置为2意味着路径可以是 A -> DEX1 -> B -> DEX2 -> C (两跳)。
// (Setting it to 2 means a path can be A -> DEX1 -> B -> DEX2 -> C (two hops).)
const MAX_HOP_COUNT: usize = 2;

// `MAX_POOL_COUNT`: 当从DEX索引器为每种代币对获取可用交易池列表时，我们只取流动性最高的前N个池。
// (When fetching a list of available trading pools for each token pair from the DEX indexer, we only take the top N pools with the highest liquidity.)
// 这个常量定义了这个N值。限制处理的池数量可以减少计算量，并优先关注那些最有可能提供良好交易条件的池。
// (This constant defines this N value. Limiting the number of pools processed can reduce computation and prioritize pools most likely to offer good trading conditions.)
const MAX_POOL_COUNT: usize = 10;

// `MIN_LIQUIDITY`: 一个DEX池必须拥有的最小流动性阈值。
// (Minimum liquidity threshold a DEX pool must have.)
// 如果一个池的流动性低于这个值，它在路径搜索中可能会被忽略。
// (If a pool's liquidity is below this value, it might be ignored in path searches.)
// 流动性太低的池子无法支持较大金额的交易，否则会导致巨大的价格滑点，不利于套利。
// (Pools with too low liquidity cannot support large trades without causing significant price slippage, which is detrimental to arbitrage.)
// 注意：这个值 `1000` 可能是一个非常小的值，特别是对于那些有很高小数位数（如9位）的代币（如SUI的MIST单位）。
// (Note: This value `1000` might be very small, especially for tokens with high decimal places (e.g., 9 decimals) like SUI's MIST unit.)
// 它的有效性取决于代币的精度和通常的流动性水平。对于1 SUI = 10^9 MIST来说，1000 MIST几乎为零。
// (Its effectiveness depends on the token's precision and typical liquidity levels. For 1 SUI = 10^9 MIST, 1000 MIST is almost negligible.)
// 可能需要根据实际情况（例如，统一按美元价值估算流动性，或针对不同代币使用不同阈值）来调整这个常量。
// (This constant might need adjustment based on actual conditions (e.g., estimating liquidity uniformly in USD value, or using different thresholds for different tokens).)
const MIN_LIQUIDITY: u128 = 1000;

// `CETUS_AGGREGATOR`: Cetus DEX 提供的聚合器（Aggregator）智能合约的Sui Package ID (包ID)。
// (Sui Package ID of the Aggregator smart contract provided by Cetus DEX.)
// 聚合器合约通常允许用户通过一个统一的接口与该DEX的多个不同池子或者甚至不同类型的池子（如稳定币池、非稳定币池）进行交互。
// (Aggregator contracts usually allow users to interact with multiple different pools of the DEX, or even different types of pools (like stablecoin pools, non-stablecoin pools), through a unified interface.)
// 有时，直接与聚合器交互可能比直接与单个池子交互能获得更优的交易价格或更方便的操作。
// (Sometimes, interacting directly with an aggregator might offer better trading prices or more convenient operations than interacting directly with individual pools.)
// 这个ID是硬编码的，如果Cetus升级其聚合器合约并更改了包ID，这里也需要更新。
// (This ID is hardcoded; if Cetus upgrades its aggregator contract and changes the package ID, this will need to be updated here as well.)
pub const CETUS_AGGREGATOR: &str = "0x11451575c775a3e633437b827ecbc1eb51a5964b0302210b28f5b89880be21a2";


/// `DexSearcher` Trait (DEX搜索器接口 / DEX Searcher Interface)
///
/// 这个trait定义了查找可用DEX实例的通用行为或契约。
/// (This trait defines the common behavior or contract for finding available DEX instances.)
/// 任何实现了这个trait的类型，都可以被用作一个“DEX发现服务”。
/// (Any type that implements this trait can be used as a "DEX discovery service".)
///
/// `Send + Sync` 这两个标记trait（marker traits）是Rust并发安全的重要组成部分：
/// (`Send + Sync` are two marker traits crucial for Rust's concurrency safety:)
/// - `Send`: 表示实现了该trait的类型可以安全地从一个线程发送到另一个线程（所有权转移）。
///           (Indicates that a type implementing this trait can be safely sent from one thread to another (ownership transfer).)
/// - `Sync`: 表示实现了该trait的类型可以安全地在多个线程之间共享（通过引用，例如 `&T` 或 `Arc<T>`）。
///           (Indicates that a type implementing this trait can be safely shared among multiple threads (via references, e.g., `&T` or `Arc<T>`).)
/// 将它们作为 `DexSearcher` 的“超trait”（supertrait）约束，意味着任何 `DexSearcher` 的实现都必须是线程安全的。
/// (Making them "supertrait" constraints for `DexSearcher` means any implementation of `DexSearcher` must be thread-safe.)
#[async_trait::async_trait] // 这个宏使得trait中的方法可以使用 `async` 关键字，成为异步方法。
                            // (This macro enables the use of the `async` keyword for methods in a trait, making them asynchronous methods.)
pub trait DexSearcher: Send + Sync {
    /// `find_dexes` 方法 (查找DEX / Find DEXs method)
    ///
    /// 根据输入的代币类型 (`coin_in_type`) 和可选的输出代币类型 (`coin_out_type`)，
    /// 异步地查找所有相关的、可用的DEX实例。
    /// (Asynchronously finds all relevant, available DEX instances based on the input coin type (`coin_in_type`) and an optional output coin type (`coin_out_type`).)
    ///
    /// **参数 (Parameters)**:
    /// - `self`: 表示调用该方法的 `DexSearcher` 实例自身。 (Represents the `DexSearcher` instance itself calling this method.)
    /// - `coin_in_type`: 一个字符串切片 (`&str`)，代表你想要卖出或用作输入的代币的完整类型字符串 (例如 "0x2::sui::SUI")。
    ///                   (A string slice (`&str`) representing the full type string of the coin you want to sell or use as input (e.g., "0x2::sui::SUI").)
    /// - `coin_out_type`: 一个 `Option<String>`，代表你想要买入或得到的输出代币的类型字符串。
    ///                    (An `Option<String>` representing the type string of the output coin you want to buy or receive.)
    ///   - 如果是 `Some(type_string)`，则只查找那些能够将 `coin_in_type` 转换成这个特定 `coin_out_type` 的DEX池。
    ///     (If `Some(type_string)`, only find DEX pools that can convert `coin_in_type` to this specific `coin_out_type`.)
    ///   - 如果是 `None`，则查找所有那些以 `coin_in_type` 作为其交易对中一个币种的DEX池（即可以用来卖出 `coin_in_type` 换取任何其他币种的池）。
    ///     (If `None`, find all DEX pools that have `coin_in_type` as one of the coins in their trading pair (i.e., pools that can be used to sell `coin_in_type` for any other coin).)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Vec<Box<dyn Dex>>>`: 这是一个异步操作的结果。
    ///                                (This is the result of an asynchronous operation.)
    ///   - 如果成功，返回 `Ok(Vec<Box<dyn Dex>>)`，其中 `Vec` 是一个动态数组（向量），
    ///     包含了一系列 `Box<dyn Dex>` 类型的元素。
    ///     (If successful, returns `Ok(Vec<Box<dyn Dex>>)`, where `Vec` is a dynamic array (vector)
    ///      containing a series of `Box<dyn Dex>` type elements.)
    ///     `Box<dyn Dex>` 是一个“特征对象”，它允许我们存储任何实现了 `Dex` trait 的具体DEX类型的实例，
    ///     即使这些具体类型在编译时是未知的或者大小不同。这使得我们可以用统一的方式处理来自不同协议的DEX。
    ///     (`Box<dyn Dex>` is a "trait object" that allows us to store instances of any concrete DEX type implementing the `Dex` trait,
    ///      even if these concrete types are unknown at compile time or have different sizes. This allows us to handle DEXs from different protocols in a unified way.)
    ///   - 如果在查找过程中发生错误（例如网络请求失败、索引服务无响应等），则返回 `Err(...)`，其中包含具体的错误信息。
    ///     (If an error occurs during the search (e.g., network request failure, unresponsive indexing service, etc.), returns `Err(...)` containing specific error information.)
    async fn find_dexes(&self, coin_in_type: &str, coin_out_type: Option<String>) -> Result<Vec<Box<dyn Dex>>>;

    /// `find_test_path` 方法 (查找测试路径，主要用于测试目的 / Find Test Path method, mainly for testing purposes)
    ///
    /// 根据一个给定的 `ObjectID` 序列（这个序列代表了一个交易路径中连续经过的DEX池的ID），
    /// 构建并返回一个 `Path` 对象。
    /// (Constructs and returns a `Path` object based on a given sequence of `ObjectID`s (this sequence represents the IDs of DEX pools consecutively traversed in a trading path).)
    /// 这个方法主要用于测试场景，例如，当你想手动指定一条路径并测试其模拟结果或执行情况时。
    /// (This method is primarily used in testing scenarios, for example, when you want to manually specify a path and test its simulation results or execution.)
    /// 它需要根据池ID查询到对应的DEX实例，并按顺序组装成 `Path`。
    /// (It needs to query the corresponding DEX instances based on pool IDs and assemble them into a `Path` in order.)
    ///
    /// **参数 (Parameters)**:
    /// - `path`: 一个 `ObjectID` 的切片 (`&[ObjectID]`)，表示路径中各个DEX池的ID。
    ///           (A slice of `ObjectID`s (`&[ObjectID]`) representing the IDs of various DEX pools in the path.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Path>`: 如果成功构建路径，则返回 `Ok(Path)`；否则返回错误。
    ///                   (Returns `Ok(Path)` if the path is successfully constructed; otherwise, returns an error.)
    async fn find_test_path(&self, path: &[ObjectID]) -> Result<Path>;
}

/// `Dex` Trait (去中心化交易所接口 / Decentralized Exchange Interface)
///
/// 这个trait定义了与单个DEX的资金池（pool）进行交互的通用行为或契约。
/// (This trait defines the common behavior or contract for interacting with a single DEX's liquidity pool.)
/// 任何代表特定DEX协议（如Cetus, Turbos等）的池的结构体，如果想要被套利机器人统一处理，
/// 都需要实现这个 `Dex` trait。
/// (Any struct representing a pool of a specific DEX protocol (like Cetus, Turbos, etc.), if it's intended to be handled uniformly by the arbitrage bot,
/// needs to implement this `Dex` trait.)
///
/// **超trait约束 (Supertrait Constraints)**:
/// - `Send + Sync`: 如上所述，确保DEX实例可以安全地在多线程环境中使用。
///                  (As mentioned above, ensures DEX instances can be safely used in a multi-threaded environment.)
/// - `CloneBoxedDex`: 这是一个自定义的trait (定义见下文)，它提供了一个克隆 `Box<dyn Dex>` 特征对象的方法。
///                    由于Rust原生的 `Clone` trait 不能直接用于特征对象 `dyn Trait`，需要这种变通方法来实现克隆。
///                    (This is a custom trait (defined below) that provides a method for cloning `Box<dyn Dex>` trait objects.
///                     Since Rust's native `Clone` trait cannot be directly used for trait objects `dyn Trait`, this workaround is needed for cloning.)
#[async_trait::async_trait]
pub trait Dex: Send + Sync + CloneBoxedDex {
    /// `support_flashloan` 方法 (是否支持闪电贷 / Does it support flash loans? method)
    ///
    /// 返回一个布尔值，指示当前的DEX实例（特指其代表的那个池子）是否支持“闪电贷”（Flash Loan）功能。
    /// (Returns a boolean value indicating whether the current DEX instance (specifically, the pool it represents) supports the "Flash Loan" feature.)
    /// 闪电贷是一种特殊的DeFi贷款，允许用户在一次原子交易（一个区块内完成所有操作）中借入大量资金，
    /// 条件是在同一笔交易结束前必须连本带息归还所借资金。如果未能归还，整个交易（包括借款操作）都会回滚失败。
    /// (A flash loan is a special type of DeFi loan that allows users to borrow large amounts of funds within a single atomic transaction (all operations completed within one block),
    ///  on the condition that the borrowed funds plus interest must be returned before the end of the same transaction. If not returned, the entire transaction (including the borrowing operation) will fail and roll back.)
    /// 闪电贷是许多套利策略（尤其是那些需要大量启动资金的策略）的关键组成部分。
    /// (Flash loans are a key component of many arbitrage strategies, especially those requiring significant starting capital.)
    ///
    /// **默认实现 (Default Implementation)**: `false`。
    /// 这意味着如果一个具体的DEX实现没有重写（override）这个方法，它将被认为不支持闪电贷。
    /// (This means if a concrete DEX implementation does not override this method, it will be considered as not supporting flash loans.)
    /// 只有那些确实提供了闪电贷功能的DEX才需要重写此方法并返回 `true`。
    /// (Only DEXs that actually provide flash loan functionality need to override this method and return `true`.)
    fn support_flashloan(&self) -> bool {
        false
    }

    /// `extend_flashloan_tx` 方法 (将发起闪电贷的操作添加到交易上下文中 / Add flash loan initiation operation to transaction context method)
    ///
    /// 这个异步方法负责将“发起一笔闪电贷”的操作指令添加到当前正在构建的Sui可编程交易块（PTB）中。
    /// (This asynchronous method is responsible for adding the "initiate a flash loan" operation instruction to the currently being built Sui Programmable Transaction Block (PTB).)
    /// 它会被 `Trader` 在构建包含闪电贷的套利交易时调用。
    /// (It will be called by `Trader` when constructing an arbitrage transaction that includes a flash loan.)
    ///
    /// **参数 (Parameters)**:
    /// - `_ctx`: 一个对 `TradeCtx`（交易上下文）的可变引用。`TradeCtx` 用于跟踪PTB中已有的指令和参数，
    ///           并管理新生成的参数（如借到的代币）。`_` 前缀表示这个参数在默认实现中未使用。
    ///           (A mutable reference to `TradeCtx` (transaction context). `TradeCtx` is used to track existing instructions and arguments in the PTB,
    ///            and to manage newly generated arguments (like borrowed coins). The `_` prefix indicates this parameter is unused in the default implementation.)
    /// - `_amount`: 一个 `u64` 值，表示希望通过闪电贷借入的代币数量（通常是输入代币 `coin_in_type()` 的数量）。
    ///              (A `u64` value representing the amount of tokens desired to be borrowed via flash loan (usually the amount of the input coin `coin_in_type()`).)
    ///
    /// **返回 (Returns)**:
    /// - `Result<FlashResult>`:
    ///   - `FlashResult` 可能是一个结构体或元组，包含了闪电贷操作的结果，主要是：
    ///     (`FlashResult` might be a struct or tuple containing the result of the flash loan operation, primarily:)
    ///     - `coin_out`: 一个 `Argument` 类型的对象，代表在PTB中实际借到的代币。这个 `Argument` 可以在后续的交易指令中使用。
    ///                   (An `Argument` type object representing the coins actually borrowed in the PTB. This `Argument` can be used in subsequent transaction instructions.)
    ///     - `receipt`: 可能是一个 `Argument` 类型的“回执”对象，代表闪电贷的凭证，在偿还时可能需要用到。
    ///                  (Possibly an `Argument` type "receipt" object, representing the flash loan's proof, which might be needed for repayment.)
    ///   - `Result` 表示操作可能失败（例如DEX不支持、金额过大等）。
    ///     (`Result` indicates the operation might fail (e.g., DEX doesn't support it, amount too large, etc.).)
    /// **默认实现 (Default Implementation)**: 返回一个错误，明确表示“此DEX不支持闪电贷”。
    /// (Returns an error explicitly stating "This DEX does not support flash loans".)
    /// 具体的DEX实现如果支持闪电贷，必须重写此方法，提供调用其闪电贷合约接口的逻辑。
    /// (Concrete DEX implementations that support flash loans must override this method to provide logic for calling their flash loan contract interface.)
    async fn extend_flashloan_tx(&self, _ctx: &mut TradeCtx, _amount: u64) -> Result<FlashResult> {
        bail!("此DEX不支持闪电贷 (This DEX does not support flash loans)") // `bail!` 来自 `eyre` 库，用于方便地返回一个错误。 (`bail!` from `eyre` library for conveniently returning an error.)
    }

    /// `extend_repay_tx` 方法 (将偿还闪电贷的操作添加到交易上下文中 / Add flash loan repayment operation to transaction context method)
    ///
    /// 这个异步方法负责将“偿还一笔闪电贷”的操作指令添加到PTB中。
    /// (This asynchronous method is responsible for adding the "repay a flash loan" operation instruction to the PTB.)
    /// 它会在套利路径中的所有代币交换操作都完成后被调用，以确保借款被归还。
    /// (It is called after all token swap operations in the arbitrage path are completed to ensure the loan is repaid.)
    ///
    /// **参数 (Parameters)**:
    /// - `_ctx`: 可变的交易上下文。(Mutable transaction context.)
    /// - `_coin`: 一个 `Argument` 对象，代表用于偿还本金和可能利息的代币。这个代币通常是执行完套利路径后得到的最终输出代币。
    ///            (An `Argument` object representing the coins used to repay the principal and possible interest. This coin is usually the final output coin obtained after executing the arbitrage path.)
    /// - `_flash_res`: 从 `extend_flashloan_tx` 方法返回的 `FlashResult` 对象，其中可能包含了偿还时需要的回执信息。
    ///                 (The `FlashResult` object returned from the `extend_flashloan_tx` method, which might contain receipt information needed for repayment.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Argument>`:
    ///   - 操作成功时，可能返回一个 `Argument` 代表找零的代币（如果偿还金额大于实际应还金额），或者一个表示操作完成的空结果。
    ///     (If successful, might return an `Argument` representing change coins (if repayment amount is greater than actual amount due), or an empty result indicating completion.)
    ///   - 操作失败时返回错误。(Returns an error if the operation fails.)
    /// **默认实现 (Default Implementation)**: 返回一个错误，明确表示“此DEX不支持闪电贷的偿还操作”。
    /// (Returns an error explicitly stating "This DEX does not support flash loan repayment operations".)
    /// 支持闪电贷的DEX实现必须重写此方法。
    /// (DEX implementations supporting flash loans must override this method.)
    async fn extend_repay_tx(&self, _ctx: &mut TradeCtx, _coin: Argument, _flash_res: FlashResult) -> Result<Argument> {
        bail!("此DEX不支持闪电贷的偿还操作 (This DEX does not support flash loan repayment operations)")
    }

    /// `extend_trade_tx` 方法 (将常规代币交换操作添加到交易上下文中 / Add regular token swap operation to transaction context method)
    ///
    /// 这个异步方法负责将一个常规的“代币A换代币B”的交换操作指令添加到PTB中。
    /// (This asynchronous method is responsible for adding a regular "swap token A for token B" operation instruction to the PTB.)
    /// 这是DEX最基本的功能。
    /// (This is the most basic functionality of a DEX.)
    ///
    /// **参数 (Parameters)**:
    /// - `ctx`: 一个对 `TradeCtx` 的可变引用，用于构建交易。
    ///          (A mutable reference to `TradeCtx`, used for building the transaction.)
    /// - `sender`: 交易的发送者（Sui地址）。虽然在PTB中发送者是固定的，但某些DEX的合约函数可能仍需要这个参数。
    ///             (The sender of the transaction (Sui address). Although the sender is fixed in a PTB, some DEX contract functions might still require this parameter.)
    /// - `coin_in`: 一个 `Argument` 对象，代表用于支付的输入代币。它可以是初始的gas币，也可以是上一步交换操作得到的输出代币。
    ///              (An `Argument` object representing the input coin used for payment. It can be the initial gas coin or the output coin from a previous swap operation.)
    /// - `amount_in`: 一个可选的 `u64` 值，表示输入代币的数量。
    ///                (An optional `u64` value representing the amount of the input coin.)
    ///   - 如果是 `Some(value)`，则表示只使用 `coin_in` 中的一部分（`value` 数量）进行交换。
    ///     (If `Some(value)`, it means only a part of `coin_in` (amount `value`) is used for the swap.)
    ///   - 如果是 `None`，则通常表示使用 `coin_in` 参数所代表的全部代币进行交换。
    ///     (If `None`, it usually means all coins represented by the `coin_in` argument are used for the swap.)
    ///   具体的DEX实现需要根据其合约接口来决定如何处理这个参数。
    ///   (The specific DEX implementation needs to decide how to handle this parameter based on its contract interface.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Argument>`:
    ///   - 如果交换操作指令成功添加到PTB中，返回 `Ok(Argument)`，其中 `Argument` 代表了从这次交换中获得的输出代币。
    ///     (If the swap operation instruction is successfully added to the PTB, returns `Ok(Argument)`, where `Argument` represents the output coins obtained from this swap.)
    ///     这个输出代币的 `Argument` 可以在PTB的下一步操作中作为输入使用。
    ///     (This output coin `Argument` can be used as input in the next operation of the PTB.)
    ///   - 如果发生错误（例如池子不存在、代币不匹配等），返回 `Err(...)`。
    ///     (If an error occurs (e.g., pool doesn't exist, coins don't match, etc.), returns `Err(...):`)
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        sender: SuiAddress,
        coin_in: Argument,
        amount_in: Option<u64>,
    ) -> Result<Argument>;

    // --- Getter 方法 (获取DEX基本信息) ---
    // (Getter methods (Get basic DEX information))
    // 这些方法用于获取当前DEX实例的一些基本属性。
    // (These methods are used to get some basic attributes of the current DEX instance.)

    /// 返回当前交易方向下，该DEX期望的输入代币的类型字符串。
    /// (Returns the type string of the input coin expected by this DEX for the current trading direction.)
    fn coin_in_type(&self) -> String;
    /// 返回当前交易方向下，该DEX将输出的代币的类型字符串。
    /// (Returns the type string of the output coin this DEX will produce for the current trading direction.)
    fn coin_out_type(&self) -> String;
    /// 返回该DEX所属的协议类型 (例如 `Protocol::Cetus`, `Protocol::Turbos`)。
    /// (Returns the protocol type this DEX belongs to (e.g., `Protocol::Cetus`, `Protocol::Turbos`).)
    fn protocol(&self) -> Protocol;
    /// 返回该DEX池的当前流动性估值 (通常是一个较大的整数，具体单位取决于实现)。
    /// (Returns the current liquidity valuation of this DEX pool (usually a large integer; specific unit depends on implementation).)
    fn liquidity(&self) -> u128;
    /// 返回该DEX池在Sui区块链上的唯一对象ID (`ObjectID`)。
    /// (Returns the unique Object ID (`ObjectID`) of this DEX pool on the Sui blockchain.)
    fn object_id(&self) -> ObjectID;

    /// `flip` 方法 (翻转DEX的交易方向 / Flip DEX's trading direction method)
    ///
    /// 这个方法会修改DEX实例的内部状态，将其交易方向翻转。
    /// (This method modifies the internal state of the DEX instance, flipping its trading direction.)
    /// 例如，如果一个DEX实例之前表示的是 “用SUI买OCEAN”，调用 `flip()` 后，它将表示 “用OCEAN买SUI”（假设是同一个池子支持双向交易）。
    /// (For example, if a DEX instance previously represented "buy OCEAN with SUI", after calling `flip()`, it will represent "buy SUI with OCEAN" (assuming the same pool supports bidirectional trading).)
    /// 这通常涉及到交换内部的 `coin_in_type` 和 `coin_out_type`，以及可能调整其他与方向相关的内部状态（如 `is_a2b`）。
    /// (This usually involves swapping the internal `coin_in_type` and `coin_out_type`, and possibly adjusting other direction-related internal states (like `is_a2b`).)
    /// 这个方法对于构建“买入路径”（通常从卖出路径翻转得到）非常有用。
    /// (This method is very useful for constructing "buy paths" (often obtained by flipping sell paths).)
    fn flip(&mut self); // `&mut self` 表示这个方法会修改调用它的实例。 (`&mut self` indicates this method modifies the instance it's called on.)

    // --- 调试和测试用的方法 ---
    // (Methods for debugging and testing)

    /// `is_a2b` 方法 (判断当前方向是否为 A 到 B / Is current direction A to B? method)
    ///
    /// 返回一个布尔值，指示当前DEX实例的交易方向是否为“A到B”。
    /// (Returns a boolean value indicating whether the current DEX instance's trading direction is "A to B".)
    /// 在许多DEX（尤其是那些基于X*Y=K模型的AMM）的实现中，一个交易池通常包含两种代币，可以称它们为代币A和代币B（或者token0和token1）。
    /// (In many DEX implementations (especially AMMs based on the X*Y=K model), a trading pool usually contains two types of tokens, which can be called token A and token B (or token0 and token1).)
    /// 合约内部可能会有两个不同的交换函数，一个用于 A->B 的交换，另一个用于 B->A 的交换。
    /// (The contract might internally have two different swap functions, one for A->B swaps and another for B->A swaps.)
    /// `is_a2b` 这个标志（或类似逻辑）就是用来决定在当前 `coin_in_type` 和 `coin_out_type` 的配置下，应该调用哪个底层合约函数。
    /// (The `is_a2b` flag (or similar logic) is used to determine which underlying contract function should be called under the current `coin_in_type` and `coin_out_type` configuration.)
    /// “A”和“B”的具体含义（例如，哪个是 `coin_type_a`，哪个是 `coin_type_b`）取决于DEX池的初始化方式或其全局排序规则。
    /// (The specific meaning of "A" and "B" (e.g., which one is `coin_type_a` and which is `coin_type_b`) depends on the DEX pool's initialization method or its global ordering rules.)
    fn is_a2b(&self) -> bool;

    /// `swap_tx` 方法 (构建一个简单的交换交易，主要用于测试 / Build a simple swap transaction, mainly for testing method)
    ///
    /// 这个异步方法构建一个完整独立的Sui交易数据 (`TransactionData`)，用于执行一次简单的、从 `sender` 到 `recipient` 的代币交换。
    /// (This asynchronous method constructs a complete, independent Sui transaction data (`TransactionData`) for performing a simple token swap from `sender` to `recipient`.)
    /// 它不涉及复杂的多跳路径组合或闪电贷，主要目的是方便对单个DEX池的交换功能进行直接测试。
    /// (It does not involve complex multi-hop path combinations or flash loans; its main purpose is to facilitate direct testing of a single DEX pool's swap functionality.)
    ///
    /// **参数 (Parameters)**:
    /// - `sender`: 交易的发送者地址。(Sender's address for the transaction.)
    /// - `recipient`: 接收交换后输出代币的地址。(Address to receive the output tokens after the swap.)
    /// - `amount_in`: 要输入的代币数量。(Amount of input tokens.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<TransactionData>`: 构建好的、可提交的交易数据，或在出错时返回错误。
    ///                             (The constructed, submittable transaction data, or an error if something goes wrong.)
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData>;
}

/// `CloneBoxedDex` Trait (克隆Boxed Dex的接口 / Interface for Cloning Boxed Dex)
///
/// 这是一个辅助性质的trait，其目的是让 `Box<dyn Dex>` 这种特征对象能够被克隆。
/// (This is an auxiliary trait whose purpose is to enable `Box<dyn Dex>` trait objects to be cloned.)
/// 在Rust中，特征对象 `dyn Trait` 本身并不能直接实现标准的 `Clone` trait，
/// 因为编译器在编译时不知道 `dyn Trait` 背后具体类型的大小和克隆逻辑。
/// (In Rust, trait objects `dyn Trait` themselves cannot directly implement the standard `Clone` trait
/// because the compiler doesn't know the size and cloning logic of the concrete type behind `dyn Trait` at compile time.)
/// `CloneBoxedDex` 提供了一个 `clone_boxed` 方法，要求具体的 `Dex` 实现者来定义如何克隆它们自己，
/// 并返回一个新的 `Box<dyn Dex>`。
/// (`CloneBoxedDex` provides a `clone_boxed` method, requiring concrete `Dex` implementers to define how to clone themselves
/// and return a new `Box<dyn Dex>`.)
pub trait CloneBoxedDex {
    /// 克隆自身并返回一个新的 `Box<dyn Dex>`。
    /// (Clones itself and returns a new `Box<dyn Dex>`.)
    fn clone_boxed(&self) -> Box<dyn Dex>;
}

/// 为所有同时实现了 `Dex` 和 `Clone` (并且生命周期是 `'static`) 的类型 `T` 来自动实现 `CloneBoxedDex` trait。
/// (Automatically implement the `CloneBoxedDex` trait for all types `T` that implement `Dex`, `Clone`, and have a `'static` lifetime.)
///
/// **约束条件 (Constraints)**:
/// - `T: 'static`: 类型 `T` 必须具有 `'static` 生命周期，意味着它不能包含任何非静态的引用（即它拥有其所有数据，或者其包含的引用也都是 `'static` 的）。
///                  (Type `T` must have a `'static` lifetime, meaning it cannot contain any non-static references (i.e., it owns all its data, or any references it contains are also `'static`).)
/// - `T: Dex`: 类型 `T` 必须已经实现了我们上面定义的 `Dex` trait。
///             (Type `T` must have already implemented the `Dex` trait defined above.)
/// - `T: Clone`: 类型 `T` 必须已经实现了标准的 `Clone` trait，知道如何创建自己的一个副本。
///               (Type `T` must have already implemented the standard `Clone` trait, knowing how to create a copy of itself.)
///
/// 这样，对于任何满足这些条件的具体DEX结构体（例如 `CetusDexPool`, `TurbosDexPool` 等，假设它们都 `#[derive(Clone)]`），
/// 它们就自动获得了 `CloneBoxedDex` 的能力。
/// (Thus, any concrete DEX struct satisfying these conditions (e.g., `CetusDexPool`, `TurbosDexPool`, assuming they all `#[derive(Clone)]`)
/// automatically gains the `CloneBoxedDex` capability.)
impl<T> CloneBoxedDex for T
where
    T: 'static + Dex + Clone,
{
    /// `clone_boxed` 的实现：
    /// (Implementation of `clone_boxed`:)
    /// 调用 `self.clone()` 来获取类型 `T` 的一个副本，然后将这个副本用 `Box::new()` 包装起来，
    /// 转换成一个 `Box<dyn Dex>` 特征对象并返回。
    /// (Call `self.clone()` to get a copy of type `T`, then wrap this copy with `Box::new()`,
    ///  convert it into a `Box<dyn Dex>` trait object, and return it.)
    fn clone_boxed(&self) -> Box<dyn Dex> {
        Box::new(self.clone())
    }
}

/// 为 `Box<dyn Dex>` 特征对象本身实现标准的 `Clone` trait。
/// (Implement the standard `Clone` trait for `Box<dyn Dex>` trait objects themselves.)
/// 现在，当我们在一个 `Box<dyn Dex>` 类型的变量上调用 `.clone()` 方法时，
/// 它实际上会调用上面通过 `CloneBoxedDex` trait提供的 `self.clone_boxed()` 方法。
/// (Now, when we call the `.clone()` method on a variable of type `Box<dyn Dex>`,
///  it will actually call the `self.clone_boxed()` method provided via the `CloneBoxedDex` trait above.)
/// 这就完成了让 `Box<dyn Dex>` 变得可克隆的目标。
/// (This achieves the goal of making `Box<dyn Dex>` cloneable.)
impl Clone for Box<dyn Dex> {
    fn clone(&self) -> Box<dyn Dex> {
        self.clone_boxed()
    }
}

// --- 为 `Box<dyn Dex>` 实现比较和哈希相关的 traits ---
// (Implement comparison and hashing related traits for `Box<dyn Dex>`)
// 为了能够将 `Box<dyn Dex>` 对象存储在像 `HashMap` (作为键) 或 `HashSet` 这样的集合中，
// 或者对它们进行比较（例如，判断两条路径是否经过了同一个DEX池），
// 我们需要为它实现 `PartialEq`, `Eq` 和 `Hash` traits。
// (To be able to store `Box<dyn Dex>` objects in collections like `HashMap` (as keys) or `HashSet`,
//  or to compare them (e.g., to determine if two paths go through the same DEX pool),
//  we need to implement `PartialEq`, `Eq`, and `Hash` traits for it.)
// 这里的比较和哈希逻辑都是基于DEX池的唯一对象ID (`object_id()`) 来实现的。
// (The comparison and hashing logic here is based on the unique object ID (`object_id()`) of the DEX pool.)

/// 实现 `PartialEq` (部分相等性比较) for `Box<dyn Dex>`。
/// (Implement `PartialEq` (partial equality comparison) for `Box<dyn Dex>`.)
impl PartialEq for Box<dyn Dex> {
    /// `eq` 方法定义了两个 `Box<dyn Dex>` 实例如何被认为是相等的。
    /// (The `eq` method defines how two `Box<dyn Dex>` instances are considered equal.)
    /// 在这里，如果它们的 `object_id()` 返回值相同，则认为它们相等。
    /// (Here, if their `object_id()` return values are the same, they are considered equal.)
    fn eq(&self, other: &Self) -> bool {
        self.object_id() == other.object_id()
    }
}

/// 实现 `Eq` (完全相等性标记) for `Box<dyn Dex>`。
/// (Implement `Eq` (full equality marker) for `Box<dyn Dex>`.)
/// `Eq` 是 `PartialEq` 的一个子trait，它表明相等关系是自反的、对称的和传递的。
/// (`Eq` is a subtrait of `PartialEq` indicating that the equality relation is reflexive, symmetric, and transitive.)
/// 由于 `ObjectID` 的比较满足这些性质，所以这里可以直接声明 `Eq`。
/// (Since `ObjectID` comparison satisfies these properties, `Eq` can be directly declared here.)
impl Eq for Box<dyn Dex> {}

/// 实现 `Hash` (哈希计算) for `Box<dyn Dex>`。
/// (Implement `Hash` (hashing) for `Box<dyn Dex>`.)
impl Hash for Box<dyn Dex> {
    /// `hash` 方法定义了如何为一个 `Box<dyn Dex>` 实例计算哈希值。
    /// (The `hash` method defines how to calculate a hash value for a `Box<dyn Dex>` instance.)
    /// 它将 `object_id()` 的哈希值作为整个DEX实例的哈希值。
    /// (It uses the hash value of `object_id()` as the hash value for the entire DEX instance.)
    /// `state: &mut H` 是一个实现了 `std::hash::Hasher` trait 的哈希器状态对象。
    /// (`state: &mut H` is a hasher state object that implements the `std::hash::Hasher` trait.)
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.object_id().hash(state);
    }
}

/// 为 `Box<dyn Dex>` 实现 `fmt::Debug` trait，用于在调试时打印DEX实例的信息。
/// (Implement `fmt::Debug` trait for `Box<dyn Dex>` for printing DEX instance information during debugging.)
impl fmt::Debug for Box<dyn Dex> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 打印DEX的协议名称、池对象ID、当前交易方向的输入代币类型和输出代币类型。
        // (Print the DEX's protocol name, pool object ID, and input/output coin types for the current trading direction.)
        // 这种格式化的输出使得在查看日志或调试器中的 `Box<dyn Dex>` 对象时，能更容易地识别它是什么。
        // (This formatted output makes it easier to identify what a `Box<dyn Dex>` object is when viewing logs or in a debugger.)
        write!(
            f,
            // 格式: ProtocolName(Pool: ObjectID_String, In: CoinInType_String, Out: CoinOutType_String)
            // (Format: ProtocolName(Pool: ObjectID_String, In: CoinInType_String, Out: CoinOutType_String))
            // 例如: Cetus(Pool: 0x123..., In: 0x2::sui::SUI, Out: ...::ocean::OCEAN)
            // (Example: Cetus(Pool: 0x123..., In: 0x2::sui::SUI, Out: ...::ocean::OCEAN))
            "{}(Pool: {}, In: {}, Out: {})",
            self.protocol(),    // 获取协议名称 (如 "Cetus") (Get protocol name (e.g., "Cetus"))
            self.object_id(),   // 获取池的ObjectID (会自动调用其Display/Debug实现) (Get pool's ObjectID (will automatically call its Display/Debug implementation))
            self.coin_in_type(),// 获取输入代币类型 (Get input coin type)
            self.coin_out_type()// 获取输出代币类型 (Get output coin type)
        )
    }
}


/// `Defi` 结构体 (DeFi交互核心 / DeFi Interaction Core struct)
///
/// 这个结构体是与所有DeFi协议进行交互的顶层逻辑封装。
/// (This struct is the top-level logic encapsulation for interacting with all DeFi protocols.)
/// 它持有一个 `DexSearcher` 的实例（用于发现各种DEX池）和
/// 一个 `Trader` 的实例（来自 `trade` 子模块，用于执行和模拟实际的交易操作）。
/// (It holds an instance of `DexSearcher` (for discovering various DEX pools) and
///  an instance of `Trader` (from the `trade` submodule, for executing and simulating actual trading operations).)
/// `Defi` 结构体旨在提供一个更高层次、更易用的API给上层应用（如套利策略模块）。
/// (The `Defi` struct aims to provide a higher-level, more user-friendly API to upper-layer applications (like the arbitrage strategy module).)
///
/// `#[derive(Clone)]` 使得 `Defi` 结构体本身也可以被克隆。
/// (`#[derive(Clone)]` makes the `Defi` struct itself cloneable.)
/// 由于其内部成员 `dex_searcher` 和 `trader` 都是 `Arc` (原子引用计数) 包裹的智能指针，
/// 克隆一个 `Defi` 实例实际上只是克隆这两个 `Arc` 指针（即增加引用计数），
/// 而不是深拷贝底层的 `DexSearcher` 或 `Trader` 对象。这使得克隆操作非常轻量级和高效。
/// (Since its internal members `dex_searcher` and `trader` are smart pointers wrapped in `Arc` (Atomic Reference Counting),
///  cloning a `Defi` instance actually just clones these two `Arc` pointers (i.e., increments the reference count),
///  rather than deep copying the underlying `DexSearcher` or `Trader` objects. This makes the cloning operation very lightweight and efficient.)
#[derive(Clone)]
pub struct Defi {
    dex_searcher: Arc<dyn DexSearcher>, // 一个共享的、实现了 `DexSearcher` trait 的DEX搜索器实例。
                                        // (A shared instance of a DEX searcher that implements the `DexSearcher` trait.)
                                        // `Arc<dyn Trait>` 允许多处代码共享同一个实现了该trait的对象，并且是线程安全的。
                                        // (`Arc<dyn Trait>` allows multiple parts of the code to share the same object implementing the trait, and is thread-safe.)
    trader: Arc<Trader>,               // 一个共享的 `Trader` 实例，用于处理交易的模拟和执行。
                                       // (A shared `Trader` instance, used for handling simulation and execution of trades.)
}

impl Defi {
    /// `new` 构造函数 (new constructor)
    ///
    /// 创建一个新的 `Defi` 实例。这是一个异步函数，因为初始化其内部组件（如 `DexSearcher` 和 `Trader`）可能需要执行异步操作（例如，从网络加载配置或连接到RPC节点）。
    /// (Creates a new `Defi` instance. This is an asynchronous function because initializing its internal components (like `DexSearcher` and `Trader`) might require performing asynchronous operations (e.g., loading configuration from the network or connecting to an RPC node).)
    ///
    /// **参数 (Parameters)**:
    /// - `http_url`: 一个字符串切片 (`&str`)，表示Sui RPC节点的URL地址。这个URL会被传递给 `DexSearcher`（例如 `IndexerDexSearcher`）和可能的 `Trader` 内部组件，用于与Sui链进行通信。
    ///               (A string slice (`&str`) representing the URL address of the Sui RPC node. This URL will be passed to `DexSearcher` (e.g., `IndexerDexSearcher`) and potentially internal components of `Trader` for communication with the Sui chain.)
    /// - `simulator_pool`: 一个共享的交易模拟器对象池 (`Arc<ObjectPool<Box<dyn Simulator>>>`)。这个池子会被传递给 `DexSearcher`（用于在发现池子时可能需要模拟检查）和 `Trader`（用于模拟交易路径）。
    ///                     (A shared transaction simulator object pool (`Arc<ObjectPool<Box<dyn Simulator>>>`). This pool will be passed to `DexSearcher` (which might need simulation checks when discovering pools) and `Trader` (for simulating trading paths).)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Self>`: 如果成功初始化所有组件，则返回 `Ok(Defi)` 实例；如果初始化过程中发生任何错误，则返回 `Err(...)`。
    ///                   (Returns an `Ok(Defi)` instance if all components are successfully initialized; returns `Err(...)` if any error occurs during initialization.)
    pub async fn new(http_url: &str, simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>) -> Result<Self> {
        // 初始化 `IndexerDexSearcher`，它是一个具体的 `DexSearcher` 实现，可能通过查询外部索引服务来发现DEX池。
        // (Initialize `IndexerDexSearcher`, a concrete `DexSearcher` implementation that might discover DEX pools by querying external indexing services.)
        // 它需要RPC URL和模拟器池。 (It requires the RPC URL and the simulator pool.)
        let dex_searcher_instance = IndexerDexSearcher::new(http_url, Arc::clone(&simulator_pool)).await?; // 使用 `Arc::clone` 来传递共享的模拟器池引用。(Use `Arc::clone` to pass the shared simulator pool reference.)
        // 初始化 `Trader` 实例。 (Initialize the `Trader` instance.)
        let trader_instance = Trader::new(simulator_pool).await?; // `Trader` 也需要模拟器池。 (`Trader` also needs the simulator pool.)

        // 创建 `Defi` 实例，将其内部的 `dex_searcher` 和 `trader` 字段用 `Arc::new()` 包裹起来，
        // 以便它们可以被安全地共享（如果 `Defi` 实例本身被克隆的话）。
        // (Create the `Defi` instance, wrapping its internal `dex_searcher` and `trader` fields with `Arc::new()`
        //  so they can be safely shared (if the `Defi` instance itself is cloned).)
        Ok(Self {
            dex_searcher: Arc::new(dex_searcher_instance),
            trader: Arc::new(trader_instance),
        })
    }

    /// `find_dexes` 方法 (查找DEX / Find DEXs method)
    ///
    /// 这个方法是一个简单的包装器，直接调用其内部 `dex_searcher` 实例的同名方法。
    /// (This method is a simple wrapper that directly calls the identically named method of its internal `dex_searcher` instance.)
    /// `#[allow(dead_code)]` 属性宏表示，即使这个函数在当前项目中没有被直接调用（即编译器认为它是“死代码”），也不要发出警告。
    /// (`#[allow(dead_code)]` attribute macro indicates that even if this function is not directly called in the current project (i.e., the compiler considers it "dead code"), do not issue a warning.)
    /// 这有时用于保留一个API，即使它目前主要供内部使用或未来可能使用。
    /// (This is sometimes used to preserve an API, even if it's currently mainly for internal use or potential future use.)
    ///
    /// **参数 (Parameters)** 和 **返回 (Returns)** 与 `DexSearcher::find_dexes` 方法完全相同。
    /// (Parameters and Returns are identical to the `DexSearcher::find_dexes` method.)
    #[allow(dead_code)]
    pub async fn find_dexes(&self, coin_in_type: &str, coin_out_type: Option<String>) -> Result<Vec<Box<dyn Dex>>> {
        self.dex_searcher.find_dexes(coin_in_type, coin_out_type).await
    }

    /// `find_sell_paths` 方法 (查找卖出路径 / Find Sell Paths method)
    ///
    /// 这个核心异步方法用于查找从指定的输入代币 (`coin_in_type`) 开始，经过一系列DEX交换后，
    /// 最终能够换回SUI原生代币的有效交易路径。
    /// (This core asynchronous method is used to find valid trading paths starting from a specified input coin (`coin_in_type`),
    ///  going through a series of DEX swaps, and eventually swapping back to the SUI native coin.)
    ///
    /// **核心逻辑 (Core Logic)**: (详情见上方中文总览 / See Chinese overview above for details)
    /// 1.  特殊情况处理: 如果输入是SUI，返回空路径。
    ///     (Special case: If input is SUI, return an empty path.)
    /// 2.  迭代路径发现 (BFS/迭代加深风格):
    ///     (Iterative path discovery (BFS/Iterative Deepening style):)
    ///     -   使用 `stack` (实际行为更像队列，用于广度优先的层级遍历) 和 `visited` 集合。
    ///         (Uses `stack` (actually behaves more like a queue for breadth-first level traversal) and `visited` set.)
    ///     -   `MAX_HOP_COUNT` 轮迭代。 (Iterate for `MAX_HOP_COUNT` rounds.)
    ///     -   每轮为 `stack` 中的每个代币查找下一跳的DEX。
    ///         (In each round, for each coin in `stack`, find DEXs for the next hop.)
    ///     -   目标输出代币：如果是锚定币或最后一跳，则目标是SUI；否则是任意代币 (`None`)。
    ///         (Target output coin: SUI if it's a pegged coin or the last hop; otherwise, any coin (`None`).)
    ///     -   过滤DEX (流动性、数量上限、避免重复)。
    ///         (Filter DEXs (liquidity, max count, avoid duplicates).)
    ///     -   将找到的DEX的输出代币加入下一轮的 `new_stack`。
    ///         (Add output coins of found DEXs to `new_stack` for the next round.)
    ///     -   将 (当前代币 -> [可达DEX列表]) 存入 `all_hops`。
    ///         (Store (current coin -> [list of reachable DEXs]) into `all_hops`.)
    /// 3.  构建完整路径 (DFS):
    ///     (Build complete paths (DFS):)
    ///     -   从 `coin_in_type` 开始，使用 `all_hops` 中的信息，通过DFS递归构建到SUI的路径。
    ///         (Starting from `coin_in_type`, recursively build paths to SUI using information in `all_hops` via DFS.)
    /// 4.  结果转换: `Vec<Vec<Box<dyn Dex>>>` 转为 `Vec<Path>`。
    ///     (Result conversion: `Vec<Vec<Box<dyn Dex>>>` to `Vec<Path>`.)
    ///
    /// **参数 (Parameters)**:
    /// - `coin_in_type`: 一个字符串切片 (`&str`)，表示你想要卖出的起始代币的类型。
    ///                   (A string slice (`&str`) representing the type of the starting coin you want to sell.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Vec<Path>>`: 一个包含所有找到的有效卖出路径的向量。每条路径都是一个 `Path` 对象。
    ///                       (A vector containing all found valid sell paths. Each path is a `Path` object.)
    pub async fn find_sell_paths(&self, coin_in_type: &str) -> Result<Vec<Path>> {
        // 特殊情况：如果输入的代币已经是SUI，那么不需要进行任何交易。
        // (Special case: If the input coin is already SUI, then no transaction is needed.)
        // 直接返回一个包含单个默认（可能是空的）路径的向量。
        // (Directly return a vector containing a single default (possibly empty) path.)
        if coin::is_native_coin(coin_in_type) { // `coin::is_native_coin` 检查是否是SUI (`coin::is_native_coin` checks if it's SUI)
            return Ok(vec![Path::default()]); // `Path::default()` 可能代表一个0跳的路径 (`Path::default()` might represent a 0-hop path)
        }

        let mut all_hops = HashMap::new(); // 存储: 代币类型 -> 可达的DEX列表
        let mut stack = vec![coin_in_type.to_string()]; // 当前层级待处理的代币
        let mut visited = HashSet::new(); // 已完整处理过出路的代币
        let mut visited_dexes = HashSet::new(); // 已添加的DEX ID，避免重复

        for nth_hop in 0..MAX_HOP_COUNT { // 最多进行 MAX_HOP_COUNT 跳
            let is_last_hop = nth_hop == MAX_HOP_COUNT - 1;
            let mut new_stack = vec![];

            while let Some(current_coin_type) = stack.pop() {
                if visited.contains(&current_coin_type) || coin::is_native_coin(&current_coin_type) {
                    continue;
                }
                visited.insert(current_coin_type.clone());

                let target_coin_out_type =
                    if pegged_coin_types().contains(current_coin_type.as_str()) || is_last_hop {
                        Some(SUI_COIN_TYPE.to_string())
                    } else {
                        None
                    };

                let mut dexes_from_current_coin =
                    if let Ok(dexes) = self.dex_searcher.find_dexes(&current_coin_type, target_coin_out_type).await {
                        dexes
                    } else {
                        continue;
                    };

                dexes_from_current_coin.retain(|dex| dex.liquidity() >= MIN_LIQUIDITY);

                if dexes_from_current_coin.len() > MAX_POOL_COUNT {
                    dexes_from_current_coin.retain(|dex| !visited_dexes.contains(&dex.object_id()));
                    dexes_from_current_coin.sort_by_key(|dex| std::cmp::Reverse(dex.liquidity()));
                    dexes_from_current_coin.truncate(MAX_POOL_COUNT);
                }

                if dexes_from_current_coin.is_empty() {
                    continue;
                }

                for dex in &dexes_from_current_coin {
                    let out_coin_type = dex.coin_out_type();
                    if !visited.contains(&out_coin_type) {
                        new_stack.push(out_coin_type.clone());
                    }
                    visited_dexes.insert(dex.object_id());
                }
                all_hops.insert(current_coin_type.clone(), dexes_from_current_coin);
            }

            if is_last_hop {
                break;
            }
            stack = new_stack;
        }

        let mut routes = vec![];
        let mut current_path_segment = vec![];
        dfs(coin_in_type, &mut current_path_segment, &all_hops, &mut routes);

        Ok(routes.into_iter().map(Path::new).collect())
    }

    /// `find_buy_paths` 方法 (查找买入路径 / Find Buy Paths method)
    ///
    /// 这个异步方法用于查找从SUI原生代币开始，购买到指定的目标代币 (`coin_out_type`) 的交易路径。
    /// (This asynchronous method is used to find trading paths starting from the SUI native coin to buy a specified target coin (`coin_out_type`).)
    ///
    /// **实现策略 (Implementation Strategy)**: (详情见上方中文总览 / See Chinese overview above for details)
    /// 1. 调用 `find_sell_paths(coin_out_type)` 找到从目标代币卖回SUI的路径。
    ///    (Call `find_sell_paths(coin_out_type)` to find paths from the target coin back to SUI.)
    /// 2. 对每条找到的路径进行反转：
    ///    (For each found path, reverse it:)
    ///    a. 反转路径中DEX的顺序。 (Reverse the order of DEXs in the path.)
    ///    b. 对路径中的每个DEX实例调用 `flip()` 方法，翻转其内部交易方向。
    ///       (Call the `flip()` method on each DEX instance in the path to flip its internal trading direction.)
    ///
    /// **参数 (Parameters)**:
    /// - `coin_out_type`: 一个字符串切片 (`&str`)，表示你想要购买的目标代币的类型。
    ///                    (A string slice (`&str`) representing the type of the target coin you want to buy.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<Vec<Path>>`: 一个包含所有找到的有效买入路径的向量。每条路径都是一个 `Path` 对象。
    ///                       (A vector containing all found valid buy paths. Each path is a `Path` object.)
    pub async fn find_buy_paths(&self, coin_out_type: &str) -> Result<Vec<Path>> {
        let mut paths = self.find_sell_paths(coin_out_type).await?;
        for path in &mut paths {
            path.path.reverse();
            for dex in &mut path.path {
                dex.flip();
            }
        }
        Ok(paths)
    }

    /// `find_best_path_exact_in` 方法 (为精确输入金额查找最佳路径 / Find Best Path for Exact Input Amount method)
    ///
    /// 在一组给定的候选交易路径 (`paths`) 中，对于一个指定的精确输入代币数量 (`amount_in`)，
    /// 通过模拟每条路径的交易过程，找到那条能够产生最佳输出结果的路径。
    /// (Among a given set of candidate trading paths (`paths`), for a specified exact input coin amount (`amount_in`),
    ///  finds the path that yields the best output result by simulating the trading process for each path.)
    /// “最佳输出结果”通常是指获得最多的输出代币数量，或者在套利场景下获得最高的利润。
    /// ("Best output result" usually means obtaining the maximum amount of output coins, or the highest profit in an arbitrage scenario.)
    ///
    /// **参数 (Parameters)**:
    /// - `paths`: 一个包含多个 `Path` 对象的切片 (`&[Path]`)，这些是待评估的候选交易路径。
    ///            (A slice (`&[Path]`) containing multiple `Path` objects, which are the candidate trading paths to be evaluated.)
    /// - `sender`: 交易的发送者Sui地址。(Sender's Sui address for the transaction.)
    /// - `amount_in`: 输入代币的确切数量 (通常是u64类型，代表代币的最小单位)。
    ///                (The exact amount of the input coin (usually u64 type, representing the smallest unit of the coin).)
    /// - `trade_type`: 交易类型 (`TradeType` 枚举)，例如是普通的兑换 (`Swap`) 还是闪电贷 (`Flashloan`)。
    ///                 这会影响模拟的方式和最终利润的计算。
    ///                 (`TradeType` enum, e.g., normal `Swap` or `Flashloan`. This affects the simulation method and final profit calculation.)
    /// - `gas_coins`: 一个 `ObjectRef` 的切片 (`&[ObjectRef]`)，代表用于支付交易Gas费用的SUI代币对象。
    ///                (A slice of `ObjectRef`s (`&[ObjectRef]`) representing the SUI coin objects used to pay transaction Gas fees.)
    /// - `sim_ctx`: 一个对 `SimulateCtx` (模拟上下文) 的引用，其中包含了当前Sui网络的纪元信息（如当前的Gas价格），
    ///              这对于准确模拟交易成本和结果至关重要。
    ///              (A reference to `SimulateCtx` (simulation context), which contains current Sui network epoch information (like current Gas price),
    ///               crucial for accurately simulating transaction costs and results.)
    ///
    /// **返回 (Returns)**:
    /// - `Result<PathTradeResult>`:
    ///   - 如果成功找到最佳路径并且模拟结果有效，则返回 `Ok(PathTradeResult)`。
    ///     (If the best path is successfully found and the simulation result is valid, returns `Ok(PathTradeResult)`.)
    ///     `PathTradeResult` 结构体（定义见下文）封装了最佳路径本身以及该路径在给定输入下的详细交易结果
    ///     （如输出金额、Gas成本、缓存未命中次数等）。
    ///     (The `PathTradeResult` struct (defined below) encapsulates the best path itself and the detailed trading result for that path with the given input
    ///      (such as output amount, Gas cost, cache misses, etc.).)
    ///   - 如果所有路径都无效、模拟失败，或者找不到任何有利可图的路径，则返回 `Err(...)`。
    ///     (If all paths are invalid, simulation fails, or no profitable path is found, returns `Err(...):`)
    pub async fn find_best_path_exact_in(
        &self,
        paths: &[Path],
        sender: SuiAddress,
        amount_in: u64,
        trade_type: TradeType,
        gas_coins: &[ObjectRef],
        sim_ctx: &SimulateCtx,
    ) -> Result<PathTradeResult> {
        let mut joinset = JoinSet::new(); // 创建 JoinSet 以并发模拟路径 (Create JoinSet to simulate paths concurrently)

        for (idx, path) in paths.iter().enumerate() {
            if path.is_empty() {
                continue;
            }

            let trader_clone = Arc::clone(&self.trader);
            let path_clone = path.clone();
            let gas_coins_clone = gas_coins.to_vec();
            let sim_ctx_clone = sim_ctx.clone();

            joinset.spawn(
                async move {
                    let result = trader_clone
                        .get_trade_result(&path_clone, sender, amount_in, trade_type, gas_coins_clone, sim_ctx_clone)
                        .await;
                    (idx, result) // 返回路径索引和模拟结果
                }
                .in_current_span(),
            );
        }

        let (mut best_idx, mut best_trade_res) = (0, TradeResult::default()); // 初始化最佳结果

        while let Some(join_result) = joinset.join_next().await { // 等待并处理并发任务的结果
            match join_result {
                Ok((idx, trade_res_result)) => {
                    match trade_res_result {
                        Ok(current_trade_res) => {
                            if current_trade_res > best_trade_res { // 比较找到更优的
                                best_idx = idx;
                                best_trade_res = current_trade_res;
                            }
                        }
                        Err(_error) => { /* 忽略单个路径模拟失败 */ }
                    }
                }
                Err(_join_error) => { /* 记录并发任务执行错误 */ }
            }
        }

        ensure!(best_trade_res.amount_out > 0, "所有评估路径的最佳输出金额均为零或无效 (Best output amount for all evaluated paths is zero or invalid)");

        Ok(PathTradeResult::new(paths[best_idx].clone(), amount_in, best_trade_res))
    }


    /// `build_final_tx_data` 方法 (构建最终的Sui交易数据 / Build Final Sui Transaction Data method)
    ///
    /// 这个异步方法负责构建最终的、可以在Sui区块链上执行的交易数据 (`TransactionData`)。
    /// (This asynchronous method is responsible for building the final transaction data (`TransactionData`) that can be executed on the Sui blockchain.)
    /// 它通常用于那些已经确定了最佳套利路径、输入金额，并且可能涉及到闪电贷操作的场景。
    /// (It's typically used in scenarios where the best arbitrage path and input amount have been determined, and may involve flash loan operations.)
    ///
    /// **参数 (Parameters)**: (详情见上方中文总览 / See Chinese overview above for details)
    /// - `sender`: 交易发送者地址。(Sender's address for the transaction.)
    /// - `amount_in`: 初始输入金额 (或闪电贷金额)。(Initial input amount (or flash loan amount).)
    /// - `path`: 选定的最佳交易路径。(The selected best trading path.)
    /// - `gas_coins`: 用于支付Gas的SUI代币对象列表。(List of SUI coin objects for paying Gas.)
    /// - `gas_price`: 当前网络的Gas价格。(Current network Gas price.)
    /// - `source`: 交易机会的来源信息 (对MEV重要)。(Source information of the trading opportunity (important for MEV).)
    ///
    /// **返回 (Returns)**:
    /// - `Result<TransactionData>`:
    ///   - 如果成功构建了交易，则返回 `Ok(TransactionData)`。
    ///     (If the transaction is successfully built, returns `Ok(TransactionData)`.)
    ///   - 如果在构建过程中发生错误，则返回 `Err(...)`。
    ///     (If an error occurs during building, returns `Err(...):`)
    pub async fn build_final_tx_data(
        &self,
        sender: SuiAddress,
        amount_in: u64,
        path: &Path,
        gas_coins: Vec<ObjectRef>,
        gas_price: u64,
        source: Source,
    ) -> Result<TransactionData> {
        // 调用 `Trader` 的方法来构建包含闪电贷和交易序列的PTB。
        // (Call `Trader`'s method to build a PTB containing flash loan and trade sequence.)
        let (tx_data, _expected_profit) = self
            .trader
            .get_flashloan_trade_tx(path, sender, amount_in, gas_coins, gas_price, source)
            .await?;

        Ok(tx_data)
    }
}

/// `dfs` (Depth-First Search, 深度优先搜索) 辅助函数
/// (dfs (Depth-First Search) helper function)
///
/// 这是一个递归函数，用于从 `hops` (一个表示代币之间可以通过哪些DEX进行连接的图) 中，
/// 构建出所有可能的、从起始代币到目标代币（这里是SUI）的交易路径。
/// (This is a recursive function used to build all possible trading paths from a starting coin to a target coin (SUI here)
///  from `hops` (a graph representing which DEXs can connect between coins).)
///
/// **参数 (Parameters)**:
/// - `coin_type`: 当前DFS路径探索到的末端代币的类型。
///                (Type of the coin at the end of the current DFS path exploration.)
/// - `current_path_segment`: 当前正在构建的路径段 (DEX序列)。
///                           (The currently building path segment (sequence of DEXs).)
/// - `hops`: 交易图的邻接列表表示 (代币 -> 可达DEX列表)。
///           (Adjacency list representation of the trading graph (coin -> list of reachable DEXs).)
/// - `routes`: 用于存储所有找到的完整路径 (从起始代币到SUI)。
///             (Used to store all found complete paths (from starting coin to SUI).)
fn dfs(
    coin_type: &str,
    current_path_segment: &mut Vec<Box<dyn Dex>>,
    hops: &HashMap<String, Vec<Box<dyn Dex>>>,
    routes: &mut Vec<Vec<Box<dyn Dex>>>,
) {
    // 终止条件1: 到达SUI (Termination Condition 1: Reached SUI)
    if coin::is_native_coin(coin_type) {
        routes.push(current_path_segment.clone()); // 存路径 (Store the path)
        return;
    }

    // 终止条件2: 路径超长 (Termination Condition 2: Path too long)
    if current_path_segment.len() >= MAX_HOP_COUNT {
        return;
    }

    // 终止条件3: 当前代币无路可走 (Termination Condition 3: No way out for the current coin)
    if !hops.contains_key(coin_type) {
        return;
    }

    // 递归探索 (Recursive Exploration)
    for dex_instance in hops.get(coin_type).unwrap() {
        current_path_segment.push(dex_instance.clone()); // 将DEX加入当前路径段 (Add DEX to current path segment)
        dfs(&dex_instance.coin_out_type(), current_path_segment, hops, routes); // 递归到下一层 (Recurse to the next level)
        current_path_segment.pop(); // 回溯：移除刚加入的DEX，尝试其他分支 (Backtrack: remove the just-added DEX, try other branches)
    }
}


/// `PathTradeResult` 结构体 (路径交易结果 / Path Trade Result struct)
///
/// 这个结构体用于封装一条特定的交易路径 (`Path`) 在给定输入金额 (`amount_in`) 下，
/// 经过交易模拟后得到的详细结果。
/// (This struct is used to encapsulate the detailed results obtained after simulating a trade
///  for a specific trading path (`Path`) with a given input amount (`amount_in`).)
#[derive(Debug, Clone)]
pub struct PathTradeResult {
    pub path: Path,         // 发生交易的路径 (`Path` 对象)。(The path where the trade occurred (`Path` object).)
    pub amount_in: u64,     // 输入到这条路径的代币的确切数量。(Exact amount of input coin to this path.)
    pub amount_out: u64,    // 从这条路径最终输出的代币的确切数量（经过模拟计算得到）。
                            // (Exact amount of output coin from this path (obtained from simulation).)
    pub gas_cost: i64,      // 执行这条路径所需的大致Gas成本（以SUI的MIST单位计）。
                            // (Approximate Gas cost required to execute this path (in SUI's MIST unit).)
                            // 注意类型是 `i64`，可能是为了表示调整或估算。(Note the type is `i64`, possibly for adjustments or estimations.)
    pub cache_misses: u64,  // 在模拟这条路径的交易过程中，模拟器缓存未命中的次数。
                            // (Number of cache misses by the simulator during the simulation of this path.)
}

impl PathTradeResult {
    /// `new` 构造函数 (new constructor)
    pub fn new(path: Path, amount_in: u64, trade_res: TradeResult) -> Self {
        Self {
            path,
            amount_in,
            amount_out: trade_res.amount_out,
            gas_cost: trade_res.gas_cost,
            cache_misses: trade_res.cache_misses,
        }
    }

    /// `profit` 方法 (计算利润 / Calculate Profit method)
    ///
    /// 计算此交易路径的预期利润。
    /// (Calculates the expected profit for this trading path.)
    /// **逻辑 (Logic)**: (详情见上方中文总览 / See Chinese overview above for details)
    /// 1. SUI -> ... -> SUI: 利润 = 输出SUI - 输入SUI - Gas成本。
    ///    (Profit = Output SUI - Input SUI - Gas Cost.)
    /// 2. SUI -> ... -> 非SUI: 视为成本 = -(输入SUI + Gas成本)。
    ///    (Considered as cost = -(Input SUI + Gas Cost).)
    /// 3. 非SUI -> ...: 利润为0 (当前逻辑下)。
    ///    (Profit is 0 (under current logic).)
    ///
    /// **返回 (Returns)**:
    /// - `i128`: 计算出的利润值（或成本）。(Calculated profit value (or cost).)
    pub fn profit(&self) -> i128 {
        if self.path.coin_in_type() == SUI_COIN_TYPE {
            if self.path.coin_out_type() == SUI_COIN_TYPE {
                return self.amount_out as i128 - self.amount_in as i128 - self.gas_cost as i128;
            }
            0i128 - self.gas_cost as i128 - self.amount_in as i128 // 视为用SUI购买资产的成本 (Cost of buying asset with SUI)
        } else {
            0 // 非SUI起始的路径，当前不计算其原生利润 (Paths not starting with SUI, native profit not calculated currently)
        }
    }
}

/// 为 `PathTradeResult` 实现 `fmt::Display` trait，用于格式化打印。
/// (Implement `fmt::Display` trait for `PathTradeResult` for formatted printing.)
impl fmt::Display for PathTradeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "路径交易结果 {{ 输入金额: {}, 输出金额: {}, 利润: {}, 路径: {:?} ... }} \
            (PathTradeResult {{ amount_in: {}, amount_out: {}, profit: {}, path: {:?} ... }})",
            self.amount_in,
            self.amount_out,
            self.profit(),
            self.path // Path的Debug输出 (Debug output of Path)
        )
    }
}


// --- 测试模块 (`tests`) ---
// (Test module (`tests`))
#[cfg(test)]
mod tests {

    use simulator::HttpSimulator;
    use tracing::info;

    use super::*;
    use crate::config::tests::TEST_HTTP_URL;

    /// `test_find_sell_paths` 测试函数：测试查找从特定代币卖出到SUI的路径。
    /// (Test function `test_find_sell_paths`: Tests finding paths to sell a specific coin to SUI.)
    #[tokio::test]
    async fn test_find_sell_paths() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let simulator_pool = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(HttpSimulator::new(&TEST_HTTP_URL, &None).await) as Box<dyn Simulator> })
        }));

        let defi = Defi::new(TEST_HTTP_URL, simulator_pool).await.unwrap();

        let coin_in_type = "0xa8816d3a6e3136e86bc2873b1f94a15cadc8af2703c075f2d546c2ae367f4df9::ocean::OCEAN"; // 测试用OCEAN代币
        let paths = defi.find_sell_paths(coin_in_type).await.unwrap();

        assert!(!paths.is_empty(), "为代币 {} 未找到任何卖出到SUI的路径 (No sell paths to SUI found for coin {})", coin_in_type);

        for path in paths {
            info!(?path, "找到的卖出路径 (Found sell path)");
        }
    }

    /// `test_find_buy_paths` 测试函数：测试查找从SUI买入特定代币的路径。
    /// (Test function `test_find_buy_paths`: Tests finding paths to buy a specific coin from SUI.)
    #[tokio::test]
    async fn test_find_buy_paths() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let simulator_pool = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(HttpSimulator::new(&TEST_HTTP_URL, &None).await) as Box<dyn Simulator> })
        }));

        let defi = Defi::new(TEST_HTTP_URL, simulator_pool).await.unwrap();

        let coin_out_type = "0xa8816d3a6e3136e86bc2873b1f94a15cadc8af2703c075f2d546c2ae367f4df9::ocean::OCEAN"; // 测试用OCEAN代币
        let paths = defi.find_buy_paths(coin_out_type).await.unwrap();

        assert!(!paths.is_empty(), "为代币 {} 未找到任何从SUI买入的路径 (No buy paths from SUI found for coin {})", coin_out_type);

        for path in paths {
            info!(?path, "找到的买入路径 (Found buy path)");
        }
    }
}

[end of bin/arb/src/defi/mod.rs]
