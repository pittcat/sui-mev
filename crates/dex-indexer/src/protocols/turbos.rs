// 该文件 `turbos.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 Turbos Finance DEX 协议 (一个Sui上的CLMM DEX) 相关的事件和数据结构。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” Turbos Finance 这个CLMM DEX发生的两种主要“事件”的：
// “有人创建了一个新的交易池 (`PoolCreatedEvent`)” 和 “有人在这个池子里完成了一笔代币交换 (`SwapEvent`)”。
// 它还负责收集与Turbos协议本身以及特定池子相关联的一些重要的“全局对象”和“动态子对象”的ID。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别Turbos的特定事件**:
//     -   `TURBOS_POOL_CREATED` 和 `TURBOS_SWAP_EVENT` 常量是这两种事件在Sui链上的“专属门牌号”。
//         `PoolCreatedEvent` 来自 `pool_factory` 模块，`SwapEvent` 来自 `pool` 模块。
//     -   `turbos_event_filter()` 函数生成一个“过滤器”，告诉Sui节点只订阅Turbos创建新池子的事件。
//
// 2.  **`TurbosPoolCreated` 结构体 (新池子信息卡)**:
//     -   当监听到Turbos新池子创建事件 (`PoolCreatedEvent`) 时，这个结构体记录事件信息
//         (池ID `pool`、手续费率 `fee`)。
//     -   `TryFrom<&SuiEvent>` 是“填卡员”，从原始Sui事件中提取数据。
//     -   `to_pool()` 方法将这张Turbos专用的“信息卡”转换为通用的 `Pool` 结构。
//         这个方法需要异步调用 `get_pool_coins_type` 来从链上查询该池的两种代币类型，
//         然后再查询这两种代币的精度，并将手续费率存储在 `PoolExtra::Turbos` 中。
//
// 3.  **`TurbosSwapEvent` 结构体 (交换记录卡)**:
//     -   记录Turbos交换事件 (`SwapEvent`) 的详细信息（池ID、输入金额、输出金额、交易方向 `a2b`）。
//         Turbos的 `SwapEvent` JSON负载中包含 `amount_a`, `amount_b` 和 `a_to_b` (布尔值)，
//         需要根据 `a_to_b` 来确定哪个是输入金额/代币，哪个是输出。
//     -   `TryFrom` 实现能从不同来源（`SuiEvent`, `ShioEvent`, 原始JSON `Value`）提取信息。
//     -   `to_swap_event_v1/_v2()` 方法将其转换为通用的 `SwapEvent`。
//
// 4.  **收集相关对象ID (Collecting Related Object IDs)**:
//     -   `TURBOS_POOL_LAYOUT`: 使用 `OnceLock` 缓存池对象的布局。
//     -   `turbos_related_object_ids()`: 列出了一些硬编码的Turbos相关对象ID
//         (如不同版本的包ID、版本对象 `VERSIONED` ID)。
//     -   `turbos_pool_children_ids()`: 这是一个用于获取Turbos CLMM池的动态子对象ID的函数，
//         特别是与 `tick_map` (tick映射表) 相关的对象。它会解析池对象，找到 `tick_map` 表的ID，
//         然后通过 `get_dynamic_fields` API 分页获取该表中的所有动态字段对象ID (代表已初始化的ticks)。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
// (与 cetus.rs, kriya_clmm.rs, flowx_clmm.rs 等文件中的解释类似，主要涉及CLMM、Tick、动态字段、版本对象等。)
//
// -   **Tick Map (Tick映射表)**:
//     Turbos CLMM 使用一个名为 `tick_map` 的结构 (通常是一个链上的 `Table` 对象) 来存储和管理池中所有已初始化的tick的信息。
//     `turbos_pool_children_ids` 函数通过解析池对象找到这个 `tick_map` 的ID，并进而获取其所有动态子字段，
//     这些子字段即代表了具体的tick数据对象。

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
    rpc_types::{EventFilter, SuiEvent},       // EventFilter用于订阅事件，SuiEvent代表链上事件。
    types::{base_types::ObjectID, TypeTag}, // ObjectID, TypeTag。 (TypeTag当前在此文件中未直接使用，但常与协议解析相关)
    SuiClient, SuiClientBuilder,              // Sui RPC客户端和构建器。
};
// 引入 Sui 核心类型中的动态字段ID派生函数 (当前文件未使用，但常用于获取子对象)。
// use sui_types::{dynamic_field::derive_dynamic_field_id, TypeTag};

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

/// `TURBOS_POOL_CREATED` 常量
///
/// 定义了Turbos Finance协议在Sui链上发出的“新池创建”事件的全局唯一类型字符串。
/// 事件由 `pool_factory` 模块发出。
const TURBOS_POOL_CREATED: &str =
    "0x91bfbc386a41afcfd9b2533058d7e915a1d3829089cc268ff4333d54d6339ca1::pool_factory::PoolCreatedEvent";

/// `TURBOS_SWAP_EVENT` 常量
///
/// 定义了Turbos Finance协议在Sui链上发出的“代币交换完成”事件的全局唯一类型字符串。
/// 事件由 `pool` 模块发出。
pub const TURBOS_SWAP_EVENT: &str =
    "0x91bfbc386a41afcfd9b2533058d7e915a1d3829089cc268ff4333d54d6339ca1::pool::SwapEvent";

/// `TURBOS_POOL_LAYOUT` 静态变量
///
/// 使用 `OnceLock` 确保Turbos池对象的 `MoveStructLayout` 只被获取和初始化一次。
static TURBOS_POOL_LAYOUT: OnceLock<MoveStructLayout> = OnceLock::new();

/// `turbos_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，专门用于订阅Turbos Finance的“新池创建”事件。
pub fn turbos_event_filter() -> EventFilter {
    EventFilter::MoveEventType(TURBOS_POOL_CREATED.parse().unwrap())
}

/// `TurbosPoolCreated` 结构体
///
/// 用于存储从Turbos Finance的 `PoolCreatedEvent` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)]
pub struct TurbosPoolCreated {
    pub pool: ObjectID, // 新创建的池的ObjectID
    pub fee: u32,       // 池的手续费率 (以10000为基数，例如3000代表0.3%)
}

/// 为 `TurbosPoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for TurbosPoolCreated {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let parsed_json = &event.parsed_json;
        // 从JSON中提取 "pool" 字段作为池ID。
        let pool_id_str = parsed_json["pool"]
            .as_str()
            .ok_or_else(|| eyre!("TurbosPoolCreated事件JSON中缺少'pool'字段 (作为pool_id)"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 从JSON中提取 "fee" 字段并解析为 u32。
        let fee_val = parsed_json["fee"].as_u64().ok_or_else(|| eyre!("TurbosPoolCreated事件JSON中缺少'fee'字段或非u64类型"))? as u32;

        Ok(Self {
            pool: pool_object_id,
            fee: fee_val,
        })
    }
}

impl TurbosPoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `TurbosPoolCreated` 事件数据转换为通用的 `Pool` 结构。
    /// 此方法需要异步查询池的两种代币类型及其精度。手续费率已在事件中提供。
    pub async fn to_pool(&self, sui: &SuiClient) -> Result<Pool> {
        // 调用父模块的 `get_pool_coins_type` 函数从链上获取池的两种代币类型。
        let (token0_type_str, token1_type_str) = get_pool_coins_type(sui, self.pool).await?;

        // 异步获取token0和token1的精度信息。
        let token0_decimals = get_coin_decimals(sui, &token0_type_str).await?;
        let token1_decimals = get_coin_decimals(sui, &token1_type_str).await?;

        // 创建 Token 结构列表。
        let tokens_vec = vec![
            Token::new(&token0_type_str, token0_decimals),
            Token::new(&token1_type_str, token1_decimals),
        ];
        // 创建 PoolExtra::Turbos，存储Turbos特定的手续费率。
        let extra_data = PoolExtra::Turbos { fee: self.fee };

        Ok(Pool {
            protocol: Protocol::Turbos, // 指明协议为Turbos
            pool: self.pool,           // 池的ObjectID
            tokens: tokens_vec,        // 池中代币列表
            extra: extra_data,         // 协议特定附加信息
        })
    }
}

/// `TurbosSwapEvent` 结构体
///
/// 用于存储从Turbos Finance的 `SwapEvent` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)]
pub struct TurbosSwapEvent {
    pub pool: ObjectID,       // 发生交换的池的ObjectID
    pub amount_in: u64,       // 输入代币的数量
    pub amount_out: u64,      // 输出代币的数量
    pub a2b: bool,            // 交易方向：true表示从代币A到代币B，false表示从B到A
}

/// 为 `TurbosSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for TurbosSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        // 确保事件类型与 `TURBOS_SWAP_EVENT` 匹配。
        // Turbos SwapEvent 类型没有泛型参数。
        ensure!(event.type_.to_string() == TURBOS_SWAP_EVENT, "事件类型不匹配Turbos SwapEvent (Not a TurbosSwapEvent)");
        // 直接尝试从事件的 `parsed_json` 字段转换。
        (&event.parsed_json).try_into()
    }
}

/// 为 `TurbosSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for TurbosSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        ensure!(event.event_type == TURBOS_SWAP_EVENT, "事件类型不匹配Turbos SwapEvent (Not a TurbosSwapEvent)");
        event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json in ShioEvent)")?.try_into()
    }
}

/// 为 `TurbosSwapEvent` 实现 `TryFrom<&Value>` trait (从 `serde_json::Value` 转换)。
/// 这是实际的JSON解析逻辑。
/// Turbos的 `SwapEvent` JSON结构包含 `pool`, `amount_a`, `amount_b`, `a_to_b` 字段。
impl TryFrom<&Value> for TurbosSwapEvent {
    type Error = eyre::Error;

    fn try_from(parsed_json: &Value) -> Result<Self> {
        // 从JSON中提取 "pool" 字段并解析为 ObjectID。
        let pool_id_str = parsed_json["pool"]
            .as_str()
            .ok_or_else(|| eyre!("TurbosSwapEvent JSON中缺少'pool'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 提取 "amount_a" (代表代币A的变动量) 并解析为 u64。
        let amount_a_val: u64 = parsed_json["amount_a"]
            .as_str()
            .ok_or_else(|| eyre!("TurbosSwapEvent JSON中缺少'amount_a'字段"))?
            .parse()?;

        // 提取 "amount_b" (代表代币B的变动量) 并解析为 u64。
        let amount_b_val: u64 = parsed_json["amount_b"]
            .as_str()
            .ok_or_else(|| eyre!("TurbosSwapEvent JSON中缺少'amount_b'字段"))?
            .parse()?;

        // 提取 "a_to_b" 字段 (布尔值)，表示交易方向是否为 A -> B。
        let a_to_b_direction = parsed_json["a_to_b"].as_bool().ok_or_else(|| eyre!("TurbosSwapEvent JSON中缺少'a_to_b'布尔字段"))?;

        // 根据 `a_to_b` 的值确定实际的输入金额和输出金额。
        // 如果 a_to_b 为 true，则 amount_a 是输入，amount_b 是输出。
        // 否则，amount_b 是输入，amount_a 是输出。
        let (final_amount_in, final_amount_out) = if a_to_b_direction {
            (amount_a_val, amount_b_val)
        } else {
            (amount_b_val, amount_a_val)
        };

        Ok(Self {
            pool: pool_object_id,
            amount_in: final_amount_in,
            amount_out: final_amount_out,
            a2b: a_to_b_direction,
        })
    }
}

impl TurbosSwapEvent {
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
            protocol: Protocol::Turbos,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }

    /// `to_swap_event_v2` 异步方法 (新版本，使用Simulator查询代币类型)
    ///
    /// 将 `TurbosSwapEvent` 转换为通用的 `SwapEvent`。
    /// 使用 `Simulator` 实例和 `get_coin_in_out_v2!` 宏来获取和判断代币类型。
    pub async fn to_swap_event_v2(&self, provider: Arc<dyn Simulator>) -> Result<SwapEvent> {
        let (final_coin_in, final_coin_out) = get_coin_in_out_v2!(self.pool, provider, self.a2b);

        Ok(SwapEvent {
            protocol: Protocol::Turbos,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

/// `turbos_related_object_ids` 函数
///
/// 返回与Turbos Finance协议本身相关的核心对象ID列表 (硬编码)。
pub fn turbos_related_object_ids() -> Vec<String> {
    vec![
        // Turbos Finance 主包ID或核心模块ID的不同版本/实例
        "0x91bfbc386a41afcfd9b2533058d7e915a1d3829089cc268ff4333d54d6339ca1", // Turbos 1 (与事件包ID一致)
        "0x1a3c42ded7b75cdf4ebc7c7b7da9d1e1db49f16fcdca934fac003f35f39ecad9", // Turbos 4
        "0xdc67d6de3f00051c505da10d8f6fbab3b3ec21ec65f0dc22a2f36c13fc102110", // Turbos 9
        // Turbos Finance 版本对象ID
        "0xf1cf0e81048df168ebeb1b8030fad24b3e0b53ae827c25053fff0779c1445b6f", // Versioned Object ID
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

/// `turbos_pool_children_ids` 异步函数
///
/// 获取特定Turbos CLMM池的动态子对象ID，主要是与 `tick_map` (tick映射表) 相关的对象。
///
/// 参数:
/// - `pool`: 一个对 `Pool` 结构体的引用，代表要检查的Turbos池。
/// - `simulator`: 一个共享的模拟器实例，用于从链上（或缓存）获取对象数据和布局。
///
/// 返回:
/// - `Result<Vec<String>>`: 包含所有相关子对象ID字符串的列表。
pub async fn turbos_pool_children_ids(pool: &Pool, simulator: Arc<dyn Simulator>) -> Result<Vec<String>> {
    // 获取池对象的详细数据并解析其Move结构体
    let parsed_pool_move_struct = {
        let pool_sui_object = simulator
            .get_object(&pool.pool)
            .await
            .ok_or_else(|| eyre!("Turbos池对象未找到: {}", pool.pool))?;

        let object_layout = pool_layout(pool.pool, simulator.clone()); // 获取并缓存池对象布局

        let move_object_data = pool_sui_object.data.try_as_move().ok_or_eyre("对象不是有效的Move对象")?;
        MoveStruct::simple_deserialize(move_object_data.contents(), &object_layout).map_err(|e| eyre!(e))?
    };

    // 从池结构中提取 `tick_map` 结构体 (代表tick表)
    let tick_map_struct = extract_struct_from_move_struct(&parsed_pool_move_struct, "tick_map")?;

    // 获取 `tick_map` 表本身的ObjectID
    let tick_map_table_id = {
        let id_field_wrapper = extract_struct_from_move_struct(&tick_map_struct, "id")?;       // `id` 字段是 `UID` 类型
        let id_field_actual_id = extract_struct_from_move_struct(&id_field_wrapper, "id")?;   // `UID` 内部的 `ID` 类型
        extract_object_id_from_move_struct(&id_field_actual_id, "bytes")? // `ID` 内部的 `bytes` (实际ObjectID)
    };

    // 创建SuiClient以调用 `get_dynamic_fields`
    let sui_client_instance = SuiClientBuilder::default().build(SUI_RPC_NODE).await.unwrap();

    // 分页获取 `tick_map` 表的所有动态字段 (每个字段代表一个已初始化的tick对象)
    let mut next_page_cursor = None;
    let mut initialized_tick_object_ids_info = Vec::new();
    loop {
        let dynamic_fields_page = sui_client_instance
            .read_api()
            .get_dynamic_fields(tick_map_table_id, next_page_cursor, None)
            .await?;
        next_page_cursor = dynamic_fields_page.next_cursor;
        initialized_tick_object_ids_info.extend(dynamic_fields_page.data);
        if !dynamic_fields_page.has_next_page {
            break;
        }
    }

    // 将这些已初始化的tick对象的ID转换为字符串并收集到结果向量中
    let tick_ids_str_vec: Vec<String> = initialized_tick_object_ids_info.iter().map(|field_info| {
        field_info.object_id.to_string()
    }).collect();

    Ok(tick_ids_str_vec)
}

/// `pool_layout` 函数
///
/// 获取并缓存指定Turbos池对象的 `MoveStructLayout`。
/// 使用 `OnceLock` (TURBOS_POOL_LAYOUT) 确保布局信息只从模拟器获取一次。
fn pool_layout(pool_id: ObjectID, simulator: Arc<dyn Simulator>) -> MoveStructLayout {
    TURBOS_POOL_LAYOUT
        .get_or_init(|| {
            simulator
                .get_object_layout(&pool_id)
                .expect("获取Turbos池对象布局失败 (Failed to get Turbos pool layout)")
        })
        .clone()
}

// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use std::str::FromStr; // FromStr trait

    use super::*; // 导入外部模块 (turbos.rs) 的所有公共成员
    use mev_logger::LevelFilter; // 日志级别过滤器
    use simulator::DBSimulator;  // 数据库模拟器
    use simulator::HttpSimulator; // HTTP模拟器

    /// `test_swap_event_http` 测试函数 (HTTP模拟器)
    ///
    /// 测试 `TurbosSwapEvent::to_swap_event_v2` 方法。
    #[tokio::test]
    async fn test_swap_event_http() {
        // 创建HttpSimulator (RPC URL为空，可能依赖环境变量或默认配置)
        let provider_http = HttpSimulator::new("", &None).await;

        // 创建一个示例的 TurbosSwapEvent
        let turbos_swap_event_data = TurbosSwapEvent {
            // 这是一个示例池ID，需要替换为实际有效的Turbos池ID
            pool: ObjectID::from_str("0x77f786e7bbd5f93f7dc09edbcffd9ea073945564767b65cf605f388328449d50").unwrap(),
            amount_in: 0x1337,  // 示例输入金额
            amount_out: 0x1338, // 示例输出金额
            a2b: true,          // 假设方向是 A -> B
        };

        // 调用被测试方法
        let converted_swap_event = turbos_swap_event_data.to_swap_event_v2(Arc::new(provider_http)).await.unwrap();
        // 预期的代币A和代币B的类型 (需要与上面池ID实际对应的代币类型一致)
        let expected_coin_a_type = "0x2::sui::SUI";
        // Wormhole USDC (示例，实际池可能不同)
        let expected_coin_b_type = "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC";

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

        let turbos_swap_event_data = TurbosSwapEvent {
            pool: ObjectID::from_str("0x77f786e7bbd5f93f7dc09edbcffd9ea073945564767b65cf605f388328449d50").unwrap(),
            amount_in: 0x1337,
            amount_out: 0x1338,
            a2b: true,
        };

        let converted_swap_event = turbos_swap_event_data.to_swap_event_v2(Arc::new(provider_db)).await.unwrap();
        let expected_coin_a_type = "0x2::sui::SUI";
        let expected_coin_b_type = "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC";

        assert_eq!(converted_swap_event.coins_in[0], expected_coin_a_type, "输入代币类型不匹配");
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_b_type, "输出代币类型不匹配");
    }

    /// `test_turbos_pool_children_ids` 测试函数
    ///
    /// 测试 `turbos_pool_children_ids` 函数是否能为给定的Turbos池正确派生相关的子对象ID。
    #[tokio::test]
    async fn test_turbos_pool_children_ids() {
        mev_logger::init_console_logger(Some(LevelFilter::INFO)); // 初始化日志

        // 创建一个示例的 Pool 结构体
        let pool_info = Pool {
            protocol: Protocol::Turbos,
            // 示例池ID，需要替换为测试网络上有效的Turbos池ID
            pool: ObjectID::from_str("0x0df4f02d0e210169cb6d5aabd03c3058328c06f2c4dbb0804faa041159c78443").unwrap(),
            tokens: vec![ // 池中的两种代币
                Token::new("0x2::sui::SUI", 9),
                Token::new(
                    "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC", // Wormhole USDC (示例)
                    9, // 假设精度为9 (USDC通常是6或8)
                ),
            ],
            extra: PoolExtra::None, // 对于此测试，extra信息不关键
        };

        // 创建一个DBSimulator实例用于测试 (带回退)
        let simulator_instance: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);

        // 调用被测试函数
        let children_ids_vec = turbos_pool_children_ids(&pool_info, simulator_instance).await.unwrap();
        // 打印结果
        println!("为池 {} 派生的子对象ID列表 ({} 个): {:?}", pool_info.pool, children_ids_vec.len(), children_ids_vec);
        // 可以在这里添加断言来验证结果。
    }

    // 注释掉的测试 (可能用于其他或旧的逻辑)
    // #[tokio::test]
    // async fn test_turbos_pool_children_ids2() {
    //     mev_logger::init_console_logger(Some(LevelFilter::INFO));

    //     let pool = Pool {
    //         protocol: Protocol::Turbos,
    //         pool: ObjectID::from_str("0x0df4f02d0e210169cb6d5aabd03c3058328c06f2c4dbb0804faa041159c78443").unwrap(),
    //         tokens: vec![
    //             Token::new("0x2::sui::SUI", 9),
    //             Token::new(
    //                 "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC",
    //                 9,
    //             ),
    //         ],
    //         extra: PoolExtra::None,
    //     };

    //     let simulator: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);

    //     let children_ids = turbos_pool_children_ids2(&pool, simulator).await.unwrap();
    //     // println!("{:?}", children_ids);
    // }
}

[end of crates/dex-indexer/src/protocols/turbos.rs]
