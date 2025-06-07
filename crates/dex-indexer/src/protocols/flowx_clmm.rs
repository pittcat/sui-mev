// 该文件 `flowx_clmm.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 FlowX CLMM (集中流动性做市商) DEX 协议相关的事件和数据结构。
// FlowX Finance 同时提供传统的 AMM 池和 CLMM 池，此文件专注于其 CLMM 部分。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” FlowX CLMM 这个DEX发生的两种主要“事件”的：
// “有人创建了一个新的交易池 (`PoolCreated`)” 和 “有人在这个池子里完成了一笔代币交换 (`Swap`)”。
// 它还负责收集与FlowX CLMM协议本身以及特定池子相关联的一些重要的“全局对象”和“动态子对象”的ID。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别FlowX CLMM的特定事件**:
//     -   `FLOWX_CLMM_POOL_CREATED` 和 `FLOWX_CLMM_SWAP_EVENT` 常量是这两种事件在Sui链上的“专属门牌号”。
//     -   `flowx_clmm_event_filter()` 函数生成一个“过滤器”，告诉Sui节点只订阅FlowX CLMM创建新池子的事件。
//
// 2.  **`FlowxClmmPoolCreated` 结构体 (新池子信息卡)**:
//     -   当监听到FlowX CLMM新池子创建事件 (`PoolCreated`) 时，这个结构体记录事件信息
//         (池ID `pool_id`、两种代币类型 `coin_type_x`, `coin_type_y`、手续费率 `fee_rate`)。
//     -   `TryFrom<&SuiEvent>` 是“填卡员”，从原始Sui事件中提取数据。
//         注意：事件中的代币类型需要手动添加 "0x" 前缀。
//     -   `to_pool()` 方法将这张FlowX CLMM专用的“信息卡”转换为通用的 `Pool` 结构，
//         并将手续费率存储在 `PoolExtra::FlowxClmm` 中。
//
// 3.  **`FlowxClmmSwapEvent` 结构体 (交换记录卡)**:
//     -   记录FlowX CLMM交换事件 (`Swap`) 的详细信息（池ID、输入金额 `amount_x` 或 `amount_y`、
//         输出金额、交易方向 `x_for_y` 即 a2b）。
//     -   `TryFrom` 实现能从不同来源（`SuiEvent`, `ShioEvent`, 原始JSON `Value`）提取信息。
//         `TryFrom<&Value>` 的逻辑会根据 `x_for_y` (布尔值) 来确定哪个是输入金额/代币，哪个是输出。
//     -   `to_swap_event_v1/_v2()` 方法将其转换为通用的 `SwapEvent`。
//
// 4.  **收集相关对象ID (Collecting Related Object IDs)**:
//     -   `FLOWX_CLMM_POOL_LAYOUT`: 使用 `OnceLock` 缓存池对象的布局。
//     -   `flowx_clmm_related_object_ids()`: 列出了一些硬编码的FlowX CLMM相关对象ID
//         (如主包ID `FLOWX_CLMM`、版本对象 `VERSIONED`、池注册表 `POOL_REGISTRY`)。
//     -   `flowx_clmm_pool_children_ids()`: 这是一个复杂函数，用于获取一个FlowX CLMM池的动态子对象ID。
//         -   它会尝试获取与 `tick_bitmap` (tick位图) 和 `ticks` (tick表) 相关的动态字段对象ID。
//         -   还会根据池的代币类型和手续费率，从 `PoolRegistry` 派生出一个特定的动态字段ID (代表该池在注册表中的条目)。
//         -   这反映了CLMM池子需要管理大量与tick相关的子对象，以及通过注册表来组织池。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
// (与 cetus.rs, kriya_clmm.rs 等文件中的解释类似，主要涉及CLMM、Tick、动态字段、版本对象、池注册表等。)
//
// -   **PoolRegistry (池注册表)**:
//     FlowX CLMM 使用一个中心化的 `PoolRegistry` 对象来管理其下的所有池。
//     当需要获取某个特定交易对和费率的池对象时，可以通过查询这个注册表（通常是读取其动态字段）来实现。
//     `flowx_clmm_pool_children_ids` 中的一部分逻辑就是模拟这种查询，以找出与特定池相关的动态字段ID。
//     键通常是 `(CoinA_type, CoinB_type, fee_rate)` 的组合。
//
// -   **Tick Bitmap (Tick位图)**:
//     在CLMM中，为了高效地查找有流动性的tick区间，通常会使用“位图”（bitmap）数据结构。
//     位图中的每一位（bit）对应一个或一组tick。如果某一位被设置，则表示对应的tick或tick区间有流动性。
//     `tick_bitmap` 对象及其动态子字段就是这种位图的链上表示。

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
// 引入 Rayon 库的并行迭代器 (当前文件未使用，但可能在其他protocols模块中用到)。
use rayon::prelude::*;
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
    rpc_types::{EventFilter, SuiData, SuiEvent, SuiObjectDataOptions}, // 事件过滤器, Sui数据容器, Sui事件, 对象数据选项。
    types::{base_types::ObjectID, TypeTag},                             // ObjectID, TypeTag。
    SuiClient, SuiClientBuilder,                                        // Sui RPC客户端和构建器。
};
// 引入 Sui 核心类型中的动态字段ID派生函数, Object类型, Identifier。
use sui_types::{dynamic_field::derive_dynamic_field_id, object::Object, Identifier};
// 引入 utils 库的 object 模块中的辅助函数。
use utils::object::{
    extract_object_id_from_move_struct, extract_struct_from_move_struct,
    extract_u64_from_move_struct,
};

// 从当前crate的父模块 (protocols) 引入 get_coin_decimals, get_pool_coins_type 函数和 SUI_RPC_NODE 常量。
use super::{get_coin_decimals, get_pool_coins_type, SUI_RPC_NODE};
// 从当前crate的根模块引入 get_coin_in_out_v2 宏和相关类型定义。
use crate::{
    get_coin_in_out_v2,
    types::{Pool, PoolExtra, Protocol, SwapEvent, Token},
};

/// `FLOWX_CLMM_POOL_CREATED` 常量
///
/// 定义了FlowX CLMM协议在Sui链上发出的“新池创建”事件的全局唯一类型字符串。
/// 事件由 `pool_manager` 模块发出。
const FLOWX_CLMM_POOL_CREATED: &str =
    "0x25929e7f29e0a30eb4e692952ba1b5b65a3a4d65ab5f2a32e1ba3edcb587f26d::pool_manager::PoolCreated";

/// `FLOWX_CLMM_SWAP_EVENT` 常量
///
/// 定义了FlowX CLMM协议在Sui链上发出的“代币交换完成”事件的全局唯一类型字符串。
/// 事件由 `pool` 模块发出。
pub const FLOWX_CLMM_SWAP_EVENT: &str =
    "0x25929e7f29e0a30eb4e692952ba1b5b65a3a4d65ab5f2a32e1ba3edcb587f26d::pool::Swap";

/// `FLOWX_CLMM_POOL_LAYOUT` 静态变量
///
/// 使用 `OnceLock` 确保FlowX CLMM池对象的 `MoveStructLayout` 只被获取和初始化一次。
static FLOWX_CLMM_POOL_LAYOUT: OnceLock<MoveStructLayout> = OnceLock::new();

/// `flowx_clmm_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，专门用于订阅FlowX CLMM的“新池创建”事件。
pub fn flowx_clmm_event_filter() -> EventFilter {
    EventFilter::MoveEventType(FLOWX_CLMM_POOL_CREATED.parse().unwrap())
}

/// `FlowxClmmPoolCreated` 结构体
///
/// 用于存储从FlowX CLMM的 `PoolCreated` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)]
pub struct FlowxClmmPoolCreated {
    pub pool: ObjectID,   // 新创建的池的ObjectID
    pub token0: String, // 池中第一个代币 (CoinTypeX) 的类型字符串 (已添加 "0x" 前缀)
    pub token1: String, // 池中第二个代币 (CoinTypeY) 的类型字符串 (已添加 "0x" 前缀)
    pub fee_rate: u64,  // 池的手续费率
}

/// 为 `FlowxClmmPoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for FlowxClmmPoolCreated {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let parsed_json = &event.parsed_json;
        // 从JSON中提取 "pool_id" 字段 (注意FlowX事件中是 "pool_id" 而非 "pool")
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxClmmPoolCreated事件JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 从JSON中提取 "coin_type_x" 对象，再从中取 "name" 字段作为token0。
        let coin_type_x_obj = parsed_json["coin_type_x"]
            .as_object()
            .ok_or_else(|| eyre!("FlowxClmmPoolCreated事件JSON中缺少'coin_type_x'对象"))?;
        let token0_raw_str = coin_type_x_obj["name"]
            .as_str()
            .ok_or_else(|| eyre!("'coin_type_x'对象中缺少'name'字段"))?;
        let token0_formatted_str = format!("0x{}", token0_raw_str); // 添加 "0x" 前缀

        // 类似地提取 "coin_type_y" 作为token1。
        let coin_type_y_obj = parsed_json["coin_type_y"]
            .as_object()
            .ok_or_else(|| eyre!("FlowxClmmPoolCreated事件JSON中缺少'coin_type_y'对象"))?;
        let token1_raw_str = coin_type_y_obj["name"]
            .as_str()
            .ok_or_else(|| eyre!("'coin_type_y'对象中缺少'name'字段"))?;
        let token1_formatted_str = format!("0x{}", token1_raw_str);

        // 提取 "fee_rate" 字段并解析为u64。
        let fee_rate_val: u64 = parsed_json["fee_rate"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxClmmPoolCreated事件JSON中缺少'fee_rate'字段"))?
            .parse()?;

        Ok(Self {
            pool: pool_object_id,
            token0: token0_formatted_str,
            token1: token1_formatted_str,
            fee_rate: fee_rate_val,
        })
    }
}

impl FlowxClmmPoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `FlowxClmmPoolCreated` 事件数据转换为通用的 `Pool` 结构。
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
        // 创建 PoolExtra::FlowxClmm，存储FlowX CLMM特定的手续费率
        let extra_data = PoolExtra::FlowxClmm {
            fee_rate: self.fee_rate,
        };

        Ok(Pool {
            protocol: Protocol::FlowxClmm, // 指明协议为FlowxClmm
            pool: self.pool,              // 池的ObjectID
            tokens: tokens_vec,           // 池中代币列表
            extra: extra_data,            // 协议特定附加信息
        })
    }
}

/// `FlowxClmmSwapEvent` 结构体
///
/// 用于存储从FlowX CLMM的 `Swap` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)]
pub struct FlowxClmmSwapEvent {
    pub pool: ObjectID,       // 发生交换的池的ObjectID (从 "pool_id" 字段获取)
    pub amount_in: u64,       // 输入代币的数量
    pub amount_out: u64,      // 输出代币的数量
    pub a2b: bool,            // 交易方向：true表示从代币X到代币Y (x_for_y)，false表示从Y到X
                              // (Trading direction: true for token X to token Y (x_for_y), false for Y to X)
                              // 代币X和代币Y的定义取决于池创建时的顺序。
}

/// 为 `FlowxClmmSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for FlowxClmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        // 确保事件类型与 `FLOWX_CLMM_SWAP_EVENT` 匹配。
        ensure!(
            event.type_.to_string() == FLOWX_CLMM_SWAP_EVENT, // FlowX CLMM Swap事件类型没有泛型参数直接在类型字符串中
            "事件类型不匹配FlowX CLMM SwapEvent (Not a FlowxClmmSwapEvent)"
        );
        // 直接尝试从事件的 `parsed_json` 字段转换。
        (&event.parsed_json).try_into()
    }
}

/// 为 `FlowxClmmSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for FlowxClmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        ensure!(event.event_type == FLOWX_CLMM_SWAP_EVENT, "事件类型不匹配FlowX CLMM SwapEvent (Not a FlowxClmmSwapEvent)");
        event
            .parsed_json
            .as_ref()
            .ok_or_else(|| eyre!("ShioEvent中缺少parsed_json字段 (Missing parsed_json in ShioEvent)"))?
            .try_into()
    }
}

/// 为 `FlowxClmmSwapEvent` 实现 `TryFrom<&Value>` trait (从 `serde_json::Value` 转换)。
/// 这是实际的JSON解析逻辑。
/// FlowX CLMM的 `Swap` 事件JSON结构包含 `pool_id`, `amount_x`, `amount_y`, `x_for_y` 字段。
impl TryFrom<&Value> for FlowxClmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(parsed_json: &Value) -> Result<Self> {
        // 从JSON中提取 "pool_id" 字段并解析为 ObjectID。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxClmmSwapEvent JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 提取 "amount_x" (代表代币X的变动量) 并解析为 u64。
        let amount_x_val: u64 = parsed_json["amount_x"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxClmmSwapEvent JSON中缺少'amount_x'字段"))?
            .parse()?;

        // 提取 "amount_y" (代表代币Y的变动量) 并解析为 u64。
        let amount_y_val: u64 = parsed_json["amount_y"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxClmmSwapEvent JSON中缺少'amount_y'字段"))?
            .parse()?;

        // 提取 "x_for_y" 字段 (布尔值)，表示交易方向是否为 X -> Y。
        let x_to_y_direction = parsed_json["x_for_y"]
            .as_bool()
            .ok_or_else(|| eyre!("FlowxClmmSwapEvent JSON中缺少'x_for_y'布尔字段"))?;

        // 根据 `x_for_y` 的值确定实际的输入金额和输出金额。
        // 如果 x_for_y 为 true，则 amount_x 是输入，amount_y 是输出。
        // 否则，amount_y 是输入，amount_x 是输出。
        let (final_amount_in, final_amount_out) = if x_to_y_direction {
            (amount_x_val, amount_y_val)
        } else {
            (amount_y_val, amount_x_val)
        };

        Ok(Self {
            pool: pool_object_id,
            amount_in: final_amount_in,
            amount_out: final_amount_out,
            a2b: x_to_y_direction, // a2b 直接对应 x_for_y
        })
    }
}

impl FlowxClmmSwapEvent {
    /// `to_swap_event_v1` 异步方法 (旧版本，使用SuiClient查询代币类型)
    ///
    /// 将 `FlowxClmmSwapEvent` 转换为通用的 `SwapEvent`。
    /// 这个版本需要一个 `SuiClient` 来从链上查询池对象的具体代币类型。
    #[allow(dead_code)] // 标记为允许死代码，因为优先使用v2版本
    pub async fn to_swap_event_v1(&self, sui: &SuiClient) -> Result<SwapEvent> {
        // 调用 `get_pool_coins_type` (定义在父模块 protocols::mod.rs) 来获取池的两种代币类型。
        let (coin_a_type, coin_b_type) = get_pool_coins_type(sui, self.pool).await?;
        // 根据 `self.a2b` 字段确定实际的输入和输出代币类型。
        let (final_coin_in, final_coin_out) = if self.a2b {
            (coin_a_type, coin_b_type) // a2b (x_for_y) 为 true，则输入是 coin_a (X), 输出是 coin_b (Y)
        } else {
            (coin_b_type, coin_a_type) // a2b (x_for_y) 为 false，则输入是 coin_b (Y), 输出是 coin_a (X)
        };

        Ok(SwapEvent {
            protocol: Protocol::FlowxClmm,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }

    /// `to_swap_event_v2` 异步方法 (新版本，使用Simulator查询代币类型)
    ///
    /// 将 `FlowxClmmSwapEvent` 转换为通用的 `SwapEvent`。
    /// 使用 `Simulator` 实例获取池的代币类型，更通用。
    pub async fn to_swap_event_v2(&self, provider: Arc<dyn Simulator>) -> Result<SwapEvent> {
        // `get_coin_in_out_v2!` 宏用于获取输入输出代币类型。
        let (final_coin_in, final_coin_out) = get_coin_in_out_v2!(self.pool, provider, self.a2b);

        Ok(SwapEvent {
            protocol: Protocol::FlowxClmm,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

/// `flowx_clmm_related_object_ids` 函数
///
/// 返回与FlowX CLMM协议本身相关的核心对象ID列表 (硬编码)。
pub fn flowx_clmm_related_object_ids() -> Vec<String> {
    vec![
        // FlowX CLMM 主包ID
        "0x25929e7f29e0a30eb4e692952ba1b5b65a3a4d65ab5f2a32e1ba3edcb587f26d", // FlowxClmm Package ID
        // FlowX CLMM 版本对象ID
        "0x67624a1533b5aff5d0dfcf5e598684350efd38134d2d245f475524c03a64e656", // Versioned Object ID
        // FlowX CLMM 池注册表对象ID
        "0x27565d24a4cd51127ac90e4074a841bbe356cca7bf5759ddc14a975be1632abc", // PoolRegistry Object ID
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

/// `flowx_clmm_pool_children_ids` 异步函数
///
/// 获取特定FlowX CLMM池的动态子对象ID，主要是与tick位图和tick表相关的对象，
/// 以及该池在池注册表中的动态字段ID。
///
/// 参数:
/// - `pool`: 一个对 `Pool` 结构体的引用，代表要检查的FlowX CLMM池。
/// - `simulator`: 一个共享的模拟器实例，用于从链上（或缓存）获取对象数据和布局。
///
/// 返回:
/// - `Result<Vec<String>>`: 包含所有相关子对象和派生对象ID字符串的列表。
pub async fn flowx_clmm_pool_children_ids(pool: &Pool, simulator: Arc<dyn Simulator>) -> Result<Vec<String>> {
    let mut result_ids_str_vec = vec![]; // 用于存储结果ID字符串的向量

    // 获取池对象的详细数据
    let pool_sui_object = simulator
        .get_object(&pool.pool)
        .await
        .ok_or_else(|| eyre!("FlowX CLMM池对象未找到: {}", pool.pool))?;

    // 解析池对象的Move结构体
    let parsed_pool_move_struct = {
        let object_layout = pool_layout(pool.pool, simulator.clone()); // 获取并缓存池对象布局
        let move_object_data = pool_sui_object.data.try_as_move().ok_or_eyre("对象不是有效的Move对象")?;
        MoveStruct::simple_deserialize(move_object_data.contents(), &object_layout).map_err(|e| eyre!(e))?
    };

    // 创建SuiClient以调用 `get_dynamic_fields` (这个API不在Simulator trait中)
    let sui_client_instance = SuiClientBuilder::default().build(SUI_RPC_NODE).await.unwrap();

    // --- (可选) 获取与池在 PoolRegistry 中对应的动态字段ID ---
    // FlowX CLMM池通过 (CoinA, CoinB, fee_rate) 的组合作为键存储在PoolRegistry的动态字段中。
    {
        let pool_registry_object_id =
            ObjectID::from_str("0x27565d24a4cd51127ac90e4074a841bbe356cca7bf5759ddc14a975be1632abc")?; // PoolRegistry的固定ID

        // 格式化池的两种代币类型和手续费率，用于构建动态字段的键。
        let coin_a_type_formatted = format_coin_type_for_derive(&pool.token0_type());
        let coin_b_type_formatted = format_coin_type_for_derive(&pool.token1_type());
        // 从池结构中提取手续费率 (swap_fee_rate 字段)
        let fee_rate_val = extract_u64_from_move_struct(&parsed_pool_move_struct, "swap_fee_rate")?;

        // 构建键值元组 (String, String, u64)
        let key_tuple_value = (coin_a_type_formatted, coin_b_type_formatted, fee_rate_val);
        let key_bcs_bytes = bcs::to_bytes(&key_tuple_value)?; // 将键BCS序列化
        // 动态字段键的类型标签 (Wrapper<PoolDfKey>)
        let key_type_tag = TypeTag::from_str("0x02::dynamic_object_field::Wrapper<0x25929e7f29e0a30eb4e692952ba1b5b65a3a4d65ab5f2a32e1ba3edcb587f26d::pool_manager::PoolDfKey>").unwrap();
        // 派生动态字段ID
        let derived_child_id = derive_dynamic_field_id(pool_registry_object_id, &key_type_tag, &key_bcs_bytes)?;
        result_ids_str_vec.push(derived_child_id.to_string());
    }

    // --- 获取tick_bitmap (tick位图) 的所有动态子字段ID ---
    {
        // 从池结构中提取 `tick_bitmap` 结构体
        let tick_bitmap_struct = extract_struct_from_move_struct(&parsed_pool_move_struct, "tick_bitmap")?;
        // 从 `tick_bitmap` 结构中提取其 `id` 字段 (即 `tick_bitmap` 表对象的ID)
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

    // (可选) FlowX CLMM的 `next_tick_initialized_map` 字段也可能是一个包含动态字段的表。
    // 如果需要，可以仿照上面 `tick_bitmap` 和 `ticks` 的逻辑来提取。
    // 这里暂时注释掉了对 `next_tick_initialized_map` 的处理，可能是因为其子对象不常用或获取逻辑复杂。
    // {
    //     let parent_id =
    //     ObjectID::from_str("0xe746a19bf5e4ef0e5aa7993d9a36e49bd8f0928390723d43f2ebbbf87c416ef2")?;
    //     let key_tag =
    //     TypeTag::from_str("0x25929e7f29e0a30eb4e692952ba1b5b65a3a4d65ab5f2a32e1ba3edcb587f26d::i32::I32").unwrap();
    //     let bit_map_len = 256u32;
    //     for i in 0..bit_map_len {
    //         let key_bytes = bcs::to_bytes(&i)?;
    //         let child_id = derive_dynamic_field_id(parent_id, &key_tag, &key_bytes)?;
    //         result_ids_str_vec.push(child_id.to_string());
    //     }
    // }

    Ok(result_ids_str_vec)
}

/// `pool_layout` 函数
///
/// 获取并缓存指定FlowX CLMM池对象的 `MoveStructLayout`。
/// 使用 `OnceLock` (FLOWX_CLMM_POOL_LAYOUT) 确保布局信息只从模拟器获取一次。
fn pool_layout(pool_id: ObjectID, simulator: Arc<dyn Simulator>) -> MoveStructLayout {
    FLOWX_CLMM_POOL_LAYOUT
        .get_or_init(|| {
            simulator
                .get_object_layout(&pool_id)
                .expect("获取FlowX CLMM池对象布局失败 (Failed to get FlowxClmm pool layout)")
        })
        .clone()
}

/// `format_coin_type_for_derive` (内联辅助函数)
///
/// 将一个完整的代币类型字符串转换为其用于动态字段键派生的规范显示格式 (不含 "0x" 前缀)。
#[inline]
fn format_coin_type_for_derive(coin_type: &str) -> String {
    let coin_type_tag = TypeTag::from_str(coin_type).unwrap();
    format!("{}", coin_type_tag.to_canonical_display(false))
}

// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use super::*; // 导入外部模块 (flowx_clmm.rs) 的所有公共成员
    use mev_logger::LevelFilter; // 日志级别过滤器
    use simulator::{DBSimulator, HttpSimulator}; // 各种模拟器
    use tokio::time::Instant; // 精确时间点，用于计时

    /// `test_swap_event_http` 测试函数 (HTTP模拟器)
    ///
    /// 测试 `FlowxClmmSwapEvent::to_swap_event_v2` 方法。
    #[tokio::test]
    async fn test_swap_event_http() {
        // 创建HttpSimulator (RPC URL为空，可能依赖环境变量或默认配置)
        let provider_http = HttpSimulator::new("", &None).await;

        // 创建示例FlowxClmmSwapEvent
        let swap_event_data = FlowxClmmSwapEvent {
            // 示例池ID，需要替换为实际有效的FlowX CLMM池ID
            pool: ObjectID::from_str("0x2e88a6a61327ba517dcf1c57346ed1fdd25d98e78007e389f208658224baa72f").unwrap(),
            amount_in: 0x1337,
            amount_out: 0x1338,
            a2b: true, // 假设方向是 X -> Y
        };

        // 调用被测试方法
        let converted_swap_event = swap_event_data.to_swap_event_v2(Arc::new(provider_http)).await.unwrap();
        // 预期的代币X和代币Y的类型 (需要与上面池ID实际对应的代币类型一致)
        let expected_coin_x_type = "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC";
        let expected_coin_y_type = "0xdeeb7a4662eec9f2f3def03fb937a663dddaa2e215b8078a284d026b7946c270::deep::DEEP";

        // 断言转换后的输入输出代币类型
        assert_eq!(converted_swap_event.coins_in[0], expected_coin_x_type);
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_y_type);
    }

    /// `test_swap_event_db` 测试函数 (数据库模拟器)
    ///
    /// 与上一个测试类似，但使用 `DBSimulator`。
    #[tokio::test]
    async fn test_swap_event_db() {
        let provider_db = DBSimulator::new_default_slow().await; // 创建DBSimulator

        let swap_event_data = FlowxClmmSwapEvent {
            pool: ObjectID::from_str("0x2e88a6a61327ba517dcf1c57346ed1fdd25d98e78007e389f208658224baa72f").unwrap(),
            amount_in: 0x1337,
            amount_out: 0x1338,
            a2b: true,
        };

        let converted_swap_event = swap_event_data.to_swap_event_v2(Arc::new(provider_db)).await.unwrap();
        let expected_coin_x_type = "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC";
        let expected_coin_y_type = "0xdeeb7a4662eec9f2f3def03fb937a663dddaa2e215b8078a284d026b7946c270::deep::DEEP";

        assert_eq!(converted_swap_event.coins_in[0], expected_coin_x_type);
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_y_type);
    }

    /// `test_flowx_clmm_pool_children_ids` 测试函数
    ///
    /// 测试 `flowx_clmm_pool_children_ids` 函数是否能为给定的FlowX CLMM池正确派生相关的子对象ID。
    #[tokio::test]
    async fn test_flowx_clmm_pool_children_ids() {
        mev_logger::init_console_logger(Some(LevelFilter::INFO)); // 初始化日志

        // 创建一个示例的 Pool 结构体
        let pool_info = Pool {
            protocol: Protocol::FlowxClmm,
            // 示例池ID，需要替换为测试网络上有效的FlowX CLMM池ID
            pool: ObjectID::from_str("0x1903c1715a382457f04fb5c3c3ee718871f976a4b4a589eb899096b96f8d5eba").unwrap(),
            tokens: vec![ // 池中的两种代币
                Token::new("0x2::sui::SUI", 9),
                Token::new(
                    "0x3fb8bdeced0dc4bf830267652ef33fe8fb60b107b3d3b6e5e088dcc0067efa06::prh::PRH", // 示例代币PRH
                    9,
                ),
            ],
            extra: PoolExtra::None, // 对于此测试，extra信息不关键
        };

        // 创建一个DBSimulator实例用于测试 (带回退)
        let simulator_instance: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);
        let start_time = Instant::now(); // 开始计时
        // 调用被测试函数
        let children_ids_vec = flowx_clmm_pool_children_ids(&pool_info, simulator_instance).await.unwrap();
        // 打印结果和耗时
        println!("派生的子对象ID列表 ({} 个): {:?}", children_ids_vec.len(), children_ids_vec);
        println!("获取FlowX CLMM池子对象耗时: {} ms", start_time.elapsed().as_millis());
        // 可以在这里添加断言来验证结果。
    }
}

[end of crates/dex-indexer/src/protocols/flowx_clmm.rs]
