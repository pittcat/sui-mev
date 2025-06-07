// 该文件 `suiswap.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 SuiSwap DEX 协议相关的交换事件。
// SuiSwap 是 Sui 生态中的一个去中心化交易所。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“翻译” SuiSwap 这个DEX发生的“代币交换事件”的。
// 当用户在 SuiSwap 上用代币A换了代币B，Sui区块链上会记录一个“事件”。这个文件里的代码就是负责把这个原始的链上事件信息，
// 提取成我们程序自己能看懂的、标准化的格式。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别SuiSwap的交换事件**:
//     -   `SUISWAP_SWAP_EVENT` 常量定义了其交换事件 (`SwapTokenEvent`) 在Sui链上特有的“名字”（类型字符串）。
//         事件由 `pool` 模块发出。
//
// 2.  **`SuiswapSwapEvent` 结构体 (信息卡片)**:
//     -   这个结构体用于存储从SuiSwap的 `SwapTokenEvent` 事件中解析出来的关键交换信息：
//         输入输出代币类型 (`coin_in`, `coin_out`) 和对应的金额 (`amount_in`, `amount_out`)。
//
// 3.  **转换逻辑 (Translation Logic)**:
//     -   `TryFrom<&SuiEvent>` 和 `TryFrom<&ShioEvent>`: 这两个“翻译机”负责检查收到的事件是否真的是SuiSwap的交换事件。
//         -   它们首先会检查事件类型字符串是否以 `SUISWAP_SWAP_EVENT` 开头。
//         -   然后，它们会检查事件的泛型参数数量是否为2。SuiSwap的 `SwapTokenEvent`
//             (例如 `PackageID::pool::SwapTokenEvent<CoinX, CoinY>`) 的泛型参数代表了交换的两种代币类型。
//         -   这两个代币类型 (`coin_x`, `coin_y`) 被提取出来。
//         -   最后调用 `SuiswapSwapEvent::new()` 来从事件的JSON数据中解析金额和实际的交易方向。
//
// 4.  **`SuiswapSwapEvent::new()` (创建卡片 - 金额和方向解析)**:
//     -   这个构造函数接收从泛型参数中提取的两种基础代币类型 (`coin_x`, `coin_y`)，以及事件的 `parsed_json`。
//     -   它从JSON中读取以下字段：
//         -   `in_amount`: 输入金额。
//         -   `out_amount`: 输出金额。
//         -   `x_to_y`: 一个布尔值，指示交易方向。如果为 `true`，则表示 `coin_x` 是输入币，`coin_y` 是输出币；
//           反之，则 `coin_y` 是输入币，`coin_x` 是输出币。
//     -   根据 `x_to_y` 的值，将 `coin_x` 和 `coin_y` 正确地赋给 `coin_in` 和 `coin_out` 字段。
//
// 5.  **`SuiswapSwapEvent::to_swap_event()` (统一格式)**:
//     -   这个方法把SuiSwap专用的“信息卡片”(`SuiswapSwapEvent`)，转换成程序内部所有DEX事件都使用的标准“事件报告”格式 (`SwapEvent`)。
//     -   与一些其他协议类似，`pool` 字段被设置为 `None`，因为SuiSwap的 `SwapTokenEvent` 本身可能不直接包含池的ObjectID。
//
// **SuiSwap事件的特殊性 (Peculiarity of SuiSwap Event)**:
// -   **方向指示**: 交易方向 (X到Y，还是Y到X) 是通过事件JSON负载中的一个明确的布尔字段 `x_to_y` 来指示的。
// -   **泛型参数**: 事件的泛型参数列表有两个，直接对应池中的两种代币类型。
// -   **金额字段**: JSON负载中直接提供了 `in_amount` 和 `out_amount`，而不是像某些协议那样提供双向的流量金额。

// 引入标准库的 FromStr trait，用于从字符串转换。
use std::str::FromStr;

// 引入 eyre 库，用于更方便的错误处理和上下文管理。
use eyre::{ensure, eyre, OptionExt, Result};
// 引入 Move 核心类型中的 StructTag，用于解析和表示Move结构体的类型信息，特别是泛型参数。
use move_core_types::language_storage::StructTag;
// 引入 serde 库的 Deserialize trait，用于从如JSON这样的格式反序列化数据。
use serde::Deserialize;
// 引入 serde_json 的 Value 类型，用于表示通用的JSON值。
use serde_json::Value;
// 引入 shio 库的 ShioEvent 类型，这可能是与MEV相关的特定事件格式。
use shio::ShioEvent;
// 引入 Sui SDK 中的 SuiEvent 类型，代表Sui链上的标准事件。
use sui_sdk::rpc_types::SuiEvent;

// 从当前crate的根模块或utils模块引入 normalize_coin_type 函数和 Protocol、SwapEvent 类型。
use crate::{
    normalize_coin_type, // 用于将代币类型字符串规范化的函数
    types::{Protocol, SwapEvent}, // Protocol枚举 (如SuiSwap) 和通用的SwapEvent结构
};

/// `SUISWAP_SWAP_EVENT` 常量
///
/// 定义了SuiSwap DEX协议在Sui链上发出的“代币交换”事件 (`SwapTokenEvent`) 的全局唯一类型字符串。
/// 事件由 `pool` 模块发出。
pub const SUISWAP_SWAP_EVENT: &str =
    "0x361dd589b98e8fcda9a7ee53b85efabef3569d00416640d2faa516e3801d7ffc::pool::SwapTokenEvent";

/// `SuiswapSwapEvent` 结构体
///
/// 用于存储从SuiSwap的 `SwapTokenEvent` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)] // 自动派生Debug, Clone, Deserialize trait
pub struct SuiswapSwapEvent {
    pub coin_in: String,    // 输入代币的规范化类型字符串
    pub coin_out: String,   // 输出代币的规范化类型字符串
    pub amount_in: u64,     // 输入代币的数量 (通常是最小单位)
    pub amount_out: u64,    // 输出代币的数量 (通常是最小单位)
}

/// 为 `SuiswapSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
/// 这使得可以尝试将一个通用的 `&SuiEvent` 引用转换为一个 `SuiswapSwapEvent`。
impl TryFrom<&SuiEvent> for SuiswapSwapEvent {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let event_type_str = event.type_.to_string(); // 获取事件的完整类型字符串
        // 确保事件类型以 SUISWAP_SWAP_EVENT 开头。
        // SuiSwap的 `SwapTokenEvent` 有两个泛型参数，代表池中的两种代币类型 (CoinX, CoinY)。
        ensure!(
            event_type_str.starts_with(SUISWAP_SWAP_EVENT) && event.type_.type_params.len() == 2,
            "事件类型不匹配SuiSwap SwapTokenEvent的要求 (Not a SuiswapSwapEvent: type or type_params mismatch)"
        );

        // 从事件的第一个泛型参数提取代币X (coin_x) 的类型，并规范化。
        let coin_x_type_str = event.type_.type_params[0].to_string();
        let normalized_coin_x = normalize_coin_type(&coin_x_type_str);
        // 从事件的第二个泛型参数提取代币Y (coin_y) 的类型，并规范化。
        let coin_y_type_str = event.type_.type_params[1].to_string();
        let normalized_coin_y = normalize_coin_type(&coin_y_type_str);

        // 调用下面的 `new` 方法，从事件的 `parsed_json` 内容和已提取的 coin_x, coin_y 类型
        // 来解析实际的输入输出金额和方向，并创建 `SuiswapSwapEvent` 实例。
        Self::new(&event.parsed_json, normalized_coin_x, normalized_coin_y)
    }
}

/// 为 `SuiswapSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for SuiswapSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        // 从 `ShioEvent` 的 `event_type` 字符串解析出 `StructTag`。
        let event_type_tag = StructTag::from_str(&event.event_type).map_err(|e| eyre!(e))?;
        // 同样检查事件类型前缀和泛型参数数量。
        ensure!(
            event.event_type.starts_with(SUISWAP_SWAP_EVENT) && event_type_tag.type_params.len() == 2,
            "事件类型不匹配SuiSwap SwapTokenEvent的要求 (Not a SuiswapSwapEvent: type or type_params mismatch)"
        );

        // 从 `StructTag` 中提取并规范化 coin_x 和 coin_y 的类型。
        let coin_x_type_str = event_type_tag.type_params[0].to_string();
        let normalized_coin_x = normalize_coin_type(&coin_x_type_str);
        let coin_y_type_str = event_type_tag.type_params[1].to_string();
        let normalized_coin_y = normalize_coin_type(&coin_y_type_str);

        // 获取 `ShioEvent` 中的 `parsed_json`。
        let parsed_json_value = event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json field in ShioEvent)")?;

        // 调用 `new` 方法创建 `SuiswapSwapEvent` 实例。
        Self::new(parsed_json_value, normalized_coin_x, normalized_coin_y)
    }
}

impl SuiswapSwapEvent {
    /// `new` 构造函数
    ///
    /// 从已解析的JSON值 (`parsed_json`) 和池中两种基础代币类型 (`coin_x`, `coin_y`) 创建 `SuiswapSwapEvent`。
    /// SuiSwap的事件JSON结构包含 `in_amount`, `out_amount`, `x_to_y` (布尔值) 字段。
    ///
    /// 参数:
    /// - `parsed_json`: 一个对 `serde_json::Value` 的引用，代表事件中包含的JSON数据。
    /// - `coin_x`: 池中代币X的规范化类型字符串 (从事件泛型参数获取)。
    /// - `coin_y`: 池中代币Y的规范化类型字符串 (从事件泛型参数获取)。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `SuiswapSwapEvent` 实例，如果JSON中缺少关键字段或解析失败则返回错误。
    pub fn new(parsed_json: &Value, coin_x: String, coin_y: String) -> Result<Self> {
        // 从JSON中提取 "in_amount" 字段并解析为 u64。
        let amount_in_val: u64 = parsed_json["in_amount"]
            .as_str()
            .ok_or_else(|| eyre!("SuiSwap事件JSON中缺少'in_amount'字段"))?
            .parse()?;

        // 从JSON中提取 "out_amount" 字段并解析为 u64。
        let amount_out_val: u64 = parsed_json["out_amount"]
            .as_str()
            .ok_or_else(|| eyre!("SuiSwap事件JSON中缺少'out_amount'字段"))?
            .parse()?;

        // 从JSON中提取 "x_to_y" 字段 (布尔值)，表示交易方向是否为 X -> Y。
        let x_to_y_direction: bool = parsed_json["x_to_y"].as_bool().ok_or_else(|| eyre!("SuiSwap事件JSON中缺少'x_to_y'布尔字段"))?;

        // 根据 `x_to_y_direction` 的值确定实际的输入代币和输出代币。
        let (final_coin_in, final_coin_out) = if x_to_y_direction {
            (coin_x, coin_y) // 如果 x_to_y 为 true, 则 coin_x 是输入, coin_y 是输出
        } else {
            (coin_y, coin_x) // 否则, coin_y 是输入, coin_x 是输出
        };

        Ok(Self { // 返回构造好的 SuiswapSwapEvent 实例
            coin_in: final_coin_in,
            coin_out: final_coin_out,
            amount_in: amount_in_val,
            amount_out: amount_out_val,
        })
    }

    /// `to_swap_event` 异步方法
    ///
    /// 将 `SuiswapSwapEvent` 转换为通用的 `SwapEvent` 枚举成员。
    /// `async` 关键字在这里可能是为了保持与其他协议 `to_swap_event` 方法签名的一致性。
    pub async fn to_swap_event(&self) -> Result<SwapEvent> {
        Ok(SwapEvent {
            protocol: Protocol::SuiSwap, // 指明协议为SuiSwap
            pool: None,                   // SuiSwap的SwapTokenEvent不直接包含池的ObjectID，
                                          // 索引器可能需要从事件的 `sender` 字段 (即池ID) 或其他上下文来获取。
                                          // (SuiSwap's SwapTokenEvent does not directly contain the pool ObjectID;
                                          //  the indexer might need to get it from the event's `sender` field (which is the pool ID) or other context.)
            coins_in: vec![self.coin_in.clone()],
            coins_out: vec![self.coin_out.clone()],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

[end of crates/dex-indexer/src/protocols/suiswap.rs]
