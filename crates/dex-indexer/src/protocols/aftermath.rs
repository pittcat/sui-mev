// 该文件 `aftermath.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 Aftermath Finance DEX 协议相关的事件和数据结构。
// Aftermath Finance 是Sui生态中的一个去中心化交易所，支持多代币池和加权池。
// 这个文件的主要功能是：
// 1. 定义 Aftermath "池创建" (`CreatedPoolEvent`) 和 "交换" (`SwapEventV2`) 事件的类型字符串常量。
// 2. 提供 `aftermath_event_filter()` 函数，用于创建Sui事件订阅的过滤器，专门监听Aftermath的池创建事件。
// 3. 定义 `AftermathPoolCreated` 结构体，用于存储从链上 `CreatedPoolEvent` 事件解析出来的具体池子创建信息，
//    例如池ID、LP代币类型、池内代币类型列表、各种手续费率等。
// 4. 实现 `TryFrom<&SuiEvent>` for `AftermathPoolCreated`，使得可以从通用的 `SuiEvent`
//    转换为特定于 Aftermath 的 `AftermathPoolCreated` 结构。
// 5. `AftermathPoolCreated::to_pool()` 方法，将解析出的 `AftermathPoolCreated` 事件数据
//    转换为一个更通用的 `Pool` 结构 (在 `types.rs` 中定义)，这个 `Pool` 结构是 `dex-indexer`
//    用来统一表示不同DEX协议池信息的内部标准格式。它会异步查询代币精度。
// 6. 定义 `AftermathSwapEvent` 结构体，用于存储从链上 `SwapEventV2` 事件解析出来的具体交换信息，
//    包括池ID、输入输出代币列表（Aftermath支持多币进出）、输入输出金额列表。
// 7. 实现 `TryFrom<&SuiEvent>`、`TryFrom<&ShioEvent>` 和 `TryFrom<&Value>` for `AftermathSwapEvent`，
//    使得可以从不同来源的事件数据或原始JSON值转换为 `AftermathSwapEvent`。
// 8. `AftermathSwapEvent::to_swap_event()` 方法，将 `AftermathSwapEvent` 转换为通用的 `SwapEvent` 枚举成员。
// 9. `aftermath_related_object_ids()` 异步函数，返回一个包含许多硬编码的Aftermath协议相关的核心对象ID列表。
//    这些对象可能包括协议的全局配置、各种金库（vaults）、以及不同版本的核心模块或接口对象。
//    它还会尝试获取这些对象的子对象ID (通过 `get_children_ids`)，以确保索引器能够预加载尽可能多的相关对象。
// 10. `aftermath_pool_children_ids()` 异步函数，用于获取特定Aftermath池对象的子对象ID，
//     以及池泛型参数中定义的代币类型的对象ID。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” Aftermath Finance 这个DEX发生的两种主要“事件”的：
// 一是“有人创建了一个新的交易池”，二是“有人在这个池子里完成了一笔代币交换”。
// 它还负责收集与Aftermath协议本身相关联的一些重要的“全局对象”的ID。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别Aftermath的特定事件**:
//     -   `AFTERMATH_POOL_CREATED` 和 `AFTERMATH_SWAP_EVENT` 这两个常量，就像是Aftermath这两种事件在Sui链上的“专属门牌号”。
//         程序用它们来从一大堆链上事件中准确地挑出Aftermath的事件。
//     -   `aftermath_event_filter()` 函数会生成一个“过滤器”，告诉Sui节点：“我只对Aftermath创建新池子的事件感兴趣，其他的别发给我。”
//
// 2.  **`AftermathPoolCreated` 结构体 (新池子信息卡)**:
//     -   当监听到一个Aftermath新池子被创建的事件时，这个结构体就负责像填一张“新池子信息卡”一样，把事件里的重要信息（比如池子的ID、池子里有哪些代币、LP代币是什么类型、交易手续费是多少等）都记录下来。
//     -   `TryFrom<&SuiEvent>` 部分就是“填卡员”，负责从原始的Sui事件中读取数据并填到这张卡上。
//     -   `to_pool()` 方法则是把这张Aftermath专用的“信息卡”，转换成一张我们程序内部所有DEX池子通用的标准“池子信息表”（`Pool`结构）。这个转换过程中，它还会顺便去查一下池子里各种代币的“精度”（比如1个SUI等于多少个MIST）。
//
// 3.  **`AftermathSwapEvent` 结构体 (交换记录卡)**:
//     -   当监听到一笔Aftermath交换事件时，这个结构体就负责记录这笔交换的详细情况，比如在哪个池子（`pool`）、用了哪些币作为输入（`coins_in`）、换回了哪些币（`coins_out`），以及各自的数量（`amounts_in`, `amounts_out`）。Aftermath比较特殊，它支持一次用多种币换多种币。
//     -   `TryFrom<&SuiEvent>`、`TryFrom<&ShioEvent>`、`TryFrom<&Value>` 这三个都是“填卡员”，能从不同来源的原始数据（标准Sui事件、Shio MEV事件、或者原始JSON数据）中提取信息填到这张“交换记录卡”上。
//     -   `to_swap_event()` 方法也是把这张Aftermath专用的“交换记录卡”，转换成程序内部通用的标准“交换事件报告”（`SwapEvent`）。
//
// 4.  **收集相关对象ID (Collecting Related Object IDs)**:
//     -   `aftermath_related_object_ids()` 函数列出了一大堆硬编码的Aftermath协议的核心组件的ID。这些可能是协议的全局设置、资金库、不同版本的合约等等。它还会尝试找出这些核心对象的“子对象”，目的是把所有可能跟Aftermath协议运作相关的重要对象的ID都收集起来。
//     -   `aftermath_pool_children_ids()` 函数则是针对某一个特定的Aftermath池子，找出它的“子对象”以及池子参数里定义的代币类型的ID。
//     -   **为什么要做这个？** 同样是为了给 `DBSimulator`（数据库模拟器）提供“预加载清单”，让模拟器能提前把这些对象的数据准备好，加快后续的交易模拟速度。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
//
// -   **多代币池 (Multi-token Pools)**:
//     传统的AMM（如Uniswap V2）通常只支持包含两种代币的交易池。而像Aftermath这样的协议（类似于Balancer）可能支持一个池子中包含三种或更多不同类型的代币。
//     这允许更复杂的资产组合和交易策略。例如，一个池子可以同时包含SUI、USDC和ETH。
//
// -   **多币种交换 (Multi-asset Swaps)**:
//     由于支持多代币池，Aftermath的交换事件 (`SwapEventV2`) 也比较复杂，它可以表示一次交换中，输入是多种代币，输出也是多种代币。
//     例如，你可以用SUI和USDC一起去换ETH和BTC（如果池子支持这种组合）。
//     因此，`AftermathSwapEvent` 中的 `coins_in`, `coins_out`, `amounts_in`, `amounts_out` 都是列表（`Vec`）。
//
// -   **LP代币类型 (`lp_type`)**:
//     当用户向Aftermath的池子提供流动性时，他们会收到代表其在池中份额的LP（Liquidity Provider）代币。
//     `lp_type` 就是这个LP代币在Sui链上的具体类型字符串。
//
// -   **各种手续费 (`fees_swap_in`, `fees_swap_out`, `fees_deposit`, `fees_withdraw`)**:
//     Aftermath协议为其池子定义了多种类型的手续费：
//     -   `fees_swap_in`: 当你用池中的某种代币A去换代币B时，针对你输入的代币A（或者说，针对“进入”池子的代币流）收取的手续费率。
//     -   `fees_swap_out`: 针对你从池中换出的代币B（或者说，针对“离开”池子的代币流）收取的手续费率。
//     -   `fees_deposit`: 当你向池子存入流动性时收取的手续费率。
//     -   `fees_withdraw`: 当你从池子提取流动性时收取的手续费率。
//     这些费率都是以 `u64` 形式存储的，通常代表一个非常大的整数，需要除以某个基数（比如10^18或10^9）才能得到实际的百分比费率。
//     (例如，费率500可能代表0.05%，如果基数是1,000,000的话)。

// 引入标准库的Arc (原子引用计数) 和 FromStr (从字符串转换)。
use std::sync::Arc;
use std::str::FromStr;

// 引入eyre库，用于错误处理和上下文管理。
use eyre::{ensure, eyre, OptionExt, Result};
// 引入Move核心类型中的MoveStruct和StructTag，用于解析Move对象的结构和类型。
use move_core_types::{annotated_value::MoveStruct, language_storage::StructTag};
// 引入serde库的Deserialize trait，用于从如JSON这样的格式反序列化数据。
use serde::Deserialize;
// 引入serde_json的Value类型，用于表示通用的JSON值。
use serde_json::Value;
// 引入shio库的ShioEvent类型，可能用于处理与MEV相关的特定事件。
use shio::ShioEvent;
// 引入simulator库的Simulator trait，定义了交易模拟器的通用接口。
use simulator::Simulator;
// 引入Sui SDK中的相关类型。
use sui_sdk::{
    rpc_types::{EventFilter, SuiEvent}, // EventFilter用于订阅事件，SuiEvent代表链上事件。
    types::base_types::ObjectID,      // ObjectID是Sui对象的唯一标识符。
    SuiClient,                        // Sui RPC客户端，用于与Sui网络交互。
};
// 引入Sui核心类型中的TypeTag，用于在运行时表示Move类型。
use sui_types::TypeTag;

// 从当前crate的父模块 (protocols) 引入 get_children_ids (获取子对象ID) 和 get_coin_decimals (获取代币精度) 函数。
use super::{get_children_ids, get_coin_decimals};
// 从当前crate的根模块引入 normalize_coin_type (规范化代币类型字符串) 函数和相关类型定义。
use crate::{
    normalize_coin_type,
    types::{Pool, PoolExtra, Protocol, SwapEvent, Token}, // Pool是通用池结构, PoolExtra是协议特定附加信息, Protocol枚举, SwapEvent通用交换事件, Token代币信息结构。
};

/// `AFTERMATH_POOL_CREATED` 常量
///
/// 定义了Aftermath Finance协议在Sui链上发出的“新池创建”事件的全局唯一类型字符串。
/// 这个字符串用于从Sui事件流中准确识别出Aftermath创建新池的事件。
pub const AFTERMATH_POOL_CREATED: &str =
    "0xefe170ec0be4d762196bedecd7a065816576198a6527c99282a2551aaa7da38c::events::CreatedPoolEvent";

/// `AFTERMATH_SWAP_EVENT` 常量
///
/// 定义了Aftermath Finance协议在Sui链上发出的“代币交换完成”事件 (V2版本) 的全局唯一类型字符串。
/// 这个字符串用于识别Aftermath的交换事件。
pub const AFTERMATH_SWAP_EVENT: &str =
    "0xc4049b2d1cc0f6e017fda8260e4377cecd236bd7f56a54fee120816e72e2e0dd::events::SwapEventV2";

/// `aftermath_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，该过滤器专门用于订阅Aftermath的“新池创建”事件。
/// `EventFilter::MoveEventType` 表示只关心特定Move事件类型。
/// `.parse().unwrap()` 将字符串常量解析为 `StructTag`，这里假设常量总是有效的。
pub fn aftermath_event_filter() -> EventFilter {
    EventFilter::MoveEventType(AFTERMATH_POOL_CREATED.parse().unwrap())
}

/// `AftermathPoolCreated` 结构体
///
/// 用于存储从Aftermath的 `CreatedPoolEvent` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)] // 自动派生常用trait
pub struct AftermathPoolCreated {
    pub pool: ObjectID,            // 新创建的池的ObjectID
    pub lp_type: String,           // 该池的LP代币的完整类型字符串
    pub token_types: Vec<String>,  // 池中包含的所有代币的类型字符串列表
    pub fees_swap_in: Vec<u64>,    // 对应 `token_types` 列表中每个代币的输入方向交换手续费率
    pub fees_swap_out: Vec<u64>,   // 对应 `token_types` 列表中每个代币的输出方向交换手续费率
    pub fees_deposit: Vec<u64>,    // 对应 `token_types` 列表中每个代币的存款手续费率
    pub fees_withdraw: Vec<u64>,   // 对应 `token_types` 列表中每个代币的取款手续费率
}

/// 为 `AftermathPoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
/// 使得可以尝试将一个通用的 `&SuiEvent` 引用转换为 `AftermathPoolCreated`。
impl TryFrom<&SuiEvent> for AftermathPoolCreated {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &SuiEvent) -> Result<Self> {
        // `parsed_json` 字段包含了事件的具体数据，以JSON格式存储。
        let parsed_json = &event.parsed_json;

        // 从JSON中提取 "pool_id" 字段，期望它是一个字符串，然后解析为 ObjectID。
        // `ok_or_else` 用于在字段缺失或类型不匹配时提供自定义错误。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("AftermathPoolCreated事件JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?; // 将字符串解析为ObjectID

        // 提取 "lp_type" 字段。
        let lp_token_type_str = parsed_json["lp_type"]
            .as_str()
            .ok_or_else(|| eyre!("AftermathPoolCreated事件JSON中缺少'lp_type'字段"))?;

        // 提取 "coins" 字段，它应该是一个包含代币类型字符串的数组。
        // 对数组中的每个元素，都格式化为 "0x{token_type_without_prefix}" 的形式。
        let token_types_vec = parsed_json["coins"]
            .as_array() // 尝试将JSON值作为数组获取
            .ok_or_else(|| eyre!("AftermathPoolCreated事件JSON中缺少'coins'数组"))?
            .iter() // 遍历数组中的每个JSON值
            .map(|json_val| { // 对每个值进行转换
                let token_type_str_inner = json_val.as_str().ok_or_else(|| eyre!("'coins'数组中的元素非字符串类型"))?;
                Ok(format!("0x{}", token_type_str_inner)) // 添加 "0x" 前缀
            })
            .collect::<Result<Vec<String>>>()?; // 将结果收集到 Vec<String>，如果任何转换失败则整个Result为Err

        // 提取各种手续费率数组，与 `token_types_vec` 的提取逻辑类似。
        // 每个费率数组中的元素顺序应与 `token_types_vec` 中的代币顺序对应。
        let fees_swap_in_vec = parsed_json["fees_swap_in"]
            .as_array()
            .ok_or_else(|| eyre!("AftermathPoolCreated事件JSON中缺少'fees_swap_in'数组"))?
            .iter()
            .map(|json_val| json_val.as_str().ok_or_else(|| eyre!("'fees_swap_in'数组元素非字符串"))?.parse::<u64>().map_err(|e| eyre!(e)))
            .collect::<Result<Vec<u64>>>()?;

        let fees_swap_out_vec = parsed_json["fees_swap_out"]
            .as_array()
            .ok_or_else(|| eyre!("AftermathPoolCreated事件JSON中缺少'fees_swap_out'数组"))?
            .iter()
            .map(|json_val| json_val.as_str().ok_or_else(|| eyre!("'fees_swap_out'数组元素非字符串"))?.parse::<u64>().map_err(|e| eyre!(e)))
            .collect::<Result<Vec<u64>>>()?;

        let fees_deposit_vec = parsed_json["fees_deposit"]
            .as_array()
            .ok_or_else(|| eyre!("AftermathPoolCreated事件JSON中缺少'fees_deposit'数组"))?
            .iter()
            .map(|json_val| json_val.as_str().ok_or_else(|| eyre!("'fees_deposit'数组元素非字符串"))?.parse::<u64>().map_err(|e| eyre!(e)))
            .collect::<Result<Vec<u64>>>()?;

        let fees_withdraw_vec = parsed_json["fees_withdraw"]
            .as_array()
            .ok_or_else(|| eyre!("AftermathPoolCreated事件JSON中缺少'fees_withdraw'数组"))?
            .iter()
            .map(|json_val| json_val.as_str().ok_or_else(|| eyre!("'fees_withdraw'数组元素非字符串"))?.parse::<u64>().map_err(|e| eyre!(e)))
            .collect::<Result<Vec<u64>>>()?;

        // 返回构造好的 AftermathPoolCreated 实例
        Ok(Self {
            pool: pool_object_id,
            lp_type: format!("0x{}", lp_token_type_str), // 为LP类型也添加 "0x" 前缀
            token_types: token_types_vec,
            fees_swap_in: fees_swap_in_vec,
            fees_swap_out: fees_swap_out_vec,
            fees_deposit: fees_deposit_vec,
            fees_withdraw: fees_withdraw_vec,
        })
    }
}

impl AftermathPoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `AftermathPoolCreated` 事件数据转换为通用的 `Pool` 结构。
    /// 这个过程需要异步查询每个代币的精度 (decimals)。
    ///
    /// 参数:
    /// - `sui`: 一个对 `SuiClient` 的引用，用于查询代币精度。
    ///
    /// 返回:
    /// - `Result<Pool>`: 转换后的 `Pool` 对象。
    pub async fn to_pool(&self, sui: &SuiClient) -> Result<Pool> {
        let mut tokens_vec = vec![]; // 用于存储转换后的 Token 结构
        // 遍历池中所有代币类型
        for token_type_str in &self.token_types {
            // 异步获取每个代币的精度信息
            let token_decimals = get_coin_decimals(sui, token_type_str).await?;
            // 创建 Token 结构并添加到列表中
            tokens_vec.push(Token::new(token_type_str, token_decimals));
        }

        // 创建 PoolExtra::Aftermath 枚举成员，存储Aftermath特定的附加信息
        let extra_data = PoolExtra::Aftermath {
            lp_type: self.lp_type.clone(),
            fees_swap_in: self.fees_swap_in.clone(),
            fees_swap_out: self.fees_swap_out.clone(),
            fees_deposit: self.fees_deposit.clone(),
            fees_withdraw: self.fees_withdraw.clone(),
        };

        // 返回构造好的 Pool 对象
        Ok(Pool {
            protocol: Protocol::Aftermath, // 指明协议为Aftermath
            pool: self.pool,               // 池的ObjectID
            tokens: tokens_vec,            // 池中代币列表 (包含精度信息)
            extra: extra_data,             // 协议特定附加信息
        })
    }
}

/// `AftermathSwapEvent` 结构体
///
/// 用于存储从Aftermath的 `SwapEventV2` 事件中解析出来的具体交换信息。
/// Aftermath支持多代币输入和多代币输出的交换。
#[derive(Debug, Clone, Deserialize)] // 自动派生常用trait
pub struct AftermathSwapEvent {
    pub pool: ObjectID,           // 发生交换的池的ObjectID
    pub coins_in: Vec<String>,    // 输入代币的类型字符串列表 (已规范化)
    pub coins_out: Vec<String>,   // 输出代币的类型字符串列表 (已规范化)
    pub amounts_in: Vec<u64>,     // 对应 `coins_in` 列表中每个代币的输入金额
    pub amounts_out: Vec<u64>,    // 对应 `coins_out` 列表中每个代币的输出金额
}

/// 为 `AftermathSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for AftermathSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        // 确保事件类型与 `AFTERMATH_SWAP_EVENT` 匹配。
        ensure!(
            event.type_.to_string() == AFTERMATH_SWAP_EVENT,
            "事件类型不匹配Aftermath SwapEvent (Not an AftermathSwapEvent)"
        );
        // 直接尝试从事件的 `parsed_json` 字段转换。
        // 这要求 `AftermathSwapEvent` 实现了 `TryFrom<&Value>` (见下文)。
        (&event.parsed_json).try_into()
    }
}

/// 为 `AftermathSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for AftermathSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        // 确保事件类型字符串与 `AFTERMATH_SWAP_EVENT` 匹配。
        ensure!(event.event_type == AFTERMATH_SWAP_EVENT, "事件类型不匹配Aftermath SwapEvent (Not an AftermathSwapEvent)");
        // 从 `ShioEvent` 的 `parsed_json` (是一个 `Option<Value>`) 中获取 `Value` 的引用并尝试转换。
        event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json in ShioEvent)")?.try_into()
    }
}

/// 为 `AftermathSwapEvent` 实现 `TryFrom<&Value>` trait (从 `serde_json::Value` 转换)。
/// 这是实际的JSON解析逻辑。
impl TryFrom<&Value> for AftermathSwapEvent {
    type Error = eyre::Error;

    fn try_from(parsed_json: &Value) -> Result<Self> {
        // 从JSON中提取 "pool_id" 字段并解析为 ObjectID。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("AftermathSwapEvent JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 提取 "types_in" 数组，并将其中每个代币类型字符串规范化。
        let coins_in_vec = parsed_json["types_in"]
            .as_array()
            .ok_or_else(|| eyre!("AftermathSwapEvent JSON中缺少'types_in'数组"))?
            .iter()
            .map(|json_val| {
                let coin_type_str_inner = json_val.as_str().ok_or_else(|| eyre!("'types_in'数组元素非字符串"))?;
                // 注意：Aftermath事件中的类型可能不带 "0x" 前缀，这里统一添加。
                Ok(normalize_coin_type(format!("0x{}", coin_type_str_inner).as_str()))
            })
            .collect::<Result<Vec<String>>>()?;

        // 提取 "types_out" 数组，逻辑同上。
        let coins_out_vec = parsed_json["types_out"]
            .as_array()
            .ok_or_else(|| eyre!("AftermathSwapEvent JSON中缺少'types_out'数组"))?
            .iter()
            .map(|json_val| {
                let coin_type_str_inner = json_val.as_str().ok_or_else(|| eyre!("'types_out'数组元素非字符串"))?;
                Ok(normalize_coin_type(format!("0x{}", coin_type_str_inner).as_str()))
            })
            .collect::<Result<Vec<String>>>()?;

        // 提取 "amounts_in" 数组，并将每个元素（期望是字符串形式的数字）解析为 u64。
        let amounts_in_vec = parsed_json["amounts_in"]
            .as_array()
            .ok_or_else(|| eyre!("AftermathSwapEvent JSON中缺少'amounts_in'数组"))?
            .iter()
            .map(|json_val| {
                json_val.as_str()
                    .ok_or_else(|| eyre!("'amounts_in'数组元素非字符串"))?
                    .parse::<u64>() // 解析字符串为u64
                    .map_err(|e| eyre!(e)) // 将解析错误转换为eyre::Error
            })
            .collect::<Result<Vec<u64>>>()?;

        // 提取 "amounts_out" 数组，逻辑同上。
        let amounts_out_vec = parsed_json["amounts_out"]
            .as_array()
            .ok_or_else(|| eyre!("AftermathSwapEvent JSON中缺少'amounts_out'数组"))?
            .iter()
            .map(|json_val| {
                json_val.as_str()
                    .ok_or_else(|| eyre!("'amounts_out'数组元素非字符串"))?
                    .parse::<u64>()
                    .map_err(|e| eyre!(e))
            })
            .collect::<Result<Vec<u64>>>()?;

        // 返回构造好的 AftermathSwapEvent 实例
        Ok(Self {
            pool: pool_object_id,
            coins_in: coins_in_vec,
            coins_out: coins_out_vec,
            amounts_in: amounts_in_vec,
            amounts_out: amounts_out_vec,
        })
    }
}

impl AftermathSwapEvent {
    /// `to_swap_event` 异步方法
    ///
    /// 将 `AftermathSwapEvent` 转换为通用的 `SwapEvent` 枚举成员。
    pub async fn to_swap_event(&self) -> Result<SwapEvent> {
        Ok(SwapEvent {
            protocol: Protocol::Aftermath, // 指明协议为Aftermath
            pool: Some(self.pool),         // 交换发生的池的ObjectID
            coins_in: self.coins_in.clone(),   // 输入代币类型列表
            coins_out: self.coins_out.clone(), // 输出代币类型列表
            amounts_in: self.amounts_in.clone(), // 输入金额列表
            amounts_out: self.amounts_out.clone(), // 输出金额列表
        })
    }
}

/// `aftermath_related_object_ids` 异步函数
///
/// 返回一个包含许多硬编码的Aftermath协议相关的核心对象ID列表。
/// 这些对象对于索引器预加载数据或与协议交互可能很重要。
/// 它还会尝试获取这些核心对象的子对象ID。
pub async fn aftermath_related_object_ids() -> Vec<String> {
    // 硬编码的Aftermath协议核心对象ID列表
    let mut res = vec![
        "0xc4049b2d1cc0f6e017fda8260e4377cecd236bd7f56a54fee120816e72e2e0dd", // Aftermath AmmV2 (可能是主包或路由合约)
        "0xfcc774493db2c45c79f688f88d28023a3e7d98e4ee9f48bbf5c7990f651577ae", // PoolRegistry (池注册表)
        "0xf194d9b1bcad972e45a7dd67dd49b3ee1e3357a00a50850c52cd51bb450e13b4", // ProtocolFeeVault (协议手续费库)
        "0x28e499dff5e864a2eafe476269a4f5035f1c16f338da7be18b103499abf271ce", // Treasury (国库)
        "0xf0c40d67b078000e18032334c3325c47b9ec9f3d9ae4128be820d54663d14e3b", // InsuranceFund (保险基金)
        "0x35d35b0e5b177593d8c3a801462485572fc30861e6ce96a55af6dc4730709278", // ReferralVault (推荐人库)
        // 以下可能是一些特定版本的模块、接口或重要的共享对象
        "0x0c4a3be43155b87e13082d178b04707d30d764279c8df0c224803ae57ca78f23", // Aftermath 1
        "0x1ec6a8c5ac0b8b97c287cd34b9fc6a94b53a07c930a8505952679dc8d4b3780a", // Aftermath 2
        "0xf63c58d762286cff1ef8eab36a24c836d23ec0ca19eacbafec7a0275a09cd520", // Aftermath 3
        "0xcc9864d3e331b308875c5fc8da278ee5fdb187ec3923064801e8d2883b80eca1", // Aftermath 4
        "0xc66fabf1a9253e43c70f1cc02d40a1d18db183140ecaae2a3f58fa6b66c55acf", // Aftermath 5
        "0x3ac8d096a3ee492d40cfe5307f2df364e30b6da6cb515266bca901fc08211d89", // Aftermath 6
        "0x705b7644364a8d1c04425da3cb8eea8cdc28f58bb2c1cb8f438e4888b8de3178", // Aftermath 7
        "0xdc15721baa82ba64822d585a7349a1508f76d94ae80e899b06e48369c257750e", // Aftermath 8
        "0x0f460b32bc4aae750e803c6ce1f0e231b47f4209cd0a644990e6ab0491c68e00", // Aftermath 9
        "0x2880a6bbbd8636d9e39cd35cebf78154e3843f08cf846cadb920f3f008ce1b89", // Aftermath 10
        "0x2a3beb3c89759988ac1ae0ca3b06837ea7ac263fe82aae50c8a9c1e855224f08", // Aftermath 11
        "0x4f0a1a923dd063757fd37e04a9c2cee8980008e94433c9075c390065f98e9e4b", // Aftermath 12
        "0xdb982f402a039f196f3e13cd73795db441393b5bc6eef7a0295a333808982a7d", // Aftermath 13
        "0x712579292f80c11a0c9de4ff553d6e5c4757105e83a8a3129823d2b39e65d062", // Aftermath 14
        "0x640514f8576f8515cd7925db405140e7dedb523921da48cbae1d5d4f72347ea8", // Aftermath 15
        "0x6c0e485deedfadcd39511ec3bfda765ec9048275d4730fc2c78250526181c152", // Aftermath 16
        "0xb547b6e8b963c1d183d262080b67417c99bee2670e8bbad6efd477d75d271fa5", // Aftermath 17
        "0x418cb79536e45a7003dff6237218747656f22f3db15fac474ae54b016a2ddc33", // Aftermath 18
        "0x0625dc2cd40aee3998a1d6620de8892964c15066e0a285d8b573910ed4c75d50", // Aftermath Amm Interface
        "0xefe170ec0be4d762196bedecd7a065816576198a6527c99282a2551aaa7da38c", // Aftermath AmmV1 (与上面的PoolCreated事件包ID相同)
        "0x0b572349baf4526c92c4e5242306e07a1658fa329ae93d1b9db0fc38b8a592bb", // Aftermath Safe (可能是安全相关的模块或对象)
        "0x2d9316f1f1a95f6d7c85a4e690ef7c359e6649773ef2c37ad7d9857adb6bef06", // ProtocolFee (协议手续费相关)
        "0x64213b0e4a52bac468d4ac3f140242f70714381653a1919a6d57cd49c628207a", // Treasury (国库)
        "0x8d8bba50c626753589aa5abbc006c9fa07736f55f4e6fb57481682997c0b0d52", // Interface V2 (V2接口)
        "0xd2b95022244757b0ab9f74e2ee2fb2c3bf29dce5590fa6993a85d64bd219d7e8", // ReferralVault (推荐人库)
        "0xe5099fcd45747074d0ef5eabce07a9bd1c3b0c1862435bf2a09c3a81e0604373", // Core (核心模块)
        "0xceb3b6f35b71dbd0296cd96f8c00959c230854c7797294148b413094b9621b0e", // Treasury (另一个国库?)
        "0xa6baab1e668c7868991c1c3c11e144100f5734c407d020f72a01b9d1a8bcb97f", // Insurance (保险基金)
        "0xe7d60660de1c258e33987560d657d94fbf965b063cef84077afb4c702ba3c085", // TreasuryFund (国库基金)
        "0xc505da612b69f7e39d2c8ad591cf5675691a70209567c3520f2b90f10504eb1e", // InsuranceFund (另一个保险基金?)
    ]
    .into_iter()
    .map(|s| s.to_string()) // 将 &str 转换为 String
    .collect::<Vec<_>>(); // 收集为 Vec<String>

    // 将字符串ID列表转换为 ObjectID 列表
    let parent_ids = res
        .iter()
        .map(|id_str| ObjectID::from_hex_literal(id_str).unwrap()) // 解析为ObjectID，unwrap假设ID格式正确
        .collect::<Vec<_>>();

    // 遍历所有核心对象ID，尝试获取它们的子对象ID，并将子对象ID也添加到结果列表中。
    // 这有助于发现与核心协议对象动态关联的其他对象。
    for parent_object_id in parent_ids {
        if let Ok(children_object_ids_str) = get_children_ids(parent_object_id).await { // get_children_ids 是异步的
            res.extend(children_object_ids_str); // 将子对象ID字符串列表追加到结果中
        }
    }

    res // 返回包含所有相关对象ID字符串的列表
}

/// `aftermath_pool_children_ids` 异步函数
///
/// 获取特定Aftermath池对象的子对象ID，以及池的泛型类型参数中定义的代币类型的对象ID。
///
/// 参数:
/// - `pool`: 一个对 `Pool` 结构体的引用，代表要检查的Aftermath池。
/// - `simulator`: 一个共享的模拟器实例，用于从链上（或缓存）获取池对象的详细数据。
///
/// 返回:
/// - `Result<Vec<String>>`: 包含所有相关子对象和代币类型对象ID字符串的列表。
pub async fn aftermath_pool_children_ids(pool: &Pool, simulator: Arc<dyn Simulator>) -> Result<Vec<String>> {
    let mut result_ids_str_vec = vec![]; // 用于存储结果ID字符串的向量

    // 获取池对象的详细数据
    let pool_sui_object = simulator
        .get_object(&pool.pool) // pool.pool 是池的ObjectID
        .await
        .ok_or_else(|| eyre!("Aftermath池对象未找到: {}", pool.pool))?;

    // 解析池对象的Move结构体
    let parsed_pool_move_struct = {
        let object_layout = simulator
            .get_object_layout(&pool.pool)
            .ok_or_eyre("Aftermath池对象的布局(layout)未找到")?;

        let move_object_data = pool_sui_object.data.try_as_move().ok_or_eyre("对象不是有效的Move对象")?;
        MoveStruct::simple_deserialize(move_object_data.contents(), &object_layout).map_err(|e| eyre!(e))?
    };
    // 获取池的泛型类型参数 (例如 Pool<CoinA, CoinB, ...>)
    let pool_type_parameters = parsed_pool_move_struct.type_.type_params.clone();
    // 遍历这些泛型类型参数
    for type_param_tag in pool_type_parameters {
        match type_param_tag {
            TypeTag::Struct(struct_tag) => { // 如果泛型参数是一个结构体类型 (通常是代币类型)
                // 将其地址 (即定义该类型的包ID) 转换为十六进制字符串并添加到结果中。
                // 这有助于预加载定义池内代币的Move包对象。
                result_ids_str_vec.push(struct_tag.address.to_hex_literal());
            }
            _ => continue, // 如果不是结构体类型 (例如基本类型或其他)，则跳过
        }
    }

    // 获取池对象本身的子对象ID列表，并将它们添加到结果中。
    if let Ok(children_object_ids_str) = get_children_ids(pool.pool).await {
        result_ids_str_vec.extend(children_object_ids_str);
    }

    Ok(result_ids_str_vec)
}

[end of crates/dex-indexer/src/protocols/aftermath.rs]
