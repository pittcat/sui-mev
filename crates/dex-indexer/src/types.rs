// 该文件 `types.rs` (位于 `dex-indexer` crate中) 定义了 `dex-indexer` crate 内部及对外暴露的
// 核心数据结构和枚举类型。这些类型用于统一表示不同DEX协议的池信息、交换事件、协议标识等。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 的“公共词典”，定义了各种数据应该长什么样，以及它们的名字。
//
// **主要定义 (Key Definitions)**:
//
// 1.  **类型别名 (Type Aliases)**:
//     -   `TokenPools`: `DashMap<String, HashSet<Pool>>`
//         这是一个并发安全的哈希图，键是代币类型字符串，值是一个包含多个 `Pool` 对象的哈希集合。
//         用于快速查找与某个特定代币相关的所有池。
//     -   `Token01Pools`: `DashMap<(String, String), HashSet<Pool>>`
//         并发安全的哈希图，键是一个由两个代币类型字符串组成的元组 (通常按字典序规范化)，值是一个池集合。
//         用于快速查找特定交易对的所有池。
//
// 2.  **`PoolCache` 结构体**:
//     -   在内存中缓存DEX池数据，以提供快速查询。
//     -   包含三个用 `Arc` (原子引用计数) 包裹的 `DashMap` 实例，分别对应上述的 `TokenPools`、`Token01Pools`，
//         以及一个直接从 `ObjectID` (池ID) 映射到 `Pool` 对象的 `pool_map`。
//         使用 `Arc<DashMap>` 允许多个线程安全地共享和修改这些缓存映射。
//
// 3.  **`Pool` 结构体**:
//     -   `dex-indexer` 中用于统一表示不同协议的DEX交易池的核心数据结构。
//     -   `protocol: Protocol`: 该池属于哪个DEX协议 (例如 Cetus, Turbos)。
//     -   `pool: ObjectID`: 池对象的Sui ObjectID。
//     -   `tokens: Vec<Token>`: 池中包含的代币列表，每个元素是 `Token` 结构。
//     -   `extra: PoolExtra`: 一个枚举，用于存储特定协议独有的附加信息 (例如手续费率、LP代币类型等)。
//     -   为 `Pool` 实现了 `PartialEq`, `Eq`, `Hash` (基于 `pool` 字段的ObjectID)，使其可以被存储在 `HashSet` 中或用作 `HashMap` 的键。
//     -   实现了 `fmt::Display` 和 `TryFrom<&str>`，用于将 `Pool` 对象序列化为自定义的字符串格式 (用 `|` 分隔字段) 以及从该字符串格式反序列化。
//         这主要用于 `FileDB` 中池数据的文本文件存储。
//     -   提供了一些辅助方法，如 `token0_type()`, `token1_type()`, `token_count()`, `token_index()`, `token()`, `token01_pairs()` (获取所有可能的交易对),
//         以及一个核心的异步方法 `related_object_ids()`。
//         -   `related_object_ids()`: 异步收集与此 `Pool` 相关的所有Sui对象ID，包括池本身、池中代币的包ID、以及通过调用特定协议的 `*_pool_children_ids` 函数获取的动态子对象ID。
//             这些ID对于 `DBSimulator` 预加载数据非常重要。
//
// 4.  **`Token` 结构体**:
//     -   表示池中的一个代币。
//     -   `token_type: String`: 代币的完整Sui类型字符串 (已规范化)。
//     -   `decimals: u8`: 代币的精度 (小数点位数)。
//
// 5.  **`PoolExtra` 枚举**:
//     -   用于存储不同DEX协议池子的特有附加信息。
//     -   每个变体对应一个协议，并包含该协议特定的字段。例如：
//         -   `Cetus { fee_rate: u64 }`
//         -   `Turbos { fee: u32 }`
//         -   `Aftermath { lp_type: String, fees_swap_in: Vec<u64>, ... }`
//         -   `DeepbookV2 { taker_fee_rate: u64, maker_rebate_rate: u64, tick_size: u64, lot_size: u64 }`
//     -   `None` 变体用于那些没有特定附加信息或尚未处理的协议。
//
// 6.  **`SwapEvent` 结构体**:
//     -   `dex-indexer` 中用于统一表示不同协议的DEX交换事件的核心数据结构。
//     -   `protocol: Protocol`: 发生交换的DEX协议。
//     -   `pool: Option<ObjectID>`: (可选) 发生交换的池的ObjectID。某些协议的交换事件可能不直接包含池ID。
//     -   `coins_in: Vec<String>`: 输入代币的类型列表 (支持多币输入，如Aftermath)。
//     -   `coins_out: Vec<String>`: 输出代币的类型列表 (支持多币输出)。
//     -   `amounts_in: Vec<u64>`: 对应 `coins_in` 的金额列表。
//     -   `amounts_out: Vec<u64>`: 对应 `coins_out` 的金额列表。
//     -   提供了 `pool_id()` 和 `involved_coin_one_side()` 辅助方法。
//         `involved_coin_one_side()` 用于获取交易对中非SUI的那一方，或者如果双方都不是SUI，则获取输入方。
//
// 7.  **`Protocol` 枚举**:
//     -   定义了 `dex-indexer` 支持的所有DEX协议的标识符。
//     -   为 `Protocol` 实现了 `fmt::Display` (用于转换为小写字符串) 和 `TryFrom<&str>` (用于从字符串转换回枚举)。
//     -   实现了 `TryFrom<&SuiEvent>` 和 `TryFrom<&ShioEvent>`，用于从事件类型字符串推断出协议类型。
//         这是通过匹配事件类型字符串与各协议在 `protocols/*.rs` 文件中定义的事件常量来实现的。
//     -   `event_filter()`: 根据协议类型返回相应的 `EventFilter` (通常用于订阅该协议的“池创建”事件)。
//     -   `sui_event_to_pool()`: (核心方法) 将一个通用的 `SuiEvent` 转换为标准化的 `Pool` 结构。
//         它会根据推断出的协议类型，调用相应 `protocols/*.rs` 文件中定义的特定事件结构 (`*PoolCreated`) 的 `TryFrom` 和 `to_pool` 方法。
//     -   `sui_event_to_swap_event()` / `shio_event_to_swap_event()`: (核心方法) 将 `SuiEvent` 或 `ShioEvent` 转换为标准化的 `SwapEvent` 结构。
//         逻辑与 `sui_event_to_pool` 类似，调用特定协议的 `*SwapEvent` 的 `TryFrom` 和 `to_swap_event_vX` 方法。
//     -   `related_object_ids()`: 根据协议类型返回与该协议全局相关的核心对象ID列表 (调用相应 `protocols/*.rs` 中的 `*_related_object_ids` 函数)。
//
// 8.  **`Event` 枚举**:
//     -   定义了 `dex-indexer` 内部事件处理引擎 (`burberry::Engine`) 使用的事件类型。
//     -   目前只有一个成员 `QueryEventTrigger`，由 `collector::QueryEventCollector` 定期产生，用于触发索引策略执行。
//
// 9.  **`NoAction` 和 `DummyExecutor`**:
//     -   用于 `burberry::Engine`。由于 `dex-indexer` 的主要目的是数据索引和更新数据库/缓存，
//         其策略 (`PoolCreatedStrategy`) 通常不直接产生需要外部执行的“动作”。
//     -   `NoAction` 是一个空结构体，用作引擎的动作类型参数。
//     -   `DummyExecutor` 是一个实现了 `Executor<NoAction>` trait 的虚拟执行器，它不执行任何实际操作。

// 引入标准库的 Arc (原子引用计数), collections::HashSet, fmt (格式化), hash (哈希相关trait)。
use std::{
    collections::HashSet,
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

// 引入 burberry 框架的 Executor trait 和 async_trait 宏。
use burberry::{async_trait, Executor};
// 引入 dashmap 提供的并发安全哈希图。
use dashmap::DashMap;
// 引入 eyre 库，用于错误处理。
use eyre::{bail, ensure, Result};
// 引入 serde 库的 Deserialize 和 Serialize trait，用于数据的序列化和反序列化。
use serde::{Deserialize, Serialize};
// 引入 shio 库的 ShioEvent 类型。
use shio::ShioEvent;
// 引入 simulator 库的 Simulator trait。
use simulator::Simulator;
// 引入 Sui SDK 中的相关类型。
use sui_sdk::{
    rpc_types::{EventFilter, SuiEvent},       // 事件过滤器, Sui事件。
    types::{base_types::ObjectID, TypeTag}, // ObjectID, TypeTag。 (TypeTag当前在此文件中未直接使用，但可能被宏展开或依赖项使用)
    SuiClient, SUI_COIN_TYPE,                // Sui RPC客户端, SUI原生代币类型字符串。
};
// 引入 tracing 库的 error! 宏，用于记录错误日志。
use tracing::error;

// 从当前crate的根模块引入 normalize_coin_type 函数。
use crate::normalize_coin_type;
// 从当前crate的 protocols 子模块引入所有协议特定的事件处理逻辑和常量。
// `abex::*` 语法导入 `abex` 模块下的所有公共项。
use crate::protocols::{
    abex::*, aftermath::*, babyswap::*, blue_move::*, cetus::*, deepbook_v2::*, flowx_amm::*, flowx_clmm::*,
    interest::*, kriya_amm::*, kriya_clmm::*, navi::*, suiswap::*, turbos::*,
};

// --- 类型别名定义 ---

/// `TokenPools` 类型别名
///
/// 一个并发安全的哈希图 (`DashMap`)，其键是代币的类型字符串 (`String`)，
/// 值是一个哈希集合 (`HashSet<Pool>`)，包含所有与该代币相关的 `Pool` 对象。
/// 用于快速查找某个代币存在于哪些池中。
pub type TokenPools = DashMap<String, HashSet<Pool>>;

/// `Token01Pools` 类型别名
///
/// 一个并发安全的哈希图 (`DashMap`)，其键是一个由两个代币类型字符串组成的元组 `(String, String)`
/// (这个元组通常是经过规范化排序的，例如按字典序)，值是一个哈希集合 (`HashSet<Pool>`)，
/// 包含所有属于该交易对的 `Pool` 对象。
/// 用于快速查找特定交易对的所有池。
pub type Token01Pools = DashMap<(String, String), HashSet<Pool>>;

/// `PoolCache` 结构体
///
/// 在内存中缓存DEX池数据，以提供快速查询。
/// 包含三个主要的索引映射，都使用 `Arc<DashMap>` 以支持线程安全共享和并发访问。
#[derive(Debug, Clone)] // Clone时是浅拷贝Arc指针
pub struct PoolCache {
    /// 按单个代币类型索引的池集合。键是代币类型，值是包含该代币的池集合。
    pub token_pools: Arc<TokenPools>,
    /// 按交易对 (token0, token1) 索引的池集合。键是规范化的代币对元组。
    pub token01_pools: Arc<Token01Pools>,
    /// 按池的ObjectID直接索引的池对象。键是池的ObjectID。
    pub pool_map: Arc<DashMap<ObjectID, Pool>>,
}

impl PoolCache {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `PoolCache` 实例。
    ///
    /// 参数:
    /// - `token_pools`: 初始化后的 `TokenPools` DashMap。
    /// - `token01_pools`: 初始化后的 `Token01Pools` DashMap。
    /// - `pool_map`: 初始化后的 `DashMap<ObjectID, Pool>`。
    pub fn new(token_pools: TokenPools, token01_pools: Token01Pools, pool_map: DashMap<ObjectID, Pool>) -> Self {
        Self {
            token_pools: Arc::new(token_pools),     // 将传入的DashMap用Arc包裹
            token01_pools: Arc::new(token01_pools), // 将传入的DashMap用Arc包裹
            pool_map: Arc::new(pool_map),           // 将传入的DashMap用Arc包裹
        }
    }
}

/// `Pool` 结构体
///
/// `dex-indexer` 中用于统一表示不同协议的DEX交易池的核心数据结构。
#[derive(Debug, Clone)] // Clone时内部的Vec和String会深拷贝，PoolExtra也会
pub struct Pool {
    pub protocol: Protocol,     // 该池属于哪个DEX协议 (例如 Cetus, Turbos)
    pub pool: ObjectID,         // 池对象的Sui ObjectID
    pub tokens: Vec<Token>,     // 池中包含的代币列表，每个元素是 `Token` 结构
    pub extra: PoolExtra,       // 一个枚举，用于存储特定协议独有的附加信息 (例如手续费率等)
}

/// 为 `Pool` 实现 `PartialEq` trait。
/// 两个 `Pool` 实例当且仅当它们的 `pool` (ObjectID) 字段相等时才被认为是相等的。
impl PartialEq for Pool {
    fn eq(&self, other: &Self) -> bool {
        self.pool == other.pool // 基于池的ObjectID进行比较
    }
}

/// 为 `Pool` 实现 `Eq` trait。
/// `Eq` 是 `PartialEq` 的一个子trait，表明相等关系是自反、对称和传递的。
impl Eq for Pool {}

/// 为 `Pool` 实现 `Hash` trait。
/// `Pool` 实例的哈希值是基于其 `pool` (ObjectID) 字段的哈希值计算的。
/// 这使得 `Pool` 对象可以被存储在 `HashSet` 中或用作 `HashMap` 的键。
impl Hash for Pool {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pool.hash(state); // 对池的ObjectID进行哈希
    }
}

/// `Token` 结构体
///
/// 表示池中的一个代币及其属性。
#[derive(Debug, Clone, Serialize, Deserialize)] // Serialize/Deserialize用于Pool的字符串序列化
pub struct Token {
    pub token_type: String, // 代币的完整Sui类型字符串 (已规范化)
    pub decimals: u8,       // 代币的精度 (小数点位数)
}

/// `PoolExtra` 枚举
///
/// 用于存储不同DEX协议池子的特有附加信息。
/// 每个变体对应一个或一类协议，并包含该协议特定的字段。
#[derive(Debug, Clone, Serialize, Deserialize)] // Serialize/Deserialize用于Pool的字符串序列化
pub enum PoolExtra {
    None, // 无特定附加信息，或用于尚未特殊处理的协议
    Cetus { // Cetus CLMM 池的附加信息
        fee_rate: u64, // 手续费率 (例如，500 代表 0.05%)
    },
    Turbos { // Turbos Finance CLMM 池的附加信息
        fee: u32, // 手续费率 (具体单位和换算关系需参考Turbos文档)
    },
    Aftermath { // Aftermath Finance 池的附加信息 (支持多币种和多种费用)
        lp_type: String,         // LP代币的类型字符串
        fees_swap_in: Vec<u64>,  // 输入方向交换手续费率列表 (对应池中代币顺序)
        fees_swap_out: Vec<u64>, // 输出方向交换手续费率列表
        fees_deposit: Vec<u64>,  // 存款手续费率列表
        fees_withdraw: Vec<u64>, // 取款手续费率列表
    },
    KriyaAmm { // KriyaDEX AMM 池的附加信息
        lp_fee_percent: u64,       // LP手续费百分比
        protocol_fee_percent: u64, // 协议手续费百分比
    },
    KriyaClmm { // KriyaDEX CLMM 池的附加信息
        fee_rate: u64, // 手续费率
    },
    FlowxAmm { // FlowX AMM 池的附加信息
        fee_rate: u64, // 手续费率
    },
    FlowxClmm { // FlowX CLMM 池的附加信息
        fee_rate: u64, // 手续费率
    },
    DeepbookV2 { // DeepBook V2 订单簿的附加信息
        taker_fee_rate: u64,    // 吃单手续费率
        maker_rebate_rate: u64, // 挂单返利费率
        tick_size: u64,         // 价格精度 (最小价格变动单位)
        lot_size: u64,          // 数量精度 (最小数量变动单位)
    },
}

/// 为 `Pool` 实现 `fmt::Display` trait。
/// 用于将 `Pool` 对象序列化为一个自定义的字符串格式，字段间用 `|` 分隔。
/// `tokens` 和 `extra` 字段被序列化为JSON字符串。
/// 主要用于 `FileDB` 的文本文件存储。
impl fmt::Display for Pool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}|{}|{}|{}", // 格式: protocol|pool_id|tokens_json|extra_json
            self.protocol,                                  // Protocol枚举会自动调用其Display实现 (小写字符串)
            self.pool,                                      // ObjectID会自动调用其Display实现 (十六进制字符串)
            serde_json::to_string(&self.tokens).unwrap(),   // 将tokens Vec<Token>序列化为JSON字符串
            serde_json::to_string(&self.extra).unwrap()     // 将extra PoolExtra枚举序列化为JSON字符串
        )
    }
}

/// 为 `Pool` 实现 `TryFrom<&str>` trait。
/// 用于从自定义的字符串格式（如 `FileDB` 中存储的）反序列化回 `Pool` 对象。
impl TryFrom<&str> for Pool {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(value: &str) -> Result<Self> {
        let parts: Vec<&str> = value.split('|').collect(); // 按'|'分割字符串
        ensure!(parts.len() == 4, "无效的Pool字符串格式: {} (应有4个部分)", value); // 确保有4个部分

        // 逐个解析每个部分
        let protocol = Protocol::try_from(parts[0])?; // 解析协议 (Protocol实现了TryFrom<&str>)
        let pool_id_obj = parts[1].parse()?;          // 解析池ObjectID (ObjectID实现了FromStr)
        let tokens_vec: Vec<Token> = serde_json::from_str(parts[2])?; // 从JSON字符串反序列化tokens
        let extra_data: PoolExtra = serde_json::from_str(parts[3])?;  // 从JSON字符串反序列化extra

        Ok(Pool { // 返回构造好的Pool实例
            protocol,
            pool: pool_id_obj,
            tokens: tokens_vec,
            extra: extra_data,
        })
    }
}

impl Pool {
    /// `token0_type` 方法
    /// 返回池中第一个代币的类型字符串的克隆。
    /// 假设 `self.tokens` 至少包含一个元素。
    pub fn token0_type(&self) -> String {
        self.tokens[0].token_type.clone()
    }

    /// `token1_type` 方法
    /// 返回池中第二个代币的类型字符串的克隆。
    /// 假设 `self.tokens` 至少包含两个元素。
    pub fn token1_type(&self) -> String {
        self.tokens[1].token_type.clone()
    }

    /// `token_count` 方法
    /// 返回池中代币的数量。
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    /// `token_index` 方法
    ///
    /// 查找指定 `token_type` 在池的 `tokens` 列表中的索引。
    ///
    /// 返回: `Option<usize>`，如果找到则为 `Some(index)`，否则为 `None`。
    pub fn token_index(&self, token_type: &str) -> Option<usize> {
        self.tokens.iter().position(|token_obj| token_obj.token_type == token_type)
    }

    /// `token` 方法
    ///
    /// 根据索引获取池中代币的克隆。
    ///
    /// 返回: `Option<Token>`，如果索引有效则为 `Some(Token)`，否则为 `None`。
    pub fn token(&self, index: usize) -> Option<Token> {
        self.tokens.get(index).cloned()
    }

    /// `token01_pairs` 方法
    ///
    /// 对于池中的所有代币，生成所有可能的两两组合（交易对）。
    /// 例如，如果池中有 [A, B, C]，则生成 [(A,B), (A,C), (B,C)]。
    ///
    /// 返回: `Vec<(String, String)>`，每个元组代表一个交易对的代币类型。
    pub fn token01_pairs(&self) -> Vec<(String, String)> {
        let mut pairs_vec = Vec::new();
        // 使用嵌套循环生成所有唯一的代币对组合
        for i_idx in 0..self.tokens.len() {
            for j_idx in i_idx + 1..self.tokens.len() {
                pairs_vec.push((
                    self.tokens[i_idx].token_type.clone(),
                    self.tokens[j_idx].token_type.clone(),
                ));
            }
        }
        pairs_vec
    }

    /// `related_object_ids` 异步方法
    ///
    /// 收集与此 `Pool` 相关的所有Sui对象ID。
    /// 包括：池对象本身的ID、池中所有代币的定义包ID、以及通过调用特定协议的
    /// `*_pool_children_ids` 函数获取的动态子对象ID。
    /// 这些ID对于 `DBSimulator` 预加载数据非常重要。
    ///
    /// 参数:
    /// - `simulator`: 一个实现了 `Simulator` trait 的共享实例，用于可能的链上查询。
    ///
    /// 返回:
    /// - `HashSet<String>`: 包含所有相关对象ID字符串的哈希集合 (确保唯一性)。
    pub async fn related_object_ids(&self, simulator: Arc<dyn Simulator>) -> HashSet<String> {
        let mut result_set = HashSet::new(); // 使用HashSet确保ID的唯一性

        // 1. 添加池对象本身的ID
        result_set.insert(self.pool.to_string());

        // 2. 添加池中所有代币的包ID (通过解析代币类型字符串的第一部分 "PackageID::module::name")
        let token_package_ids = self
            .tokens
            .iter()
            .map(|token_obj| token_obj.token_type.split_once("::").unwrap().0.to_string()) // 分割并取第一部分
            .collect::<Vec<_>>();
        result_set.extend(token_package_ids); // 添加到结果集合

        // 3. 根据池的协议类型，调用相应的 `*_pool_children_ids` 函数获取特定于协议的子对象ID。
        //    这些函数通常在 `protocols/*.rs` 文件中定义。
        let children_ids_result = match self.protocol {
            Protocol::Cetus => cetus_pool_children_ids(self, simulator).await,
            Protocol::BlueMove => blue_move_pool_children_ids(self, simulator).await,
            Protocol::Turbos => turbos_pool_children_ids(self, simulator).await,
            Protocol::KriyaClmm => kriya_clmm_pool_children_ids(self, simulator).await,
            Protocol::FlowxClmm => flowx_clmm_pool_children_ids(self, simulator).await,
            Protocol::Aftermath => aftermath_pool_children_ids(self, simulator).await,
            _ => Ok(vec![]), // 对于其他协议或没有特定子对象逻辑的协议，返回空列表
        };
        // 处理获取子对象ID的结果
        match children_ids_result {
            Ok(children_ids_vec) => result_set.extend(children_ids_vec), // 添加到结果集合
            Err(error_val) => error!( // 如果出错，则记录错误日志
                "获取池 {} 的子对象ID失败: {}",
                self.pool, error_val
            ),
        }

        result_set // 返回所有收集到的唯一对象ID
    }
}

impl Token {
    /// `new` 构造函数 for `Token`
    pub fn new(token_type: &str, decimals: u8) -> Self {
        Self {
            token_type: normalize_coin_type(token_type), // 规范化代币类型字符串
            decimals,
        }
    }
}

/// `SwapEvent` 结构体
///
/// `dex-indexer` 中用于统一表示不同协议的DEX交换事件的核心数据结构。
#[derive(Debug, Clone)] // Clone时内部的Vec和String会深拷贝
pub struct SwapEvent {
    pub protocol: Protocol,        // 发生交换的DEX协议
    pub pool: Option<ObjectID>,    // (可选) 发生交换的池的ObjectID。
                                   // 某些协议的交换事件可能不直接包含池ID，此时为None。
    pub coins_in: Vec<String>,     // 输入代币的类型列表 (支持多币输入，如Aftermath)
    pub coins_out: Vec<String>,    // 输出代币的类型列表 (支持多币输出)
    pub amounts_in: Vec<u64>,      // 对应 `coins_in` 的金额列表
    pub amounts_out: Vec<u64>,     // 对应 `coins_out` 的金额列表
}

impl SwapEvent {
    /// `pool_id` 方法
    /// 返回交换事件关联的池ID (如果存在)。
    pub fn pool_id(&self) -> Option<ObjectID> {
        self.pool
    }

    /// `involved_coin_one_side` 方法
    ///
    /// 获取交换中涉及的一个“主要”代币类型。
    /// 优先返回输入代币中的第一个非SUI代币。如果输入代币是SUI，则返回输出代币中的第一个。
    /// 这可能用于某些基于SUI对的套利路径发现或机会识别逻辑。
    pub fn involved_coin_one_side(&self) -> String {
        if self.coins_in[0] != SUI_COIN_TYPE { // 如果输入代币不是SUI
            self.coins_in[0].to_string()
        } else { // 如果输入代币是SUI (或者coins_in为空，虽然不应发生)
            self.coins_out[0].to_string() // 则取输出代币的第一个
        }
    }
}

/// `Protocol` 枚举
///
/// 定义了 `dex-indexer` 支持的所有DEX协议的标识符。
/// `#[derive(...)]` 自动实现了一系列有用的trait。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)] // Serialize/Deserialize用于FileDB中Pool的字符串序列化
pub enum Protocol {
    Cetus,
    Turbos,
    Aftermath,
    KriyaAmm,
    KriyaClmm,
    FlowxAmm,
    FlowxClmm,
    DeepbookV2,
    DeepbookV3, // 虽然定义了V3，但当前代码中可能未完全支持或使用
    Volo,       // Volo可能是一个流动性质押协议，也可能涉及DEX池
    BlueMove,
    SuiSwap,
    Interest,
    Abex,
    BabySwap,
    Navi,       // Navi是借贷协议，但其事件可能被索引器关注（例如，与闪电贷相关的对象）
}

/// 为 `Protocol` 实现 `fmt::Display` trait，用于将其转换为小写字符串表示。
impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 使用 `match` 将每个枚举成员映射到其小写字符串形式。
        // `write!(f, "...")` 用于将字符串写入格式化器。
        match self {
            Protocol::Cetus => write!(f, "cetus"),
            Protocol::Turbos => write!(f, "turbos"),
            Protocol::Aftermath => write!(f, "aftermath"),
            Protocol::KriyaAmm => write!(f, "kriya_amm"),
            Protocol::KriyaClmm => write!(f, "kriya_clmm"),
            Protocol::FlowxAmm => write!(f, "flowx_amm"),
            Protocol::FlowxClmm => write!(f, "flowx_clmm"),
            Protocol::DeepbookV2 => write!(f, "deepbook_v2"),
            Protocol::DeepbookV3 => write!(f, "deepbook_v3"),
            Protocol::Volo => write!(f, "volo"),
            Protocol::BlueMove => write!(f, "blue_move"),
            Protocol::SuiSwap => write!(f, "suiswap"),
            Protocol::Interest => write!(f, "interest"),
            Protocol::Abex => write!(f, "abex"),
            Protocol::BabySwap => write!(f, "babyswap"),
            Protocol::Navi => write!(f, "navi"),
        }
    }
}

/// 为 `Protocol` 实现 `TryFrom<&str>` trait，用于从字符串反序列化回 `Protocol` 枚举。
impl TryFrom<&str> for Protocol {
    type Error = eyre::Error; // 定义转换失败的错误类型

    fn try_from(value: &str) -> Result<Self> {
        // 使用 `match` 将字符串与已知的协议名称进行比较。
        match value {
            "cetus" => Ok(Protocol::Cetus),
            "turbos" => Ok(Protocol::Turbos),
            "aftermath" => Ok(Protocol::Aftermath),
            "kriya_amm" => Ok(Protocol::KriyaAmm),
            "kriya_clmm" => Ok(Protocol::KriyaClmm),
            "flowx_amm" => Ok(Protocol::FlowxAmm),
            "flowx_clmm" => Ok(Protocol::FlowxClmm),
            "deepbook_v2" => Ok(Protocol::DeepbookV2),
            "deepbook_v3" => Ok(Protocol::DeepbookV3),
            "volo" => Ok(Protocol::Volo),
            "blue_move" => Ok(Protocol::BlueMove),
            "suiswap" => Ok(Protocol::SuiSwap),
            "interest" => Ok(Protocol::Interest),
            "abex" => Ok(Protocol::Abex),
            "babyswap" => Ok(Protocol::BabySwap),
            "navi" => Ok(Protocol::Navi),
            // 如果字符串不匹配任何已知协议，则返回错误。
            _ => bail!("不支持的协议字符串: {}", value), // `bail!` 来自 `eyre`
        }
    }
}

/// 为 `Protocol` 实现 `TryFrom<&SuiEvent>` trait，用于从Sui事件的类型推断协议。
impl TryFrom<&SuiEvent> for Protocol {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        Self::try_from_event_type(&event.type_.to_string()) // 调用下面的辅助函数
    }
}

/// 为 `Protocol` 实现 `TryFrom<&ShioEvent>` trait，用于从Shio事件的类型推断协议。
impl TryFrom<&ShioEvent> for Protocol {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        Self::try_from_event_type(&event.event_type) // 调用下面的辅助函数
    }
}

impl Protocol {
    /// `try_from_event_type` (私有辅助函数)
    ///
    /// 根据Sui事件的完整类型字符串，尝试推断出该事件属于哪个 `Protocol`。
    /// 它通过检查事件类型字符串是否以特定协议的事件类型常量开头来实现。
    pub fn try_from_event_type(event_type: &str) -> Result<Self> {
        match event_type {
            // 检查是否与各协议的SWAP_EVENT常量匹配 (或以其开头)
            s if s.starts_with(CETUS_SWAP_EVENT) => Ok(Protocol::Cetus),
            s if s.starts_with(TURBOS_SWAP_EVENT) => Ok(Protocol::Turbos),
            s if s.starts_with(AFTERMATH_SWAP_EVENT) => Ok(Protocol::Aftermath),
            s if s.starts_with(KRIYA_AMM_SWAP_EVENT) => Ok(Protocol::KriyaAmm),
            s if s.starts_with(KRIYA_CLMM_SWAP_EVENT) => Ok(Protocol::KriyaClmm),
            s if s.starts_with(FLOWX_AMM_SWAP_EVENT) => Ok(Protocol::FlowxAmm),
            s if s.starts_with(FLOWX_CLMM_SWAP_EVENT) => Ok(Protocol::FlowxClmm),
            s if s.starts_with(BLUE_MOVE_SWAP_EVENT) => Ok(Protocol::BlueMove),
            s if s.starts_with(SUISWAP_SWAP_EVENT) => Ok(Protocol::SuiSwap),
            s if s.starts_with(INTEREST_SWAP_EVENT) => Ok(Protocol::Interest),
            s if s.starts_with(ABEX_SWAP_EVENT) => Ok(Protocol::Abex),
            s if s.starts_with(BABY_SWAP_EVENT) => Ok(Protocol::BabySwap),
            // 如果不匹配任何已知的交换事件类型，则返回错误，表示这不是一个“感兴趣的”事件。
            _ => bail!("事件类型 {} 不属于任何关注的DEX交换事件 (Not an interesting event type)", event_type),
        }
    }

    /// `event_filter` 方法
    ///
    /// 根据当前的 `Protocol` 枚举成员，返回一个相应的 `EventFilter`。
    /// 这个过滤器通常用于订阅该协议的“池创建”事件。
    /// 它会调用在相应 `protocols/*.rs` 文件中定义的 `*_event_filter()` 函数。
    pub fn event_filter(&self) -> EventFilter {
        match self {
            Protocol::Cetus => cetus_event_filter(),
            Protocol::Turbos => turbos_event_filter(),
            Protocol::Aftermath => aftermath_event_filter(),
            Protocol::KriyaAmm => kriya_amm_event_filter(),
            Protocol::KriyaClmm => kriya_clmm_event_filter(),
            Protocol::FlowxAmm => flowx_amm_event_filter(),
            Protocol::FlowxClmm => flowx_clmm_event_filter(),
            Protocol::DeepbookV2 => deepbook_v2_event_filter(),
            Protocol::BlueMove => blue_move_event_filter(),
            // 对于其他协议，如果它们没有特定的池创建事件过滤器，则 `todo!()` 会在运行时panic。
            // 这表明需要为这些协议实现相应的 `*_event_filter()` 函数。
            _ => todo!("协议 {:?} 尚未实现event_filter (event_filter not yet implemented for protocol {:?})", self),
        }
    }

    /// `sui_event_to_pool` 异步方法
    ///
    /// 将一个通用的 `SuiEvent` 转换为标准化的 `Pool` 结构。
    /// 它根据 `self` (当前的 `Protocol` 枚举成员) 的值，
    /// 调用相应 `protocols/*.rs` 文件中定义的特定事件结构 (`*PoolCreated`) 的 `TryFrom` 和 `to_pool` 方法。
    pub async fn sui_event_to_pool(&self, event: &SuiEvent, sui: &SuiClient) -> Result<Pool> {
        match self {
            Protocol::Cetus => CetusPoolCreated::try_from(event)?.to_pool(sui).await,
            Protocol::Turbos => TurbosPoolCreated::try_from(event)?.to_pool(sui).await,
            Protocol::Aftermath => AftermathPoolCreated::try_from(event)?.to_pool(sui).await,
            Protocol::KriyaAmm => KriyaAmmPoolCreated::try_from(event)?.to_pool(sui).await,
            Protocol::KriyaClmm => KriyaClmmPoolCreated::try_from(event)?.to_pool(sui).await,
            Protocol::FlowxAmm => FlowxAmmPoolCreated::try_from(event)?.to_pool(sui).await,
            Protocol::FlowxClmm => FlowxClmmPoolCreated::try_from(event)?.to_pool(sui).await,
            Protocol::DeepbookV2 => DeepbookV2PoolCreated::try_from(event)?.to_pool(sui).await,
            Protocol::BlueMove => BlueMovePoolCreated::try_from(event)?.to_pool(sui).await,
            _ => todo!("协议 {:?} 尚未实现sui_event_to_pool (sui_event_to_pool not yet implemented for protocol {:?})", self),
        }
    }

    /// `sui_event_to_swap_event` 异步方法
    ///
    /// 将 `SuiEvent` 转换为标准化的 `SwapEvent` 结构。
    /// 逻辑与 `sui_event_to_pool` 类似，调用特定协议的 `*SwapEvent` 的 `TryFrom` 和 `to_swap_event_vX` 方法。
    pub async fn sui_event_to_swap_event(&self, event: &SuiEvent, provider: Arc<dyn Simulator>) -> Result<SwapEvent> {
        match self {
            Protocol::Cetus => CetusSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::Turbos => TurbosSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::Aftermath => AftermathSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::KriyaAmm => KriyaAmmSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::KriyaClmm => KriyaClmmSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::FlowxAmm => FlowxAmmSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::FlowxClmm => FlowxClmmSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::BlueMove => BlueMoveSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::SuiSwap => SuiswapSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::Interest => InterestSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::Abex => AbexSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::BabySwap => BabySwapEvent::try_from(event)?.to_swap_event().await,
            _ => todo!("协议 {:?} 尚未实现sui_event_to_swap_event (sui_event_to_swap_event not yet implemented for protocol {:?})", self),
        }
    }

    /// `shio_event_to_swap_event` 异步方法
    ///
    /// 将 `ShioEvent` 转换为标准化的 `SwapEvent` 结构。
    /// 逻辑与 `sui_event_to_swap_event` 类似。
    pub async fn shio_event_to_swap_event(&self, event: &ShioEvent, provider: Arc<dyn Simulator>) -> Result<SwapEvent> {
        match self {
            Protocol::Cetus => CetusSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::Turbos => TurbosSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::Aftermath => AftermathSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::KriyaAmm => KriyaAmmSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::KriyaClmm => KriyaClmmSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::FlowxAmm => FlowxAmmSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::FlowxClmm => FlowxClmmSwapEvent::try_from(event)?.to_swap_event_v2(provider).await,
            Protocol::BlueMove => BlueMoveSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::SuiSwap => SuiswapSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::Interest => InterestSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::Abex => AbexSwapEvent::try_from(event)?.to_swap_event().await,
            Protocol::BabySwap => BabySwapEvent::try_from(event)?.to_swap_event().await,
            // Navi 通常是借贷，不直接产生SwapEvent，所以这里可能返回错误或未实现。
            _ => todo!("协议 {:?} 尚未实现shio_event_to_swap_event (shio_event_to_swap_event not yet implemented for protocol {:?})", self),
        }
    }

    /// `related_object_ids` 异步方法
    ///
    /// 根据协议类型，返回与该协议全局相关的核心对象ID列表。
    /// 它会调用在相应 `protocols/*.rs` 文件中定义的 `*_related_object_ids()` 函数。
    pub async fn related_object_ids(&self) -> Result<HashSet<String>> {
        let object_ids_vec = match self {
            Protocol::Cetus => cetus_related_object_ids(),
            Protocol::BlueMove => blue_move_related_object_ids(),
            Protocol::Turbos => turbos_related_object_ids(),
            Protocol::KriyaAmm => kriya_amm_related_object_ids(),
            Protocol::KriyaClmm => kriya_clmm_related_object_ids(),
            Protocol::FlowxClmm => flowx_clmm_related_object_ids(),
            Protocol::Navi => navi_related_object_ids(),
            Protocol::Aftermath => aftermath_related_object_ids().await, // Aftermath的此函数是异步的
            // 对于其他未明确列出的协议，返回错误，表示它们没有预定义的全局相关对象ID列表
            // 或此功能尚未为它们实现。
            _ => bail!("协议 {:?} 没有预定义的全局相关对象ID列表 (No predefined global related object IDs for protocol {:?})", self),
        };
        // 将Vec<String>转换为HashSet<String>以确保唯一性
        Ok(object_ids_vec.into_iter().collect::<HashSet<String>>())
    }
}

/// `Event` 枚举 (dex-indexer内部事件)
///
/// 定义了 `dex-indexer` 内部事件处理引擎 (`burberry::Engine`) 使用的事件类型。
#[derive(Debug, Clone, PartialEq, Eq)] // 允许比较和克隆
pub enum Event {
    /// `QueryEventTrigger` 事件
    ///
    /// 由 `collector::QueryEventCollector` 定期产生。
    /// 用于触发 `strategy::PoolCreatedStrategy` 执行其周期性的池数据发现和更新逻辑。
    QueryEventTrigger,
}

/// `NoAction` 结构体 (无动作占位符)
///
/// 一个空结构体，用作 `burberry::Engine` 的动作类型参数，
/// 当策略 (如 `PoolCreatedStrategy`) 不直接产生需要外部执行器处理的动作时使用。
#[derive(Debug, Clone)]
pub struct NoAction;

/// `DummyExecutor` 结构体 (虚拟执行器)
///
/// 一个实现了 `Executor<NoAction>` trait 的虚拟执行器。
/// 它的 `execute` 方法不执行任何实际操作，仅返回成功。
/// 用于配合不产生动作的策略在 `burberry::Engine` 中运行。
#[derive(Debug, Clone)]
pub struct DummyExecutor;

#[async_trait] // 因为 Executor::execute 是异步的
impl Executor<NoAction> for DummyExecutor {
    /// `execute` 方法 (空实现)
    async fn execute(&self, _action: NoAction) -> Result<()> {
        Ok(()) // 不执行任何操作，直接返回成功
    }

    /// `name` 方法
    fn name(&self) -> &'static str {
        "DummyDexIndexerExecutor" // 执行器的名称
    }
}

[end of crates/dex-indexer/src/types.rs]
