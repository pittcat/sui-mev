// 该文件 `cetus.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 Cetus DEX 协议相关的事件和数据结构。
// Cetus 是 Sui 生态中一个重要的CLMM (集中流动性做市商) DEX。
// 这个文件的主要功能是：
// 1. 定义 Cetus "池创建" (`CreatePoolEvent`) 和 "交换" (`SwapEvent`) 事件的类型字符串常量。
// 2. 提供 `cetus_event_filter()` 函数，用于创建Sui事件订阅的过滤器，专门监听Cetus的池创建事件。
// 3. 定义 `CetusPoolCreated` 结构体，用于存储从链上 `CreatePoolEvent` 事件解析出来的池创建信息。
// 4. 实现 `TryFrom<&SuiEvent>` for `CetusPoolCreated`。
// 5. `CetusPoolCreated::to_pool()` 方法，将解析出的事件数据转换为通用的 `Pool` 结构，
//    并从链上对象中提取手续费率 (`fee_rate`) 存入 `PoolExtra::Cetus`。
// 6. 定义 `CetusSwapEvent` 结构体，用于存储从链上 `SwapEvent` 事件解析出来的交换信息。
// 7. 实现 `TryFrom<&SuiEvent>`、`TryFrom<&ShioEvent>` 和 `TryFrom<&Value>` for `CetusSwapEvent`。
// 8. `CetusSwapEvent::to_swap_event_v1()` 和 `to_swap_event_v2()` 方法，将 `CetusSwapEvent` 转换为通用的 `SwapEvent`。
//    - `_v1` 版本需要一个 `SuiClient` 来查询池中代币类型。
//    - `_v2` 版本使用 `Simulator` (以及一个内部宏 `get_coin_in_out_v2!`) 来获取代币类型，更通用。
// 9. `cetus_related_object_ids()` 函数，返回与Cetus协议相关的核心对象ID列表 (硬编码)。
// 10. `cetus_pool_children_ids()` 函数，用于获取特定Cetus池的动态子对象ID，特别是与 `tick_manager` 和 `position_manager` 相关的子对象。
//     这部分逻辑比较复杂，涉及到获取动态字段、解析 `tick_manager` 和 `position_manager` 的内部结构，
//     甚至调用链上脚本 (`fetcher_script::fetch_ticks`) 来获取tick信息。
// 11. `parse_tick_scores()`、`get_tick_scores()`、`pool_layout()` 是上述ID派生逻辑的辅助函数。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” Cetus 这个DEX发生的两种主要“事件”的：
// “有人创建了一个新的交易池” 和 “有人在这个池子里完成了一笔代币交换”。
// 它还负责收集与Cetus协议本身以及特定池子相关联的一些重要的“全局对象”和“动态子对象”的ID。
// Cetus作为一个CLMM DEX，其池子结构和状态管理（如tick、position）比传统AMM更复杂，这在此文件中有体现。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别Cetus的特定事件**:
//     -   `CETUS_POOL_CREATED` 和 `CETUS_SWAP_EVENT` 常量是这两种事件在Sui链上的“专属门牌号”。
//     -   `cetus_event_filter()` 函数生成一个“过滤器”，告诉Sui节点只订阅Cetus创建新池子的事件。
//
// 2.  **`CetusPoolCreated` 结构体 (新池子信息卡)**:
//     -   记录Cetus新池子创建事件的信息（池ID、两种代币类型）。
//     -   `TryFrom<&SuiEvent>` 从原始Sui事件中提取数据。
//     -   `to_pool()` 方法将其转换为通用的 `Pool` 结构，特别地，它会去链上读取池对象的 `fee_rate` 字段，
//         并将其存储在 `PoolExtra::Cetus` 中。
//
// 3.  **`CetusSwapEvent` 结构体 (交换记录卡)**:
//     -   记录Cetus交换事件的详细信息（池ID、输入金额、输出金额、交易方向 `a2b`）。
//     -   `TryFrom` 实现能从不同来源（`SuiEvent`, `ShioEvent`, 原始JSON `Value`）提取信息。
//     -   `to_swap_event_v1/_v2()` 方法将其转换为通用的 `SwapEvent`。`_v2` 版本使用 `Simulator` 获取代币类型，更灵活。
//
// 4.  **收集相关对象ID (Collecting Related Object IDs)**:
//     -   `CETUS_PACKAGE_ID`, `TICK_BOUND` 等是Cetus协议的常量。
//     -   `CETUS_POOL_LAYOUT`: 使用 `OnceLock` 缓存池对象的布局，避免重复获取。
//     -   `cetus_related_object_ids()`: 列出了一些硬编码的Cetus相关对象ID（如聚合器、配置、合作伙伴等）。
//     -   `cetus_pool_children_ids()`: 这是一个复杂函数，用于获取一个Cetus池的动态子对象ID。
//         -   它解析池对象的 `tick_manager` 和 `position_manager` 字段。
//         -   通过 `get_dynamic_fields` 获取 `positions` 表和 `ticks` 表中的所有动态字段对象ID。
//         -   还通过调用链上的 `fetcher_script::fetch_ticks` 脚本来获取 `tick_scores`，并据此派生更多tick相关的动态字段ID。
//         这反映了CLMM池子需要管理大量与tick（价格点位）和position（流动性位置）相关的子对象。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
//
// -   **CLMM (集中流动性做市商 / Concentrated Liquidity Market Maker)**:
//     允许流动性提供者 (LP) 将其资金集中在特定的价格区间内，而不是在从0到无穷大的整个价格曲线上平均分配。
//     这可以提高资本效率，使得在活跃交易价格附近有更深的流动性，从而减少滑点，并可能为LP带来更高的手续费回报。
//     Cetus是Sui上一个典型的CLMM DEX。
//
// -   **Tick (价格刻度 / Tick)**:
//     在CLMM中，整个价格范围被离散化为一系列的“刻度”（ticks）。每个tick对应一个特定的价格点。
//     流动性提供者将其流动性放置在某两个tick之间定义的有限价格区间内。
//     `tick_manager` 和 `ticks` 表就是用来管理这些价格刻度及其相关状态（如每个tick上的净流动性变化）的对象。
//     `TICK_BOUND` 可能与tick索引的计算有关。
//
// -   **Position (流动性头寸 / Liquidity Position)**:
//     一个流动性提供者在CLMM池中提供流动性的具体实例。一个Position通常由其所在的池、提供的价格区间（由lower_tick和upper_tick定义）和提供的流动性数量来定义。
//     `position_manager` 和 `positions` 表用于管理这些流动性头寸对象。
//
// -   **Fee Rate (手续费率 / Fee Rate)**:
//     Cetus的池子在创建时会关联一个特定的手续费率（例如0.01%, 0.05%, 0.3%, 1%）。
//     这个费率决定了交易者在该池进行交换时需要支付的手续费比例。`CetusPoolCreated::to_pool()` 会从链上读取这个费率。
//
// -   **`OnceLock` (单次初始化单元)**:
//     `std::sync::OnceLock` (或 `tokio::sync::OnceLock` 的同步版本) 用于确保某个值只被初始化一次。
//     这里用 `CETUS_POOL_LAYOUT: OnceLock<MoveStructLayout>` 来缓存从链上获取的Cetus池对象的布局信息，
//     避免每次需要时都重新查询，提高效率。

// 引入标准库的 Arc (原子引用计数), collections::HashSet, str::FromStr, sync::OnceLock (单次初始化)。
use std::{
    collections::HashSet,
    str::FromStr,
    sync::{Arc, OnceLock},
};

// 引入 eyre 库，用于错误处理和上下文管理。
use eyre::{ensure, eyre, OptionExt, Result};
// 引入 Move 核心类型中的 MoveStruct 和 MoveStructLayout，用于解析Move对象的结构和布局。
use move_core_types::annotated_value::{MoveStruct, MoveStructLayout};
// 引入 Rayon 库的并行迭代器，用于并行处理数据。
use rayon::prelude::*;
// 引入 serde 库的 Deserialize trait，用于从如JSON这样的格式反序列化数据。
use serde::Deserialize;
// 引入 serde_json 的 Value 类型，用于表示通用的JSON值。
use serde_json::Value;
// 引入 shio 库的 ShioEvent 类型，可能用于处理与MEV相关的特定事件。
use shio::ShioEvent;
// 引入 simulator 库的 SimulateCtx (模拟上下文) 和 Simulator trait (模拟器接口)。
use simulator::{SimulateCtx, Simulator};
// 引入 Sui SDK 中的相关类型。
use sui_sdk::{
    rpc_types::{EventFilter, SuiData, SuiEvent, SuiObjectDataOptions}, // 事件过滤器, Sui数据容器, Sui事件, 对象数据选项。
    types::{base_types::ObjectID, TypeTag},                             // ObjectID, TypeTag。
    SuiClient, SuiClientBuilder,                                        // Sui RPC客户端和构建器。
};
// 引入 Sui 核心类型中的SuiAddress, 动态字段ID派生函数, Object类型, PTB构建器, 交易命令, 交易数据, Identifier。
use sui_types::{
    base_types::SuiAddress, dynamic_field::derive_dynamic_field_id, object::Object, programmable_transaction_builder::ProgrammableTransactionBuilder, transaction::{Command, TransactionData}, Identifier
};
// 引入 utils 库的 object 模块所有内容。
use utils::object::*;

// 从当前crate的父模块 (protocols) 引入 get_coin_decimals (获取代币精度) 和 get_pool_coins_type (获取池代币类型) 函数，以及SUI RPC节点URL常量。
use super::{get_coin_decimals, get_pool_coins_type, SUI_RPC_NODE};
// 从当前crate的根模块引入 get_coin_in_out_v2 宏 (用于获取输入输出代币类型) 和相关类型定义。
use crate::{
    get_coin_in_out_v2,
    types::{Pool, PoolExtra, Protocol, SwapEvent, Token}, // 通用池结构, 协议特定附加信息, Protocol枚举, 通用交换事件, 代币信息结构。
};

/// `CETUS_POOL_CREATED` 常量
///
/// 定义了Cetus DEX协议在Sui链上发出的“新池创建”事件的全局唯一类型字符串。
pub const CETUS_POOL_CREATED: &str =
    "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb::factory::CreatePoolEvent";

/// `CETUS_SWAP_EVENT` 常量
///
/// 定义了Cetus DEX协议在Sui链上发出的“代币交换完成”事件的全局唯一类型字符串。
pub const CETUS_SWAP_EVENT: &str =
    "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb::pool::SwapEvent";

/// `CETUS_PACKAGE_ID` 常量
///
/// Cetus协议的核心智能合约包ID。
const CETUS_PACKAGE_ID: &str = "0x3a5aa90ffa33d09100d7b6941ea1c0ffe6ab66e77062ddd26320c1b073aabb10"; // Cetus主合约包 (CLMM)
/// `TICK_BOUND` 常量
///
/// Cetus CLMM池中tick索引的一个边界值，用于将有符号的tick索引转换为无符号的u64进行存储或计算。
const TICK_BOUND: i64 = 443636; // Cetus tick_spacing = 60, max_tick_index = 443636

/// `CETUS_POOL_LAYOUT` 静态变量
///
/// 使用 `OnceLock` 确保Cetus池对象的 `MoveStructLayout` 只被获取和初始化一次。
/// `MoveStructLayout` 描述了一个Move结构体的字段和类型布局，用于反序列化对象内容。
static CETUS_POOL_LAYOUT: OnceLock<MoveStructLayout> = OnceLock::new();

/// `cetus_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，专门用于订阅Cetus的“新池创建”事件。
pub fn cetus_event_filter() -> EventFilter {
    EventFilter::MoveEventType(CETUS_POOL_CREATED.parse().unwrap())
}

/// `CetusPoolCreated` 结构体
///
/// 用于存储从Cetus的 `CreatePoolEvent` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)]
pub struct CetusPoolCreated {
    pub pool: ObjectID,   // 新创建的池的ObjectID
    pub token0: String, // 池中第一个代币 (CoinTypeA) 的类型字符串 (已添加 "0x" 前缀)
    pub token1: String, // 池中第二个代币 (CoinTypeB) 的类型字符串 (已添加 "0x" 前缀)
} 

/// 为 `CetusPoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for CetusPoolCreated {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let parsed_json = &event.parsed_json;
        // 从JSON中提取 "pool_id" 字段，并解析为 ObjectID。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("CetusPoolCreated事件JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 从JSON中提取 "coin_type_a" 字段作为token0。
        // Cetus事件中的代币类型可能不带 "0x" 前缀，这里统一添加。
        let token0_raw_str = parsed_json["coin_type_a"]
            .as_str()
            .ok_or_else(|| eyre!("CetusPoolCreated事件JSON中缺少'coin_type_a'字段"))?;
        let token0_formatted_str = format!("0x{}", token0_raw_str);

        // 从JSON中提取 "coin_type_b" 字段作为token1。
        let token1_raw_str = parsed_json["coin_type_b"]
            .as_str()
            .ok_or_else(|| eyre!("CetusPoolCreated事件JSON中缺少'coin_type_b'字段"))?;
        let token1_formatted_str = format!("0x{}", token1_raw_str);

        Ok(Self {
            pool: pool_object_id,
            token0: token0_formatted_str,
            token1: token1_formatted_str,
        })
    }
}

impl CetusPoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `CetusPoolCreated` 事件数据转换为通用的 `Pool` 结构。
    /// 此方法会异步查询代币精度，并从链上对象获取池的 `fee_rate`。
    pub async fn to_pool(&self, sui: &SuiClient) -> Result<Pool> {
        // 异步获取token0和token1的精度信息
        let token0_decimals = get_coin_decimals(sui, &self.token0).await?;
        let token1_decimals = get_coin_decimals(sui, &self.token1).await?;

        // 设置获取对象数据时的选项，这里需要对象的内容 (content) 来提取 fee_rate。
        let object_data_options = SuiObjectDataOptions::default().with_content();

        // 从链上获取池对象的详细数据
        let pool_sui_object_data = sui
            .read_api()
            .get_object_with_options(self.pool, object_data_options) // 使用指定的pool ID和选项
            .await? // 等待异步调用完成
            .data // 获取RpcResult中的SuiObjectResponse
            .ok_or_else(|| eyre!("Cetus池对象 {} 在链上未找到或无数据 (Cetus pool object {} not found on-chain or has no data)", self.pool))?;

        // 从池对象的Move内容中提取 "fee_rate" 字段。
        // `fee_rate` 是一个u64值，代表手续费率 (例如，500表示0.05%)。
        let fee_rate_val: u64 = pool_sui_object_data
            .content // 获取对象内容 (SuiMoveObject)
            .ok_or_else(|| eyre!("Cetus池对象 {} 没有内容字段 (Cetus pool object {} has no content field)", self.pool))?
            .try_into_move() // 尝试将内容转换为 MoveObject
            .ok_or_else(|| eyre!("Cetus池对象 {} 的内容不是有效的MoveObject (Content of Cetus pool object {} is not a valid MoveObject)", self.pool))?
            .fields // 访问MoveObject的字段 (这是一个 BTreeMap<Identifier, MoveValue>)
            .field_value("fee_rate") // 按名称查找 "fee_rate" 字段
            .ok_or_else(|| eyre!("Cetus池对象 {} 中未找到'fee_rate'字段 (Missing 'fee_rate' field in Cetus pool object {})", self.pool))?
            .to_string() // 将MoveValue转换为字符串
            .parse()?; // 将字符串解析为u64

        // 创建 Token 结构列表
        let tokens_vec = vec![
            Token::new(&self.token0, token0_decimals),
            Token::new(&self.token1, token1_decimals),
        ];
        // 创建 PoolExtra::Cetus 枚举成员，存储Cetus特定的附加信息 (手续费率)
        let extra_data = PoolExtra::Cetus { fee_rate: fee_rate_val };

        Ok(Pool {
            protocol: Protocol::Cetus, // 指明协议为Cetus
            pool: self.pool,           // 池的ObjectID
            tokens: tokens_vec,        // 池中代币列表
            extra: extra_data,         // 协议特定附加信息
        })
    }
}

/// `CetusSwapEvent` 结构体
///
/// 用于存储从Cetus的 `SwapEvent` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)]
pub struct CetusSwapEvent {
    pub pool: ObjectID,       // 发生交换的池的ObjectID
    pub amount_in: u64,       // 输入代币的数量
    pub amount_out: u64,      // 输出代币的数量
    pub a2b: bool,            // 交易方向：true表示从代币A到代币B，false表示从B到A
                              // (Trading direction: true for token A to token B, false for B to A)
                              // 代币A和代币B的定义取决于池创建时的顺序。
                              // (Definition of token A and token B depends on the order during pool creation.)
}

/// 为 `CetusSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for CetusSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        // 确保事件类型与 `CETUS_SWAP_EVENT` 匹配。
        ensure!(event.type_.to_string() == CETUS_SWAP_EVENT, "事件类型不匹配Cetus SwapEvent (Not a CetusSwapEvent)");
        // 直接尝试从事件的 `parsed_json` 字段转换。
        (&event.parsed_json).try_into()
    }
}

/// 为 `CetusSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for CetusSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        ensure!(event.event_type == CETUS_SWAP_EVENT, "事件类型不匹配Cetus SwapEvent (Not a CetusSwapEvent)");
        event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json in ShioEvent)")?.try_into()
    }
}

/// 为 `CetusSwapEvent` 实现 `TryFrom<&Value>` trait (从 `serde_json::Value` 转换)。
impl TryFrom<&Value> for CetusSwapEvent {
    type Error = eyre::Error;

    fn try_from(parsed_json: &Value) -> Result<Self> {
        // 从JSON中提取 "pool" 字段并解析为 ObjectID。
        let pool_id_str = parsed_json["pool"]
            .as_str()
            .ok_or_else(|| eyre!("CetusSwapEvent JSON中缺少'pool'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 提取 "atob" 字段 (注意JSON中是小写 "atob"，不是 "a2b") 并解析为布尔值。
        let a_to_b_direction = parsed_json["atob"].as_bool().ok_or_else(|| eyre!("CetusSwapEvent JSON中缺少'atob'布尔字段"))?;

        // 提取 "amount_in" 字段并解析为 u64。
        let amount_in_val: u64 = parsed_json["amount_in"]
            .as_str()
            .ok_or_else(|| eyre!("CetusSwapEvent JSON中缺少'amount_in'字段"))?
            .parse()?;

        // 提取 "amount_out" 字段并解析为 u64。
        let amount_out_val: u64 = parsed_json["amount_out"]
            .as_str()
            .ok_or_else(|| eyre!("CetusSwapEvent JSON中缺少'amount_out'字段"))?
            .parse()?;

        Ok(Self {
            pool: pool_object_id,
            amount_in: amount_in_val,
            amount_out: amount_out_val,
            a2b: a_to_b_direction,
        })
    }
}

impl CetusSwapEvent {
    /// `to_swap_event_v1` 异步方法 (旧版本，使用SuiClient查询代币类型)
    ///
    /// 将 `CetusSwapEvent` 转换为通用的 `SwapEvent`。
    /// 这个版本需要一个 `SuiClient` 来从链上查询池对象的具体代币类型。
    /// (This version requires a `SuiClient` to query the specific coin types of the pool object from the chain.)
    #[allow(dead_code)] // 标记为允许死代码，因为可能优先使用v2版本
    pub async fn to_swap_event_v1(&self, sui: &SuiClient) -> Result<SwapEvent> {
        // 调用 `get_pool_coins_type` (定义在父模块 protocols::mod.rs) 来获取池的两种代币类型。
        let (coin_a_type, coin_b_type) = get_pool_coins_type(sui, self.pool).await?;
        // 根据 `self.a2b` 字段确定实际的输入和输出代币类型。
        let (final_coin_in, final_coin_out) = if self.a2b {
            (coin_a_type, coin_b_type) // a2b 为 true，则输入是 coin_a, 输出是 coin_b
        } else {
            (coin_b_type, coin_a_type) // a2b 为 false，则输入是 coin_b, 输出是 coin_a
        };

        Ok(SwapEvent {
            protocol: Protocol::Cetus,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }

    /// `to_swap_event_v2` 异步方法 (新版本，使用Simulator查询代币类型)
    ///
    /// 将 `CetusSwapEvent` 转换为通用的 `SwapEvent`。
    /// 这个版本使用一个 `Simulator` 实例 (可以是 `HttpSimulator` 或 `DBSimulator`)
    /// 来获取池的代币类型，这更通用，并且在测试或模拟环境中可能更方便。
    /// 它使用了 `get_coin_in_out_v2!` 宏来简化代币类型的获取和方向判断。
    pub async fn to_swap_event_v2(&self, provider: Arc<dyn Simulator>) -> Result<SwapEvent> {
        // `get_coin_in_out_v2!` 是一个宏，它接收池ID、模拟器provider和a2b方向，
        // 返回一个元组 `(String, String)` 代表 (输入代币类型, 输出代币类型)。
        // 这个宏内部可能调用 `provider.get_object_layout()` 和解析泛型参数的逻辑。
        let (final_coin_in, final_coin_out) = get_coin_in_out_v2!(self.pool, provider, self.a2b);

        Ok(SwapEvent {
            protocol: Protocol::Cetus,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

/// `cetus_related_object_ids` 函数
///
/// 返回与Cetus协议本身相关的核心对象ID列表 (硬编码)。
pub fn cetus_related_object_ids() -> Vec<String> {
    vec![
        // 以下是一些已知的Cetus相关包ID或核心对象ID
        "0xeffc8ae61f439bb34c9b905ff8f29ec56873dcedf81c7123ff2f1f67c45ec302", // CetusAggregator (Cetus聚合器)
        "0x11451575c775a3e633437b827ecbc1eb51a5964b0302210b28f5b89880be21a2", // CetusAggregator 2 (可能是聚合器的另一个版本或相关对象)
        "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb", // Cetus 4 (可能是Cetus协议的某个核心模块或工厂包ID，与事件类型中的包ID一致)
        "0x70968826ad1b4ba895753f634b0aea68d0672908ca1075a2abdf0fc9e0b2fc6a", // Cetus 19
        "0x3a5aa90ffa33d09100d7b6941ea1c0ffe6ab66e77062ddd26320c1b073aabb10", // Cetus 35 (可能是另一个核心模块或CLMM池的实现包ID)
        "0xdaa46292632c3c4d8f31f23ea0f9b36a28ff3677e9684980e4438403a67a3d8f", // Config (Cetus协议的全局配置对象)
        "0x639b5e433da31739e800cd085f356e64cae222966d0f1b11bd9dc76b322ff58b", // Partner (Cetus的合作伙伴对象)
        "0x714a63a0dba6da4f017b42d5d0fb78867f18bcde904868e51d951a5a6f5b7f57", // PoolTickIndex (可能与CLMM池的tick索引相关)
        "0xbe21a06129308e0495431d12286127897aff07a8ade3970495a4404d97f9eaaa", // PoolMath 1 (数学库或相关对象)
        "0xe2b515f0052c0b3f83c23db045d49dbe1732818ccfc5d4596c9482f7f2e76a85", // PoolMath 2
        "0xe93247b408fe44ed0ee5b6ac508b36325b239d6333e44ffa240dcc0c1a69cdd8", // PoolMath 3
        // 以下两个ID被注释为 "Frequent Unknown ID"，可能是调试中发现的、频繁出现但用途不明的对象ID。
        "0x74bb5afd49dddf13007101238012c033a5138474e00338126b318b5e3e4603a9", // Frequent Unknown ID
        "0xbfda3feb64a496c8d7fbb39a152d632ec1d1cefb2010b349adc3460937a592fe"  // Frequent Unknown ID
    ]
    .into_iter()
    .map(|s| s.to_string()) // 将 &str 转换为 String
    .collect::<Vec<_>>()
}

/// `cetus_pool_children_ids` 异步函数
///
/// 获取特定Cetus CLMM池的动态子对象ID，主要是与tick和position相关的对象。
/// CLMM池的流动性分布在不同的tick区间，每个tick和每个流动性头寸(position)都可能对应链上的子对象。
///
/// 参数:
/// - `pool`: 一个对 `Pool` 结构体的引用，代表要检查的Cetus池。
/// - `simulator`: 一个共享的模拟器实例，用于从链上（或缓存）获取对象数据和布局。
///
/// 返回:
/// - `Result<Vec<String>>`: 包含所有相关子对象ID字符串的列表。
pub async fn cetus_pool_children_ids(pool: &Pool, simulator: Arc<dyn Simulator>) -> Result<Vec<String>> {
    let mut result_ids_str_vec = vec![]; // 用于存储结果ID字符串的向量

    // 获取池对象的详细数据
    let pool_sui_object = simulator
        .get_object(&pool.pool)
        .await
        .ok_or_else(|| eyre!("Cetus池对象未找到: {}", pool.pool))?;

    // 解析池对象的Move结构体
    let parsed_pool_move_struct = {
        // `pool_layout` 函数会获取并缓存池对象的布局信息。
        let object_layout = pool_layout(pool.pool, simulator.clone()); // 克隆simulator的Arc指针

        let move_object_data = pool_sui_object.data.try_as_move().ok_or_eyre("对象不是有效的Move对象")?;
        MoveStruct::simple_deserialize(move_object_data.contents(), &object_layout).map_err(|e| eyre!(e))?
    };

    // 从池结构中提取 `tick_manager` 结构体
    let tick_manager_struct = extract_struct_from_move_struct(&parsed_pool_move_struct, "tick_manager")?;

    // --- 获取与 position 相关的子对象ID ---
    // 1. 从池结构中提取 `position_manager` 结构体。
    let position_manager_struct = extract_struct_from_move_struct(&parsed_pool_move_struct, "position_manager")?;
    // 2. 从 `position_manager` 中提取 `positions` 字段 (通常是一个Table或类似的数据结构)。
    let positions_table_struct = extract_struct_from_move_struct(&position_manager_struct, "positions")?;
    // 3. 从 `positions` 表结构中提取其 `id` 字段，这个ID是 `Table` 对象本身的ObjectID。
    let positions_table_id = {
        let id_field_wrapper = extract_struct_from_move_struct(&positions_table_struct, "id")?; // `id` 字段是 `UID` 类型
        let id_field_actual_id = extract_struct_from_move_struct(&id_field_wrapper, "id")?;   // `UID` 内部的 `ID` 类型
        let object_id_val = extract_object_id_from_move_struct(&id_field_actual_id, "bytes")?; // `ID` 内部的 `bytes` (实际ObjectID)
        object_id_val
    };

    // 创建SuiClient以调用 `get_dynamic_fields` (这个API不在Simulator trait中)
    // 注意: 直接在索引器内部创建新的SuiClient可能不是最佳实践，通常应复用已有的或通过参数传入。
    // 但对于一次性的辅助函数，或者如果此函数不常被调用，可能是可接受的。
    // 更好的做法可能是将 `get_dynamic_fields` 的逻辑也封装到 `Simulator` trait 中，或者作为 `SuiClient` 的扩展方法。
    let sui_client_instance = SuiClientBuilder::default()
        .build(SUI_RPC_NODE) // SUI_RPC_NODE 是在父模块定义的常量
        .await
        .unwrap(); // unwrap假设客户端构建总是成功

    // 分页获取 `positions` 表 (ObjectID: positions_table_id) 的所有动态字段。
    // 每个动态字段代表一个流动性头寸 (position) 对象。
    let mut next_page_cursor = None;
    let mut position_object_ids_vec = Vec::new();
    loop {
        let dynamic_fields_page = sui_client_instance
            .read_api()
            .get_dynamic_fields(positions_table_id, next_page_cursor, None) // None表示默认分页大小
            .await?;
        next_page_cursor = dynamic_fields_page.next_cursor; // 获取下一页的游标
        position_object_ids_vec.extend(dynamic_fields_page.data); // 追加当前页的动态字段信息
        if !dynamic_fields_page.has_next_page { // 如果没有下一页，则结束循环
            break;
        }
    }
    // 将获取到的动态字段信息中的 `object_id` 提取出来并转换为字符串。
    let position_ids_str_vec: Vec<String> = position_object_ids_vec.iter().map(|field_info| {
        field_info.object_id.to_string()
    }).collect();
    result_ids_str_vec.extend(position_ids_str_vec); // 添加到总结果列表中

    // --- 获取与 tick 相关的子对象ID ---
    let tick_key_type_tag = TypeTag::U64; // tick的键类型通常是u64 (tick_index + TICK_BOUND)
    let ticks_related_ids_set = { // 使用HashSet避免重复
        let mut unique_ids_set = HashSet::new();
        // 1. 从 `tick_manager` 结构中提取 `ticks` 字段 (通常是一个Table)。
        let ticks_table_struct = extract_struct_from_move_struct(&tick_manager_struct, "ticks")?;
        // 2. 获取 `ticks` 表本身的ObjectID。
        let ticks_table_id = {
            let id_field_wrapper = extract_struct_from_move_struct(&ticks_table_struct, "id")?;
            let id_field_actual_id = extract_struct_from_move_struct(&id_field_wrapper, "id")?;
            extract_object_id_from_move_struct(&id_field_actual_id, "bytes")?
        };

        // 分页获取 `ticks` 表的所有动态字段。每个字段代表一个已初始化的tick对象。
        let mut next_page_cursor_ticks = None;
        let mut initialized_tick_object_ids_vec = Vec::new();
        loop {
            let dynamic_fields_page_ticks = sui_client_instance
                .read_api()
                .get_dynamic_fields(ticks_table_id, next_page_cursor_ticks, None)
                .await?;
            next_page_cursor_ticks = dynamic_fields_page_ticks.next_cursor;
            initialized_tick_object_ids_vec.extend(dynamic_fields_page_ticks.data);
            if !dynamic_fields_page_ticks.has_next_page {
                break;
            }
        }
        // 将这些已初始化的tick对象的ID添加到结果中。
        let initialized_tick_ids_str_vec: Vec<String> = initialized_tick_object_ids_vec.iter().map(|field_info| {
            field_info.object_id.to_string()
        }).collect();
        unique_ids_set.extend(initialized_tick_ids_str_vec);

        // 3. 调用链上脚本 `fetcher_script::fetch_ticks` 获取与池相关的 `tick_scores`。
        //    `tick_scores` 可能代表了那些有流动性的、或者在价格曲线上比较关键的tick的索引。
        //    每个 `tick_score` (经过 `+ TICK_BOUND` 调整后) 可以用来派生出对应tick的动态字段ID。
        for tick_score_val in get_tick_scores(pool, &pool_sui_object, simulator).await? {
            if tick_score_val == 0 { // tick_score为0可能表示无效或特殊情况
                continue;
            }
            let key_bcs_bytes = bcs::to_bytes(&tick_score_val)?; // 将tick_score作为键进行BCS序列化
            // 派生出tick动态字段的ObjectID
            let tick_object_id = derive_dynamic_field_id(ticks_table_id, &tick_key_type_tag, &key_bcs_bytes)?;
            unique_ids_set.insert(tick_object_id.to_string());
        }
        unique_ids_set // 返回包含所有相关tick对象ID的HashSet
    };
    result_ids_str_vec.extend(ticks_related_ids_set); // 将tick ID集合添加到总结果列表中

    Ok(result_ids_str_vec)
}

/// `parse_tick_scores` (私有辅助函数)
///
/// 从 `fetch_ticks` 脚本调用返回的事件 (`SuiEvent`) 中解析出 `tick_scores`。
/// 事件的 `parsed_json` 中应该包含一个名为 "ticks" 的数组，数组每个元素有 "index" 和 "bits" 字段。
/// `tick_score` 由 `index.bits + TICK_BOUND` 计算得到。
fn parse_tick_scores(event: &SuiEvent) -> Result<Vec<u64>> {
    let parsed_json_data = &event.parsed_json;
    let ticks_array = parsed_json_data["ticks"].as_array().ok_or_eyre("SuiEvent JSON中缺少'ticks'数组 (Missing 'ticks' array in SuiEvent JSON)")?;

    // 使用Rayon并行处理ticks数组以提高效率
    let tick_scores_vec = ticks_array
        .par_iter() // 并行迭代器
        .filter_map(|tick_json_val| { // filter_map会过滤掉返回None的结果
            // 从每个tick的JSON中提取 "index" 对象，再从 "index" 中提取 "bits" (u64类型)。
            let index_obj = tick_json_val["index"].as_object().ok_or_eyre("tick JSON中缺少'index'对象 (Missing 'index' object in tick JSON)").ok()?;
            let index_bits_val: i32 = index_obj["bits"].as_u64().ok_or_eyre("index对象中缺少'bits'字段 (Missing 'bits' field in index object)").ok()? as i32;
            // 计算tick_score (有符号的tick索引转换为无符号的u64)
            let tick_score_val = (index_bits_val as i64 + TICK_BOUND) as u64;
            Some(tick_score_val)
        })
        .collect::<Vec<_>>(); // 收集所有有效的tick_score

    Ok(tick_scores_vec)
}

/// `get_tick_scores` 异步函数 (私有辅助函数)
///
/// 通过在模拟器中执行一个链上脚本 (`fetcher_script::fetch_ticks`) 来获取与指定Cetus池相关的 `tick_scores`。
/// 这个脚本会发出一个包含tick信息的事件，然后此函数解析该事件。
///
/// 参数:
/// - `pool`: 要查询tick scores的 `Pool` 对象。
/// - `pool_sui_object`: 该池的链上 `Object` 数据。
/// - `simulator`: 用于执行模拟交易的 `Simulator` 实例。
///
/// 返回:
/// - `Result<Vec<u64>>`: 包含 `tick_scores` 的向量。
async fn get_tick_scores(pool: &Pool, pool_sui_object: &Object, simulator: Arc<dyn Simulator>) -> Result<Vec<u64>> {
    let mut ptb_builder = ProgrammableTransactionBuilder::new(); // 创建PTB构建器

    // 定义要调用的Move合约信息
    let package_object_id = ObjectID::from_hex_literal(CETUS_PACKAGE_ID)?; // Cetus主包ID
    let module_ident = Identifier::new("fetcher_script").unwrap(); // `fetcher_script`模块
    let function_ident = Identifier::new("fetch_ticks").unwrap();  // `fetch_ticks`函数

    // 准备泛型类型参数，通常是池的两种代币类型 [CoinA, CoinB]
    let type_args_vec = vec![
        TypeTag::from_str(pool.token0_type().as_str()).unwrap(),
        TypeTag::from_str(pool.token1_type().as_str()).unwrap(),
    ];

    // 准备函数调用参数
    let call_args_vec = {
        // pool_arg: 将池对象转换为PTB参数 (这里使用可变共享对象)
        let pool_arg_ptb = ptb_builder.obj(shared_obj_arg(pool_sui_object, true)).unwrap();
        // start: 一个空的u32向量，作为 fetch_ticks 函数的起始参数
        let start_vec: Vec<u32> = vec![];
        let start_arg_ptb = ptb_builder.pure(start_vec).unwrap();
        // limit_arg: 限制获取的tick数量 (例如512个)
        let limit_arg_ptb = ptb_builder.pure(512u64).unwrap();

        vec![pool_arg_ptb, start_arg_ptb, limit_arg_ptb]
    };

    // 添加Move调用命令到PTB
    ptb_builder.command(Command::move_call(package_object_id, module_ident, function_ident, type_args_vec, call_args_vec));

    let pt_finished = ptb_builder.finish(); // 完成PTB构建
    // 创建一个随机的发送者地址用于模拟 (因为此调用通常是只读的或不依赖特定发送者)
    let sender_address = SuiAddress::random_for_testing_only();
    // 构建交易数据，Gas参数使用默认或预设值
    let tx_data_to_simulate = TransactionData::new_programmable(sender_address, vec![], pt_finished, 1000000000, 10000);

    let simulation_context = SimulateCtx::default(); // 使用默认的模拟上下文
    // 执行交易模拟
    let db_simulation_response = simulator.simulate(tx_data_to_simulate, simulation_context).await?;

    // 从模拟结果的事件中解析tick_scores
    let tick_scores_vec = db_simulation_response
        .events // 获取事件列表 (Option<Vec<SuiEvent>>)
        .data   // 获取实际的 Vec<SuiEvent> (如果存在)
        .into_iter() // 将Option<Vec>转为迭代器 (如果None则为空迭代器)
        .flatten()   // 将 Vec<SuiEvent> 扁平化为 SuiEvent 的迭代器
        .filter_map(|sui_event_item| parse_tick_scores(&sui_event_item).ok()) // 解析每个事件，忽略解析失败的
        .flatten() // 将 Vec<Vec<u64>> (因为parse_tick_scores返回Result<Vec<u64>>) 扁平化为 u64 的迭代器
        .collect::<Vec<_>>(); // 收集所有tick_scores

    Ok(tick_scores_vec)
}

/// `pool_layout` 函数
///
/// 获取并缓存指定Cetus池对象的 `MoveStructLayout`。
/// 使用 `OnceLock` (CETUS_POOL_LAYOUT) 确保布局信息只从模拟器获取一次。
fn pool_layout(pool_id: ObjectID, simulator: Arc<dyn Simulator>) -> MoveStructLayout {
    CETUS_POOL_LAYOUT
        .get_or_init(|| { // 如果 OnceLock 为空，则执行此闭包来初始化
            simulator
                .get_object_layout(&pool_id) // 从模拟器获取对象布局
                .expect("获取Cetus池对象布局失败 (Failed to get Cetus pool layout)")
        })
        .clone() // 克隆布局信息返回 (MoveStructLayout 实现了 Clone)
}

// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use super::*; // 导入外部模块 (cetus.rs) 的所有公共成员
    use mev_logger::LevelFilter; // 日志级别过滤器
    use simulator::{DBSimulator, HttpSimulator}; // 各种模拟器
    use tokio::time::Instant; // 精确时间点，用于计时
    use std::str::FromStr; // FromStr trait

    /// `test_swap_event_http` 测试函数 (HTTP模拟器)
    ///
    /// 测试 `CetusSwapEvent::to_swap_event_v2` 方法是否能正确转换事件并推断代币类型。
    /// 使用 `HttpSimulator` (通过RPC与真实节点交互)。
    #[tokio::test]
    async fn test_swap_event_http() {
        // 创建一个HttpSimulator实例 (RPC URL为空字符串，可能依赖环境变量或默认配置)
        let provider_http = HttpSimulator::new("", &None).await;

        // 创建一个示例的 CetusSwapEvent
        let swap_event_data = CetusSwapEvent {
            pool: ObjectID::from_str("0xdb36a73be4abfad79dc57e986f59294cd33f3c43bdf7cf265376f624be60cb18").unwrap(), // 一个已知的池ID
            amount_in: 0x1337, // 示例输入金额
            amount_out: 0x1338, // 示例输出金额
            a2b: true, // 假设方向是 A -> B
        };

        // 调用被测试方法
        let converted_swap_event = swap_event_data.to_swap_event_v2(Arc::new(provider_http)).await.unwrap();
        // 预期的代币A和代币B的类型 (这些需要与上面池ID实际对应的代币类型一致)
        let expected_coin_a_type = "0xa99b8952d4f7d947ea77fe0ecdcc9e5fc0bcab2841d6e2a5aa00c3044e5544b5::navx::NAVX";
        let expected_coin_b_type = "0x549e8b69270defbfafd4f94e17ec44cdbdd99820b33bda2278dea3b9a32d3f55::cert::CERT";

        // 断言转换后的输入输出代币类型是否正确
        assert_eq!(converted_swap_event.coins_in[0], expected_coin_a_type);
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_b_type);
    }

    /// `test_swap_event_db` 测试函数 (数据库模拟器)
    ///
    /// 与上一个测试类似，但使用 `DBSimulator`。
    /// `DBSimulator` 需要预先填充或配置好相关的对象数据才能正确工作。
    #[tokio::test]
    async fn test_swap_event_db() {
        // 创建一个默认配置的慢速DBSimulator (可能连接到持久化存储)
        let provider_db = DBSimulator::new_default_slow().await;

        let swap_event_data = CetusSwapEvent {
            pool: ObjectID::from_str("0xdb36a73be4abfad79dc57e986f59294cd33f3c43bdf7cf265376f624be60cb18").unwrap(),
            amount_in: 0x1337,
            amount_out: 0x1338,
            a2b: true,
        };

        let converted_swap_event = swap_event_data.to_swap_event_v2(Arc::new(provider_db)).await.unwrap();
        let expected_coin_a_type = "0xa99b8952d4f7d947ea77fe0ecdcc9e5fc0bcab2841d6e2a5aa00c3044e5544b5::navx::NAVX";
        let expected_coin_b_type = "0x549e8b69270defbfafd4f94e17ec44cdbdd99820b33bda2278dea3b9a32d3f55::cert::CERT";

        assert_eq!(converted_swap_event.coins_in[0], expected_coin_a_type);
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_b_type);
    }

    /// `test_cetus_pool_children_ids` 测试函数
    ///
    /// 测试 `cetus_pool_children_ids` 函数是否能为给定的Cetus池正确派生相关的子对象ID。
    #[tokio::test]
    async fn test_cetus_pool_children_ids() {
        mev_logger::init_console_logger(Some(LevelFilter::INFO)); // 初始化日志

        // 创建一个示例的 Pool 结构体
        let pool_info = Pool {
            protocol: Protocol::Cetus,
            // 这是一个示例池ID，实际测试时可能需要替换为测试网络上有效的Cetus池ID
            pool: ObjectID::from_str("0x3c3dd05e348fba5d8bf6958369cc3b33c8e8be85c96e10b1ca6413ad1b2d7787").unwrap(),
            tokens: vec![ // 池中的两种代币
                Token::new(
                    "0xdb5162ae510a06dd9ce09016612e64328a27914e9570048bbb8e61b2cb5d6b3e::kw::KW", // 示例代币KW
                    9,
                ),
                Token::new("0x2::sui::SUI", 9), // SUI
            ],
            extra: PoolExtra::None, // 对于此测试，extra信息不关键
        };

        // 创建一个DBSimulator实例用于测试 (带回退)
        let simulator_instance: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);

        let start_time = Instant::now(); // 开始计时
        // 调用被测试函数
        let children_ids_vec = cetus_pool_children_ids(&pool_info, simulator_instance).await.unwrap();
        println!("获取Cetus池子对象耗时: {} ms", start_time.elapsed().as_millis()); // 打印耗时
        println!("为池 {} 派生的子对象ID列表: {:?}", pool_info.pool, children_ids_vec); // 打印结果
        // 可以在这里添加断言来验证结果。
    } 

    /// `test_judge_cetus_pool_children_ids` 测试函数 (判断特定ID是否存在)
    ///
    /// 与上一个测试类似，但额外判断结果中是否包含某些预期的ID。
    #[tokio::test]
    async fn test_judge_cetus_pool_children_ids() {
        let pool_info = Pool {
            protocol: Protocol::Cetus,
            pool: ObjectID::from_str("0xefb30c2780bb10ffd4cf860049248dcc4b204927ca63c4c2e4d0ae5666a280d5").unwrap(),
            tokens: vec![
                Token::new(
                    "0xdb5162ae510a06dd9ce09016612e64328a27914e9570048bbb8e61b2cb5d6b3e::kw::KW",
                    9,
                ),
                Token::new("0x2::sui::SUI", 9),
            ],
            extra: PoolExtra::None,
        };
        let simulator_instance: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);

        let children_ids_vec = cetus_pool_children_ids(&pool_info, simulator_instance).await.unwrap();
        // 检查结果中是否包含两个特定的ID字符串
        if children_ids_vec.contains(&"0x2dd0e8a1758121da7fc615a7d8923ffeaeb9ae5852882d2d4179193e3b9e7c1e".to_string()) || children_ids_vec.contains(&"0x26e641e6c1734ed2733701e6f7708f0c8816c665c31b89a7cfd6fee3ffdcfb82".to_string()) {
            println!("==================> 成功：在子对象ID列表中找到预期ID (Success: Found expected ID in children IDs list)");
        } else {
            println!("==================> 失败：在子对象ID列表中未找到预期ID (Failed: Did not find expected ID in children IDs list)");
        }
    }
}

[end of crates/dex-indexer/src/protocols/cetus.rs]
