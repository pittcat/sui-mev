// 该文件 `kriya_clmm.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 KriyaDEX 协议的 CLMM (集中流动性做市商) 池相关的事件和数据结构。
// KriyaDEX 是 Sui 上的一个DEX，同时提供传统 AMM 池和 CLMM 池，此文件专注于其 CLMM 部分。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” KriyaDEX CLMM 池发生的两种主要“事件”的：
// “有人创建了一个新的交易池 (`PoolCreatedEvent`)” 和 “有人在这个池子里完成了一笔代币交换 (`SwapEvent`)”。
// 它还负责收集与Kriya CLMM协议本身以及特定池子相关联的一些重要的“全局对象”和“动态子对象”的ID。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别Kriya CLMM的特定事件**:
//     -   `KRIYA_CLMM_POOL_CREATED` 和 `KRIYA_CLMM_SWAP_EVENT` 常量是这两种事件在Sui链上的“专属门牌号”。
//         `PoolCreatedEvent` 来自 `create_pool` 模块，`SwapEvent` 来自 `trade` 模块。
//     -   `kriya_clmm_event_filter()` 函数生成一个“过滤器”，告诉Sui节点只订阅Kriya CLMM创建新池子的事件。
//
// 2.  **`KriyaClmmPoolCreated` 结构体 (新池子信息卡)**:
//     -   当监听到Kriya CLMM新池子创建事件 (`PoolCreatedEvent`) 时，这个结构体记录事件信息
//         (池ID `pool_id`、两种代币类型 `token0`, `token1`、手续费率 `fee_rate`)。
//     -   `TryFrom<&SuiEvent>` 是“填卡员”，从原始Sui事件中提取数据。
//         注意：事件中的代币类型 (`type_x`, `type_y`) 需要手动添加 "0x" 前缀。
//     -   `to_pool()` 方法将这张Kriya CLMM专用的“信息卡”转换为通用的 `Pool` 结构，
//         并将手续费率存储在 `PoolExtra::KriyaClmm` 中。
//
// 3.  **`KriyaClmmSwapEvent` 结构体 (交换记录卡)**:
//     -   记录Kriya CLMM交换事件 (`SwapEvent`) 的详细信息（池ID、输入金额、输出金额、交易方向 `a2b`）。
//         Kriya CLMM的 `SwapEvent` JSON负载中包含 `amount_x`, `amount_y` 和 `x_for_y` (布尔值)，
//         需要根据 `x_for_y` 来确定哪个是输入金额/代币，哪个是输出。
//     -   `TryFrom` 实现能从不同来源（`SuiEvent`, `ShioEvent`, 原始JSON `Value`）提取信息。
//     -   `to_swap_event_v1/_v2()` 方法将其转换为通用的 `SwapEvent`。
//
// 4.  **收集相关对象ID (Collecting Related Object IDs)**:
//     -   `KRIYA_CLMM_POOL_LAYOUT`: 使用 `OnceLock` 缓存池对象的布局。
//     -   `kriya_clmm_related_object_ids()`: 列出了一些硬编码的Kriya CLMM相关对象ID
//         (如主包ID `KRIYA_CLMM`、版本对象 `VERSION`、数学库、tick管理、资金库等)。
//     -   `kriya_clmm_pool_children_ids()`: 这是一个复杂函数，用于获取一个Kriya CLMM池的动态子对象ID。
//         -   它首先会派生一个与 "trading_enabled" 相关的动态字段ID。
//         -   然后解析池对象，提取 `ticks` (tick表) 和 `tick_bitmap` (tick位图) 的表ID。
//         -   通过 `get_dynamic_fields` API 分页获取这两个表中的所有动态字段对象ID (代表已初始化的ticks和bitmap节点)。
//         这反映了CLMM池子需要管理大量与tick相关的子对象。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
// (与 cetus.rs, flowx_clmm.rs 等文件中的解释类似，主要涉及CLMM、Tick、动态字段、版本对象等。)
//
// -   **Tick (价格刻度)** 和 **Tick Bitmap (Tick位图)**:
//     Kriya CLMM作为集中流动性做市商，其核心机制也围绕着离散的价格刻度（Tick）和用于高效查找有流动性刻度的位图（Bitmap）。
//     `kriya_clmm_pool_children_ids` 函数中获取 `ticks` 表和 `tick_bitmap` 表的动态子字段，
//     就是为了索引这些构成池子流动性结构的关键对象。

// 引入标准库的 Arc (原子引用计数), OnceLock (单次初始化), str::FromStr (从字符串转换)。
use std::sync::{Arc, OnceLock};
use std::str::FromStr;

// 引入 eyre 库，用于错误处理和上下文管理。
use eyre::{ensure, eyre, OptionExt, Result};
// 引入 Move 核心类型中的 MoveStruct 和 MoveStructLayout，用于解析Move对象的结构和布局。
use move_core_types::annotated_value::{MoveStruct, MoveStructLayout};
// 引入 serde 库的 Deserialize trait，用于从如JSON这样的格式反序列化数据。
use serde::Deserialize;
// 引入 serde_json 的 Value 类型，用于表示通用的JSON值。
use serde_json::Value;
// 引入 shio 库的 ShioEvent 类型，可能用于处理与MEV相关的特定事件。
use shio::ShioEvent;
// 引入 simulator 库的 Simulator trait，定义了交易模拟器的通用接口。
use simulator::Simulator;
// 引入 Sui SDK 中的相关类型。
use sui_sdk::{
    rpc_types::{EventFilter, SuiEvent}, // EventFilter用于订阅事件，SuiEvent代表链上事件。
    types::{base_types::ObjectID, TypeTag},                             // ObjectID, TypeTag。
    SuiClient, SuiClientBuilder,                                        // Sui RPC客户端和构建器。
};
// 引入 Sui 核心类型中的动态字段ID派生函数。
use sui_types::dynamic_field::derive_dynamic_field_id;
// 引入 utils 库的 object 模块中的辅助函数。
use utils::object::{
    extract_object_id_from_move_struct, extract_struct_from_move_struct,
};

// 从当前crate的父模块 (protocols) 引入 get_coin_decimals, get_pool_coins_type 函数和 SUI_RPC_NODE 常量。
use super::{get_coin_decimals, get_pool_coins_type, SUI_RPC_NODE};
// 从当前crate的根模块引入 get_coin_in_out_v2 宏和相关类型定义。
use crate::{
    get_coin_in_out_v2,
    types::{Pool, PoolExtra, Protocol, SwapEvent, Token},
};

/// `KRIYA_CLMM_POOL_CREATED` 常量
///
/// 定义了KriyaDEX CLMM在Sui链上发出的“新池创建”事件的全局唯一类型字符串。
/// 事件由 `create_pool` 模块发出。
const KRIYA_CLMM_POOL_CREATED: &str =
    "0xf6c05e2d9301e6e91dc6ab6c3ca918f7d55896e1f1edd64adc0e615cde27ebf1::create_pool::PoolCreatedEvent";

/// `KRIYA_CLMM_SWAP_EVENT` 常量
///
/// 定义了KriyaDEX CLMM在Sui链上发出的“代币交换完成”事件的全局唯一类型字符串。
/// 事件由 `trade` 模块发出。
pub const KRIYA_CLMM_SWAP_EVENT: &str =
    "0xf6c05e2d9301e6e91dc6ab6c3ca918f7d55896e1f1edd64adc0e615cde27ebf1::trade::SwapEvent";

/// `KRIYA_CLMM_POOL_LAYOUT` 静态变量
///
/// 使用 `OnceLock` 确保Kriya CLMM池对象的 `MoveStructLayout` 只被获取和初始化一次。
static KRIYA_CLMM_POOL_LAYOUT: OnceLock<MoveStructLayout> = OnceLock::new();

/// `kriya_clmm_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，专门用于订阅KriyaDEX CLMM的“新池创建”事件。
pub fn kriya_clmm_event_filter() -> EventFilter {
    EventFilter::MoveEventType(KRIYA_CLMM_POOL_CREATED.parse().unwrap())
}

/// `KriyaClmmPoolCreated` 结构体
///
/// 用于存储从KriyaDEX CLMM的 `PoolCreatedEvent` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)]
pub struct KriyaClmmPoolCreated {
    pub pool: ObjectID,   // 新创建的池的ObjectID
    pub token0: String, // 池中第一个代币 (TypeX) 的类型字符串 (已添加 "0x" 前缀)
    pub token1: String, // 池中第二个代币 (TypeY) 的类型字符串 (已添加 "0x" 前缀)
    pub fee_rate: u64,  // 池的手续费率
}

/// 为 `KriyaClmmPoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for KriyaClmmPoolCreated {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let parsed_json = &event.parsed_json;
        // 从JSON中提取 "pool_id" 字段。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaClmmPoolCreated事件JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 从JSON中提取 "type_x" 对象，再从中取 "name" 字段作为token0。
        let type_x_obj = parsed_json["type_x"]
            .as_object()
            .ok_or_else(|| eyre!("KriyaClmmPoolCreated事件JSON中缺少'type_x'对象"))?;
        let token0_raw_str = type_x_obj["name"]
            .as_str()
            .ok_or_else(|| eyre!("'type_x'对象中缺少'name'字段"))?;
        let token0_formatted_str = format!("0x{}", token0_raw_str); // 添加 "0x" 前缀

        // 类似地提取 "type_y" 作为token1。
        let type_y_obj = parsed_json["type_y"]
            .as_object()
            .ok_or_else(|| eyre!("KriyaClmmPoolCreated事件JSON中缺少'type_y'对象"))?;
        let token1_raw_str = type_y_obj["name"]
            .as_str()
            .ok_or_else(|| eyre!("'type_y'对象中缺少'name'字段"))?;
        let token1_formatted_str = format!("0x{}", token1_raw_str);

        // 提取 "fee_rate" 字段并解析为u64。
        let fee_rate_val: u64 = parsed_json["fee_rate"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaClmmPoolCreated事件JSON中缺少'fee_rate'字段"))?
            .parse()?;

        Ok(Self {
            pool: pool_object_id,
            token0: token0_formatted_str,
            token1: token1_formatted_str,
            fee_rate: fee_rate_val,
        })
    }
}

impl KriyaClmmPoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `KriyaClmmPoolCreated` 事件数据转换为通用的 `Pool` 结构。
    /// 需要异步查询代币精度。手续费率已在事件中提供。
    pub async fn to_pool(&self, sui: &SuiClient) -> Result<Pool> {
        // 异步获取token0和token1的精度信息
        let token0_decimals = get_coin_decimals(sui, &self.token0).await?;
        let token1_decimals = get_coin_decimals(sui, &self.token1).await?;

        // 创建 Token 结构列表
        let tokens_vec = vec![
            Token::new(&self.token0, token0_decimals),
            Token::new(&self.token1, token1_decimals),
        ];
        // 创建 PoolExtra::KriyaClmm，存储Kriya CLMM特定的手续费率
        let extra_data = PoolExtra::KriyaClmm {
            fee_rate: self.fee_rate,
        };

        Ok(Pool {
            protocol: Protocol::KriyaClmm, // 指明协议为KriyaClmm
            pool: self.pool,              // 池的ObjectID
            tokens: tokens_vec,           // 池中代币列表
            extra: extra_data,            // 协议特定附加信息
        })
    }
}

/// `KriyaClmmSwapEvent` 结构体
///
/// 用于存储从KriyaDEX CLMM的 `SwapEvent` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)]
pub struct KriyaClmmSwapEvent {
    pub pool: ObjectID,       // 发生交换的池的ObjectID (从 "pool_id" 字段获取)
    pub amount_in: u64,       // 输入代币的数量
    pub amount_out: u64,      // 输出代币的数量
    pub a2b: bool,            // 交易方向：true表示从代币X到代币Y (x_for_y)，false表示从Y到X
}

/// 为 `KriyaClmmSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for KriyaClmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        // 确保事件类型与 `KRIYA_CLMM_SWAP_EVENT` 匹配。
        // Kriya CLMM SwapEvent 类型没有泛型参数。
        ensure!(
            event.type_.to_string() == KRIYA_CLMM_SWAP_EVENT,
            "事件类型不匹配Kriya CLMM SwapEvent (Not a KriyaClmmSwapEvent)"
        );
        // 直接尝试从事件的 `parsed_json` 字段转换。
        (&event.parsed_json).try_into()
    }
}

/// 为 `KriyaClmmSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for KriyaClmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        ensure!(event.event_type == KRIYA_CLMM_SWAP_EVENT, "事件类型不匹配Kriya CLMM SwapEvent (Not a KriyaClmmSwapEvent)");
        event
            .parsed_json
            .as_ref()
            .ok_or_else(|| eyre!("ShioEvent中缺少parsed_json字段 (Missing parsed_json in ShioEvent)"))?
            .try_into()
    }
}

/// 为 `KriyaClmmSwapEvent` 实现 `TryFrom<&Value>` trait (从 `serde_json::Value` 转换)。
/// 这是实际的JSON解析逻辑。
/// Kriya CLMM的 `SwapEvent` JSON结构包含 `pool_id`, `amount_x`, `amount_y`, `x_for_y` 字段。
impl TryFrom<&Value> for KriyaClmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(parsed_json: &Value) -> Result<Self> {
        // 从JSON中提取 "pool_id" 字段并解析为 ObjectID。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaClmmSwapEvent JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 提取 "amount_x" (代表代币X的变动量) 并解析为 u64。
        let amount_x_val: u64 = parsed_json["amount_x"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaClmmSwapEvent JSON中缺少'amount_x'字段"))?
            .parse()?;

        // 提取 "amount_y" (代表代币Y的变动量) 并解析为 u64。
        let amount_y_val: u64 = parsed_json["amount_y"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaClmmSwapEvent JSON中缺少'amount_y'字段"))?
            .parse()?;

        // 提取 "x_for_y" 字段 (布尔值)，表示交易方向是否为 X -> Y。
        let x_to_y_direction = parsed_json["x_for_y"]
            .as_bool()
            .ok_or_else(|| eyre!("KriyaClmmSwapEvent JSON中缺少'x_for_y'布尔字段"))?;

        // 根据 `x_for_y` 的值确定实际的输入金额和输出金额。
        let (final_amount_in, final_amount_out) = if x_to_y_direction {
            (amount_x_val, amount_y_val) // X是输入, Y是输出
        } else {
            (amount_y_val, amount_x_val) // Y是输入, X是输出
        };

        Ok(Self {
            pool: pool_object_id,
            amount_in: final_amount_in,
            amount_out: final_amount_out,
            a2b: x_to_y_direction,
        })
    }
}

impl KriyaClmmSwapEvent {
    /// `to_swap_event_v1` 异步方法 (旧版本，使用SuiClient查询代币类型)
    #[allow(dead_code)] // 标记为允许死代码
    pub async fn to_swap_event_v1(&self, sui: &SuiClient) -> Result<SwapEvent> {
        let (coin_a_type, coin_b_type) = get_pool_coins_type(sui, self.pool).await?;
        let (final_coin_in, final_coin_out) = if self.a2b {
            (coin_a_type, coin_b_type)
        } else {
            (coin_b_type, coin_a_type)
        };

        Ok(SwapEvent {
            protocol: Protocol::KriyaClmm,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }

    /// `to_swap_event_v2` 异步方法 (新版本，使用Simulator查询代币类型)
    ///
    /// 将 `KriyaClmmSwapEvent` 转换为通用的 `SwapEvent`。
    /// 使用 `Simulator` 实例和 `get_coin_in_out_v2!` 宏来获取和判断代币类型。
    /// 示例链接: https://suiscan.xyz/mainnet/tx/EmDQqPrUeQDgk8bbM7YquDW4gF6PHCfpL4D41MoHbQW1
    pub async fn to_swap_event_v2(&self, provider: Arc<dyn Simulator>) -> Result<SwapEvent> {
        let (final_coin_in, final_coin_out) = get_coin_in_out_v2!(self.pool, provider, self.a2b);

        Ok(SwapEvent {
            protocol: Protocol::KriyaClmm,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

/// `kriya_clmm_related_object_ids` 函数
///
/// 返回与Kriya CLMM协议本身相关的核心对象ID列表 (硬编码)。
pub fn kriya_clmm_related_object_ids() -> Vec<String> {
    vec![
        // Kriya CLMM 主包ID
        "0xbd8d4489782042c6fafad4de4bc6a5e0b84a43c6c00647ffd7062d1e2bb7549e", // KriyaClmm Package ID
        // Kriya CLMM 版本对象ID
        "0xf5145a7ac345ca8736cf8c76047d00d6d378f30e81be6f6eb557184d9de93c78", // Version Object ID
        // Kriya CLMM 数学库/逻辑相关对象ID (与 create_pool 事件包ID相同)
        "0xf6c05e2d9301e6e91dc6ab6c3ca918f7d55896e1f1edd64adc0e615cde27ebf1", // Math / Pool Factory related (same as PoolCreated event package)
        // Kriya CLMM Tick管理相关对象ID
        "0x9d856cdba9618289f3262e2ede47d9bb49f0f98b007a5e24f66f46e85b1b9f5a", // Tick Object ID
        // Kriya CLMM 升级服务相关对象ID
        "0xe0917b74a5912e4ad186ac634e29c922ab83903f71af7500969f9411706f9b9a", // upgrade_service Object ID
        // Kriya CLMM 资金库对象ID
        "0xecf47609d7da919ea98e7fd04f6e0648a0a79b337aaad373fa37aac8febf19c8", // treasury Object ID
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect::<Vec<_>>()
}

/// `kriya_clmm_pool_children_ids` 异步函数
///
/// 获取特定Kriya CLMM池的动态子对象ID，主要是与tick位图和tick表相关的对象。
///
/// 参数:
/// - `pool`: 一个对 `Pool` 结构体的引用，代表要检查的Kriya CLMM池。
/// - `simulator`: 一个共享的模拟器实例，用于从链上（或缓存）获取对象数据和布局。
///
/// 返回:
/// - `Result<Vec<String>>`: 包含所有相关子对象ID字符串的列表。
pub async fn kriya_clmm_pool_children_ids(pool: &Pool, simulator: Arc<dyn Simulator>) -> Result<Vec<String>> {
    let mut result_ids_str_vec = vec![]; // 用于存储结果ID字符串的向量
    let parent_pool_id = pool.pool; // 池的ObjectID作为父ID

    // --- 派生与 "trading_enabled" 相关的动态字段ID ---
    // Kriya CLMM池可能使用一个动态字段来标记池是否允许交易。
    // 键的类型是 Vec<u8> (通过 bcs::to_bytes(&"trading_enabled".to_string()) 生成)。
    // 值的类型标签是 Vector(U8) (即 Vec<u8> 的Move表示)。
    {
        let key_type_tag = TypeTag::Vector(Box::new(TypeTag::U8)); // 键的类型是 vector<u8>
        let key_value_str = "trading_enabled".to_string(); // 键的值是字符串 "trading_enabled"
        let key_value_bcs_bytes = bcs::to_bytes(&key_value_str)?; // 将键BCS序列化
        // 派生动态字段ID
        let child_object_id = derive_dynamic_field_id(parent_pool_id, &key_type_tag, &key_value_bcs_bytes)?;
        result_ids_str_vec.push(child_object_id.to_string());
    };

    // 获取池对象的详细数据并解析其Move结构体
    let parsed_pool_move_struct = {
        let pool_sui_object = simulator
            .get_object(&parent_pool_id)
            .await
            .ok_or_else(|| eyre!("Kriya CLMM池对象未找到: {}", parent_pool_id))?;
        // 获取并缓存池对象布局
        let object_layout = pool_layout(parent_pool_id, simulator.clone()); // 克隆simulator的Arc指针
        let move_object_data = pool_sui_object.data.try_as_move().ok_or_eyre("对象不是有效的Move对象")?;
        MoveStruct::simple_deserialize(move_object_data.contents(), &object_layout).map_err(|e| eyre!(e))?
    };

    // 创建SuiClient以调用 `get_dynamic_fields`
    let sui_client_instance = SuiClientBuilder::default().build(SUI_RPC_NODE).await.unwrap();

    // --- 获取ticks (tick表) 的所有动态子字段ID ---
    {
        // 从池结构中提取 `ticks` 结构体 (代表tick表)
        let ticks_table_struct = extract_struct_from_move_struct(&parsed_pool_move_struct, "ticks")?;
        // 获取 `ticks` 表本身的ObjectID
        let ticks_table_id = {
            let id_field_wrapper = extract_struct_from_move_struct(&ticks_table_struct, "id")?;
            let id_field_actual_id = extract_struct_from_move_struct(&id_field_wrapper, "id")?;
            extract_object_id_from_move_struct(&id_field_actual_id, "bytes")?
        };

        // 分页获取 `ticks` 表的所有动态字段 (每个字段代表一个已初始化的tick对象)
        let mut next_page_cursor_ticks = None;
        let mut initialized_tick_object_ids_info = Vec::new();
        loop {
            let dynamic_fields_page = sui_client_instance
                .read_api()
                .get_dynamic_fields(ticks_table_id, next_page_cursor_ticks, None)
                .await?;
            next_page_cursor_ticks = dynamic_fields_page.next_cursor;
            initialized_tick_object_ids_info.extend(dynamic_fields_page.data);
            if !dynamic_fields_page.has_next_page {
                break;
            }
        }
        // 将这些已初始化的tick对象的ID添加到结果中
        let initialized_tick_ids_str_vec: Vec<String> = initialized_tick_object_ids_info.iter().map(|field_info| {
            field_info.object_id.to_string()
        }).collect();
        result_ids_str_vec.extend(initialized_tick_ids_str_vec);
    }

    // --- 获取tick_bitmap (tick位图) 的所有动态子字段ID ---
    {
        // 从池结构中提取 `tick_bitmap` 结构体
        let tick_bitmap_struct = extract_struct_from_move_struct(&parsed_pool_move_struct, "tick_bitmap")?;
        // 获取 `tick_bitmap` 表本身的ObjectID
        let tick_bitmap_table_id = {
            let id_field_wrapper = extract_struct_from_move_struct(&tick_bitmap_struct, "id")?;
            let id_field_actual_id = extract_struct_from_move_struct(&id_field_wrapper, "id")?;
            extract_object_id_from_move_struct(&id_field_actual_id, "bytes")?
        };

        // 分页获取 `tick_bitmap` 表的所有动态字段
        let mut next_page_cursor_bitmap = None;
        let mut bitmap_child_fields_info = Vec::new();
        loop {
            let dynamic_fields_page = sui_client_instance
                .read_api()
                .get_dynamic_fields(tick_bitmap_table_id, next_page_cursor_bitmap, None)
                .await?;
            next_page_cursor_bitmap = dynamic_fields_page.next_cursor;
            bitmap_child_fields_info.extend(dynamic_fields_page.data);
            if !dynamic_fields_page.has_next_page {
                break;
            }
        }
        // 将这些动态字段的ObjectID添加到结果中
        let bitmap_child_ids_str_vec: Vec<String> = bitmap_child_fields_info.iter().map(|field_info| {
            field_info.object_id.to_string()
        }).collect();
        result_ids_str_vec.extend(bitmap_child_ids_str_vec);
    }

    Ok(result_ids_str_vec)
}

/// `pool_layout` 函数
///
/// 获取并缓存指定Kriya CLMM池对象的 `MoveStructLayout`。
/// 使用 `OnceLock` (KRIYA_CLMM_POOL_LAYOUT) 确保布局信息只从模拟器获取一次。
fn pool_layout(pool_id: ObjectID, simulator: Arc<dyn Simulator>) -> MoveStructLayout {
    KRIYA_CLMM_POOL_LAYOUT
        .get_or_init(|| {
            simulator
                .get_object_layout(&pool_id)
                .expect("获取Kriya CLMM池对象布局失败 (Failed to get KriyaClmm pool layout)")
        })
        .clone()
}

// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*; // 导入外部模块 (kriya_clmm.rs) 的所有公共成员
    use mev_logger::LevelFilter; // 日志级别过滤器
    use simulator::DBSimulator;  // 数据库模拟器
    use simulator::HttpSimulator; // HTTP模拟器
    use tokio::time::Instant; // 精确时间点，用于计时

    /// `test_swap_event_http` 测试函数 (HTTP模拟器)
    ///
    /// 测试 `KriyaClmmSwapEvent::to_swap_event_v2` 方法。
    #[tokio::test]
    async fn test_swap_event_http() {
        // 创建HttpSimulator (SUI_RPC_NODE 是在父模块定义的常量)
        let provider_http = HttpSimulator::new(SUI_RPC_NODE, &None).await;

        // 创建一个示例的 KriyaClmmSwapEvent
        let kriya_swap_event_data = KriyaClmmSwapEvent {
            // 这是一个示例池ID，需要替换为实际有效的Kriya CLMM池ID
            pool: ObjectID::from_str("0x4ab1017f5a10d122fdfc6656f6c2f7cc641edc1e2d12680cd9d98cf59d4e7e7b").unwrap(),
            amount_in: 0x1337,  // 示例输入金额
            amount_out: 0x1338, // 示例输出金额
            a2b: true,          // 假设方向是 A -> B (X -> Y)
        };

        // 调用被测试方法
        let converted_swap_event = kriya_swap_event_data.to_swap_event_v2(Arc::new(provider_http)).await.unwrap();
        // 预期的代币A和代币B的类型 (需要与上面池ID实际对应的代币类型一致)
        let expected_coin_a_type = "0x549e8b69270defbfafd4f94e17ec44cdbdd99820b33bda2278dea3b9a32d3f55::cert::CERT";
        let expected_coin_b_type = "0xa99b8952d4f7d947ea77fe0ecdcc9e5fc0bcab2841d6e2a5aa00c3044e5544b5::navx::NAVX";

        // 断言转换后的输入输出代币类型
        assert_eq!(converted_swap_event.coins_in[0], expected_coin_a_type, "输入代币类型不匹配");
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_b_type, "输出代币类型不匹配");
    }

    /// `test_swap_event_db` 测试函数 (数据库模拟器)
    ///
    /// 与上一个测试类似，但使用 `DBSimulator`。
    #[tokio::test]
    async fn test_swap_event_db() {
        let provider_db = DBSimulator::new_default_slow().await; // 创建DBSimulator

        let kriya_swap_event_data = KriyaClmmSwapEvent {
            pool: ObjectID::from_str("0x4ab1017f5a10d122fdfc6656f6c2f7cc641edc1e2d12680cd9d98cf59d4e7e7b").unwrap(),
            amount_in: 0x1337,
            amount_out: 0x1338,
            a2b: true,
        };

        let converted_swap_event = kriya_swap_event_data.to_swap_event_v2(Arc::new(provider_db)).await.unwrap();
        let expected_coin_a_type = "0x549e8b69270defbfafd4f94e17ec44cdbdd99820b33bda2278dea3b9a32d3f55::cert::CERT";
        let expected_coin_b_type = "0xa99b8952d4f7d947ea77fe0ecdcc9e5fc0bcab2841d6e2a5aa00c3044e5544b5::navx::NAVX";

        assert_eq!(converted_swap_event.coins_in[0], expected_coin_a_type, "输入代币类型不匹配");
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_b_type, "输出代币类型不匹配");
    }

    /// `test_kriya_clmm_pool_children_ids` 测试函数
    ///
    /// 测试 `kriya_clmm_pool_children_ids` 函数是否能为给定的Kriya CLMM池正确派生相关的子对象ID。
    #[tokio::test]
    async fn test_kriya_clmm_pool_children_ids() {
        mev_logger::init_console_logger(Some(LevelFilter::INFO)); // 初始化日志

        // 创建一个示例的 Pool 结构体
        let pool_info = Pool {
            protocol: Protocol::KriyaClmm,
            // 示例池ID，需要替换为测试网络上有效的Kriya CLMM池ID
            pool: ObjectID::from_str("0x367e02acb99632e18db69c3e93d89d21eb721e1d1fcebc0f6853667337450acc").unwrap(),
            tokens: vec![ // 池中的两种代币
                Token::new("0x2::sui::SUI", 9),
                Token::new(
                    "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN", // Wormhole USDC (示例)
                    9, // 假设精度为9 (实际应为6或8)
                ),
            ],
            extra: PoolExtra::None, // 对于此测试，extra信息不关键
        };

        // 创建一个DBSimulator实例用于测试 (带回退)
        let simulator_instance: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);
        let start_time = Instant::now(); // 开始计时
        // 调用被测试函数
        let children_ids_vec = kriya_clmm_pool_children_ids(&pool_info, simulator_instance).await.unwrap();
        // 打印结果和耗时
        println!("Kriya CLMM池子对象子ID获取耗时: {} ms", start_time.elapsed().as_millis());
        println!("为池 {} 派生的子对象ID列表 ({} 个): {:?}", pool_info.pool, children_ids_vec.len(), children_ids_vec);
        // 可以在这里添加断言来验证结果。
    }
}

[end of crates/dex-indexer/src/protocols/kriya_clmm.rs]
