// 该文件 `kriya_clmm.rs` 实现了与 KriyaDEX 协议的 CLMM (集中流动性做市商) 池交互的逻辑。
// KriyaDEX 是 Sui 上的一个DEX，同时提供传统AMM池和CLMM池。此文件专注于CLMM部分。
// CLMM允许流动性提供者将资金集中在特定的价格区间内，以提高资本效率。
// 此实现也包含了对Kriya CLMM闪电贷功能的支持。
//
// **文件概览 (File Overview)**:
// 这个 `kriya_clmm.rs` 文件是专门用来和Sui区块链上的KriyaDEX协议的“集中流动性做市商”（CLMM）池子打交道的代码。
// KriyaDEX本身可能同时有老式的AMM池子和这种新式的CLMM池子，这个文件只管CLMM这种。
// CLMM的核心思想和Cetus、FlowX等协议类似，都是让提供流动性的人（LP）可以把钱更精确地放到他们认为最划算的价格范围，而不是平均分配。
// 这个文件里的代码也实现了对Kriya CLMM池子“闪电贷”功能的支持。
// (This `kriya_clmm.rs` file contains code specifically for interacting with the "Concentrated Liquidity Market Maker" (CLMM) pools of the KriyaDEX protocol on the Sui blockchain.
//  KriyaDEX itself might offer both traditional AMM pools and these newer CLMM pools; this file only deals with the CLMM type.
//  The core idea of CLMM is similar to protocols like Cetus and FlowX, allowing liquidity providers (LPs) to place their funds more precisely within price ranges they deem most profitable, rather than spreading them out evenly.
//  The code in this file also implements support for the "flash loan" functionality of Kriya CLMM pools.)
//
// **主要内容 (Main Contents)**:
// 1.  **常量定义 (Constant Definitions)**:
//     -   `KRIYA_CLMM`: Kriya CLMM核心智能合约的“门牌号”（Package ID）。
//     -   `VERSION`: Kriya CLMM可能用到的一个“版本控制对象”的ID。有些协议会用这样一个全局对象来管理合约的升级和版本信息。
//
// 2.  **`ObjectArgs` 结构体与 `OBJ_CACHE`**:
//     -   和Cetus文件里类似，`ObjectArgs` 用来打包缓存一些常用的对象引用，这里主要是上面提到的 `VERSION` 对象和Sui系统时钟对象。
//     -   `OBJ_CACHE` 同样是一个一次性初始化并全局共享的缓存，用来提高获取这些对象引用的效率。
//
// 3.  **`KriyaClmm` 结构体**:
//     -   代表Kriya CLMM协议里的一个具体的交易池实例。
//     -   包含了与该池交互所需的信息，如原始池信息、池对象的引用、流动性、代币类型、调用合约所需的类型参数，以及从缓存中获取的共享对象参数（`version`, `clock`）。
//     -   它也实现了项目内部定义的 `Dex` 通用接口。
//
// 4.  **`new()` 构造函数**:
//     -   异步方法，根据从`dex_indexer`获取的池信息和指定的输入代币类型来初始化一个 `KriyaClmm` 实例。
//     -   它会解析池对象的链上数据，提取流动性等信息。
//
// 5.  **常规交换相关方法 (Regular Swap Methods)**:
//     -   `build_swap_tx()` / `build_swap_args()`: 构建普通代币交换所需的交易参数和PTB（可编程交易块）指令。
//     -   一个值得注意的细节是，这里的常规交换 (`extend_trade_tx`) **也使用了 `CETUS_AGGREGATOR` 的包ID**。
//         这意味着，即便是和Kriya CLMM池子进行常规交换，实际的链上调用也可能是通过Cetus协议提供的一个“聚合器”（Aggregator）智能合约来路由的。
//         这个聚合器可能支持与多个不同DEX协议的池子进行交互，包括Kriya CLMM。
//         (A noteworthy detail is that regular swaps (`extend_trade_tx`) here **also use the `CETUS_AGGREGATOR` package ID**.
//          This implies that even for regular swaps with Kriya CLMM pools, the actual on-chain calls might be routed through an "Aggregator" smart contract provided by the Cetus protocol.
//          This aggregator might support interactions with pools from multiple different DEX protocols, including Kriya CLMM.)
//
// 6.  **闪电贷相关方法 (Flashloan Methods)**:
//     -   `build_flashloan_args()`: 准备调用Kriya CLMM发起闪电贷的合约函数（在Kriya CLMM自己的 `trade` 模块里，名为 `flash_swap`）时需要的参数。
//     -   `build_repay_args()`: 准备调用Kriya CLMM偿还闪电贷的合约函数（`trade::repay_flash_swap`）时需要的参数。
//     -   `extend_flashloan_tx()`: 实现了 `Dex` 接口，将发起Kriya CLMM闪电贷的指令添加到PTB中。
//     -   `extend_repay_tx()`: 实现了 `Dex` 接口，将偿还Kriya CLMM闪电贷的指令添加到PTB中。
//     -   `support_flashloan()`: 返回 `true`，明确表示Kriya CLMM支持闪电贷。
//
// 7.  **`Dex` trait 实现 (Implementation of `Dex` Trait)**:
//     -   `KriyaClmm` 结构体同样实现了 `Dex` 接口要求的其他方法，如 `coin_in_type()`, `coin_out_type()`, `protocol()`, `liquidity()`, `object_id()`, `flip()`, `is_a2b()`。
//
// **Sui区块链和DeFi相关的概念解释 (Relevant Sui Blockchain and DeFi Concepts Explained)**:
//
// -   **CLMM (Concentrated Liquidity Market Maker / 集中流动性做市商)**:
//     与Cetus文件中的解释相同。CLMM允许更高效的资金利用和更好的交易价格。
//     (Same explanation as in the Cetus file. CLMM allows for more efficient capital utilization and better trading prices.)
//
// -   **Version Object (版本对象 / Version Object)**:
//     一些DeFi协议可能会在链上部署一个全局的“版本对象”或“配置对象”。这个对象存储了关于当前协议版本、重要合约地址、全局参数等信息。
//     当协议升级其智能合约时，可以通过更新这个版本对象来指向新的合约地址或参数，而依赖该协议的应用则可以读取这个版本对象来获取最新的正确配置。
//     KriyaDEX CLMM 可能就使用了这样一个对象（由 `VERSION` 常量指定其ID）。
//     (Some DeFi protocols might deploy a global "version object" or "config object" on-chain. This object stores information about the current protocol version, important contract addresses, global parameters, etc.
//      When the protocol upgrades its smart contracts, it can update this version object to point to new contract addresses or parameters, and applications relying on the protocol can read this version object to get the latest correct configuration.
//      KriyaDEX CLMM might use such an object (its ID specified by the `VERSION` constant).)
//
// -   **Flashloan (闪电贷 / Flashloan)**:
//     与Cetus文件中的解释相同。Kriya CLMM也支持这种强大的DeFi功能。
//     (Same explanation as in the Cetus file. Kriya CLMM also supports this powerful DeFi feature.)

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::sync::Arc; // 原子引用计数 (Atomic Reference Counting)

use dex_indexer::types::{Pool, Protocol}; // 从 `dex_indexer` 引入Pool和Protocol类型 (Import Pool and Protocol types from `dex_indexer`)
use eyre::{ensure, eyre, OptionExt, Result}; // 错误处理库 (Error handling library)
use move_core_types::annotated_value::MoveStruct; // Move核心类型 (Move core types)
use simulator::Simulator; // 交易模拟器接口 (Transaction simulator interface)
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型 (Sui basic types)
    transaction::{Argument, Command, ObjectArg, ProgrammableTransaction, TransactionData}, // Sui交易构建类型 (Sui transaction building types)
    Identifier, TypeTag, SUI_CLOCK_OBJECT_ID, // Sui标识符, 类型标签, 时钟对象ID (Sui Identifier, TypeTag, Clock Object ID)
};
use tokio::sync::OnceCell; // Tokio异步单次初始化单元 (Tokio asynchronous single initialization cell)
use utils::{
    coin, new_test_sui_client, // 自定义工具库: coin操作, 创建Sui客户端 (Custom utility library: coin operations, create Sui client)
    object::{extract_u128_from_move_struct, shared_obj_arg}, // 对象处理工具 (Object handling tools)
};

use super::{trade::FlashResult, TradeCtx, CETUS_AGGREGATOR}; // 从父模块(defi)引入 FlashResult, TradeCtx, CETUS_AGGREGATOR
                                                            // (Import FlashResult, TradeCtx, CETUS_AGGREGATOR from parent module (defi))
use crate::{config::*, defi::Dex}; // 从当前crate引入配置和 Dex trait (Import config and Dex trait from current crate)

// --- Kriya CLMM 协议相关的常量定义 ---
// (Constant definitions related to Kriya CLMM protocol)
// Kriya CLMM核心合约包ID (Kriya CLMM core contract package ID)
const KRIYA_CLMM: &str = "0xbd8d4489782042c6fafad4de4bc6a5e0b84a43c6c00647ffd7062d1e2bb7549e";
// Kriya CLMM 版本对象ID (Version) (Kriya CLMM Version Object ID)
const VERSION: &str = "0xf5145a7ac345ca8736cf8c76047d00d6d378f30e81be6f6eb557184d9de93c78";

/// `ObjectArgs` 结构体 (对象参数结构体 / Object Arguments Struct)
///
/// 缓存Kriya CLMM交互所需的关键对象的 `ObjectArg` 形式。
/// (Caches the `ObjectArg` form of key objects required for Kriya CLMM interaction.)
#[derive(Clone)]
pub struct ObjectArgs {
    version: ObjectArg, // 版本对象的ObjectArg (Version object's ObjectArg)
    clock: ObjectArg,   // Sui时钟对象的ObjectArg (Sui clock object's ObjectArg)
}

// 用于缓存 `ObjectArgs` 的静态 `OnceCell` (Static `OnceCell` for caching `ObjectArgs`)
static OBJ_CACHE: OnceCell<ObjectArgs> = OnceCell::const_new();

/// `get_object_args` 异步函数 (获取对象参数函数 / Get Object Arguments Function)
///
/// 获取并缓存 `ObjectArgs` (包含version, clock)。
/// (Fetches and caches `ObjectArgs` (containing version, clock).)
async fn get_object_args(simulator: Arc<Box<dyn Simulator>>) -> ObjectArgs {
    OBJ_CACHE
        .get_or_init(|| async {
            let version_id = ObjectID::from_hex_literal(VERSION).unwrap();
            // 通过模拟器获取对象信息 (Fetch object information via simulator)
            let version_obj = simulator.get_object(&version_id).await.unwrap();
            let clock_obj = simulator.get_object(&SUI_CLOCK_OBJECT_ID).await.unwrap();

            ObjectArgs {
                version: shared_obj_arg(&version_obj, false), // Version对象通常是不可变的 (Version object is usually immutable)
                clock: shared_obj_arg(&clock_obj, false),   // Clock是不可变的 (Clock is immutable)
            }
        })
        .await
        .clone()
}

/// `KriyaClmm` 结构体 (KriyaClmm Struct)
///
/// 代表一个KriyaDEX的CLMM交易池。
/// (Represents a KriyaDEX CLMM trading pool.)
#[derive(Clone)]
pub struct KriyaClmm {
    pool: Pool,              // 从 `dex_indexer` 获取的原始池信息 (Original pool information from `dex_indexer`)
    pool_arg: ObjectArg,     // 池对象本身的 `ObjectArg` (The pool object's own `ObjectArg`)
    liquidity: u128,         // 池的流动性 (Pool's liquidity)
    coin_in_type: String,    // 当前交易方向的输入代币类型 (Input coin type for the current trading direction)
    coin_out_type: String,   // 当前交易方向的输出代币类型 (Output coin type for the current trading direction)
    type_params: Vec<TypeTag>,// 调用合约时需要的泛型类型参数 (通常是[CoinA, CoinB])
                              // (Generic type parameters needed when calling the contract (usually [CoinA, CoinB]))
    // 共享的对象参数 (Shared object parameters)
    version: ObjectArg,
    clock: ObjectArg,
}

impl KriyaClmm {
    /// `new` 构造函数 (new constructor)
    ///
    /// 根据 `dex_indexer` 提供的 `Pool` 信息和输入代币类型，创建 `KriyaClmm` DEX实例。
    /// (Creates a `KriyaClmm` DEX instance based on `Pool` information provided by `dex_indexer` and the input coin type.)
    ///
    /// 参数 (Parameters):
    /// - `simulator`: 共享的模拟器实例。(Shared simulator instance.)
    /// - `pool_info`: 从 `dex_indexer` 获取的池信息 (`&Pool`)。(Pool information from `dex_indexer` (`&Pool`).)
    /// - `coin_in_type`: 输入代币的类型字符串。(Type string of the input coin.)
    ///
    /// 返回 (Returns):
    /// - `Result<Self>`: 成功则返回 `KriyaClmm` 实例，否则返回错误。(Returns a `KriyaClmm` instance if successful, otherwise an error.)
    pub async fn new(simulator: Arc<Box<dyn Simulator>>, pool_info: &Pool, coin_in_type: &str) -> Result<Self> {
        // 确保池协议是KriyaClmm (Ensure pool protocol is KriyaClmm)
        ensure!(pool_info.protocol == Protocol::KriyaClmm, "提供的不是Kriya CLMM协议的池 (Provided pool is not of Kriya CLMM protocol)");

        // 获取并解析池对象的Move结构体内容 (Get and parse the Move struct content of the pool object)
        let pool_obj = simulator
            .get_object(&pool_info.pool) // pool_info.pool 是池的ObjectID (pool_info.pool is the pool's ObjectID)
            .await
            .ok_or_else(|| eyre!("Kriya CLMM池对象未找到: {} (Kriya CLMM pool object not found: {})", pool_info.pool))?;

        let parsed_pool_struct = {
            let layout = simulator
                .get_object_layout(&pool_info.pool)
                .ok_or_eyre("Kriya CLMM池对象的布局(layout)未找到 (Layout for Kriya CLMM pool object not found)")?;
            let move_obj = pool_obj.data.try_as_move().ok_or_eyre("对象不是Move对象 (Object is not a Move object)")?;
            MoveStruct::simple_deserialize(move_obj.contents(), &layout).map_err(|e| eyre!(e))?
        };

        // 从解析后的池结构体中提取流动性 (liquidity 字段)
        // (Extract liquidity from the parsed pool struct (liquidity field))
        let liquidity = extract_u128_from_move_struct(&parsed_pool_struct, "liquidity")?;

        // 根据输入代币推断输出代币 (假设是双币池)
        // (Infer output coin based on input coin (assuming a two-coin pool))
        let coin_out_type = if pool_info.token0_type() == coin_in_type {
            pool_info.token1_type().to_string()
        } else {
            pool_info.token0_type().to_string()
        };

        // 获取池本身的泛型类型参数，这通常是池中包含的两种代币的类型。
        // (Get the generic type parameters of the pool itself, which are usually the types of the two coins in the pool.)
        // 例如 `Pool<CoinTypeA, CoinTypeB>` 中的 `CoinTypeA, CoinTypeB`。
        // (E.g., `CoinTypeA, CoinTypeB` in `Pool<CoinTypeA, CoinTypeB>`.)
        let type_params = parsed_pool_struct.type_.type_params.clone();

        // 将池对象转换为 `ObjectArg` (在交易中通常是可变的)
        // (Convert the pool object to `ObjectArg` (usually mutable in transactions))
        let pool_arg = shared_obj_arg(&pool_obj, true);
        // 获取共享的协议对象参数 (version, clock)
        // (Get shared protocol object parameters (version, clock))
        let ObjectArgs { version, clock } = get_object_args(simulator).await;

        Ok(Self {
            pool: pool_info.clone(),
            liquidity,
            coin_in_type: coin_in_type.to_string(),
            coin_out_type,
            type_params, // 通常是 [TokenTypeA, TokenTypeB] (Usually [TokenTypeA, TokenTypeB])
            pool_arg,
            version,
            clock,
        })
    }

    /// `build_swap_tx` (私有辅助函数 / Private helper function)
    ///
    /// 构建一个完整的Sui可编程交易 (PTB)，用于在Kriya CLMM池中执行一次常规交换。
    /// (Builds a complete Sui Programmable Transaction (PTB) for executing a regular swap in a Kriya CLMM pool.)
    #[allow(dead_code)] // 允许存在未使用的代码 (Allow unused code)
    async fn build_swap_tx(
        &self,
        sender: SuiAddress,
        recipient: SuiAddress,
        coin_in_ref: ObjectRef,
        amount_in: u64,
    ) -> Result<ProgrammableTransaction> {
        let mut ctx = TradeCtx::default();

        let coin_in_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        // `None` 表示 `amount_in` 对于 `extend_trade_tx` 是可选的或不直接使用u64值
        // (`None` indicates `amount_in` is optional for `extend_trade_tx` or a u64 value is not directly used)
        // (Kriya CLMM的swap函数可能直接使用传入Coin对象的全部余额)。
        // (Kriya CLMM's swap function might directly use the entire balance of the passed Coin object.)
        let coin_out_arg = self.extend_trade_tx(&mut ctx, sender, coin_in_arg, None).await?;
        ctx.transfer_arg(recipient, coin_out_arg);

        Ok(ctx.ptb.finish())
    }

    /// `build_swap_args` (私有辅助函数 / Private helper function)
    ///
    /// 构建调用Kriya CLMM常规交换方法 (如聚合器中的 `kriya_clmm::swap_a2b`) 所需的参数列表。
    /// (Builds the argument list required for calling Kriya CLMM regular swap methods (e.g., `kriya_clmm::swap_a2b` in an aggregator).)
    /// 聚合器中的函数签名可能类似于 (The function signature in an aggregator might be similar to):
    /// `fun swap_a2b<CoinA, CoinB>(pool: &mut Pool<CoinA, CoinB>, coin_a: Coin<CoinA>, version: &Version, clock: &Clock, ctx: &mut TxContext): Coin<CoinB>`
    /// 参数包括: pool, 输入的coin对象, version对象, clock对象。
    /// (Arguments include: pool, input coin object, version object, clock object.)
    fn build_swap_args(&self, ctx: &mut TradeCtx, coin_in_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?;
        let version_arg = ctx.obj(self.version).map_err(|e| eyre!(e))?;
        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;

        // 返回参数列表，顺序必须与聚合器中 kriya_clmm 模块的 swap_a2b/swap_b2a 函数签名一致。
        // (Return the argument list; order must strictly match the swap_a2b/swap_b2a function signature in the aggregator's kriya_clmm module.)
        Ok(vec![pool_arg, coin_in_arg, version_arg, clock_arg])
    }

    /// `build_flashloan_args` (私有辅助函数 / Private helper function)
    ///
    /// 构建调用Kriya CLMM发起闪电贷方法 (`trade::flash_swap`) 所需的参数列表。
    /// (Builds the argument list required for calling Kriya CLMM's flash loan initiation method (`trade::flash_swap`).)
    /// 合约方法签名示例 (来自注释) (Example contract method signature (from comments)):
    /// `public fun flash_swap<T0, T1>(
    ///     _pool: &mut Pool<T0, T1>,
    ///     _a2b: bool,              // 交易方向 (true表示T0->T1, 即借T0换T1) (Trade direction (true for T0->T1, i.e., borrow T0 swap for T1))
    ///     _by_amount_in: bool,     // true表示 `_amount` 是输入数量 (要借的数量) (true means `_amount` is input amount (amount to borrow))
    ///     _amount: u64,            // 数量 (Amount)
    ///     _sqrt_price_limit: u128, // 价格限制 (Price limit)
    ///     _clock: &Clock,
    ///     _version: &Version,
    ///     _ctx: &TxContext
    /// ) : (Balance<T0>, Balance<T1>, FlashSwapReceipt)`
    fn build_flashloan_args(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?; // 可变的池对象引用 (Mutable pool object reference)
        let a2b_arg = ctx.pure(self.is_a2b()).map_err(|e| eyre!(e))?; // 交易方向 (Trade direction)
        let by_amount_in_arg = ctx.pure(true).map_err(|e| eyre!(e))?; // 按输入数量计算 (Calculate by input amount)
        let amount_arg = ctx.pure(amount_in).map_err(|e| eyre!(e))?; // 借贷/输入数量 (Loan/input amount)

        // 设置价格限制 (sqrt_price_limit)。
        // (Set price limit (sqrt_price_limit).)
        // 对于闪电贷，如果只是单纯借款而不关心虚拟交换的价格，可以设置一个较宽松的限制。
        // (For flash loans, if purely borrowing without concern for virtual swap price, a looser limit can be set.)
        // Kriya CLMM的 `flash_swap` 似乎也执行一个虚拟的swap来计算费用或确定债务。
        // (Kriya CLMM's `flash_swap` seems to also perform a virtual swap to calculate fees or determine debt.)
        // `MIN_SQRT_PRICE_X64` for a2b, `MAX_SQRT_PRICE_X64` for b2a.
        // (This indicates allowing price to reach extreme ends, as the main purpose is borrowing.)
        let sqrt_price_limit_val = if self.is_a2b() {
            MIN_SQRT_PRICE_X64 // 借 T0 (a), 换 T1 (b)。价格是 b/a。允许价格到最小。
                               // (Borrow T0 (a), swap for T1 (b). Price is b/a. Allow price to minimum.)
        } else {
            MAX_SQRT_PRICE_X64 // 借 T1 (b), 换 T0 (a)。价格是 a/b。允许价格到最大。
                               // (Borrow T1 (b), swap for T0 (a). Price is a/b. Allow price to maximum.)
        };
        let sqrt_price_limit_arg = ctx.pure(sqrt_price_limit_val).map_err(|e| eyre!(e))?;

        let clock_arg = ctx.obj(self.clock).map_err(|e| eyre!(e))?;
        let version_arg = ctx.obj(self.version).map_err(|e| eyre!(e))?;

        Ok(vec![
            pool_arg,
            a2b_arg,
            by_amount_in_arg,
            amount_arg,
            sqrt_price_limit_arg,
            clock_arg,
            version_arg,
        ])
    }

    /// `build_repay_args` (私有辅助函数 / Private helper function)
    ///
    /// 构建调用Kriya CLMM偿还闪电贷方法 (`trade::repay_flash_swap`) 所需的参数列表。
    /// (Builds the argument list required for calling Kriya CLMM's flash loan repayment method (`trade::repay_flash_swap`).)
    /// 合约方法签名示例 (来自注释) (Example contract method signature (from comments)):
    /// `public fun repay_flash_swap<T0, T1>(
    ///     _pool: &mut Pool<T0, T1>,
    ///     _receipt: FlashSwapReceipt,
    ///     _balance_a: Balance<T0>, // 用于偿还的T0代币余额 (T0 token balance for repayment)
    ///     _balance_b: Balance<T1>, // 用于偿还的T1代币余额 (T1 token balance for repayment)
    ///     _version: &Version,
    ///     _ctx: &TxContext
    /// )`
    /// 在闪电贷中，通常只提供借入方向的代币余额进行偿还。
    /// (In flash loans, usually only the token balance of the borrowed direction is provided for repayment.)
    fn build_repay_args(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, receipt_arg: Argument) -> Result<Vec<Argument>> {
        let pool_arg = ctx.obj(self.pool_arg).map_err(|e| eyre!(e))?; // 可变的池对象引用 (Mutable pool object reference)

        // 根据交易方向，将 `coin_to_repay_arg` 转换为相应类型的 `Balance` 对象。
        // (Based on the trade direction, convert `coin_to_repay_arg` to the appropriate `Balance` object type.)
        // 另一个方向的 Balance 则为空 (zero balance)。
        // (The Balance for the other direction will be zero.)
        // T0是type_params[0], T1是type_params[1] (T0 is type_params[0], T1 is type_params[1])
        let (balance_a_arg, balance_b_arg) = if self.is_a2b() {
            // 如果是 a2b (借T0/CoinA, 得到T1/CoinB), 那么偿还的是T0/CoinA。
            // (If a2b (borrow T0/CoinA, get T1/CoinB), then T0/CoinA is repaid.)
            // `coin_to_repay_arg` 应该是 `Coin<T0>`。
            // (`coin_to_repay_arg` should be `Coin<T0>`.)
            (
                ctx.coin_into_balance(coin_to_repay_arg, self.type_params[0].clone())?, // coin_to_repay是T0类型 (coin_to_repay is T0 type)
                ctx.balance_zero(self.type_params[1].clone())?,                     // T1的Balance为空 (T1's Balance is zero)
            )
        } else {
            // 如果是 b2a (借T1/CoinB, 得到T0/CoinA), 那么偿还的是T1/CoinB。
            // (If b2a (borrow T1/CoinB, get T0/CoinA), then T1/CoinB is repaid.)
            // `coin_to_repay_arg` 应该是 `Coin<T1>`。
            // (`coin_to_repay_arg` should be `Coin<T1>`.)
            (
                ctx.balance_zero(self.type_params[0].clone())?,                     // T0的Balance为空 (T0's Balance is zero)
                ctx.coin_into_balance(coin_to_repay_arg, self.type_params[1].clone())?, // coin_to_repay是T1类型 (coin_to_repay is T1 type)
            )
        };

        let version_arg = ctx.obj(self.version).map_err(|e| eyre!(e))?;
        Ok(vec![pool_arg, receipt_arg, balance_a_arg, balance_b_arg, version_arg])
    }
}

/// 为 `KriyaClmm` 结构体实现 `Dex` trait。
/// (Implement `Dex` trait for `KriyaClmm` struct.)
#[async_trait::async_trait]
impl Dex for KriyaClmm {
    /// `support_flashloan` 方法 (support_flashloan method)
    ///
    /// 指明该DEX是否支持闪电贷。Kriya CLMM是支持的。
    /// (Indicates if this DEX supports flash loans. Kriya CLMM does.)
    fn support_flashloan(&self) -> bool {
        true
    }

    /// `extend_flashloan_tx` (将发起Kriya CLMM闪电贷的操作添加到PTB中 / Add Kriya CLMM flash loan initiation op to PTB)
    ///
    /// Kriya CLMM的闪电贷通过其 `trade::flash_swap` 函数实现。
    /// (Kriya CLMM's flash loan is implemented via its `trade::flash_swap` function.)
    ///
    /// 返回 (Returns):
    /// - `Result<FlashResult>`: 包含借出的代币 (`coin_out`) 和闪电贷回执 (`receipt`)。
    ///                          (Contains the borrowed coin (`coin_out`) and flash loan receipt (`receipt`).)
    ///   `coin_out` 是指通过闪电贷借入并立即进行虚拟交换后得到的“目标代币”。
    ///   (`coin_out` refers to the "target coin" obtained after borrowing via flash loan and immediately performing a virtual swap.)
    async fn extend_flashloan_tx(&self, ctx: &mut TradeCtx, amount_to_borrow: u64) -> Result<FlashResult> {
        let package_id = ObjectID::from_hex_literal(KRIYA_CLMM)?; // Kriya CLMM包ID (Kriya CLMM package ID)
        let module_name = Identifier::new("trade").map_err(|e| eyre!(e))?; // `trade`模块 (`trade` module)
        let function_name = Identifier::new("flash_swap").map_err(|e| eyre!(e))?;
        // 泛型参数是池的两种代币类型 `[CoinA, CoinB]`
        // (Generic parameters are the two coin types of the pool `[CoinA, CoinB]`)
        let type_arguments = self.type_params.clone();
        let call_arguments = self.build_flashloan_args(ctx, amount_to_borrow)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx(); // `flash_swap` 命令的索引 (Index of the `flash_swap` command)

        // `flash_swap` 返回 `(Balance<T0>, Balance<T1>, FlashSwapReceipt)`
        // (`flash_swap` returns `(Balance<T0>, Balance<T1>, FlashSwapReceipt)`)
        // T0是type_params[0], T1是type_params[1]
        // (T0 is type_params[0], T1 is type_params[1])
        // 根据 `is_a2b` 判断哪个Balance是0 (对应借入的币种的初始余额，在swap后变0或剩余手续费)
        // 哪个Balance是实际交换得到的币种。
        // (Based on `is_a2b`, determine which Balance is 0 (corresponding to the initial balance of the borrowed coin, which becomes 0 or remaining fee after swap)
        //  and which Balance is the coin actually obtained from the swap.)
        let balance_t0_arg = Argument::NestedResult(last_idx, 0);
        let balance_t1_arg = Argument::NestedResult(last_idx, 1);
        let receipt_arg = Argument::NestedResult(last_idx, 2); // 闪电贷回执 (Flash loan receipt)

        // 如果 is_a2b() (借T0换T1): (If is_a2b() (borrow T0 swap for T1):)
        //   - `balance_t0_arg` 是 T0 的剩余/债务余额 (通常为0，或手续费部分)
        //     (`balance_t0_arg` is T0's remaining/debt balance (usually 0, or fee part))
        //   - `balance_t1_arg` 是交换后得到的 T1 余额 (这是我们用于后续操作的 `coin_out`)
        //     (`balance_t1_arg` is T1 balance obtained after swap (this is our `coin_out` for subsequent operations))
        //   - `coin_in_type_for_flash_result` 是 T0, `coin_out_type_for_flash_result` 是 T1
        let (zero_balance_arg, target_balance_arg, _original_borrow_coin_type, target_coin_type_tag) = if self.is_a2b() {
            (balance_t0_arg, balance_t1_arg, self.type_params[0].clone(), self.type_params[1].clone())
        } else {
            // b2a (借T1换T0) (b2a (borrow T1 swap for T0))
            (balance_t1_arg, balance_t0_arg, self.type_params[1].clone(), self.type_params[0].clone())
        };

        // 销毁那个零余额的Balance对象 (对应原始借入代币在swap后的剩余，通常是0)
        // (Destroy that zero-balance Balance object (corresponding to the remainder of the originally borrowed coin after swap, usually 0))
        let zero_balance_coin_type_tag = if self.is_a2b() { self.type_params[0].clone() } else { self.type_params[1].clone() };
        ctx.balance_destroy_zero(zero_balance_arg, zero_balance_coin_type_tag)?;

        // 将目标代币的Balance转换为Coin对象
        // (Convert the target coin's Balance to a Coin object)
        let final_coin_out_arg = ctx.coin_from_balance(target_balance_arg, target_coin_type_tag)?;

        Ok(FlashResult {
            coin_out: final_coin_out_arg, // 这是通过闪电贷借入并交换后得到的“目标代币”
                                          // (This is the "target coin" obtained after borrowing via flash loan and swapping)
            receipt: receipt_arg,         // 闪电贷回执，用于偿还原始借入的代币
                                          // (Flash loan receipt, used for repaying the originally borrowed coin)
            pool: None,                   // Kriya的flash_swap不直接返回pool对象作为PTB结果
                                          // (Kriya's flash_swap does not directly return the pool object as part of PTB result)
        })
    }

    /// `extend_repay_tx` (将偿还Kriya CLMM闪电贷的操作添加到PTB中 / Add Kriya CLMM flash loan repayment op to PTB)
    ///
    /// Kriya CLMM的闪电贷偿还通过其 `trade::repay_flash_swap` 函数实现。
    /// (Kriya CLMM's flash loan repayment is implemented via its `trade::repay_flash_swap` function.)
    ///
    /// **步骤 (Steps)**: (详情见上方中文总览 / See Chinese overview above for details)
    /// 1. 获取闪电贷回执。(Get flash loan receipt.)
    /// 2. (可选) 从回执获取确切债务 (Kriya的 `repay_flash_swap` 直接处理余额)。
    ///    ((Optional) Get exact debt from receipt (Kriya's `repay_flash_swap` handles balances directly).)
    /// 3. 调用 `trade::repay_flash_swap`。(Call `trade::repay_flash_swap`.)
    ///
    /// 返回 (Returns):
    /// - `Result<Argument>`: 偿还后多余的代币 (Kriya不返回，所以返回原输入)。
    ///                      (Excess coins after repayment (Kriya doesn't return, so original input is returned).)
    async fn extend_repay_tx(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, flash_res: FlashResult) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(KRIYA_CLMM)?;
        let module_name = Identifier::new("trade").map_err(|e| eyre!(e))?;
        let receipt_arg = flash_res.receipt; // 从FlashResult中获取回执 (Get receipt from FlashResult)

        // 从 `coin_to_repay_arg` (用于偿还的总金额) 中分割出确切的债务金额。
        // (Split the exact debt amount from `coin_to_repay_arg` (total amount available for repayment).)
        // Kriya的 `swap_receipt_debts` 函数可以从回执中读取债务。
        // (Kriya's `swap_receipt_debts` function can read debts from the receipt.)
        let repay_amount_arg = { // 这个变量名有点误导，它代表的是“债务金额”，而不是一个已经准备好的“偿还用币”
            let debts_fn_name = Identifier::new("swap_receipt_debts").map_err(|e| eyre!(e))?;
            // `swap_receipt_debts` 的泛型参数是回执的泛型 `[CoinA, CoinB]`，与池的泛型一致。
            // (Generic args for `swap_receipt_debts` are the receipt's generics `[CoinA, CoinB]`, same as pool's.)
            let debts_type_args = self.type_params.clone();
            let debts_args = vec![receipt_arg.clone()]; // 需要克隆回执参数，因为它在下面还要用
                                                        // (Need to clone receipt_arg as it's used again below)
            ctx.command(Command::move_call(
                package_id,
                module_name.clone(), // trade模块 (trade module)
                debts_fn_name,
                debts_type_args,
                debts_args,
            ));

            let last_debts_idx = ctx.last_command_idx();
            // `swap_receipt_debts` 返回 `(u64, u64)` 分别是 coin_a_debt 和 coin_b_debt
            // (returns `(u64, u64)` which are coin_a_debt and coin_b_debt respectively)
            // 我们需要偿还的是原始借入的那个币种的债务。
            // (We need to repay the debt of the originally borrowed coin type.)
            if self.is_a2b() { // 如果是借 CoinA (type_params[0]) (If CoinA (type_params[0]) was borrowed)
                Argument::NestedResult(last_debts_idx, 0) // coin_a_debt
            } else { // 如果是借 CoinB (type_params[1]) (If CoinB (type_params[1]) was borrowed)
                Argument::NestedResult(last_debts_idx, 1) // coin_b_debt
            }
        };

        // 从 `coin_to_repay_arg` (我们拥有的、用于偿还的币的总量) 中分割出确切的 `repay_amount_arg` (债务数量)。
        // (`repay_coin_exact_arg` 是精确数量的偿还用币。)
        // (Split the exact `repay_amount_arg` (debt amount) from `coin_to_repay_arg` (total coins we have for repayment).
        //  `repay_coin_exact_arg` is the coin with the exact repayment amount.)
        let repay_coin_exact_arg = ctx.split_coin_arg(coin_to_repay_arg.clone(), repay_amount_arg);

        // 调用 `repay_flash_swap` 函数
        // (Call `repay_flash_swap` function)
        let repay_fn_name = Identifier::new("repay_flash_swap").map_err(|e| eyre!(e))?;
        let repay_type_args = self.type_params.clone(); // [PoolCoin0, PoolCoin1]
        // `build_repay_args` 需要 `repay_coin_exact_arg` 和 `receipt_arg`
        // (`build_repay_args` needs `repay_coin_exact_arg` and `receipt_arg`)
        let repay_call_args = self.build_repay_args(ctx, repay_coin_exact_arg, receipt_arg)?;
        ctx.command(Command::move_call(package_id, module_name, repay_fn_name, repay_type_args, repay_call_args));

        // `repay_flash_swap` 函数没有返回值 (void)。
        // (`repay_flash_swap` function has no return value (void).)
        // `coin_to_repay_arg` 是调用者传入的，在 `split_coin_arg` 后，它代表了分割后的剩余部分。
        // (The `coin_to_repay_arg` passed by the caller now represents the remainder after `split_coin_arg`.)
        // 这个剩余部分应该被返回给调用者或转移给发送者。
        // (This remainder should be returned to the caller or transferred to the sender.)
        Ok(coin_to_repay_arg)
    }

    /// `extend_trade_tx` (常规交换 / Regular Swap)
    ///
    /// 将Kriya CLMM的常规交换操作（通过Cetus聚合器）添加到现有的PTB中。
    /// (Adds Kriya CLMM's regular swap operation (via Cetus aggregator) to the existing PTB.)
    async fn extend_trade_tx(
        &self,
        ctx: &mut TradeCtx,
        _sender: SuiAddress, // 未使用 (Unused)
        coin_in_arg: Argument,
        _amount_in: Option<u64>, // Kriya CLMM的swap函数(通过聚合器)通常消耗整个传入的Coin对象
                                 // (Kriya CLMM's swap function (via aggregator) usually consumes the entire passed Coin object)
    ) -> Result<Argument> {
        let function_name_str = if self.is_a2b() { "swap_a2b" } else { "swap_b2a" };

        // **重要**: 包ID使用的是 `CETUS_AGGREGATOR`。
        // (**IMPORTANT**: Package ID uses `CETUS_AGGREGATOR`.)
        let package_id = ObjectID::from_hex_literal(CETUS_AGGREGATOR)?;
        let module_name = Identifier::new("kriya_clmm").map_err(|e| eyre!(e))?; // 聚合器中与Kriya CLMM交互的模块
                                                                              // (Module in aggregator for interacting with Kriya CLMM)
        let function_name = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;

        let mut type_arguments = self.type_params.clone();
        if !self.is_a2b() {
            type_arguments.swap(0, 1);
        }

        let call_arguments = self.build_swap_args(ctx, coin_in_arg)?;
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        let last_idx = ctx.last_command_idx();
        Ok(Argument::Result(last_idx)) // 聚合器的swap函数返回输出的Coin对象 (Aggregator's swap function returns the output Coin object)
    }

    // --- Dex trait 的其他 getter 和 setter 方法 ---
    // (Other getter and setter methods of the `Dex` trait)
    fn coin_in_type(&self) -> String {
        self.coin_in_type.clone()
    }

    fn coin_out_type(&self) -> String {
        self.coin_out_type.clone()
    }

    fn protocol(&self) -> Protocol {
        Protocol::KriyaClmm // 协议类型为KriyaClmm (Protocol type is KriyaClmm)
    }

    fn liquidity(&self) -> u128 {
        self.liquidity
    }

    fn object_id(&self) -> ObjectID {
        self.pool.pool // 池的ObjectID (Pool's ObjectID)
    }

    /// `flip` 方法 (flip method)
    fn flip(&mut self) {
        std::mem::swap(&mut self.coin_in_type, &mut self.coin_out_type);
    }

    /// `is_a2b` 方法 (is_a2b method)
    fn is_a2b(&self) -> bool {
        self.pool.token_index(&self.coin_in_type) == Some(0)
    }

    /// `swap_tx` 方法 (主要用于测试 / Mainly for testing)
    async fn swap_tx(&self, sender: SuiAddress, recipient: SuiAddress, amount_in: u64) -> Result<TransactionData> {
        let sui_client = new_test_sui_client().await;

        let coin_in_obj = coin::get_coin(&sui_client, sender, &self.coin_in_type, amount_in).await?;

        let pt = self
            .build_swap_tx(sender, recipient, coin_in_obj.object_ref(), amount_in)
            .await?;

        let gas_coins = coin::get_gas_coin_refs(&sui_client, sender, Some(coin_in_obj.coin_object_id)).await?;
        let gas_price = sui_client.read_api().get_reference_gas_price().await?;
        let tx_data = TransactionData::new_programmable(sender, gas_coins, pt, GAS_BUDGET, gas_price);

        Ok(tx_data)
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

    /// `test_kriya_clmm_swap_tx` 测试函数
    /// (test_kriya_clmm_swap_tx test function)
    ///
    /// 测试通过Kriya CLMM (可能经由Cetus聚合器) 进行常规交换的流程。
    /// (Tests the process of regular swap via Kriya CLMM (possibly through Cetus aggregator).)
    #[tokio::test]
    async fn test_kriya_clmm_swap_tx() {
        mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

        let http_simulator = HttpSimulator::new(TEST_HTTP_URL, &None).await;

        let owner = SuiAddress::from_str(TEST_ATTACKER).unwrap();
        let recipient =
            SuiAddress::from_str("0x0cbe287984143ef232336bb39397bd10607fa274707e8d0f91016dceb31bb829").unwrap();
        let token_in_type = "0x2::sui::SUI";
        // DEEP是Cetus上的一个代币，这里可能只是作为示例，实际Kriya CLMM上交易对可能不同
        // (DEEP is a token on Cetus, used here as an example; actual Kriya CLMM trading pairs might differ)
        let token_out_type = "0xdeeb7a4662eec9f2f3def03fb937a663dddaa2e215b8078a284d026b7946c270::deep::DEEP";
        let amount_in = 10000; // 输入少量 (0.00001 SUI) (Input small amount (0.00001 SUI))

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
        info!("🧀 找到的DEX数量 (Number of DEXs found): {}", dexes.len());

        let dex_to_test = dexes
            .into_iter()
            .filter(|dex| dex.protocol() == Protocol::KriyaClmm)
            .sorted_by(|a, b| a.liquidity().cmp(&b.liquidity()))
            .last()
            .expect("测试中未找到KriyaClmm的池 (KriyaClmm pool not found in test)");

        let tx_data = dex_to_test.swap_tx(owner, recipient, amount_in).await.unwrap();
        info!("🧀 构建的交易数据 (Constructed transaction data): {:?}", tx_data);

        let response = http_simulator.simulate(tx_data, Default::default()).await.unwrap();
        info!("🧀 模拟结果 (Simulation result): {:?}", response);

        assert!(response.is_ok(), "交易模拟应成功 (Transaction simulation should succeed)");
    }
}

[end of bin/arb/src/defi/kriya_clmm.rs]
