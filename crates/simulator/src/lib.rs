// 该文件 `lib.rs` 是 `simulator` crate (库) 的根文件和主入口。
// `simulator` crate 的主要功能是定义通用的交易模拟器接口 (`Simulator` trait)
// 以及与模拟相关的核心数据结构，如 `SimulateResult` (模拟结果), `SimEpoch` (纪元信息),
// 和 `SimulateCtx` (模拟上下文)。
// 它还声明并重新导出了具体的模拟器实现，如 `DBSimulator`, `ReplaySimulator` (来自 `db_simulator` 模块)
// 和 `HttpSimulator` (来自 `http_simulator` 模块)。
//
// **文件概览 (File Overview)**:
// 这个文件是 `simulator` 库的“总纲”和“蓝图”。它规定了：
// 1.  一个“模拟器”应该长什么样，能做什么（通过 `Simulator` trait/接口）。
// 2.  当模拟完成时，结果应该包含哪些信息（通过 `SimulateResult` 结构体）。
// 3.  在进行模拟时，需要提供哪些环境信息（通过 `SimulateCtx` 结构体和 `SimEpoch` 结构体）。
// 它还把项目中其他文件定义的具体模拟器（如 `DBSimulator`）“介绍”给大家，让其他部分可以直接使用。
//
// **核心定义 (Key Definitions)**:
//
// 1.  **模块声明 (Module Declarations)**:
//     -   `mod db_simulator;`: 声明了 `db_simulator` 子模块，其中可能包含了 `DBSimulator` 和 `ReplaySimulator` 的实现。
//     -   `mod http_simulator;`: 声明了 `http_simulator` 子模块，其中包含了 `HttpSimulator` 的实现。
//
// 2.  **类型重新导出 (Type Re-exports)**:
//     -   `pub use db_simulator::{DBSimulator, ReplaySimulator};`
//     -   `pub use http_simulator::HttpSimulator;`
//         这两行代码将子模块中定义的具体模拟器类型重新导出到 `simulator` crate的顶层命名空间。
//         这意味着 `simulator` crate的使用者可以直接通过 `simulator::DBSimulator` 来访问这些类型，
//         而不需要关心它们具体是在哪个子模块中定义的。
//
// 3.  **`SimulateResult` 结构体**:
//     -   用于封装一次交易模拟执行后返回的各种结果信息。
//     -   `effects: SuiTransactionBlockEffects`: 交易的核心执行效果，如状态变更摘要、gas消耗、对象创建/修改/删除的引用等。
//     -   `events: SuiTransactionBlockEvents`: 交易执行过程中发出的所有Sui事件的结构化列表。
//     -   `object_changes: Vec<ObjectReadResult>`: 一个列表，包含了所有在模拟过程中被读取或写入的对象的详细状态。
//         `ObjectReadResult` 包含了对象ID、版本、内容及其作为输入对象的种类。
//         （注意：`HttpSimulator` 的实现可能不填充此字段）。
//     -   `balance_changes: Vec<BalanceChange>`: 交易执行后，与交易相关的各个地址的代币余额变化列表。
//     -   `cache_misses: u64`: （主要与 `DBSimulator` 相关）在模拟过程中，底层对象缓存未命中的次数。
//
// 4.  **`SimEpoch` 结构体**:
//     -   用于存储与Sui网络当前或特定纪元（Epoch）相关的信息，这些信息在模拟时可能需要。
//     -   `epoch_id: EpochId`: 纪元的唯一ID。
//     -   `epoch_start_timestamp: CheckpointTimestamp`: 纪元开始时的时间戳（毫秒）。
//     -   `epoch_duration_ms: u64`: 纪元的持续时长（毫秒）。
//     -   `gas_price: u64`: 当前纪元的参考Gas价格。
//     -   它还实现了一个 `is_stale()` 方法，用于判断当前缓存的 `SimEpoch` 信息是否可能已“过时”
//         （通过比较当前系统时间与 `epoch_start_timestamp + epoch_duration_ms`）。
//         **注意**: `is_stale()` 的当前逻辑是 `now < start + duration`，这似乎是在判断纪元是否“尚未结束”，
//         而不是判断信息是否“陈旧需要更新”。如果纪元尚未结束，信息通常被认为是“新鲜”的。
//         如果其意图是检查信息是否“已过时”（即纪元已结束），逻辑应为 `now >= start + duration`。
//         或者，如果“stale”意味着“太老了，需要刷新以获取最新可能的gas价格等”，那么这个逻辑可能需要结合一个额外的最大缓存时间。
//         当前的解释是，它返回 `true` 表示“纪元尚未结束，信息仍然有效/新鲜”。
//     -   它还实现了 `From<SuiSystemStateSummary> for SimEpoch`，允许从Sui SDK提供的 `SuiSystemStateSummary` 对象方便地创建 `SimEpoch`。
//
// 5.  **`SimulateCtx` 结构体 (模拟上下文 / Simulate Context)**:
//     -   封装了执行一次特定交易模拟时所需的所有上下文环境信息。
//     -   `epoch: SimEpoch`: 当前模拟所依据的纪元信息。
//     -   `override_objects: Vec<ObjectReadResult>`: 一个对象列表，用于在模拟时“覆盖”链上这些对象的真实状态。
//         这对于模拟特定场景（如MEV机会分析）或测试非常有用。
//     -   `borrowed_coin: Option<(Object, u64)>`: (可选) 一个元组，包含一个模拟的“借入代币” (`Object`) 及其数量 (`u64`)。
//         这主要用于模拟闪电贷的第一步，即假设我们已经通过闪电贷获得了这笔资金。
//         `DBSimulator` 会将这个模拟代币添加到其内部的 `OverrideCache` 和输入对象列表中。
//     -   提供了 `new()`, `with_borrowed_coin()`, `with_gas_price()` 等辅助方法来方便地创建和修改上下文。
//
// 6.  **`Simulator` Trait (模拟器接口 / Simulator Interface)**:
//     -   定义了所有具体模拟器类型（如 `DBSimulator`, `HttpSimulator`）必须实现的通用行为。
//     -   `Send + Sync`: 约束表明实现了此trait的类型必须是线程安全的。
//     -   **`simulate(&self, tx: TransactionData, ctx: SimulateCtx) -> Result<SimulateResult>`**:
//         核心的异步方法，接收一个未签名的交易数据 (`TransactionData`) 和一个模拟上下文 (`SimulateCtx`)，
//         执行模拟，并返回一个包含结果的 `SimulateResult`。
//     -   **`get_object(&self, obj_id: &ObjectID) -> Option<Object>`**:
//         异步方法，用于从模拟器（或其底层存储/RPC）获取指定ObjectID的最新对象数据。
//     -   **`name(&self) -> &str`**: 返回模拟器的名称字符串 (例如 "DBSimulator", "HttpSimulator")。
//     -   **`get_object_layout(&self, _: &ObjectID) -> Option<MoveStructLayout>`**:
//         (可选提供) 获取对象的Move结构布局。提供了一个默认实现，直接返回 `None`。
//         具体的模拟器如果能提供此功能，则应重写此方法。`DBSimulator` 实现了它。
//
// **用途 (Purpose in Project)**:
// -   **抽象和统一**: `Simulator` trait 提供了一个统一的接口，使得项目中的其他部分（如套利策略模块）
//     可以以同样的方式与不同类型的模拟器进行交互，而无需关心其具体实现细节。
// -   **模拟环境配置**: `SimulateCtx` 和 `SimEpoch` 允许精确地配置模拟执行的环境，
//     包括链状态（通过覆盖对象）、纪元参数和Gas价格。
// -   **结果标准化**: `SimulateResult` 提供了一个标准化的结构来承载模拟的各种输出，方便上层逻辑进行分析和决策。

// 声明子模块 db_simulator 和 http_simulator。
// 这些模块包含了具体的模拟器实现。
mod db_simulator;
mod http_simulator;

// 引入 async_trait 宏，用于在trait中定义异步方法。
use async_trait::async_trait;
// 引入 eyre 库的 Result 类型，用于错误处理。
use eyre::Result;
// 引入 Move 核心类型中的 MoveStructLayout，用于描述Move结构体的布局。
use move_core_types::annotated_value::MoveStructLayout;
// 引入 Sui JSON RPC 类型，这些是Sui节点RPC接口返回数据时使用的数据结构。
use sui_json_rpc_types::{BalanceChange, SuiTransactionBlockEffects, SuiTransactionBlockEvents};
// 引入 Sui 核心类型库中的各种基本类型。
use sui_types::{
    base_types::ObjectID, // 对象ID
    committee::EpochId,   // Epoch ID (纪元ID)
    messages_checkpoint::CheckpointTimestamp, // 检查点时间戳 (通常是毫秒)
    object::Object,       // Sui对象结构
    sui_system_state::sui_system_state_summary::SuiSystemStateSummary, // Sui系统状态摘要，用于获取纪元信息
    transaction::{ObjectReadResult, TransactionData}, // 对象读取结果 (包含对象及其元数据), 未签名交易数据
};

// 从子模块中重新导出具体的模拟器类型，使得外部可以直接通过 `simulator::DBSimulator` 等路径访问。
pub use db_simulator::{DBSimulator, ReplaySimulator}; // DBSimulator 和 ReplaySimulator (一种特殊的DBSimulator)
pub use http_simulator::HttpSimulator;               // HttpSimulator (通过RPC进行模拟)

/// `SimulateResult` 结构体
///
/// 封装了一次交易模拟执行后返回的各种结果信息。
#[derive(Debug, Clone)] // 自动派生Debug (用于调试打印) 和 Clone (用于创建副本) trait。
pub struct SimulateResult {
    /// `effects`: 交易的核心执行效果。
    /// 例如，状态变更摘要、gas消耗详情、被修改/创建/删除的对象的引用列表等。
    pub effects: SuiTransactionBlockEffects,
    /// `events`: 交易执行过程中发出的所有Sui事件的结构化列表。
    pub events: SuiTransactionBlockEvents,
    /// `object_changes`: 一个列表，包含了所有在模拟过程中被读取或写入的对象的详细状态。
    /// 注意：`HttpSimulator` 可能不填充此字段。
    pub object_changes: Vec<ObjectReadResult>,
    /// `balance_changes`: 交易执行后，与交易相关的各个地址的代币余额变化列表。
    pub balance_changes: Vec<BalanceChange>,
    /// `cache_misses`: (主要与 `DBSimulator` 相关) 在模拟过程中，底层对象缓存未命中的次数。
    pub cache_misses: u64,
}

/// `SimEpoch` 结构体
///
/// 用于存储与Sui网络当前或特定纪元（Epoch）相关的信息。
#[derive(Debug, Clone, Copy, Default)] // Default 可以创建 SimEpoch::default() 实例 (所有字段为0或默认)
pub struct SimEpoch {
    pub epoch_id: EpochId,                          // 纪元的唯一ID
    pub epoch_start_timestamp: CheckpointTimestamp, // 纪元开始时的时间戳（毫秒）
    pub epoch_duration_ms: u64,                     // 纪元的持续时长（毫秒）
    pub gas_price: u64,                             // 当前纪元的参考Gas价格 (以MIST为单位)
}

/// 为 `SimEpoch` 实现 `From<SuiSystemStateSummary>` trait。
/// 这允许从Sui SDK提供的 `SuiSystemStateSummary` 对象方便地创建 `SimEpoch` 实例。
impl From<SuiSystemStateSummary> for SimEpoch {
    fn from(summary: SuiSystemStateSummary) -> Self {
        Self {
            epoch_id: summary.epoch,                         // 从摘要中获取epoch ID
            epoch_start_timestamp: summary.epoch_start_timestamp_ms, // 获取epoch开始时间戳
            epoch_duration_ms: summary.epoch_duration_ms,     // 获取epoch持续时长
            gas_price: summary.reference_gas_price,          // 获取参考Gas价格
        }
    }
}

/// `SimulateCtx` 结构体 (模拟上下文 / Simulate Context)
///
/// 封装了执行一次特定交易模拟时所需的所有上下文环境信息。
#[derive(Debug, Clone, Default)] // Default 可以创建空的上下文
pub struct SimulateCtx {
    /// `epoch`: 当前模拟所依据的纪元信息。
    pub epoch: SimEpoch,
    /// `override_objects`: 一个对象列表，用于在模拟时“覆盖”链上这些对象的真实状态。
    /// 列表中的每个 `ObjectReadResult` 包含了要覆盖的对象的ID、版本、内容等。
    pub override_objects: Vec<ObjectReadResult>,
    /// `borrowed_coin`: (可选) 一个元组，包含一个模拟的“借入代币” (`Object`) 及其数量 (`u64`)。
    /// 用于模拟闪电贷的第一步，即假设已通过闪电贷获得了这笔资金。
    pub borrowed_coin: Option<(Object, u64)>,
}

impl SimulateCtx {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `SimulateCtx` 实例。
    ///
    /// 参数:
    /// - `epoch`: 模拟所依据的 `SimEpoch` 信息。
    /// - `override_objects`: 初始化时要覆盖的对象列表。
    pub fn new(epoch: SimEpoch, override_objects: Vec<ObjectReadResult>) -> Self {
        Self {
            epoch,
            override_objects,
            borrowed_coin: None, // 默认没有模拟的借入代币
        }
    }

    /// `with_borrowed_coin` 方法 (构建器模式风格)
    ///
    /// 设置模拟上下文中“借入的代币”信息。
    ///
    /// 参数:
    /// - `borrowed_coin_tuple`: 一个元组 `(Object, u64)`，包含模拟的借入代币对象及其数量。
    pub fn with_borrowed_coin(&mut self, borrowed_coin_tuple: (Object, u64)) {
        self.borrowed_coin = Some(borrowed_coin_tuple);
    }

    /// `with_gas_price` 方法 (构建器模式风格)
    ///
    /// 修改模拟上下文中 `epoch` 信息的 `gas_price` 字段。
    /// 这对于模拟MEV机会时，需要使用机会交易的Gas价格来准确评估利润非常有用。
    pub fn with_gas_price(&mut self, new_gas_price: u64) {
        self.epoch.gas_price = new_gas_price;
    }
}

impl SimEpoch {
    /// `is_stale` 方法
    ///
    /// 判断当前缓存的 `SimEpoch` 信息是否可能已“过时”。
    /// **当前逻辑**: `now < start + duration` 返回 `true` 表示“纪元尚未结束，信息仍有效/新鲜”。
    /// 如果返回 `false`，则表示纪元已结束，信息可能已过时。
    ///
    /// 返回:
    /// - `bool`: 如果当前时间早于纪元结束时间，则返回 `true` (表示信息“不陈旧”)，否则 `false`。
    pub fn is_stale(&self) -> bool {
        // 获取当前系统时间的毫秒级Unix时间戳
        let current_timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap() // 假设当前时间总是在UNIX纪元之后，不会panic
            .as_millis() as u64;
        // 判断当前时间是否仍然在当前纪元的有效期内
        // 如果当前时间 < (纪元开始时间 + 纪元持续时间)，则纪元尚未结束，信息不陈旧。
        current_timestamp_ms < (self.epoch_start_timestamp + self.epoch_duration_ms)
    }
}

/// `Simulator` Trait (模拟器接口 / Simulator Interface)
///
/// 定义了所有具体模拟器类型（如 `DBSimulator`, `HttpSimulator`）必须实现的通用行为。
/// `Send + Sync` 约束表明实现了此trait的类型必须是线程安全的。
#[async_trait] // 表明trait中可以有异步方法
pub trait Simulator: Sync + Send {
    /// `simulate` 异步方法
    ///
    /// 接收一个未签名的交易数据 (`TransactionData`) 和一个模拟上下文 (`SimulateCtx`)，
    /// 执行模拟，并返回一个包含结果的 `SimulateResult`。
    async fn simulate(&self, tx: TransactionData, ctx: SimulateCtx) -> Result<SimulateResult>;

    /// `get_object` 异步方法
    ///
    /// 用于从模拟器（或其底层存储/RPC）获取指定ObjectID的最新对象数据。
    async fn get_object(&self, obj_id: &ObjectID) -> Option<Object>;

    /// `name` 方法
    ///
    /// 返回模拟器的名称字符串 (例如 "DBSimulator", "HttpSimulator")。
    fn name(&self) -> &str;

    /// `get_object_layout` 方法 (可选提供)
    ///
    /// 获取对象的Move结构布局。
    /// 提供了一个默认实现，直接返回 `None`。
    /// 具体的模拟器如果能提供此功能 (如 `DBSimulator`)，则应重写此方法。
    fn get_object_layout(&self, _obj_id: &ObjectID) -> Option<MoveStructLayout> {
        None // 默认实现返回None
    }
}

[end of crates/simulator/src/lib.rs]
