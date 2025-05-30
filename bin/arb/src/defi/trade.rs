// 该文件 `trade.rs` 是DeFi模块中负责交易执行和模拟的核心。
// 它定义了如何将一个抽象的交易路径 (`Path`) 转换为可执行的Sui交易 (`TransactionData`)，
// 并如何模拟这些交易以获取预期结果。
//
// 文件概览:
// 1. `TradeType` 枚举: 定义了两种交易类型：`Swap` (常规交换) 和 `Flashloan` (闪电贷基础的交换)。
// 2. `FlashResult` 结构体: 用于封装闪电贷操作返回的结果，主要是借出的代币和回执。
// 3. `Trader` 结构体: 核心的交易执行器/模拟器。
//    - 持有模拟器对象池 (`simulator_pool`)、Shio竞价协议实例 (`shio`) 和Navi借贷协议实例 (`navi`)。
//    - `new()`: 构造函数。
//    - `get_trade_result()`: 模拟一条交易路径，并返回预期的输出金额、Gas成本等。
//    - `get_swap_trade_tx()`: 构建一个基于常规交换的交易 (`TransactionData`)。
//    - `get_flashloan_trade_tx()`: 构建一个基于闪电贷的交易 (`TransactionData`)。它会处理借款、执行路径交换、偿还借款、可选的MEV竞价等步骤。
// 4. `TradeCtx` 结构体: 一个辅助结构体，用于逐步构建Sui的可编程交易块 (Programmable Transaction Block - PTB)。
//    - 封装了 `ProgrammableTransactionBuilder`。
//    - 提供了一些便捷方法来添加常用的Sui命令 (如 `split_coin`, `transfer_arg`, `coin_from_balance` 等)。
//    - 实现了 `Deref` 和 `DerefMut`，使其可以像 `ProgrammableTransactionBuilder` 一样直接使用。
// 5. `TradeResult` 结构体: 存储单次交易模拟的结果，如输出金额、Gas成本、缓存未命中次数。
// 6. `Path` 结构体: 代表一条交易路径，即一个或多个连续的DEX交换。
//    - 包含一个 `Vec<Box<dyn Dex>>`，存储路径上每个DEX的实例。
//    - 提供方法如 `is_empty()`, `is_disjoint()` (判断两条路径是否有共同的DEX池), `coin_in_type()`, `coin_out_type()`。
//
// Sui/DeFi概念:
// - Programmable Transaction Block (PTB): Sui的一种强大交易构建方式，允许将多个原子操作（如Move调用、对象转移、分割合并Coin等）
//   组合成一个单一的、原子性的交易。这对于执行复杂的多步DeFi操作（如套利、闪电贷）至关重要。
// - Argument (交易参数): 在PTB中，`Argument` 类型用于表示传递给Move调用或PTB命令的参数。
//   它可以是链上对象 (`ObjectArg`)、纯值 (`Pure`)、或者之前命令的结果 (`Result`, `NestedResult`)。
// - Mocked Coin (模拟代币): 在模拟交易时，如果实际的输入代币对象不存在或不方便获取，
//   可以创建一个“模拟的”代币对象 (`Object`) 及其引用 (`ObjectRef`) 来代表输入。
//   `sim_ctx.with_borrowed_coin()` 可能用于在模拟器中注册这个模拟代币。
// - Balance Changes (余额变更): 交易模拟结果 (`SuiTransactionBlockEffects`) 中包含了交易执行后各账户地址代币余额的变化情况。
//   这用于确定交易的最终输出金额。
// - Transaction Digest (交易摘要/哈希): 每笔Sui交易的唯一标识符。在MEV竞价中，可能需要确保竞价交易的摘要大于机会交易的摘要。

// 引入标准库及第三方库
use std::{
    collections::HashSet, // 用于判断路径是否不相交 (is_disjoint)
    fmt,                  // 用于格式化输出 (实现Debug, Display trait)
    ops::{Deref, DerefMut}, // 用于实现智能指针的解引用行为 (TradeCtx -> PTB)
    str::FromStr,         // 用于从字符串转换 (例如TypeTag)
    sync::Arc,            // 原子引用计数
};

use ::utils::coin; // 外部 `utils` crate的代币工具
use eyre::{ensure, eyre, Result}; // 错误处理库
use object_pool::ObjectPool; // 对象池 (用于模拟器)
use simulator::{SimulateCtx, Simulator}; // 模拟器上下文和Simulator trait
use sui_json_rpc_types::SuiExecutionStatus; // Sui交易执行状态枚举
use sui_sdk::rpc_types::SuiTransactionBlockEffectsAPI; // 用于访问交易效果API的trait
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui基本类型
    object::{Object, Owner}, // Sui对象和所有者类型
    programmable_transaction_builder::ProgrammableTransactionBuilder, // PTB构建器
    transaction::{Argument, Command, ObjectArg, TransactionData}, // Sui交易参数、命令、对象参数、交易数据
    Identifier, TypeTag, SUI_FRAMEWORK_PACKAGE_ID, // Sui标识符, 类型标签, Sui框架包ID (用于调用标准模块如coin, balance)
};
use tracing::instrument; // `tracing`库的 `instrument` 宏，用于自动为函数添加追踪span

// 从父模块(defi)引入Navi和Shio的具体实现，以及Dex trait
use super::{navi::Navi, shio::Shio, Dex};
// 从当前crate的根模块引入配置和自定义类型
use crate::{config::*, types::Source};

/// `TradeType` 枚举
///
/// 定义了交易的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)] // 派生常用trait
pub enum TradeType {
    Swap,      // 常规交换：使用已有的代币进行一系列DEX交换。
    Flashloan, // 闪电贷交换：先通过闪电贷借入初始资金，然后进行交换，最后在同一交易内偿还贷款。
}

/// `FlashResult` 结构体
///
/// 封装了从闪电贷操作（如 `Dex::extend_flashloan_tx`）返回的关键结果。
#[derive(Debug, Clone)]
pub struct FlashResult {
    pub coin_out: Argument, // 代表通过闪电贷借入并可能已初步转换的代币的 `Argument`。
                            // 这是后续交易步骤可以使用的主要资金。
    pub receipt: Argument,  // 代表闪电贷回执的 `Argument`。这个回执对象在偿还贷款时是必需的。
    pub pool: Option<Argument>, // (可选) 代表闪电贷提供方池对象的 `Argument`。
                                // 某些协议 (如FlowX) 在偿还时可能需要再次传入池对象。
}

/// `Trader` 结构体
///
/// 负责执行或模拟交易路径。
/// 它组合了模拟器、Shio（用于MEV竞价）和Navi（用于SUI闪电贷）的功能。
#[derive(Clone)]
pub struct Trader {
    simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>, // 共享的模拟器对象池
    shio: Arc<Shio>,                                     // 共享的Shio协议实例
    navi: Arc<Navi>,                                     // 共享的Navi协议实例 (主要用于SUI闪电贷)
}

/// `TradeCtx` 结构体 (交易上下文)
///
/// 一个辅助结构体，用于逐步构建Sui的可编程交易块 (PTB)。
/// 它内部封装了 `ProgrammableTransactionBuilder`，并增加了一个命令计数器。
#[derive(Default)] // 可以使用 TradeCtx::default() 创建一个新实例
pub struct TradeCtx {
    pub ptb: ProgrammableTransactionBuilder, // Sui官方提供的PTB构建器
    pub command_count: u16,                  // 当前PTB中已添加的命令数量
}

/// `TradeResult` 结构体
///
/// 存储单次交易模拟的结果。
#[derive(Default, Debug, Clone)] // Default用于初始化 (例如零输出)，Debug和Clone是常用派生
pub struct TradeResult {
    pub amount_out: u64, // 模拟交易后，最终输出代币的数量
    pub gas_cost: i64,   // 模拟交易的Gas成本 (有符号整数)
    pub cache_misses: u64, // 模拟器在模拟过程中缓存未命中的次数
}

impl Trader {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `Trader` 实例。
    ///
    /// 参数:
    /// - `simulator_pool`: 一个共享的模拟器对象池。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `Trader` 实例。
    pub async fn new(simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>) -> Result<Self> {
        // 初始化 Shio 实例 (用于MEV竞价)
        let shio_instance = Arc::new(Shio::new().await?); // Shio::new() 是异步的
        // 从池中获取一个模拟器实例，用于初始化 Navi
        let simulator_instance_for_navi = simulator_pool.get();
        // 初始化 Navi 实例 (用于SUI闪电贷)
        let navi_instance = Arc::new(Navi::new(simulator_instance_for_navi).await?);

        Ok(Self {
            simulator_pool,
            shio: shio_instance,
            navi: navi_instance,
        })
    }

    /// `get_trade_result` 方法
    ///
    /// 模拟给定的交易路径 (`path`)，并返回交易结果 (`TradeResult`)。
    ///
    /// `#[instrument]` 宏用于自动为这个函数创建一个追踪span，方便日志和性能分析。
    /// - `name = "result"`: span的名称。
    /// - `skip_all`: 不自动记录所有函数参数作为span的字段。
    /// - `fields(...)`: 自定义记录的字段。
    ///   - `len`: 路径的长度 (跳数)。
    ///   - `paths`: 将路径中每个DEX的信息格式化为字符串。
    ///
    /// 参数:
    /// - `path`: 要模拟的交易路径 (`&Path`)。
    /// - `sender`: 交易发送者的Sui地址。
    /// - `amount_in`: 输入代币的数量。
    /// - `trade_type`: 交易类型 (`Swap` 或 `Flashloan`)。
    /// - `gas_coins`: 用于支付Gas的代币对象引用列表。
    /// - `sim_ctx`: 可变的模拟上下文 (`SimulateCtx`)。
    ///
    /// 返回:
    /// - `Result<TradeResult>`: 包含模拟输出金额、Gas成本等的交易结果。
    #[instrument(name = "result", skip_all, fields(
        len = %format!("{:<2}", path.path.len()), // 路径长度，左对齐，宽度2
        // 将路径中每个DEX格式化为 "Protocol:CoinInShort->CoinOutShort" 的形式
        paths = %path.path.iter().map(|d| {
            let coin_in_short = d.coin_in_type().split("::").last().unwrap_or("").to_string();
            let coin_out_short = d.coin_out_type().split("::").last().unwrap_or("").to_string();
            format!("{:?}:{}-{}", d.protocol(), coin_in_short, coin_out_short)
        }).collect::<Vec<_>>().join(" ") // 用空格连接
    ))]
    pub async fn get_trade_result(
        &self,
        path: &Path,
        sender: SuiAddress,
        amount_in: u64,
        trade_type: TradeType,
        gas_coins: Vec<ObjectRef>,
        mut sim_ctx: SimulateCtx, // sim_ctx是可变的，因为可能会在其中注册模拟代币
    ) -> Result<TradeResult> {
        ensure!(!path.is_empty(), "模拟的交易路径不能为空"); // 确保路径非空
        let gas_price = sim_ctx.epoch.gas_price; // 从模拟上下文中获取当前Gas价格

        // 根据交易类型构建交易数据 (`TransactionData`)。
        // `mocked_coin_in` 是一个可选的 `Object`，如果构建的是Swap交易且输入是SUI，
        // 它会是一个模拟的SUI输入代币对象，需要在模拟器中注册。
        let (tx_data, mocked_coin_in) = match trade_type {
            TradeType::Swap => {
                // 构建常规Swap交易
                self.get_swap_trade_tx(path, sender, amount_in, gas_coins, gas_price)
                    .await?
            }
            TradeType::Flashloan => {
                // 构建基于闪电贷的交易
                // `Source::Public` 表示这是一个公开的、非MEV竞价的闪电贷交易。
                self.get_flashloan_trade_tx(path, sender, amount_in, gas_coins, gas_price, Source::Public)
                    .await?
            }
        };

        // 如果在构建Swap交易时创建了模拟的输入代币，则将其添加到模拟上下文中。
        // `with_borrowed_coin` 可能是模拟器的一个方法，用于告知模拟器这个代币对象的存在及其面额。
        if let Some(mocked_coin) = mocked_coin_in {
            sim_ctx.with_borrowed_coin((mocked_coin, amount_in));
        }

        // 从对象池中获取一个模拟器实例并执行交易模拟。
        let simulation_response = self.simulator_pool.get().simulate(tx_data.clone(), sim_ctx).await?;
        let status = simulation_response.effects.status(); // 获取交易执行状态

        // 处理交易状态
        match status {
            SuiExecutionStatus::Success => {} // 成功则无需额外操作
            SuiExecutionStatus::Failure { error } => {
                // 如果失败，检查错误类型。
                // 忽略 "MoveAbort" (Move合约主动中止) 和 "InsufficientCoinBalance" (余额不足) 错误，
                // 因为这些在套利尝试中是可能发生的，不一定表示代码逻辑错误。
                // 对于其他类型的错误，则记录更详细的错误日志。
                if !error.contains("MoveAbort") && !error.contains("InsufficientCoinBalance") {
                    tracing::error!("交易模拟失败，状态: {:?}", status);
                }
            }
        }

        // 确保交易模拟成功，否则后续的余额检查无意义。
        ensure!(status.is_ok(), "交易模拟未成功: {:?}", status);

        // 从模拟效果中获取Gas成本。
        let gas_cost = simulation_response.effects.gas_cost_summary().net_gas_usage();
        // 解析路径的输入和输出代币类型。
        let coin_in_type_tag = TypeTag::from_str(&path.coin_in_type()).map_err(|_| eyre!("无效的输入代币类型"))?;
        let coin_out_type_tag = TypeTag::from_str(&path.coin_out_type()).map_err(|_| eyre!("无效的输出代币类型"))?;
        let is_output_native_sui = coin::is_native_coin(&path.coin_out_type());

        // 从余额变更 (`balance_changes`) 中找到发送者 (`sender`) 的输出代币 (`coin_out_type_tag`) 的最终余额。
        let mut final_amount_out = i128::MIN; // 初始化为一个极小值，用于检测是否找到余额变更
        for balance_change_event in &simulation_response.balance_changes {
            if balance_change_event.owner == Owner::AddressOwner(sender) && balance_change_event.coin_type == coin_out_type_tag {
                final_amount_out = balance_change_event.amount; // 这是 +/- 的净变化量
                // 特殊处理：如果输入输出都是SUI (原生币)，则输出金额需要加上初始输入和Gas成本，
                // 因为 `balance_change_event.amount` 通常表示的是净变化。
                // 例如，如果用100 SUI换回105 SUI，Gas是1 SUI，那么amount可能是 +4 SUI。
                // 我们关心的最终绝对数量是 100 + 4 = 104 (不考虑Gas)，或 100 + 5 - 1 (考虑Gas的净利润)。
                // 这里的逻辑是: 最终SUI = (原SUI - amount_in - gas_cost) + (原SUI + 净变化)
                // 如果 amount 是净变化: final_amount_out (作为净变化)
                // 如果路径是 SUI -> ... -> SUI:
                //   我们关心的 amount_out 是最终得到的SUI数量。
                //   balance_change_event.amount 代表的是 SUI 余额的净变化。
                //   所以，最终得到的 SUI = 初始SUI(amount_in) + 净变化(final_amount_out)
                //   这里的逻辑 `final_amount_out = final_amount_out + amount_in as i128 + gas_cost as i128`
                //   似乎是在尝试从一个“最终绝对余额”反推“净获利部分作为输出”。
                //   如果 `final_amount_out` 是最终绝对余额，那么 `amount_out - amount_in - gas_cost` 是利润。
                //   如果 `final_amount_out` 是净变化，那么 `amount_out - gas_cost` 是利润。
                //   **澄清**: `balance_changes.amount` 是一个有符号整数，表示余额的净变化。
                //   如果 SUI -> SUI, 且 amount_in 是 100, gas_cost 是 1, 最终得到 105 SUI。
                //   那么 sender 的 SUI 余额变化是 105 - 100 - 1 = +4。 `final_amount_out` 会是 4。
                //   我们期望的 `TradeResult.amount_out` 应该是最终得到的绝对数量，即 105。
                //   所以，如果 `final_amount_out` 是净变化 `+4`，
                //   那么绝对输出 `abs_amount_out = amount_in + final_amount_out` (不考虑gas)。
                //   或者，如果 `final_amount_out` 已经是最终绝对数量，那么这个调整是不必要的。
                //   假设 `balance_change_event.amount` 是净变化。
                //   那么，最终得到的数量应该是 `amount_in (作为成本被扣除)` + `final_amount_out (净收益)`。
                //   所以 `abs_amount_out = amount_in + final_amount_out`。
                //   这里的 `final_amount_out = final_amount_out + amount_in as i128 + gas_cost as i128`
                //   是在 `final_amount_out` (净变化) 的基础上，加回 `amount_in` 和 `gas_cost`，
                //   这似乎是为了得到一个“总收入”或“总价值”，而不是单纯的“输出代币数量”。
                //   **重新审视**: `PathTradeResult::profit()` 中对 SUI->SUI 的处理是 `amount_out - amount_in - gas_cost`。
                //   这意味着 `TradeResult::amount_out` 应该就是最终收到的绝对数量。
                //   如果 `final_amount_out` 是净变化，则 `TradeResult::amount_out` 应该是 `amount_in + final_amount_out`。
                //   当前代码的调整 `final_amount_out = final_amount_out + amount_in as i128 + gas_cost as i128`
                //   如果 `final_amount_out` 是净变化，那么这个结果是 `净变化 + amount_in + gas_cost`。
                //   这似乎是为了让 `profit()` 计算时 `(净变化 + amount_in + gas_cost) - amount_in - gas_cost = 净变化`。
                //   这使得 `amount_out` 字段存储的是一个调整后的值，而不是直接的币数量。
                //   **结论**: 为了使 `profit()` 计算正确，`amount_out` 字段需要存储调整后的值。
                //   如果 `final_amount_out` 是净变化 `P_net`，则 `amount_out` 存 `P_net + amount_in + gas_cost`。
                //   则 `profit = (P_net + amount_in + gas_cost) - amount_in - gas_cost = P_net`。
                if coin_in_type_tag == coin_out_type_tag && is_output_native_sui { // 如果是 SUI -> SUI 交易
                    final_amount_out = final_amount_out + amount_in as i128 + gas_cost as i128;
                }

                ensure!(final_amount_out >= 0, "计算出的输出金额为负: {}", final_amount_out);
                break; // 找到对应的余额变更后即可跳出循环
            }
        }
        // 确保找到了发送者的输出代币余额变更记录。
        ensure!(final_amount_out != i128::MIN, "未找到发送者 {} 的输出代币余额变更", sender);

        Ok(TradeResult {
            amount_out: final_amount_out as u64, // 将调整后的 i128 值转为 u64
            gas_cost,
            cache_misses: simulation_response.cache_misses,
        })
    }

    /// `get_swap_trade_tx` 方法
    ///
    /// 构建一个基于常规交换的交易 (`TransactionData`) 和一个可选的模拟输入代币对象。
    ///
    /// 步骤:
    /// 1. 创建交易上下文 (`TradeCtx`)。
    /// 2. 创建一个模拟的SUI输入代币对象 (`mocked_sui`) 及其引用 (`coin_in_ref`)。
    ///    这个模拟代币用于代表交易的初始输入，尤其是在模拟环境中。
    /// 3. 将模拟代币分割出所需的 `amount_in` 数量，得到 `coin_in_arg`。
    /// 4. 遍历路径中的每个DEX，调用其 `extend_trade_tx` 方法，将交换操作添加到PTB中，
    ///    并将上一步的输出 (`coin_in_arg`) 作为下一步的输入。
    /// 5. 将最终的输出代币转移给发送者自身。
    /// 6. 完成PTB构建，并创建 `TransactionData`。
    ///
    /// 返回:
    /// - `(TransactionData, Option<Object>)`: 包含构建好的交易数据和模拟的SUI输入代币对象。
    pub async fn get_swap_trade_tx(
        &self,
        path: &Path,
        sender: SuiAddress,
        amount_in: u64,
        gas_coins: Vec<ObjectRef>,
        gas_price: u64,
    ) -> Result<(TransactionData, Option<Object>)> {
        ensure!(!path.is_empty(), "Swap交易路径不能为空");
        let mut ctx = TradeCtx::default();

        // 1. 准备输入代币 (这里总是创建一个模拟的SUI代币作为输入)
        // `coin::mocked_sui` 创建一个具有指定所有者和面额的SUI代币对象。
        // 这在模拟时非常有用，因为我们不需要拥有实际的链上对象。
        let mocked_sui_input_obj = coin::mocked_sui(sender, amount_in);
        let coin_in_ref = mocked_sui_input_obj.compute_object_reference(); // 获取其对象引用

        // 2. 执行交换路径
        // 将模拟代币对象作为PTB的第一个输入参数 (通过分割得到精确数量)
        let mut current_coin_arg = ctx.split_coin(coin_in_ref, amount_in)?;
        for (i, dex_step) in path.path.iter().enumerate() {
            // 对于路径的第一步，`amount_in_option` 是 Some(amount_in)，
            // 对于后续步骤，是 None (因为DEX通常消耗整个传入的Coin对象)。
            let amount_in_option = if i == 0 { Some(amount_in) } else { None };
            current_coin_arg = dex_step.extend_trade_tx(&mut ctx, sender, current_coin_arg, amount_in_option).await?;
        }

        // 3. 将最终输出的代币转移给发送者 (recipient = sender)
        ctx.transfer_arg(sender, current_coin_arg);
        let ptb_finished = ctx.ptb.finish(); // 完成PTB构建

        // 创建 TransactionData
        let tx_data = TransactionData::new_programmable(sender, gas_coins, ptb_finished, GAS_BUDGET, gas_price);

        // 返回交易数据和模拟的输入代币对象
        Ok((tx_data, Some(mocked_sui_input_obj)))
    }

    /// `get_flashloan_trade_tx` 方法
    ///
    /// 构建一个基于闪电贷的交易 (`TransactionData`)。
    ///
    /// 步骤:
    /// 1. 创建交易上下文 (`TradeCtx`)。
    /// 2. 决定闪电贷提供方：优先使用路径中的第一个DEX（如果它支持闪电贷），否则使用Navi的SUI闪电贷。
    /// 3. 调用相应提供方的 `extend_flashloan_tx` 方法，将借款操作添加到PTB，获得借出的代币 (`flash_res.coin_out`) 和回执。
    /// 4. 遍历交易路径（如果第一个DEX提供了闪电贷，则跳过它），使用上一步借来的代币作为输入，
    ///    调用每个DEX的 `extend_trade_tx` 方法执行交换。
    /// 5. 调用相应提供方的 `extend_repay_tx` 方法，将偿还贷款的操作添加到PTB，
    ///    使用路径交换最终得到的代币 (`coin_in_arg` 此时是路径的最终输出) 来偿还。
    ///    得到偿还后的利润代币 (`coin_profit`)。
    /// 6. (可选) 如果交易来源是Shio (`source.is_shio()`)，则从利润中分割一部分作为MEV竞价金额，
    ///    并调用 `shio.submit_bid` 将竞价操作添加到PTB。
    /// 7. 将最终剩余的利润代币转移给发送者。
    /// 8. 完成PTB构建，并创建 `TransactionData`。
    /// 9. (可选) 如果机会交易摘要 (`source.opp_tx_digest()`) 存在（用于MEV），
    ///    则调整Gas预算以确保当前竞价交易的摘要大于机会交易的摘要。
    ///
    /// 返回:
    /// - `(TransactionData, Option<Object>)`: 包含构建好的交易数据。`Option<Object>` 为 `None`，因为闪电贷不涉及模拟的外部输入代币。
    pub async fn get_flashloan_trade_tx(
        &self,
        path: &Path,
        sender: SuiAddress,
        amount_in: u64, // 这是希望通过闪电贷借入的金额
        gas_coins: Vec<ObjectRef>,
        gas_price: u64,
        source: Source, // 交易来源，可能包含MEV竞价信息
    ) -> Result<(TransactionData, Option<Object>)> {
        ensure!(!path.is_empty(), "闪电贷交易路径不能为空");
        let first_dex_on_path = &path.path[0]; // 获取路径上的第一个DEX

        let mut ctx = TradeCtx::default(); // 初始化交易上下文

        // 1. 执行闪电贷操作
        // 判断是使用路径中第一个DEX的闪电贷功能，还是使用Navi的SUI闪电贷。
        // 假设：如果第一个DEX支持闪电贷，并且其输入代币与我们希望借的代币类型一致 (这里隐式为路径的输入币)，
        // 则优先使用该DEX的闪电贷。否则，默认使用Navi的SUI闪电贷 (这意味着 `amount_in` 必须是SUI的数量)。
        // **注意**: 当前逻辑假设如果 `first_dex_on_path.support_flashloan()` 为true，
        // 那么它能提供路径所需的 `path.coin_in_type()` 的闪电贷，金额为 `amount_in`。
        let flash_loan_result = if first_dex_on_path.support_flashloan() {
            // 使用第一个DEX的闪电贷
            first_dex_on_path.extend_flashloan_tx(&mut ctx, amount_in).await?
        } else {
            // 使用Navi的SUI闪电贷 (此时 `amount_in` 必须是SUI的数量，且路径的输入也应是SUI)
            // 如果路径输入不是SUI，但这里调用Navi SUI闪电贷，则逻辑不匹配。
            // **重要假设**: 如果第一个DEX不支持闪电贷，则认为整个路径的初始资金来源于Navi的SUI闪电贷，
            // 并且 `amount_in` 是指SUI的数量。路径的第一个DEX的输入必须是SUI。
            self.navi.extend_flashloan_tx(&mut ctx, amount_in)?
        };

        // 2. 执行路径上的DEX交换
        // `current_active_coin_arg` 初始化为闪电贷借来的代币。
        let mut current_active_coin_arg = flash_loan_result.coin_out;
        // 决定从路径的哪个DEX开始迭代：
        // - 如果第一个DEX提供了闪电贷，那么它已经作为借款步骤被处理了，
        //   `flash_loan_result.coin_out` 可能是该DEX借款并立即执行一步交换后的结果。
        //   此时，后续交换应该从路径的第二个DEX开始。
        // - 如果使用的是外部闪电贷 (如Navi)，那么 `flash_loan_result.coin_out` 是原始借来的代币 (如SUI)，
        //   后续交换应该从路径的第一个DEX开始。
        let dex_iterator: Box<dyn Iterator<Item = &Box<dyn Dex>> + Send> = if first_dex_on_path.support_flashloan() {
            // 如果第一个DEX是闪电贷提供者，其 flashloan_tx 可能已包含第一步交易，
            // 所以我们从路径的第二个DEX开始迭代 (skip(1))。
            // **但要注意**：`extend_flashloan_tx` 的 `FlashResult.coin_out` 的含义。
            //    - 对于Cetus/FlowX/Kriya CLMM, `coin_out` 是闪电贷借入并立即进行一次虚拟swap后得到的“目标币种”。
            //      这意味着第一步DEX操作（借入并转换为另一种币）已经包含在 `flash_loan_result.coin_out` 中。
            //      所以，确实应该 `skip(1)`。
            //    - 对于Navi，`coin_out` 是直接借出的SUI。
            // 所以这个逻辑分支是正确的。
            Box::new(path.path.iter().skip(1))
        } else {
            // 如果使用外部闪电贷 (Navi SUI)，则从路径的第一个DEX开始。
            Box::new(path.path.iter())
        };

        for (i, dex_step) in dex_iterator.enumerate() {
            // 对于DEX迭代器中的第一步 (可能是原始路径的第二步或第一步)，
            // `amount_in_option` 是 `Some(amount_in)` (闪电贷借入的原始金额)。
            // 对于后续步骤，是 `None`。
            // **修正**: `amount_in` 是闪电贷的初始借款额。
            //   `current_active_coin_arg` 是上一步的输出，其全部余额将用于下一步。
            //   所以，`amount_in_option` 应该总是 `None`，因为DEX的 `extend_trade_tx`
            //   通常被设计为消耗整个传入的 `coin_in_arg`。
            //   除非第一个DEX的 `extend_trade_tx` (如果不是闪电贷提供者时) 需要显式amount_in。
            //   当前 `dex.extend_trade_tx` 的 `amount_in` 参数在很多实现中被忽略或只在第一步使用。
            //   为了安全和一致，如果 `extend_trade_tx` 确实需要初始金额，应明确传递。
            //   但大多数内部实现会用整个输入Coin。
            //   这里的 `Some(amount_in)` (如果i=0) 可能是为了兼容某些 `extend_trade_tx` 的特定行为。
            //   然而，更通用的模式是 `current_active_coin_arg` 本身就代表了可用的全部金额。
            //   因此，`None` 应该更合适。
            let amount_in_option = if i == 0 && !first_dex_on_path.support_flashloan() {
                 // 如果是外部闪电贷 (Navi)，则路径的第一步DEX需要知道初始输入金额。
                 Some(amount_in)
            } else {
                 // 如果是DEX自身闪电贷 (第一步已处理) 或路径的后续步骤，则消耗全部输入Coin。
                 None
            };
            current_active_coin_arg = dex_step.extend_trade_tx(&mut ctx, sender, current_active_coin_arg, amount_in_option).await?;
        }

        // 3. 偿还闪电贷
        // `current_active_coin_arg` 此时是整个路径交易完成后最终得到的代币。
        // 这个代币将用于偿还闪电贷。
        let coin_after_repay = if first_dex_on_path.support_flashloan() {
            first_dex_on_path.extend_repay_tx(&mut ctx, current_active_coin_arg, flash_loan_result).await?
        } else {
            self.navi.extend_repay_tx(&mut ctx, current_active_coin_arg, flash_loan_result).await?
        };
        // `coin_after_repay` 是偿还贷款后剩余的代币 (即利润)。

        // 4. (可选) MEV竞价：提交一部分利润作为给验证者的出价。
        if source.is_shio() { // 如果交易来源是Shio (表示这是一个MEV机会，需要竞价)
            let bid_value = source.bid_amount(); // 获取竞价金额
            // 从利润 (`coin_after_repay`) 中分割出 `bid_value` 用于竞价。
            // `ctx.pure(bid_value)` 创建一个 `Argument` 代表 `bid_value`。
            let bid_amount_arg = ctx.pure(bid_value).map_err(|e| eyre!(e))?;
            let coin_for_bid = ctx.split_coin_arg(coin_after_repay.clone(), bid_amount_arg); //分割出用于出价的Coin
            // 调用 Shio 协议提交出价。
            self.shio.submit_bid(&mut ctx, coin_for_bid, bid_value)?;
            // 注意：`coin_after_repay` 在 `split_coin_arg` 后仍然代表分割前的原始Coin（或其剩余部分，取决于split实现）。
            // 如果 `submit_bid` 消耗了 `coin_for_bid`，则 `coin_after_repay` (如果它是可变引用或被重新赋值)
            // 现在代表了支付竞价费用后剩余的利润。
            // 为了清晰，通常 `split_coin_arg` 返回的是分割出来的部分，原Coin参数代表剩余。
            // 这里 `coin_after_repay` 被克隆后用于分割，所以原 `coin_after_repay` 仍代表总利润。
            // `submit_bid` 消耗 `coin_for_bid`。剩余的利润在 `coin_after_repay` 中，需要转移。
            // **修正**：`split_coin_arg` 通常返回分割出来的部分，而原Argument (第一个参数)被修改为剩余部分。
            // 如果是这样，`coin_after_repay` 在 `split_coin_arg` 之后就是支付 MEV bid 后的余额。
            // 假设 `split_coin_arg` 的行为是：第一个参数 `coin_after_repay.clone()` 是输入，
            // 返回的是分割出来的 `coin_for_bid`。原 `coin_after_repay` 不变。
            // 那么，需要从 `coin_after_repay` 中将 `coin_for_bid` 对应的金额减去，
            // 或者更简单地，PTB会自动处理这些Coin的合并和转移。
            // 最终转移给sender的是 `coin_after_repay`。如果 `submit_bid` 成功，
            // 意味着 `coin_for_bid` 被消耗了。`coin_after_repay` 若要转移，
            // 需要确保它代表的是竞价后的余额。
            // 假设 `coin_profit` 在这里指的就是最终要转移给用户的部分。
            // 如果 `submit_bid` 后，我们希望将竞价后的剩余利润转移，那么 `coin_after_repay` 需要更新。
            // 但PTB的逻辑是基于 `Argument` 的，转移的是 `coin_after_repay` 这个 `Argument` 所代表的最终对象。
            // 如果 `split_coin_arg` 返回的是分割出来的，那么 `coin_after_repay` 仍然是总利润。
            // 这意味着 `submit_bid` 消耗 `coin_for_bid`，然后我们把总利润 `coin_after_repay` 转移给用户，这是不正确的。
            // 正确的流程：
            // 1. `total_profit_arg = coin_after_repay`
            // 2. `bid_amount_arg = ctx.pure(bid_value)`
            // 3. `coin_for_bid_arg = ctx.split_coin_arg(total_profit_arg.clone(), bid_amount_arg)`
            // 4. `self.shio.submit_bid(&mut ctx, coin_for_bid_arg, bid_value)?`
            // 5. `remaining_profit_arg = total_profit_arg` (此时 `total_profit_arg` 指向的是分割后剩余的部分)
            // 6. `ctx.transfer_arg(sender, remaining_profit_arg)`
            // 当前代码中 `split_coin_arg` 的第一个参数是 `coin_after_repay.clone()`，这意味着 `coin_after_repay` 本身
            // 作为Argument没有改变。`coin_for_bid` 是新产生的Argument。这是正确的。
            // 所以，`coin_after_repay` 仍然代表竞价前的总利润。
            // 如果要转移竞价后的利润，需要从 `coin_after_repay` 中减去 `coin_for_bid`。
            // 或者，如果 `split_coin_arg` 修改了其第一个参数（使其成为剩余部分），那么直接转移 `coin_after_repay` 是对的。
            // 从 `ProgrammableTransactionBuilder::split_coins` 的行为看，它返回一个新的Argument代表分割出来的部分。
            // 原Coin参数不会被自动修改为剩余部分。
            // 因此，这里的逻辑需要调整，以确保正确转移竞价后的利润。
            // **一个简化的处理**：PTB会自动处理资金流。我们创建了 `coin_for_bid` 并用于支付。
            // 然后将 `coin_after_repay` (总利润) 转移给sender。Sui会自动处理合并和扣除。
            // 例如，如果 `coin_after_repay` 代表100 SUI，`coin_for_bid` 从中分出10 SUI用于支付。
            // 那么最终转移给sender的 `coin_after_repay` (如果它指向同一个逻辑上的币)，应该只剩90 SUI。
            // PTB的 `transfer_object` 会转移整个对象。
            // 这里的 `coin_after_repay` 是一个 `Argument`，它指向一个逻辑上的Coin。
            // `split_coin_arg` 会从这个逻辑Coin中分出一部分。
            // 如果 `coin_after_repay` 是 `Result(cmd_idx)`，那么它代表该命令产生的Coin。
            // `split_coin_arg` 会以它为输入，产生新的 `Result(new_cmd_idx)`。
            // 如果 `coin_after_repay` 是 `GasCoin` 或 `Input(k)`，则它代表一个具体的对象。
            // 假设 `coin_after_repay` 是一个指向可变Coin的 `Argument`。
            // `split_coin_arg` 应该返回分割出的部分，并修改原 `Argument` 指向剩余部分。
            // 但 `split_coin_arg` 的实现是 `Command::SplitCoins(coin, vec![amount])` 后返回 `Argument::Result(last_idx)`。
            // 这意味着 `Argument::Result(last_idx)` 是分割出来的部分。原 `coin` Argument 何去何从？
            // `Command::SplitCoins` 的第一个参数 `coin` 是被分割的币，它本身会被消耗或修改。
            // 所以，`coin_after_repay` 在 `split_coin_arg` 之后应该代表剩余部分。
            // 因此，后续转移 `coin_after_repay` 是正确的。
        }


        // 5. 将最终利润 (偿还贷款并可能支付MEV竞价后剩余的代币) 转移给发送者。
        ctx.transfer_arg(sender, coin_after_repay);

        let ptb_finished = ctx.ptb.finish(); // 完成PTB构建

        // 6. 根据MEV竞价需求调整交易摘要 (通过增加Gas预算)
        let mut final_tx_data =
            TransactionData::new_programmable(sender, gas_coins.clone(), ptb_finished.clone(), GAS_BUDGET, gas_price);

        // 如果提供了“机会交易”的摘要 (通常在MEV场景中，我们的交易需要“击败”或排在某个特定交易之后)
        if let Some(opportunity_tx_digest) = source.opp_tx_digest() {
            // MEV竞价的一个策略是确保当前套利/竞价交易的哈希值（摘要）在字典序上大于机会交易的哈希值。
            // 这有时可以影响交易的排序（虽然不是Sui共识的保证，但可能在某些MEV中继或提议者逻辑中起作用）。
            // 摘要是通过对交易数据进行哈希计算得到的。Gas预算是交易数据的一部分。
            // 因此，通过微调Gas预算，可以改变交易数据的整体内容，从而改变其摘要。
            // 这里循环增加Gas预算，直到生成的交易摘要大于机会交易的摘要。
            let mut current_gas_budget = GAS_BUDGET;
            while final_tx_data.digest() <= opportunity_tx_digest {
                current_gas_budget += 1; // 每次增加1个单位的Gas预算 (非常小的调整)
                final_tx_data = TransactionData::new_programmable(
                    sender,
                    gas_coins.clone(), // Gas币对象列表
                    ptb_finished.clone(),  // PTB (保持不变)
                    current_gas_budget,  // 更新后的Gas预算
                    gas_price,           // Gas价格 (保持不变)
                );
            }
        };

        // 因为闪电贷的输入资金是在交易内部产生的，所以不像常规Swap那样有外部的“模拟输入代币”。
        // 因此返回 `None` 作为模拟代币部分。
        Ok((final_tx_data, None))
    }
}

impl TradeCtx {
    /// `new` 构造函数 (关联函数)
    #[allow(dead_code)] // 允许存在未使用的代码 (因为通常用 Default::default())
    pub fn new() -> Self {
        Self::default() // 等同于 TradeCtx { ptb: ProgrammableTransactionBuilder::new(), command_count: 0 }
    }

    /// `command` 方法
    ///
    /// 向内部的 `ProgrammableTransactionBuilder` 添加一个命令，并增加命令计数。
    pub fn command(&mut self, cmd: Command) {
        self.ptb.command(cmd);
        self.command_count += 1;
    }

    /// `transfer_arg` 方法
    ///
    /// 向PTB添加一个将 `coin_arg` 转移给 `recipient` 的命令。
    pub fn transfer_arg(&mut self, recipient: SuiAddress, coin_arg: Argument) {
        self.ptb.transfer_object(recipient, coin_arg).unwrap(); // transfer_object返回Result,这里简化处理
        self.command_count += 1;
    }

    /// `last_command_idx` 方法
    ///
    /// 返回最后一个添加到PTB的命令的索引 (基于0)。
    pub fn last_command_idx(&self) -> u16 {
        // 如果命令计数为0 (即没有命令)，则减1会下溢。
        // PTB的索引是从0开始的，所以如果command_count是1，最后一个索引是0。
        if self.command_count == 0 {
            panic!("PTB中没有命令，无法获取最后一个命令的索引");
        }
        self.command_count - 1
    }

    /// `split_coin` 方法
    ///
    /// 从一个已有的 `ObjectRef` (代表一个Coin对象) 中分割出指定数量 (`amount`) 的代币。
    /// 返回一个 `Argument` 代表新分割出来的、面额为 `amount` 的Coin对象。
    ///
    /// 参数:
    /// - `coin_ref`: 要分割的原始Coin对象的引用。
    /// - `amount`: 要分割出来的数量。
    ///
    /// 返回:
    /// - `Result<Argument>`: 代表新分割出Coin的 `Argument`。
    pub fn split_coin(&mut self, coin_ref: ObjectRef, amount: u64) -> Result<Argument> {
        // 将 ObjectRef 转换为 ObjectArg (作为PTB的输入对象)
        let coin_obj_arg = self.obj(ObjectArg::ImmOrOwnedObject(coin_ref)).map_err(|e| eyre!(e))?;
        // 将 u64 金额转换为纯值 Argument
        let amount_arg = self.pure(amount).map_err(|e| eyre!(e))?;

        // 调用内部的 split_coin_arg 方法
        Ok(self.split_coin_arg(coin_obj_arg, amount_arg))
    }

    /// `split_coin_arg` 方法
    ///
    /// 从一个已作为 `Argument` 传入的Coin对象中分割出由另一个 `Argument` (代表数量) 指定的代币。
    /// 这更通用，因为数量也可以是之前命令的结果。
    ///
    /// 参数:
    /// - `coin_arg`: 代表要被分割的Coin对象的 `Argument`。
    /// - `amount_arg`: 代表要分割出数量的 `Argument`。
    ///
    /// 返回:
    /// - `Argument`: 代表新分割出Coin的 `Argument` (通常是 `Argument::Result(cmd_idx)`)。
    pub fn split_coin_arg(&mut self, coin_arg: Argument, amount_arg: Argument) -> Argument {
        // 添加 SplitCoins 命令到PTB。
        // `coin_arg` 是被分割的币，`vec![amount_arg]` 是一个包含单个元素的向量，指定分割出的数量。
        // SplitCoins 命令会产生一个新的Coin对象（分割出来的部分）。
        self.command(Command::SplitCoins(coin_arg, vec![amount_arg]));
        let last_idx = self.last_command_idx(); // 获取 SplitCoins 命令的索引
        // 返回一个指向该命令结果 (即新分割出的Coin) 的 Argument。
        Argument::Result(last_idx)
    }

    // --- Sui框架标准模块 (如 balance, coin) 的便捷调用方法 ---
    // 这些方法简化了调用Sui框架标准功能的PTB命令构建。

    /// `balance_destroy_zero`
    /// 调用 `sui::balance::destroy_zero<CoinType>(balance)`
    /// 销毁一个面额为零的 `Balance` 对象。
    pub fn balance_destroy_zero(&mut self, balance_arg: Argument, coin_type: TypeTag) -> Result<()> {
        self.build_command(
            SUI_FRAMEWORK_PACKAGE_ID, // "0x2"
            "balance",
            "destroy_zero",
            vec![coin_type], // 泛型类型参数 <CoinType>
            vec![balance_arg],  // 函数参数 (balance)
        )?;
        Ok(())
    }

    /// `balance_zero`
    /// 调用 `sui::balance::zero<CoinType>()`
    /// 创建一个指定 `CoinType` 的、面额为零的 `Balance` 对象。
    /// 返回代表这个新创建的零面额Balance的 `Argument`。
    pub fn balance_zero(&mut self, coin_type: TypeTag) -> Result<Argument> {
        self.build_command(SUI_FRAMEWORK_PACKAGE_ID, "balance", "zero", vec![coin_type], vec![])?;
        let last_idx = self.last_command_idx();
        Ok(Argument::Result(last_idx)) // 返回新Balance的Argument
    }

    /// `coin_from_balance`
    /// 调用 `sui::coin::from_balance<CoinType>(balance, ctx)`
    /// 从一个 `Balance` 对象创建一个新的 `Coin` 对象。
    /// 返回代表这个新Coin的 `Argument`。
    pub fn coin_from_balance(&mut self, balance_arg: Argument, coin_type: TypeTag) -> Result<Argument> {
        self.build_command(
            SUI_FRAMEWORK_PACKAGE_ID,
            "coin",
            "from_balance",
            vec![coin_type],
            vec![balance_arg],
        )?;
        let last_idx = self.last_command_idx();
        Ok(Argument::Result(last_idx)) // 返回新Coin的Argument
    }

    /// `coin_into_balance`
    /// 调用 `sui::coin::into_balance<CoinType>(coin)`
    /// 将一个 `Coin` 对象转换为一个 `Balance` 对象。
    /// 返回代表这个新Balance的 `Argument`。
    pub fn coin_into_balance(&mut self, coin_arg: Argument, coin_type: TypeTag) -> Result<Argument> {
        self.build_command(
            SUI_FRAMEWORK_PACKAGE_ID,
            "coin",
            "into_balance",
            vec![coin_type],
            vec![coin_arg],
        )?;
        let last_idx = self.last_command_idx();
        Ok(Argument::Result(last_idx)) // 返回新Balance的Argument
    }

    /// `build_command` (内联私有辅助函数)
    ///
    /// 一个通用的辅助函数，用于构建并添加一个 `Command::move_call` 到PTB。
    #[inline]
    fn build_command(
        &mut self,
        package_id: ObjectID,
        module_name_str: &str,
        function_name_str: &str,
        type_arguments: Vec<TypeTag>,
        call_arguments: Vec<Argument>,
    ) -> Result<()> {
        // 将字符串转换为Identifier类型
        let module_ident = Identifier::new(module_name_str).map_err(|e| eyre!(e))?;
        let function_ident = Identifier::new(function_name_str).map_err(|e| eyre!(e))?;
        // 添加Move调用命令
        self.command(Command::move_call(package_id, module_ident, function_ident, type_arguments, call_arguments));
        Ok(())
    }
}

// --- 为 TradeCtx 实现 Deref 和 DerefMut ---
// 这使得 TradeCtx 的实例可以像它内部的 ProgrammableTransactionBuilder 一样被直接调用方法。
// 例如，可以直接在 `TradeCtx` 实例上调用 `ctx.pure(...)` 或 `ctx.obj(...)`，
// 它们实际上会调用 `ctx.ptb.pure(...)` 和 `ctx.ptb.obj(...)`。

impl Deref for TradeCtx {
    type Target = ProgrammableTransactionBuilder; // 指定解引用后的目标类型

    fn deref(&self) -> &Self::Target {
        &self.ptb // 返回对内部ptb的不可变引用
    }
}

impl DerefMut for TradeCtx {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ptb // 返回对内部ptb的可变引用
    }
}

// --- 为 TradeResult 实现 PartialEq 和 PartialOrd ---
// 这使得 TradeResult 的实例可以进行比较，主要基于 `amount_out` 字段。
// 用于在多条路径的模拟结果中找到最优的那个。

impl PartialEq for TradeResult {
    fn eq(&self, other: &Self) -> bool {
        self.amount_out == other.amount_out // 基于输出金额判断是否相等
    }
}

impl PartialOrd for TradeResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.amount_out.partial_cmp(&other.amount_out) // 基于输出金额进行比较
    }
}

/// `Path` 结构体
///
/// 代表一条交易路径，即一个或多个连续的DEX交换。
#[derive(Default, Clone)] // Default用于创建空路径, Clone用于复制路径
pub struct Path {
    // `path` 字段是一个向量，存储了路径上按顺序排列的每个DEX的实例。
    // 每个DEX实例都是一个 `Box<dyn Dex>` 特征对象，允许路径包含不同协议的DEX。
    pub path: Vec<Box<dyn Dex>>,
}

impl Path {
    /// `new` 构造函数
    pub fn new(path_dexes: Vec<Box<dyn Dex>>) -> Self {
        Self { path: path_dexes }
    }

    /// `is_empty` 方法
    /// 检查路径是否为空 (不包含任何DEX)。
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// `is_disjoint` 方法
    ///
    /// 判断当前路径与另一条路径 `other` 是否不相交 (即不包含任何相同的DEX池对象)。
    /// 这在组合路径或避免循环时可能有用。
    ///
    /// 实现逻辑:
    /// 1. 将两条路径中的DEX实例 (通过其 `object_id()`) 分别收集到两个 `HashSet` 中。
    /// 2. 使用 `HashSet::is_disjoint()` 方法判断两个集合是否有交集。
    pub fn is_disjoint(&self, other: &Self) -> bool {
        // 将 self.path 中的 dex (通过其 object_id) 收集到 HashSet a
        // `Box<dyn Dex>` 已经实现了 Hash 和 Eq (基于 object_id)，所以可以直接用于 HashSet。
        let set_a = self.path.iter().collect::<HashSet<_>>();
        // 将 other.path 中的 dex 收集到 HashSet b
        let set_b = other.path.iter().collect::<HashSet<_>>();
        set_a.is_disjoint(&set_b) // 判断两个集合是否没有共同元素
    }

    /// `coin_in_type` 方法
    ///
    /// 返回路径的初始输入代币类型 (即路径中第一个DEX的输入代币类型)。
    /// 如果路径为空，则会panic。
    pub fn coin_in_type(&self) -> String {
        // 假设路径非空，否则 self.path[0] 会panic。
        // 调用者应确保路径非空或先检查 is_empty()。
        self.path[0].coin_in_type()
    }

    /// `coin_out_type` 方法
    ///
    /// 返回路径的最终输出代币类型 (即路径中最后一个DEX的输出代币类型)。
    /// 如果路径为空，则会panic。
    pub fn coin_out_type(&self) -> String {
        // 假设路径非空。
        self.path.last().unwrap().coin_out_type() // last()返回Option, unwrap()假设非空
    }

    /// `contains_pool` 方法
    ///
    /// 检查路径中是否包含具有指定 `pool_id` 的DEX池。
    ///
    /// 参数:
    /// - `pool_id`: (可选) 要检查的池的ObjectID。如果为 `None`，则方法返回 `false`。
    ///
    /// 返回:
    /// - `bool`: 如果路径包含该池ID则为true，否则为false。
    pub fn contains_pool(&self, pool_id_option: Option<ObjectID>) -> bool {
        if let Some(target_pool_id) = pool_id_option {
            // 遍历路径中的每个DEX，检查其 object_id() 是否与目标ID匹配。
            self.path.iter().any(|dex_instance| dex_instance.object_id() == target_pool_id)
        } else {
            // 如果 pool_id_option 是 None，则认为不包含任何特定池 (或此检查无意义)。
            false
        }
    }
}

/// 为 `Path` 实现 `fmt::Debug` trait，用于调试时打印路径信息。
impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 将路径中每个DEX的Debug表示连接成一个字符串。
        // `Box<dyn Dex>` 已经实现了 Debug (在 defi::mod.rs 中)。
        let path_str_vec: Vec<String> = self.path.iter().map(|dex_instance| format!("{:?}", dex_instance)).collect();
        write!(f, "[{}]", path_str_vec.join(" -> ")) // 用 " -> " 连接各个DEX步骤
    }
}
