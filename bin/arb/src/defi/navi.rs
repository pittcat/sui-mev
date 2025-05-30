// 该文件 `navi.rs` 实现了与 Navi Protocol (一个Sui区块链上的借贷协议) 交互的逻辑。
// Navi 允许用户存入资产作为抵押品来借出其他资产，或者直接从资金池中借款（例如闪电贷）。
// 这个文件主要关注 Navi 的闪电贷 (Flashloan) 功能，这对于套利策略非常重要，
// 因为它允许在单笔交易中无抵押借入大量资金，执行套利操作，然后立即归还。
//
// 文件概览:
// 1. 定义了与 Navi 协议相关的常量，如合约包ID (`NAVI_PROTOCOL`) 和关键对象ID
//    (如 `NAVI_POOL`, `NAVI_CONFIG`, `NAVI_STORAGE`)。
//    这些对象是调用Navi合约功能时必需的参数。
// 2. `Navi` 结构体: 代表与Navi协议的一个特定资金池（这里特指SUI池）进行交互的实例。
//    它存储了与Navi交互所需的配置和对象参数。
//    注意：当前的实现似乎硬编码为只与SUI代币的资金池进行闪电贷交互。
// 3. `new()` 方法: `Navi` 结构体的构造函数，负责异步获取并缓存Navi协议所需的链上对象。
// 4. `extend_flashloan_tx()` 方法: 将发起Navi闪电贷的操作添加到现有的交易上下文 (`TradeCtx`) 中。
//    它会构建一个调用Navi `lending::flash_loan_with_ctx` 函数的命令。
// 5. `extend_repay_tx()` 方法: 将偿还Navi闪电贷的操作添加到现有的交易上下文 (`TradeCtx`) 中。
//    它会构建一个调用Navi `lending::flash_repay_with_ctx` 函数的命令。
//
// Sui/DeFi概念:
// - Lending Protocol (借贷协议): DeFi中的一类协议，允许用户存入资产赚取利息（存款），
//   或抵押资产借出其他资产（借款）。Navi是Sui上的一个主流借贷协议。
// - Flashloan (闪电贷): 一种特殊的无抵押贷款，要求在同一笔原子交易内归还本金和手续费。
//   如果未能满足条件，整个交易将回滚。闪电贷是执行套利、清算等操作的强大工具。
// - Pool (资金池): 在借贷协议中，用户存入的资产会汇集到相应的资金池中，供其他用户借款。
//   每个支持的代币类型通常都有一个独立的资金池。
// - Config / Storage Objects: Navi协议可能使用特定的链上对象来存储其全局配置、每个资金池的状态、
//   用户账户信息等。这些对象在调用合约功能时需要作为参数传入。

// 引入标准库及第三方库
use std::{str::FromStr, sync::Arc}; // FromStr用于从字符串转换, Arc原子引用计数

use eyre::{eyre, OptionExt, Result}; // 错误处理库
use simulator::Simulator; // 交易模拟器接口 (用于获取链上对象)
use sui_sdk::SUI_COIN_TYPE; // SUI原生代币的类型字符串 ("0x2::sui::SUI")
use sui_types::{
    base_types::ObjectID, // Sui对象ID类型
    transaction::{Argument, Command, ObjectArg}, // Sui交易构建相关类型: Argument, Command, ObjectArg
    Identifier, TypeTag, SUI_CLOCK_OBJECT_ID, // Sui标识符, 类型标签, 时钟对象ID
};
use utils::object::shared_obj_arg; // 自定义工具库中用于创建共享对象参数的函数

use super::{trade::FlashResult, TradeCtx}; // 从父模块(defi)引入 FlashResult (闪电贷结果类型), TradeCtx (交易上下文)

// --- Navi协议相关的常量定义 ---
// Navi核心合约包ID (Package ID)
const NAVI_PROTOCOL: &str = "0x834a86970ae93a73faf4fff16ae40bdb72b91c47be585fff19a2af60a19ddca3";
// Navi SUI资金池对象ID。每个支持的资产在Navi中都有一个对应的池对象。
const NAVI_POOL: &str = "0x96df0fce3c471489f4debaaa762cf960b3d97820bd1f3f025ff8190730e958c5";
// Navi全局配置对象ID (FlashLoanConfig / ProtocolConfig)
const NAVI_CONFIG: &str = "0x3672b2bf471a60c30a03325f104f92fb195c9d337ba58072dce764fe2aa5e2dc";
// Navi存储对象ID (Storage) - 可能包含用户账户数据、利率模型等状态。
const NAVI_STORAGE: &str = "0xbb4e2f4b6205c2e2a2db47aeb4f830796ec7c005f88537ee775986639bc442fe";

/// `Navi` 结构体
///
/// 代表与Navi协议进行交互的实例，主要用于发起和偿还SUI的闪电贷。
#[derive(Clone)] // 允许克隆 Navi 实例
pub struct Navi {
    sui_coin_type: TypeTag, // SUI代币的TypeTag，因为此实现专注于SUI闪电贷
    pool: ObjectArg,        // Navi SUI资金池对象的ObjectArg
    config: ObjectArg,      // Navi全局配置对象的ObjectArg
    storage: ObjectArg,     // Navi存储对象的ObjectArg
    clock: ObjectArg,       // Sui时钟对象的ObjectArg
}

impl Navi {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `Navi` 实例。
    /// 它会异步从链上获取Navi协议所需的关键对象（Pool, Config, Storage, Clock）
    /// 并将它们转换为 `ObjectArg` 格式以备后续构建交易时使用。
    /// 注意：对象的获取只在初始化时执行一次，以避免影响套利操作的性能。
    ///
    /// 参数:
    /// - `simulator`: 一个共享的模拟器实例 (`Arc<Box<dyn Simulator>>`)，用于从链上获取对象数据。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `Navi` 实例，否则返回错误。
    pub async fn new(simulator: Arc<Box<dyn Simulator>>) -> Result<Self> {
        // 通过模拟器获取Navi的SUI资金池对象
        let pool_obj = simulator
            .get_object(&ObjectID::from_hex_literal(NAVI_POOL)?) // 从十六进制字符串解析ObjectID
            .await
            .ok_or_eyre("Navi SUI资金池对象未找到")?; // 如果找不到对象，则返回错误

        // 获取Navi的全局配置对象
        let config_obj = simulator
            .get_object(&ObjectID::from_hex_literal(NAVI_CONFIG)?)
            .await
            .ok_or_eyre("Navi配置对象未找到")?;

        // 获取Navi的存储对象
        let storage_obj = simulator
            .get_object(&ObjectID::from_hex_literal(NAVI_STORAGE)?)
            .await
            .ok_or_eyre("Navi存储对象未找到")?;

        // 获取Sui系统的时钟对象 (这是一个标准的共享对象)
        let clock_obj = simulator
            .get_object(&SUI_CLOCK_OBJECT_ID)
            .await
            .ok_or_eyre("Sui时钟对象未找到")?;

        Ok(Self {
            sui_coin_type: TypeTag::from_str(SUI_COIN_TYPE).unwrap(), // 将SUI类型字符串转换为TypeTag
            // 将获取到的SuiObject转换为ObjectArg。
            // `shared_obj_arg` 会根据对象是否可变来创建合适的ObjectArg。
            // Navi的Pool和Storage在操作中通常是可变的。Config和Clock通常是不可变的。
            pool: shared_obj_arg(&pool_obj, true),     // Pool对象可变
            config: shared_obj_arg(&config_obj, false),  // Config对象不可变
            storage: shared_obj_arg(&storage_obj, true), // Storage对象可变
            clock: shared_obj_arg(&clock_obj, false),  // Clock对象不可变
        })
    }

    /// `extend_flashloan_tx` 方法
    ///
    /// 将发起Navi闪电贷的操作添加到现有的交易上下文 (`TradeCtx`) 中。
    /// 调用Navi的 `lending::flash_loan_with_ctx` 函数。
    ///
    /// 合约方法签名示例 (来自注释):
    /// `public fun flash_loan_with_ctx<CoinType>(
    ///     config: &FlashLoanConfig, // Navi的全局配置对象
    ///     pool: &mut Pool<CoinType>,// 特定代币的资金池对象 (这里是SUI池)
    ///     amount: u64,              // 希望借入的代币数量
    ///     ctx: &mut TxContext       // 交易上下文 (由Sui运行时提供)
    /// ): (Balance<CoinType>, FlashLoanReceipt<CoinType>)`
    /// 返回一个元组：借出的代币余额 (`Balance`) 和一个闪电贷回执 (`FlashLoanReceipt`)。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文 (`&mut TradeCtx`)，用于构建PTB。
    /// - `amount_in`: 希望借入的SUI代币数量。
    ///
    /// 返回:
    /// - `Result<FlashResult>`: 包含借出的SUI代币 (`coin_out`) 和闪电贷回执 (`receipt`) 的 `Argument`。
    pub fn extend_flashloan_tx(&self, ctx: &mut TradeCtx, amount_in: u64) -> Result<FlashResult> {
        let package_id = ObjectID::from_hex_literal(NAVI_PROTOCOL)?; // Navi合约包ID
        let module_name = Identifier::new("lending").map_err(|e| eyre!(e))?; // `lending`模块
        let function_name = Identifier::new("flash_loan_with_ctx").map_err(|e| eyre!(e))?;
        // 泛型类型参数，因为我们只处理SUI闪电贷，所以是 `[0x2::sui::SUI]`
        let type_arguments = vec![self.sui_coin_type.clone()];

        // 构建调用参数列表
        let call_arguments = vec![
            ctx.obj(self.config).map_err(|e| eyre!(e))?,   // config: &FlashLoanConfig
            ctx.obj(self.pool).map_err(|e| eyre!(e))?,     // pool: &mut Pool<SUI>
            ctx.pure(amount_in).map_err(|e| eyre!(e))?,    // amount: u64
        ];

        // 向PTB中添加Move调用命令
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));
        let last_idx = ctx.last_command_idx(); // 获取刚添加的命令的索引

        // `flash_loan_with_ctx` 返回一个元组 `(Balance<SUI>, FlashLoanReceipt<SUI>)`
        // 我们需要将元组的第一个元素 (Balance<SUI>) 转换为 Coin<SUI> 对象。
        let balance_out_arg = Argument::NestedResult(last_idx, 0); // 元组的第一个元素 (Balance)
        // `ctx.coin_from_balance` 会添加一个命令将Balance转换为Coin
        let coin_out_arg = ctx.coin_from_balance(balance_out_arg, self.sui_coin_type.clone())?;

        Ok(FlashResult {
            coin_out: coin_out_arg, // 借到的SUI代币 (作为Coin对象)
            receipt: Argument::NestedResult(last_idx, 1), // 元组的第二个元素 (FlashLoanReceipt)
            pool: None, // Navi的flash_loan不直接返回pool对象作为PTB结果，所以是None
                        // （与Cetus或FlowX不同，它们可能在FlashResult中传递pool的引用）
        })
    }

    /// `extend_repay_tx` 方法
    ///
    /// 将偿还Navi闪电贷的操作添加到现有的交易上下文 (`TradeCtx`) 中。
    /// 调用Navi的 `lending::flash_repay_with_ctx` 函数。
    ///
    /// 合约方法签名示例 (来自注释):
    /// `public fun flash_repay_with_ctx<CoinType>(
    ///     clock: &Clock,
    ///     storage: &mut Storage,
    ///     pool: &mut Pool<CoinType>,
    ///     receipt: FlashLoanReceipt<CoinType>, // 从 flash_loan_with_ctx 获取的回执
    ///     repay_balance: Balance<CoinType>,   // 用于偿还的代币余额 (必须包含本金+费用)
    ///     ctx: &mut TxContext
    /// ): Balance<CoinType>`
    /// 返回一个 `Balance<CoinType>`，代表偿还后多余的金额 (如果支付的超过了应还款额，通常为0)。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文 (`&mut TradeCtx`)。
    /// - `coin_to_repay_arg`: 代表用于偿还的SUI代币的 `Argument` (必须包含本金+费用)。
    /// - `flash_res`: 从 `extend_flashloan_tx` 返回的 `FlashResult`，主要使用其中的 `receipt`。
    ///
    /// 返回:
    /// - `Result<Argument>`: 代表偿还后多余的SUI代币 (作为Coin对象)。
    pub fn extend_repay_tx(&self, ctx: &mut TradeCtx, coin_to_repay_arg: Argument, flash_res: FlashResult) -> Result<Argument> {
        let package_id = ObjectID::from_hex_literal(NAVI_PROTOCOL)?;
        let module_name = Identifier::new("lending").map_err(|e| eyre!(e))?;
        let function_name = Identifier::new("flash_repay_with_ctx").map_err(|e| eyre!(e))?;
        // 泛型类型参数，仍然是SUI
        let type_arguments = vec![self.sui_coin_type.clone()];

        // 将用于偿还的 `Coin<SUI>` (coin_to_repay_arg) 转换为 `Balance<SUI>`
        let repay_balance_arg = ctx.coin_into_balance(coin_to_repay_arg, self.sui_coin_type.clone())?;

        // 构建调用参数列表
        let call_arguments = vec![
            ctx.obj(self.clock).map_err(|e| eyre!(e))?,    // clock: &Clock
            ctx.obj(self.storage).map_err(|e| eyre!(e))?,  // storage: &mut Storage
            ctx.obj(self.pool).map_err(|e| eyre!(e))?,     // pool: &mut Pool<SUI>
            flash_res.receipt,                             // receipt: FlashLoanReceipt<SUI>
            repay_balance_arg,                             // repay_balance: Balance<SUI>
        ];

        // 向PTB中添加Move调用命令
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));
        let last_idx = ctx.last_command_idx(); // 获取刚添加的命令的索引

        // `flash_repay_with_ctx` 返回一个 `Balance<SUI>` (代表找零)
        let remaining_balance_arg = Argument::Result(last_idx);
        // 将这个找零的Balance转换为Coin对象
        let remaining_coin_arg = ctx.coin_from_balance(remaining_balance_arg, self.sui_coin_type.clone())?;

        Ok(remaining_coin_arg) // 返回代表找零的Coin对象的Argument
    }
}
