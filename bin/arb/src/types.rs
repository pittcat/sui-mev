// 该文件 `types.rs` 定义了在整个套利机器人应用程序中使用的多种核心数据类型，
// 特别是与 `burberry` 引擎的事件驱动架构相关的 `Event` (事件) 和 `Action` (动作) 枚举。
// 它还定义了 `Source` 枚举，用于追踪和管理套利机会的来源及其特定属性（例如MEV竞价信息）。
//
// 文件概览:
// - `Action` 枚举: 定义了机器人策略可能产生的不同类型的输出动作。
//   例如：发送Telegram通知、执行公开的Sui交易、提交Shio MEV竞价。
//   它为具体动作类型实现了 `From` trait，方便将它们转换为通用的 `Action` 枚举。
// - `Event` 枚举: 定义了机器人可以接收和处理的不同类型的输入事件。
//   例如：新的公开交易的效果和Sui事件、新的私有交易数据、新的Shio MEV机会。
//   `#[allow(clippy::large_enum_variant)]` 属性用于抑制编译器关于某些枚举成员可能比其他成员大很多（在内存占用上）的警告。
// - `Source` 枚举: 表示一个套利机会的来源。
//   - `Public`: 来自公开的链上活动。
//   - `Shio`: 来自Shio MEV协议，包含机会交易摘要、当前计算的竞价金额、以及多个时间戳（开始处理时间、发现套利时间、竞价截止时间）。
//   - `ShioDeadlineMissed`: 表示一个Shio机会在发现套利时已经错过了其竞价截止时间。
//   它还实现了一些方法来检查类型、访问数据和更新状态，以及一个 `fmt::Display` 实现用于日志输出。
//
// 这些类型是连接不同组件（收集器、策略、执行器）的纽带。

// 引入标准库及第三方库
use std::fmt; // 用于格式化输出 (实现 Display trait)

use burberry::executor::telegram_message::Message as BurberryTelegramMessage; // 从 `burberry` 框架引入其Telegram消息类型，并重命名以避免与可能的其他Message类型冲突
use shio::ShioItem; // 从 `shio` crate 引入Shio MEV机会的类型
use sui_json_rpc_types::{SuiEvent, SuiTransactionBlockEffects}; // Sui RPC相关的类型：Sui原生事件、交易块执行效果
use sui_types::{digests::TransactionDigest, transaction::TransactionData}; // Sui核心类型：交易摘要、交易数据

/// `Action` 枚举
///
/// 代表套利策略在分析事件后可能产生的不同类型的输出动作。
/// 这些动作随后会被提交给相应的执行器 (Executor) 进行处理。
#[derive(Debug, Clone)] // 派生Debug和Clone trait
pub enum Action {
    /// 动作：通过Telegram发送一条通知消息。
    /// 包含一个 `BurberryTelegramMessage` 实例。
    NotifyViaTelegram(BurberryTelegramMessage),

    /// 动作：执行一笔公开的Sui交易。
    /// 包含一个 `TransactionData` 实例 (已签名的交易)。
    ExecutePublicTx(TransactionData),

    /// 动作：向Shio MEV协议提交一个竞价。
    /// 包含一个元组：
    ///   - `TransactionData`: 用于执行套利并支付竞价的交易。
    ///   - `u64`: 竞价金额。
    ///   - `TransactionDigest`: Shio机会对应的原始机会交易的摘要。
    ShioSubmitBid((TransactionData, u64, TransactionDigest)),
}

// --- 为 Action 枚举实现 From trait ---
// 这些 `From` trait的实现使得可以方便地从具体的动作数据类型创建 `Action` 枚举实例。
// 例如，可以直接将 `BurberryTelegramMessage` 转换为 `Action::NotifyViaTelegram(...)`。

impl From<BurberryTelegramMessage> for Action {
    fn from(msg: BurberryTelegramMessage) -> Self {
        Self::NotifyViaTelegram(msg) // 将 BurberryTelegramMessage 包装成 Action::NotifyViaTelegram
    }
}

impl From<TransactionData> for Action {
    fn from(tx_data: TransactionData) -> Self {
        Self::ExecutePublicTx(tx_data) // 将 TransactionData 包装成 Action::ExecutePublicTx
    }
}

impl From<(TransactionData, u64, TransactionDigest)> for Action {
    fn from(data: (TransactionData, u64, TransactionDigest)) -> Self { // 直接解构元组
        Self::ShioSubmitBid(data) // 将元组包装成 Action::ShioSubmitBid
    }
}


/// `Event` 枚举
///
/// 代表套利策略可以接收和处理的不同类型的输入事件。
/// 这些事件通常由不同的收集器 (Collector) 产生。
/// `#[allow(clippy::large_enum_variant)]` 用于抑制Clippy关于某些枚举成员
/// (例如 `PublicTx`) 可能比其他成员（例如 `PrivateTx` 如果 `TransactionData` 通常较小）
/// 在内存占用上大很多的警告。这在枚举设计中是正常的，尤其当不同事件携带的数据量差异很大时。
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)] // 派生Clone和Debug trait
pub enum Event {
    /// 事件：新的公开交易及其效果。
    /// 包含：
    ///   - `SuiTransactionBlockEffects`: 交易的执行效果。
    ///   - `Vec<SuiEvent>`: 该交易产生的所有Sui原生事件。
    PublicTx(SuiTransactionBlockEffects, Vec<SuiEvent>),

    /// 事件：新的私有交易数据。
    /// 包含一个 `TransactionData` 实例 (这通常是未签名的交易数据)。
    PrivateTx(TransactionData),

    /// 事件：来自Shio MEV协议的新机会。
    /// 包含一个 `ShioItem` 实例。
    Shio(ShioItem),
}

/// `Source` 枚举
///
/// 表示一个套利机会的来源及其相关的特定信息。
/// 这对于后续的决策（例如是否竞价、竞价多少、截止时间等）非常重要。
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)] // 派生常用trait，Copy表示此类型可以按位复制
pub enum Source {
    /// 来源：公开的链上活动。
    /// 没有额外的特定信息。
    Public,

    /// 来源：Shio MEV协议。
    /// 包含与MEV竞价相关的详细信息：
    Shio {
        opp_tx_digest: TransactionDigest, // Shio机会对应的原始“机会交易”的摘要。
        bid_amount: u64,                  // 当前计算出的或已提交的竞价金额。
        start: u64,                       // 机器人开始处理此Shio机会的时间戳 (毫秒)。
        arb_found: u64,                   // 机器人实际找到套利方案的时间戳 (毫秒)。
        deadline: u64,                    // 此Shio机会的竞价截止时间戳 (毫秒)。
    },

    /// 来源：一个Shio MEV机会，但在机器人发现套利方案时，其竞价截止时间已过。
    /// 用于记录和分析错过的机会。
    ShioDeadlineMissed {
        start: u64,     // 开始处理时间戳。
        arb_found: u64, // 发现套利方案的时间戳 (此时已晚于deadline)。
        deadline: u64,  // 已过的截止时间戳。
    },
}

/// 为 `Source` 枚举实现 `fmt::Display` trait，用于日志输出和调试。
impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Public => write!(f, "来源: 公开"), // Public
            Source::Shio {
                start,
                arb_found,
                deadline,
                bid_amount, // 添加 bid_amount 到显示中
                .. // 其他字段如 opp_tx_digest 不直接显示，但可以通过Debug查看
            } => write!(
                f,
                "来源: Shio (开始时间={}, 截止时间={}, 时间窗口={}ms, 套利发现时间={}, 提前量={}ms, 竞价={})", // Shio(...)
                *start,
                *deadline,
                (*deadline).saturating_sub(*start), // 计算时间窗口，使用saturating_sub防止下溢
                *arb_found,
                (*deadline).saturating_sub(*arb_found), // 计算距离截止时间的提前量
                *bid_amount
            ),
            Source::ShioDeadlineMissed {
                start,
                arb_found,
                deadline,
            } => write!(
                f,
                "来源: Shio机会已错过 (开始时间={}, 截止时间={}, 时间窗口={}ms, 套利发现时间={}, 已逾期={}ms)", // ShioDeadlineMissed(...)
                *start,
                *deadline,
                (*deadline).saturating_sub(*start),
                *arb_found,
                (*arb_found).saturating_sub(*deadline) // 计算已逾期的时间
            ),
        }
    }
}

impl Source {
    /// `is_shio` 方法
    ///
    /// 判断来源是否是 `Source::Shio` (不包括 `ShioDeadlineMissed`)。
    pub fn is_shio(&self) -> bool {
        matches!(self, Source::Shio { .. })
    }

    /// `opp_tx_digest` 方法
    ///
    /// 如果来源是 `Source::Shio`，则返回机会交易的摘要；否则返回 `None`。
    pub fn opp_tx_digest(&self) -> Option<TransactionDigest> {
        match self {
            Source::Shio { opp_tx_digest, .. } => Some(*opp_tx_digest),
            _ => None,
        }
    }

    /// `deadline` 方法
    ///
    /// 如果来源是 `Source::Shio`，则返回其竞价截止时间戳；否则返回 `None`。
    pub fn deadline(&self) -> Option<u64> {
        match self {
            Source::Shio { deadline, .. } => Some(*deadline),
            _ => None, // Public 和 ShioDeadlineMissed 没有这个意义上的“未来”截止时间
        }
    }

    /// `bid_amount` 方法
    ///
    /// 如果来源是 `Source::Shio`，则返回其当前记录的竞价金额；否则返回0。
    pub fn bid_amount(&self) -> u64 {
        match self {
            Source::Shio { bid_amount, .. } => *bid_amount,
            _ => 0,
        }
    }

    /// `with_bid_amount` 方法 (builder pattern)
    ///
    /// 如果来源是 `Source::Shio`，则更新其 `bid_amount` 字段并返回新的 `Source` 实例。
    /// 其他来源类型则原样返回。
    /// 这用于在计算出竞价金额后更新 `Source` 对象。
    pub fn with_bid_amount(self, new_bid_amount: u64) -> Self {
        match self {
            Source::Shio {
                opp_tx_digest,
                start,
                deadline,
                arb_found,
                .. // 旧的 bid_amount 被忽略
            } => Source::Shio { // 创建一个新的 Source::Shio 实例
                opp_tx_digest,
                bid_amount: new_bid_amount, // 使用新的竞价金额
                start,
                deadline,
                arb_found,
            },
            _ => self, // 其他来源类型保持不变
        }
    }

    /// `with_arb_found_time` 方法 (builder pattern)
    ///
    /// 如果来源是 `Source::Shio`，则更新其 `arb_found` (套利发现时间) 字段。
    /// 同时，它会检查更新后的 `arb_found` 是否仍然在 `deadline` 之前。
    /// - 如果仍在截止时间之前，则返回更新后的 `Source::Shio`。
    /// - 如果已超过截止时间，则将来源转换为 `Source::ShioDeadlineMissed`。
    /// 其他来源类型则原样返回。
    pub fn with_arb_found_time(self, new_arb_found_time: u64) -> Self {
        match self {
            Source::Shio {
                opp_tx_digest,
                start,
                deadline,
                bid_amount,
                .. // 旧的 arb_found 被忽略
            } => {
                if new_arb_found_time < deadline { // 如果发现套利时仍在截止时间之前
                    Source::Shio {
                        opp_tx_digest,
                        bid_amount,
                        start,
                        arb_found: new_arb_found_time, // 更新套利发现时间
                        deadline,
                    }
                } else { // 如果已错过截止时间
                    Source::ShioDeadlineMissed {
                        start,
                        arb_found: new_arb_found_time,
                        deadline,
                    }
                }
            }
            _ => self, // 其他来源类型保持不变
        }
    }
}
