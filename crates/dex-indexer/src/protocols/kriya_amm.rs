// 该文件 `kriya_amm.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 KriyaDEX 协议的传统 AMM (自动做市商) 池相关的事件和数据结构。
// KriyaDEX 是 Sui 生态中的一个DEX，它同时提供传统 AMM 池和 CLMM 池，此文件专注于其 AMM (spot_dex) 部分。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” KriyaDEX 的传统 AMM 池发生的两种主要“事件”的：
// “有人创建了一个新的交易池 (`PoolCreatedEvent`)” 和 “有人在这个池子里完成了一笔代币交换 (`SwapEvent`)”。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别Kriya AMM的特定事件**:
//     -   `KRIYA_AMM_POOL_CREATED` 和 `KRIYA_AMM_SWAP_EVENT` 常量是这两种事件在Sui链上的“专属门牌号”。
//         它们都属于 `spot_dex` 模块。
//     -   `kriya_amm_event_filter()` 函数生成一个“过滤器”，告诉Sui节点只订阅Kriya AMM创建新池子的事件。
//
// 2.  **`KriyaAmmPoolCreated` 结构体 (新池子信息卡)**:
//     -   当监听到Kriya AMM新池子创建事件 (`PoolCreatedEvent`) 时，这个结构体记录事件信息
//         (池ID `pool_id`、LP手续费百分比 `lp_fee_percent`、协议手续费百分比 `protocol_fee_percent`)。
//     -   `TryFrom<&SuiEvent>` 是“填卡员”，从原始Sui事件中提取数据。
//     -   `to_pool()` 方法将这张Kriya AMM专用的“信息卡”转换为通用的 `Pool` 结构。
//         这个方法需要异步调用 `get_pool_coins_type` (从父模块导入) 来从链上查询该池的两种代币类型，
//         然后再查询这两种代币的精度，并将手续费信息存储在 `PoolExtra::KriyaAmm` 中。
//
// 3.  **`KriyaAmmSwapEvent` 结构体 (交换记录卡)**:
//     -   记录Kriya AMM交换事件 (`SwapEvent`) 的详细信息（池ID、输入代币类型 `coin_in`、输入金额 `amount_in`、输出金额 `amount_out`）。
//         Kriya AMM的 `SwapEvent` 事件的泛型参数中只包含输入代币类型 (`CoinIn`)。输出代币类型需要额外推断。
//     -   `TryFrom<&SuiEvent>` 和 `TryFrom<&ShioEvent>`: 这两个“翻译机”负责检查收到的事件是否真的是Kriya AMM的交换事件。
//         它们会从事件的泛型参数中提取出输入代币类型 (`coin_in`)。
//     -   `KriyaAmmSwapEvent::new()`: 构造函数，根据已知的输入代币类型和从JSON中解析出的金额创建实例。
//     -   `to_swap_event_v1/_v2()` 方法将其转换为通用的 `SwapEvent`。
//         这两个方法都需要推断出输出代币的类型：
//         -   `_v1` 版本使用 `SuiClient` 和 `get_pool_coins_type` 来查询池中的两种代币类型，然后根据输入代币确定输出代币。
//         -   `_v2` 版本使用 `Simulator` 和 `get_coin_in_out_v2!` 宏来做同样的事情，但更为通用。
//
// 4.  **`kriya_amm_related_object_ids()` 函数**:
//     -   返回与Kriya AMM协议本身相关的核心对象ID列表 (当前只包含其主包ID)。
//
// **Kriya AMM事件的特殊性 (Peculiarity of Kriya AMM Event)**:
// -   **`PoolCreatedEvent`**: 这个事件直接提供了池ID和两种手续费率，但**不直接提供池中的两种代币类型**。
//     因此，在 `KriyaAmmPoolCreated::to_pool()` 中，需要额外调用 `get_pool_coins_type` (可能通过查询链上池对象状态) 来获取这些信息。
// -   **`SwapEvent`**: 这个事件的泛型参数 (`event.type_.type_params`) 只有一个，即输入代币的类型 (`CoinIn`)。
//     输出代币的类型没有直接在事件类型中给出，因此在 `KriyaAmmSwapEvent::to_swap_event_v1/_v2()` 中，
//     也需要通过查询池对象的状态来推断出输出代币的类型。

// 引入标准库的 Arc (原子引用计数) 和 FromStr (从字符串转换)。
use std::{str::FromStr, sync::Arc};

// 引入 eyre 库，用于更方便的错误处理和上下文管理。
use eyre::{ensure, eyre, OptionExt, Result};
// 引入 Move 核心类型中的 StructTag，用于解析和表示Move结构体的类型信息。
use move_core_types::language_storage::StructTag;
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
    types::base_types::ObjectID,      // ObjectID是Sui对象的唯一标识符。
    SuiClient,                        // Sui RPC客户端，用于与Sui网络交互。
};

// 从当前crate的父模块 (protocols) 引入 get_coin_decimals 和 get_pool_coins_type 函数。
use super::{get_coin_decimals, get_pool_coins_type};
// 从当前crate的根模块引入 get_coin_in_out_v2 宏, normalize_coin_type 函数和相关类型定义。
use crate::{
    get_coin_in_out_v2, normalize_coin_type,
    types::{Pool, PoolExtra, Protocol, SwapEvent, Token}, // 通用池结构, 协议特定附加信息, Protocol枚举, 通用交换事件, 代币信息结构。
};

/// `KRIYA_AMM_POOL_CREATED` 常量
///
/// 定义了KriyaDEX AMM (spot_dex模块) 在Sui链上发出的“新池创建”事件的全局唯一类型字符串。
pub const KRIYA_AMM_POOL_CREATED: &str =
    "0xa0eba10b173538c8fecca1dff298e488402cc9ff374f8a12ca7758eebe830b66::spot_dex::PoolCreatedEvent";

/// `KRIYA_AMM_SWAP_EVENT` 常量
///
/// 定义了KriyaDEX AMM (spot_dex模块) 在Sui链上发出的“代币交换完成”事件的全局唯一类型字符串。
pub const KRIYA_AMM_SWAP_EVENT: &str =
    "0xa0eba10b173538c8fecca1dff298e488402cc9ff374f8a12ca7758eebe830b66::spot_dex::SwapEvent";

/// `kriya_amm_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，专门用于订阅KriyaDEX AMM的“新池创建”事件。
pub fn kriya_amm_event_filter() -> EventFilter {
    EventFilter::MoveEventType(KRIYA_AMM_POOL_CREATED.parse().unwrap()) // 解析类型字符串为StructTag
}

/// `KriyaAmmPoolCreated` 结构体
///
/// 用于存储从KriyaDEX AMM的 `PoolCreatedEvent` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)]
pub struct KriyaAmmPoolCreated {
    pub pool: ObjectID,             // 新创建的池的ObjectID
    pub lp_fee_percent: u64,        // 流动性提供者 (LP) 的手续费百分比
    pub protocol_fee_percent: u64,  // 协议收取的手续费百分比
}

/// 为 `KriyaAmmPoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
/// 使得可以尝试将一个通用的 `&SuiEvent` 引用转换为 `KriyaAmmPoolCreated`。
impl TryFrom<&SuiEvent> for KriyaAmmPoolCreated {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let parsed_json = &event.parsed_json; // 获取事件的JSON数据部分
        // 从JSON中提取 "pool_id" 字段，并解析为 ObjectID。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaAmmPoolCreated事件JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 从JSON中提取 "lp_fee_percent" 字段并解析为 u64。
        let lp_fee_val: u64 = parsed_json["lp_fee_percent"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaAmmPoolCreated事件JSON中缺少'lp_fee_percent'字段"))?
            .parse()?;

        // 从JSON中提取 "protocol_fee_percent" 字段并解析为 u64。
        let protocol_fee_val: u64 = parsed_json["protocol_fee_percent"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaAmmPoolCreated事件JSON中缺少'protocol_fee_percent'字段"))?
            .parse()?;

        Ok(Self {
            pool: pool_object_id,
            lp_fee_percent: lp_fee_val,
            protocol_fee_percent: protocol_fee_val,
        })
    }
}

impl KriyaAmmPoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `KriyaAmmPoolCreated` 事件数据转换为通用的 `Pool` 结构。
    /// 此方法需要异步查询池的两种代币类型及其精度。
    pub async fn to_pool(&self, sui: &SuiClient) -> Result<Pool> {
        // 调用父模块的 `get_pool_coins_type` 函数从链上获取池的两种代币类型。
        // `self.pool` 是池的ObjectID。
        let (token0_type_str, token1_type_str) = get_pool_coins_type(sui, self.pool).await?;

        // 异步获取token0和token1的精度信息。
        let token0_decimals = get_coin_decimals(sui, &token0_type_str).await?;
        let token1_decimals = get_coin_decimals(sui, &token1_type_str).await?;

        // 创建 Token 结构列表。
        let tokens_vec = vec![
            Token::new(&token0_type_str, token0_decimals),
            Token::new(&token1_type_str, token1_decimals),
        ];
        // 创建 PoolExtra::KriyaAmm，存储Kriya AMM特定的手续费信息。
        let extra_data = PoolExtra::KriyaAmm {
            lp_fee_percent: self.lp_fee_percent,
            protocol_fee_percent: self.protocol_fee_percent,
        };

        Ok(Pool {
            protocol: Protocol::KriyaAmm, // 指明协议为KriyaAmm
            pool: self.pool,             // 池的ObjectID
            tokens: tokens_vec,          // 池中代币列表
            extra: extra_data,           // 协议特定附加信息
        })
    }
}

/// `KriyaAmmSwapEvent` 结构体
///
/// 用于存储从KriyaDEX AMM的 `SwapEvent` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)]
pub struct KriyaAmmSwapEvent {
    pub pool: ObjectID,       // 发生交换的池的ObjectID
    pub coin_in: String,      // 输入代币的规范化类型字符串 (从事件泛型参数获取)
    pub amount_in: u64,       // 输入代币的数量
    pub amount_out: u64,      // 输出代币的数量
}

/// 为 `KriyaAmmSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for KriyaAmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let event_type_str = event.type_.to_string();
        // 确保事件类型以 `KRIYA_AMM_SWAP_EVENT` 开头。
        // Kriya AMM的 `SwapEvent` 有一个泛型参数，即输入代币的类型 (`CoinIn`)。
        ensure!(
            event_type_str.starts_with(KRIYA_AMM_SWAP_EVENT) && event.type_.type_params.len() == 1,
            "事件类型不匹配Kriya AMM SwapEvent的要求 (Not a KriyaAmmSwapEvent: type or type_params mismatch)"
        );

        // 从事件的第一个泛型参数提取输入代币类型，并规范化。
        let coin_in_type_str = event.type_.type_params[0].to_string();
        let normalized_coin_in = normalize_coin_type(&coin_in_type_str);

        // 调用下面的 `new` 方法，从事件的 `parsed_json` 内容和已提取的输入代币类型创建 `KriyaAmmSwapEvent` 实例。
        Self::new(&event.parsed_json, normalized_coin_in)
    }
}

/// 为 `KriyaAmmSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for KriyaAmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        // 从 `ShioEvent` 的 `event_type` 字符串解析出 `StructTag`。
        let event_type_tag = StructTag::from_str(&event.event_type).map_err(|e| eyre!(e))?;
        // 同样检查事件类型前缀和泛型参数数量。
        ensure!(
            event.event_type.starts_with(KRIYA_AMM_SWAP_EVENT) && event_type_tag.type_params.len() == 1,
            "事件类型不匹配Kriya AMM SwapEvent的要求 (Not a KriyaAmmSwapEvent: type or type_params mismatch)"
        );

        // 从 `StructTag` 中提取并规范化输入代币类型。
        let coin_in_type_str = event_type_tag.type_params[0].to_string();
        let normalized_coin_in = normalize_coin_type(&coin_in_type_str);

        // 获取 `ShioEvent` 中的 `parsed_json`。
        let parsed_json_value = event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json field in ShioEvent)")?;

        // 调用 `new` 方法创建 `KriyaAmmSwapEvent` 实例。
        Self::new(parsed_json_value, normalized_coin_in)
    }
}

impl KriyaAmmSwapEvent {
    /// `new` 构造函数
    ///
    /// 从已解析的JSON值 (`parsed_json`) 和已知的输入代币类型 (`coin_in`) 创建 `KriyaAmmSwapEvent`。
    /// 输出代币类型需要通过其他方式推断 (例如在 `to_swap_event_vX` 中)。
    pub fn new(parsed_json: &Value, coin_in: String) -> Result<Self> {
        // 从JSON中提取 "pool_id" 字段并解析为 ObjectID。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaAmmSwapEvent JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 提取 "amount_in" 字段并解析为 u64。
        let amount_in_val: u64 = parsed_json["amount_in"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaAmmSwapEvent JSON中缺少'amount_in'字段"))?
            .parse()?;

        // 提取 "amount_out" 字段并解析为 u64。
        let amount_out_val: u64 = parsed_json["amount_out"]
            .as_str()
            .ok_or_else(|| eyre!("KriyaAmmSwapEvent JSON中缺少'amount_out'字段"))?
            .parse()?;

        Ok(Self {
            pool: pool_object_id,
            coin_in, // 输入代币类型已知
            amount_in: amount_in_val,
            amount_out: amount_out_val,
        })
    }

    /// `to_swap_event_v1` 异步方法 (旧版本，使用SuiClient查询代币类型)
    ///
    /// 将 `KriyaAmmSwapEvent` 转换为通用的 `SwapEvent`。
    /// 此版本需要一个 `SuiClient` 来从链上查询池对象的两种代币类型，然后推断出输出代币类型。
    #[allow(dead_code)] // 标记为允许死代码
    pub async fn to_swap_event_v1(&self, sui: &SuiClient) -> Result<SwapEvent> {
        // 获取池的两种代币类型 (coin_a, coin_b)
        let (coin_a_type, coin_b_type) = get_pool_coins_type(sui, self.pool).await?;
        // 根据已知的输入代币 `self.coin_in`，推断出输出代币类型。
        let final_coin_out = if self.coin_in == coin_a_type {
            coin_b_type // 如果输入是A，则输出是B
        } else {
            coin_a_type // 否则输入是B，输出是A
        };
        let normalized_coin_out = normalize_coin_type(&final_coin_out); // 规范化输出代币类型

        Ok(SwapEvent {
            protocol: Protocol::KriyaAmm,
            pool: Some(self.pool),
            coins_in: vec![self.coin_in.clone()],
            coins_out: vec![normalized_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }

    /// `to_swap_event_v2` 异步方法 (新版本，使用Simulator查询代币类型)
    ///
    /// 将 `KriyaAmmSwapEvent` 转换为通用的 `SwapEvent`。
    /// 使用 `Simulator` 实例和 `get_coin_in_out_v2!` 宏来获取和判断代币类型。
    pub async fn to_swap_event_v2(&self, provider: Arc<dyn Simulator>) -> Result<SwapEvent> {
        // `get_coin_in_out_v2!` 宏需要池ID、模拟器provider和一个布尔值 `a2b`。
        // 对于Kriya AMM的SwapEvent，我们只知道输入代币 `self.coin_in`。
        // 我们需要先获取池中的两种代币类型 (coin_a, coin_b)，然后判断 `self.coin_in` 是 a 还是 b，
        // 从而确定 `a2b` 的值和实际的 `coin_out`。
        // 假设 `get_coin_in_out_v2!` 宏能处理这种情况，或者我们需要先确定 `a2b`。

        // 步骤1: 获取池中的两种规范代币类型 (coin_a, coin_b)
        // 这里的 `true` 只是一个占位符给 `a2b`，因为 `get_coin_in_out_v2` 的主要目的是获取两种币的类型。
        // 宏内部会获取池的泛型参数作为 coin_a 和 coin_b。
        let (pool_coin_a, pool_coin_b) = get_coin_in_out_v2!(self.pool, provider.clone(), true); // provider被克隆

        // 步骤2: 根据已知的 `self.coin_in` 确定实际的输入和输出代币。
        let (final_coin_in, final_coin_out) = if pool_coin_a == self.coin_in {
            (pool_coin_a, pool_coin_b)
        } else if pool_coin_b == self.coin_in {
            (pool_coin_b, pool_coin_a)
        } else {
            // 如果 self.coin_in 既不是 pool_coin_a也不是 pool_coin_b，则说明逻辑有误或数据不一致。
            return Err(eyre!(
                "KriyaAmmSwapEvent中的coin_in ({}) 与从池 {} 推断出的代币类型 ({}, {}) 不匹配",
                self.coin_in, self.pool, pool_coin_a, pool_coin_b
            ));
        };

        Ok(SwapEvent {
            protocol: Protocol::KriyaAmm,
            pool: Some(self.pool),
            coins_in: vec![final_coin_in],
            coins_out: vec![final_coin_out],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

/// `kriya_amm_related_object_ids` 函数
///
/// 返回与Kriya AMM协议本身相关的核心对象ID列表 (目前仅包含其主包ID)。
pub fn kriya_amm_related_object_ids() -> Vec<String> {
    vec![
        // KriyaDEX (spot_dex / AMM) 的主程序包ID
        "0xa0eba10b173538c8fecca1dff298e488402cc9ff374f8a12ca7758eebe830b66", // Kriya Dex (spot_dex)
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use super::*; // 导入外部模块 (kriya_amm.rs) 的所有公共成员
    use simulator::HttpSimulator; // HTTP模拟器

    /// `test_swap_event_http` 测试函数
    ///
    /// 测试 `KriyaAmmSwapEvent::to_swap_event_v2` 方法 (使用HttpSimulator)。
    #[tokio::test]
    async fn test_swap_event_http() {
        // 创建HttpSimulator (RPC URL为空，可能依赖环境变量或默认配置)
        let provider_http = HttpSimulator::new("", &None).await;

        // 创建一个示例的 KriyaAmmSwapEvent
        let kriya_swap_event_data = KriyaAmmSwapEvent {
            // 这是一个示例池ID，需要替换为实际有效的Kriya AMM池ID
            pool: ObjectID::from_str("0x367e02acb99632e18db69c3e93d89d21eb721e1d1fcebc0f6853667337450acc").unwrap(),
            amount_in: 0x1337,  // 示例输入金额
            amount_out: 0x1338, // 示例输出金额
            coin_in: "0x2::sui::SUI".to_string(), // 假设输入的是SUI
        };

        // 调用被测试方法
        let converted_swap_event = kriya_swap_event_data.to_swap_event_v2(Arc::new(provider_http)).await.unwrap();
        // 预期的代币类型 (需要与上面池ID实际对应的代币类型一致)
        let expected_coin_a_type = "0x2::sui::SUI";
        let expected_coin_b_type = "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN"; // Wormhole USDC (示例)

        // 断言转换后的输入输出代币类型是否正确
        assert_eq!(converted_swap_event.coins_in[0], expected_coin_a_type, "输入代币类型不匹配");
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_b_type, "输出代币类型不匹配");
    }

    /// `test_swap_event_db` 测试函数
    ///
    /// 与上一个测试类似，但使用 `DBSimulator`。
    #[tokio::test]
    async fn test_swap_event_db() {
        use simulator::DBSimulator; // 引入DBSimulator

        // 创建DBSimulator (可能需要预填充或配置数据库)
        let provider_db = DBSimulator::new_default_slow().await;

        let kriya_swap_event_data = KriyaAmmSwapEvent {
            pool: ObjectID::from_str("0x367e02acb99632e18db69c3e93d89d21eb721e1d1fcebc0f6853667337450acc").unwrap(),
            amount_in: 0x1337,
            amount_out: 0x1338,
            coin_in: "0x2::sui::SUI".to_string(),
        };

        let converted_swap_event = kriya_swap_event_data.to_swap_event_v2(Arc::new(provider_db)).await.unwrap();
        let expected_coin_a_type = "0x2::sui::SUI";
        let expected_coin_b_type = "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN";

        assert_eq!(converted_swap_event.coins_in[0], expected_coin_a_type, "输入代币类型不匹配");
        assert_eq!(converted_swap_event.coins_out[0], expected_coin_b_type, "输出代币类型不匹配");
    }
}

[end of crates/dex-indexer/src/protocols/kriya_amm.rs]
