// 该文件 `navi.rs` 实现了与 Navi Protocol (一个Sui区块链上的借贷协议) 交互的逻辑。
// Navi 允许用户存入资产作为抵押品来借出其他资产，或者直接从资金池中借款（例如闪电贷）。
// 这个文件主要关注 Navi 的闪电贷 (Flashloan) 功能，这对于套利策略非常重要，
// 因为它允许在单笔交易中无抵押借入大量资金，执行套利操作，然后立即归还。
//
// **文件概览 (File Overview)**:
// 这个 `navi.rs` 文件是用来和Sui上的Navi Protocol“对话”的代码。
// Navi Protocol是一个“借贷协议”，就像一个去中心化的银行，你可以在里面：
// 1.  存钱（提供资产作为流动性）并赚取利息。
// 2.  抵押你存入的资产，然后借出其他种类的资产。
// 3.  使用“闪电贷”功能。
// 这个文件主要关注的就是Navi的“闪电贷”功能。
// (This `navi.rs` file contains code for "communicating" with the Navi Protocol on Sui.
//  Navi Protocol is a "lending protocol", like a decentralized bank, where you can:
//  1. Deposit money (provide assets as liquidity) and earn interest.
//  2. Collateralize your deposited assets to borrow other types of assets.
//  3. Use the "flash loan" feature.
//  This file primarily focuses on Navi's "flash loan" functionality.)
//
// **核心功能 (Core Functionality)**:
// 1.  **常量定义 (Constant Definitions)**:
//     -   `NAVI_PROTOCOL`: Navi核心智能合约的“门牌号”（Package ID）。
//     -   `NAVI_POOL`: Navi协议中SUI代币资金池的“身份证号”（ObjectID）。在Navi里，每种支持的代币（比如SUI、USDC）都有一个自己的资金池，用户从这里存取和借贷。这个常量特指SUI的池子。
//     -   `NAVI_CONFIG`: Navi协议全局配置对象的ID。可能包含一些协议级别的参数，比如手续费结构、支持的资产列表等。
//     -   `NAVI_STORAGE`: Navi协议存储对象的ID。可能用来存储更详细的每个资金池的状态、用户的账户信息（借了多少钱、抵押了多少物等）、利率模型参数等。
//
// 2.  **`Navi` 结构体 (The `Navi` Struct)**:
//     -   这个结构体代表了与Navi协议进行交互的一个“实例”或“连接点”。
//     -   它内部存储了与Navi交互时需要用到的配置信息和一些重要对象的引用（比如上面那些常量ID对应的`ObjectArg`）。
//     -   **请注意**：从代码实现来看，当前的 `Navi` 结构体和相关方法似乎是**硬编码为只处理SUI代币的闪电贷**。
//         比如 `sui_coin_type` 字段直接设为SUI，并且闪电贷的泛型参数也写死为SUI。如果想用它来操作Navi上其他代币的闪电贷，可能需要修改或扩展这里的代码。
//         (This struct represents an "instance" or "connection point" for interacting with the Navi protocol.
//          It internally stores configuration information and references to important objects (like the `ObjectArg`s corresponding to the constant IDs above) needed for Navi interaction.
//          **Please note**: From the code implementation, the current `Navi` struct and related methods seem to be **hardcoded to only handle flash loans for SUI tokens**.
//          For example, the `sui_coin_type` field is directly set to SUI, and the generic parameters for flash loans are also hardcoded to SUI. If one wanted to use it for flash loans of other tokens on Navi, the code here would likely need modification or extension.)
//
// 3.  **`new()` 构造函数 (The `new()` Constructor)**:
//     -   这是创建 `Navi` 实例的工厂方法。
//     -   它会异步地（不会卡住程序）去Sui链上把上面提到的那些Navi协议的关键对象（资金池、配置对象、存储对象，还有Sui系统的时钟对象）的最新信息获取下来。
//     -   然后，它把这些链上对象的信息转换成在构建Sui交易（PTB）时可以直接使用的`ObjectArg`格式，并存到`Navi`实例里。
//     -   重要的一点是，这些对象的获取和转换通常只在机器人启动、初始化`Navi`实例时做一次，之后就缓存起来用。这样可以避免在每次套利操作中都重复查询这些基本对象，提高效率。
//
// 4.  **`extend_flashloan_tx()` 方法 (添加发起闪电贷指令到交易中)**:
//     -   这个函数的核心工作是：如果你想发起一笔Navi的SUI闪电贷，它会帮你把调用Navi合约的 `lending::flash_loan_with_ctx` 这个具体函数的指令，添加到你正在构建的Sui交易包（`TradeCtx`代表的PTB）里。
//     -   它会准备好调用合约函数所需的参数，比如Navi的配置对象、SUI资金池对象、以及你想要借多少SUI (`amount_in`)。
//     -   Navi的这个 `flash_loan_with_ctx` 函数执行后，会返回两个东西：
//         1.  借到的SUI代币（以`Balance<SUI>`的形式）。
//         2.  一个“闪电贷回执”（`FlashLoanReceipt<SUI>`），这个回执非常重要，之后还钱时需要用到它。
//     -   这个函数会进一步把返回的 `Balance<SUI>` 转换成一个 `Coin<SUI>` 对象（通过在PTB里添加一个`balance_to_coin`指令），这样更方便后续的套利操作使用这笔借来的钱。
//     -   最后，它把代表“借到的SUI币”和“闪电贷回执”的PTB参数打包成一个 `FlashResult` 对象返回。
//
// 5.  **`extend_repay_tx()` 方法 (添加偿还闪电贷指令到交易中)**:
//     -   这个函数与上面那个相对应，负责把“还钱”的指令添加到交易包里。
//     -   它会调用Navi合约的 `lending::flash_repay_with_ctx` 函数。
//     -   需要的参数包括Sui时钟、Navi的存储对象、SUI资金池对象、之前借款时拿到的那个“闪电贷回执”、以及你准备用来还钱的SUI代币（`coin_to_repay_arg`，这里需要的是 `Balance<SUI>` 形式，所以会先把传入的 `Coin<SUI>` 转一下）。
//     -   Navi的这个 `flash_repay_with_ctx` 函数执行后，会返回一个 `Balance<SUI>`，代表你还钱后多余的金额（如果你还多了的话，正常情况下应该是0）。这个函数也会把这个找零的余额转换回 `Coin<SUI>` 对象。
//
// **Sui区块链和DeFi相关的概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **借贷协议 (Lending Protocol)**:
//     DeFi领域的一大类应用。它们允许用户进行两种主要操作：
//     1.  **存款 (Supplying/Lending)**: 用户可以将自己的加密资产存入协议的资金池中，成为流动性提供者，并以此赚取利息。
//     2.  **借款 (Borrowing)**: 用户可以抵押自己存入的资产（或者在某些协议中，无需抵押但需支付更高利息），从协议的资金池中借出其他种类的资产。
//     Navi Protocol是Sui生态系统中的一个主流借贷协议。
//
// -   **闪电贷 (Flashloan)**:
//     一种特殊的、无抵押的贷款，但它有一个非常核心的约束：**借款和还款（包括本金和可能产生的手续费）必须在同一笔原子交易（Atomic Transaction）内完成**。
//     原子交易意味着这笔交易里的所有操作要么全部成功执行，要么如果其中任何一步失败（比如未能及时还款），则整个交易里的所有操作都会被自动撤销（回滚），就好像这笔交易从未发生过一样。
//     这个特性保证了借出方（在这里是Navi协议的资金池）的资金安全，因为钱要么被正确归还，要么就根本没有真正借出去。
//     闪电贷对于执行套利、快速清算抵押不足的头寸、或者在不同协议间进行复杂的资产重组等操作非常有用，因为它允许用户在极短的时间内（一笔交易的时间）调动大量资金，而无需自己实际拥有这些资金。
//
// -   **资金池 (Pool / Lending Pool)**:
//     在借贷协议中，用户存入的同一种资产（比如所有用户存入的SUI）会被汇集到一个特定的“资金池”里。当其他用户想要借这种资产时，就会从这个池子里借出。
//     借款人支付的利息会成为存款人的收益来源。每种被协议支持的代币类型通常都有一个自己独立的资金池对象。
//     `NAVI_POOL` 常量就指向Navi协议中SUI代币的资金池对象。
//
// -   **配置对象/存储对象 (Config / Storage Objects)**:
//     像Navi这样的复杂DeFi协议，通常会把它们的全局设置、每个资金池的详细状态（比如总存款、总借款、当前利率、支持的抵押因子等）、用户的账户数据（存了多少、借了多少）等重要信息，存储在链上的一些专门的智能合约对象中。
//     `NAVI_CONFIG` 和 `NAVI_STORAGE` 就是这类对象的ID。当我们要调用Navi合约的某个功能（比如发起闪电贷）时，合约可能需要读取这些配置或存储对象里的信息来正确执行操作，所以我们需要把这些对象的引用作为参数传给合约函数。

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::{str::FromStr, sync::Arc}; // FromStr用于从字符串转换, Arc原子引用计数
                                   // (FromStr for string conversion, Arc for atomic reference counting)

use eyre::{eyre, OptionExt, Result}; // 错误处理库 (Error handling library)
use simulator::Simulator; // 交易模拟器接口 (用于获取链上对象)
                         // (Transaction simulator interface (used for fetching on-chain objects))
use sui_sdk::SUI_COIN_TYPE; // SUI原生代币的类型字符串 ("0x2::sui::SUI")
                           // (Type string for SUI native coin ("0x2::sui::SUI"))
use sui_types::{
    base_types::ObjectID, // Sui对象ID类型 (Sui Object ID type)
    transaction::{Argument, Command, ObjectArg}, // Sui交易构建相关类型: Argument, Command, ObjectArg
                                                // (Sui transaction building related types: Argument, Command, ObjectArg)
    Identifier, TypeTag, SUI_CLOCK_OBJECT_ID, // Sui标识符, 类型标签, 时钟对象ID
                                              // (Sui Identifier, TypeTag, Clock Object ID)
};
use utils::object::shared_obj_arg; // 自定义工具库中用于创建共享对象参数的函数
                                   // (Function from custom utility library for creating shared object arguments)

use super::{trade::FlashResult, TradeCtx}; // 从父模块(defi)引入 FlashResult (闪电贷结果类型), TradeCtx (交易上下文)
                                         // (Import FlashResult (flash loan result type), TradeCtx (transaction context) from parent module (defi))

// --- Navi协议相关的常量定义 ---
// (Constant definitions related to Navi Protocol)
// Navi核心合约包ID (Package ID)
// (Navi core contract Package ID)
const NAVI_PROTOCOL: &str = "0x834a86970ae93a73faf4fff16ae40bdb72b91c47be585fff19a2af60a19ddca3";
// Navi SUI资金池对象ID。每个支持的资产在Navi中都有一个对应的池对象。
// (Navi SUI liquidity pool Object ID. Each supported asset in Navi has a corresponding pool object.)
const NAVI_POOL: &str = "0x96df0fce3c471489f4debaaa762cf960b3d97820bd1f3f025ff8190730e958c5";
// Navi全局配置对象ID (FlashLoanConfig / ProtocolConfig)
// (Navi global configuration Object ID (FlashLoanConfig / ProtocolConfig))
const NAVI_CONFIG: &str = "0x3672b2bf471a60c30a03325f104f92fb195c9d337ba58072dce764fe2aa5e2dc";
// Navi存储对象ID (Storage) - 可能包含用户账户数据、利率模型等状态。
// (Navi Storage Object ID - may contain user account data, interest rate model state, etc.)
const NAVI_STORAGE: &str = "0xbb4e2f4b6205c2e2a2db47aeb4f830796ec7c005f88537ee775986639bc442fe";

/// `Navi` 结构体 (Navi Struct)
///
/// 代表与Navi协议进行交互的实例，主要用于发起和偿还SUI的闪电贷。
/// (Represents an instance for interacting with the Navi protocol, primarily for initiating and repaying SUI flash loans.)
#[derive(Clone)] // 允许克隆 Navi 实例 (Allows cloning Navi instances)
pub struct Navi {
    sui_coin_type: TypeTag, // SUI代币的TypeTag，因为此实现专注于SUI闪电贷
                            // (TypeTag for SUI coin, as this implementation focuses on SUI flash loans)
    pool: ObjectArg,        // Navi SUI资金池对象的ObjectArg (ObjectArg for Navi SUI liquidity pool object)
    config: ObjectArg,      // Navi全局配置对象的ObjectArg (ObjectArg for Navi global config object)
    storage: ObjectArg,     // Navi存储对象的ObjectArg (ObjectArg for Navi storage object)
    clock: ObjectArg,       // Sui时钟对象的ObjectArg (ObjectArg for Sui clock object)
}

impl Navi {
    /// `new` 构造函数 (new constructor)
    ///
    /// 创建一个新的 `Navi` 实例。
    /// (Creates a new `Navi` instance.)
    /// 它会异步从链上获取Navi协议所需的关键对象（Pool, Config, Storage, Clock）
    /// 并将它们转换为 `ObjectArg` 格式以备后续构建交易时使用。
    /// (It asynchronously fetches key objects required by the Navi protocol (Pool, Config, Storage, Clock) from the chain
    ///  and converts them to `ObjectArg` format for subsequent use in transaction building.)
    /// 注意：对象的获取只在初始化时执行一次，以避免影响套利操作的性能。
    /// (Note: Object fetching is performed only once during initialization to avoid impacting the performance of arbitrage operations.)
    ///
    /// 参数 (Parameters):
    /// - `simulator`: 一个共享的模拟器实例 (`Arc<Box<dyn Simulator>>`)，用于从链上获取对象数据。
    ///                (A shared simulator instance (`Arc<Box<dyn Simulator>>`) for fetching object data from the chain.)
    ///
    /// 返回 (Returns):
    /// - `Result<Self>`: 成功则返回 `Navi` 实例，否则返回错误。(Returns a `Navi` instance if successful, otherwise an error.)
    pub async fn new(simulator: Arc<Box<dyn Simulator>>) -> Result<Self> {
        // 通过模拟器获取Navi的SUI资金池对象
        // (Fetch Navi's SUI liquidity pool object via simulator)
        let pool_obj = simulator
            .get_object(&ObjectID::from_hex_literal(NAVI_POOL)?) // 从十六进制字符串解析ObjectID (Parse ObjectID from hex string)
            .await
            .ok_or_eyre("Navi SUI资金池对象未找到 (Navi SUI liquidity pool object not found)")?; // 如果找不到对象，则返回错误 (If object not found, return an error)

        // 获取Navi的全局配置对象
        // (Fetch Navi's global configuration object)
        let config_obj = simulator
            .get_object(&ObjectID::from_hex_literal(NAVI_CONFIG)?)
            .await
            .ok_or_eyre("Navi配置对象未找到 (Navi config object not found)")?;

        // 获取Navi的存储对象
        // (Fetch Navi's storage object)
        let storage_obj = simulator
            .get_object(&ObjectID::from_hex_literal(NAVI_STORAGE)?)
            .await
            .ok_or_eyre("Navi存储对象未找到 (Navi storage object not found)")?;

        // 获取Sui系统的时钟对象 (这是一个标准的共享对象)
        // (Fetch Sui system's clock object (this is a standard shared object))
        let clock_obj = simulator
            .get_object(&SUI_CLOCK_OBJECT_ID)
            .await
            .ok_or_eyre("Sui时钟对象未找到 (Sui clock object not found)")?;

        Ok(Self {
            sui_coin_type: TypeTag::from_str(SUI_COIN_TYPE).unwrap(), // 将SUI类型字符串转换为TypeTag (Convert SUI type string to TypeTag)
            // 将获取到的SuiObject转换为ObjectArg。
            // (Convert the fetched SuiObject to ObjectArg.)
            // `shared_obj_arg` 会根据对象是否可变来创建合适的ObjectArg。
            // (`shared_obj_arg` creates the appropriate ObjectArg based on whether the object is mutable.)
            // Navi的Pool和Storage在操作中通常是可变的。Config和Clock通常是不可变的。
            // (Navi's Pool and Storage are usually mutable in operations. Config and Clock are usually immutable.)
            pool: shared_obj_arg(&pool_obj, true),     // Pool对象可变 (Pool object is mutable)
            config: shared_obj_arg(&config_obj, false),  // Config对象不可变 (Config object is immutable)
            storage: shared_obj_arg(&storage_obj, true), // Storage对象可变 (Storage object is mutable)
            clock: shared_obj_arg(&clock_obj, false),  // Clock对象不可变 (Clock object is immutable)
        })
    }

    /// `extend_flashloan_tx` 方法 (添加发起闪电贷指令到交易中 / Add flash loan initiation instruction to transaction method)
    ///
    /// 将发起Navi闪电贷的操作添加到现有的交易上下文 (`TradeCtx`) 中。
    /// (Adds the operation to initiate a Navi flash loan to the existing transaction context (`TradeCtx`).)
    /// 调用Navi的 `lending::flash_loan_with_ctx` 函数。
    /// (Calls Navi's `lending::flash_loan_with_ctx` function.)
    ///
    /// 合约方法签名示例 (来自注释) (Example contract method signature (from comments)):
    /// `public fun flash_loan_with_ctx<CoinType>(
    ///     config: &FlashLoanConfig, // Navi的全局配置对象 (Navi's global config object)
    ///     pool: &mut Pool<CoinType>,// 特定代币的资金池对象 (这里是SUI池) (Liquidity pool object for a specific coin (SUI pool here))
    ///     amount: u64,              // 希望借入的代币数量 (Amount of coin to borrow)
    ///     ctx: &mut TxContext       // 交易上下文 (由Sui运行时提供) (Transaction context (provided by Sui runtime))
    /// ): (Balance<CoinType>, FlashLoanReceipt<CoinType>)`
    /// 返回一个元组：借出的代币余额 (`Balance`) 和一个闪电贷回执 (`FlashLoanReceipt`)。
    /// (Returns a tuple: borrowed coin balance (`Balance`) and a flash loan receipt (`FlashLoanReceipt`).)
    ///
    /// 参数 (Parameters):
    /// - `ctx`: 可变的交易上下文 (`&mut TradeCtx`)，用于构建PTB。(Mutable transaction context (`&mut TradeCtx`) for building PTB.)
    /// - `amount_in`: 希望借入的SUI代币数量。(Amount of SUI coins to borrow.)
    ///
    /// 返回 (Returns):
    /// - `Result<FlashResult>`: 包含借出的SUI代币 (`coin_out`) 和闪电贷回执 (`receipt`) 的 `Argument`。
    ///                          (Contains `Argument`s for the borrowed SUI coin (`coin_out`) and flash loan receipt (`receipt`).)
    pub fn extend_flashloan_tx(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<FlashResult> {
        let package_id = ObjectID::from_hex_literal(NAVI_PROTOCOL)?; // Navi合约包ID (Navi contract package ID)
        let module_name = Identifier::new("lending").map_err(|e| eyre!(e))?; // `lending`模块 (`lending` module)
        let function_name = Identifier::new("flash_loan_with_ctx").map_err(|e| eyre!(e))?;
        // 泛型类型参数，因为我们只处理SUI闪电贷，所以是 `[0x2::sui::SUI]`
        // (Generic type parameter, since we only handle SUI flash loans, it's `[0x2::sui::SUI]`)
        let type_arguments = vec![self.sui_coin_type.clone()];

        // 构建调用参数列表 (Build the list of call arguments)
        let call_arguments = vec![
            ctx.obj(self.config).map_err(|e| eyre!(e))?,   // config: &FlashLoanConfig
            ctx.obj(self.pool).map_err(|e| eyre!(e))?,     // pool: &mut Pool<SUI>
            ctx.pure(amount_in).map_err(|e| eyre!(e))?,    // amount: u64
        ];

        // 向PTB中添加Move调用命令 (Add Move call command to PTB)
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));
        let last_idx = ctx.last_command_idx(); // 获取刚添加的命令的索引 (Get index of the just-added command)

        // `flash_loan_with_ctx` 返回一个元组 `(Balance<SUI>, FlashLoanReceipt<SUI>)`
        // (`flash_loan_with_ctx` returns a tuple `(Balance<SUI>, FlashLoanReceipt<SUI>)`)
        // 我们需要将元组的第一个元素 (Balance<SUI>) 转换为 Coin<SUI> 对象。
        // (We need to convert the first element of the tuple (Balance<SUI>) to a Coin<SUI> object.)
        let balance_out_arg = Argument::NestedResult(last_idx, 0); // 元组的第一个元素 (Balance) (First element of tuple (Balance))
        // `ctx.coin_from_balance` 会添加一个命令将Balance转换为Coin
        // (`ctx.coin_from_balance` will add a command to convert Balance to Coin)
        let coin_out_arg = ctx.coin_from_balance(balance_out_arg, self.sui_coin_type.clone())?;

        Ok(FlashResult {
            coin_out: coin_out_arg, // 借到的SUI代币 (作为Coin对象) (Borrowed SUI coin (as Coin object))
            receipt: Argument::NestedResult(last_idx, 1), // 元组的第二个元素 (FlashLoanReceipt) (Second element of tuple (FlashLoanReceipt))
            pool: None, // Navi的flash_loan不直接返回pool对象作为PTB结果，所以是None
                        // (Navi's flash_loan doesn't directly return the pool object as PTB result, so it's None)
                        // （与Cetus或FlowX不同，它们可能在FlashResult中传递pool的引用）
                        // (Unlike Cetus or FlowX, which might pass the pool's reference in FlashResult)
        })
    }

    /// `extend_repay_tx` 方法 (添加偿还闪电贷指令到交易中 / Add flash loan repayment instruction to transaction method)
    ///
    /// 将偿还Navi闪电贷的操作添加到现有的交易上下文 (`TradeCtx`) 中。
    /// (Adds the operation to repay a Navi flash loan to the existing transaction context (`TradeCtx`).)
    /// 调用Navi的 `lending::flash_repay_with_ctx` 函数。
    /// (Calls Navi's `lending::flash_repay_with_ctx` function.)
    ///
    /// 合约方法签名示例 (来自注释) (Example contract method signature (from comments)):
    /// `public fun flash_repay_with_ctx<CoinType>(
    ///     clock: &Clock,
    ///     storage: &mut Storage,
    ///     pool: &mut Pool<CoinType>,
    ///     receipt: FlashLoanReceipt<CoinType>, // 从 flash_loan_with_ctx 获取的回执 (Receipt from flash_loan_with_ctx)
    ///     repay_balance: Balance<CoinType>,   // 用于偿还的代币余额 (必须包含本金+费用) (Token balance for repayment (must include principal + fee))
    ///     ctx: &mut TxContext
    /// ): Balance<CoinType>`
    /// 返回一个 `Balance<CoinType>`，代表偿还后多余的金额 (如果支付的超过了应还款额，通常为0)。
    /// (Returns a `Balance<CoinType>` representing the excess amount after repayment (usually 0 if paid more than due).)
    ///
    /// 参数 (Parameters):
    /// - `ctx`: 可变的交易上下文 (`&mut TradeCtx`)。(Mutable transaction context (`&mut TradeCtx`).)
    /// - `coin_to_repay_arg`: 代表用于偿还的SUI代币的 `Argument` (必须包含本金+费用)。
    ///                        (`Argument` representing the SUI coin for repayment (must include principal + fee).)
    /// - `flash_res`: 从 `extend_flashloan_tx` 返回的 `FlashResult`，主要使用其中的 `receipt`。
    ///                (`FlashResult` returned from `extend_flashloan_tx`, primarily its `receipt` is used.)
    ///
    /// 返回 (Returns):
    /// - `Result<Argument>`: 代表偿还后多余的SUI代币 (作为Coin对象)。
    ///                       (`Argument` representing the excess SUI coin after repayment (as Coin object).)
    pub fn extend_repay_tx(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, flash_res: FlashResult) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(NAVI_PROTOCOL)?;
        let module_name = Identifier::new("lending").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new("flash_repay_with_ctx").map_err(|e| eyre!(e))?;
        // 泛型类型参数，仍然是SUI (Generic type parameter, still SUI)
        let type_arguments = vec![self.sui_coin_type.clone()];

        // 将用于偿还的 `Coin<SUI>` (coin_to_repay_arg) 转换为 `Balance<SUI>`
        // (Convert the `Coin<SUI>` for repayment (coin_to_repay_arg) to `Balance<SUI>`)
        let repay_balance_arg = ctx.coin_into_balance(coin_to_repay_arg, self.sui_coin_type.clone())?;

        // 构建调用参数列表 (Build the list of call arguments)
        let call_arguments = vec![
            ctx.obj(self.clock).map_err(|e| eyre!(e))?,    // clock: &Clock
            ctx.obj(self.storage).map_err(|e| eyre!(e))?,  // storage: &mut Storage
            ctx.obj(self.pool).map_err(|e| eyre!(e))?,     // pool: &mut Pool<SUI>
            flash_res.receipt,                             // receipt: FlashLoanReceipt<SUI>
            repay_balance_arg,                             // repay_balance: Balance<SUI>
        ];

        // 向PTB中添加Move调用命令 (Add Move call command to PTB)
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));
        let last_idx = ctx.last_command_idx(); // 获取刚添加的命令的索引 (Get index of the just-added command)

        // `flash_repay_with_ctx` 返回一个 `Balance<SUI>` (代表找零)
        // (`flash_repay_with_ctx` returns a `Balance<SUI>` (representing change))
        let remaining_balance_arg = Argument::Result(last_idx);
        // 将这个找零的Balance转换为Coin对象
        // (Convert this change Balance to a Coin object)
        let remaining_coin_arg = ctx.coin_from_balance(remaining_balance_arg, self.sui_coin_type.clone())?;

        Ok(remaining_coin_arg) // 返回代表找零的Coin对象的Argument (Return Argument representing the change Coin object)
    }
}

[end of bin/arb/src/defi/navi.rs]
