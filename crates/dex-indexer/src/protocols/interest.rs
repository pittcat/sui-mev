// 该文件 `interest.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 Interest Protocol (可能是一个借贷或生息类DEX协议) 相关的交换事件。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“翻译” Interest Protocol 这个DEX发生的“代币交换事件”的。
// 当用户在 Interest Protocol 上用代币A换了代币B，Sui区块链上会记录一个“事件”。这个文件里的代码就是负责把这个原始的链上事件信息，
// 提取成我们程序自己能看懂的、标准化的格式。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别Interest Protocol的交换事件**:
//     -   `INTEREST_SWAP_EVENT` 常量定义了其交换事件在Sui链上特有的“名字”（类型字符串）。
//         注意其事件名为 `core::SwapToken`，这暗示了其核心逻辑可能在 `core` 模块中。
//
// 2.  **`InterestSwapEvent` 结构体 (信息卡片)**:
//     -   这个结构体用于存储从Interest Protocol的 `SwapToken` 事件中解析出来的关键交换信息：
//         输入输出代币类型 (`coin_in`, `coin_out`) 和对应的金额 (`amount_in`, `amount_out`)。
//
// 3.  **转换逻辑 (Translation Logic)**:
//     -   `TryFrom<&SuiEvent>` 和 `TryFrom<&ShioEvent>`: 这两个“翻译机”负责检查收到的事件是否真的是Interest Protocol的交换事件。
//         -   它们首先会检查事件类型字符串是否以 `INTEREST_SWAP_EVENT` 开头。
//         -   然后，它们会检查事件的泛型参数数量是否为3。Interest Protocol的 `SwapToken` 事件
//             (例如 `PackageID::core::SwapToken<PoolType, CoinX, CoinY>`) 的泛型参数中，
//             第二个 (`type_params[1]`) 和第三个 (`type_params[2]`) 通常代表了交换的两种代币类型。
//         -   一个关键的判断是交易方向：事件类型字符串中是否包含 "SwapTokenX"。
//             -   如果包含 "SwapTokenX" (例如 `core::SwapTokenXtoY<...>`)，则认为方向是 X -> Y。
//             -   否则 (例如 `core::SwapTokenYtoX<...>`)，则认为是 Y -> X。
//         -   根据这个方向 (`x_to_y`)，将泛型参数中的 `coin_x` 和 `coin_y` 正确地赋给 `coin_in` 和 `coin_out`。
//         -   最后调用 `InterestSwapEvent::new()` 来从事件的JSON数据中解析金额。
//
// 4.  **`InterestSwapEvent::new()` (创建卡片 - 金额解析)**:
//     -   这个构造函数接收已确定的 `coin_in`, `coin_out` 类型以及交易方向 `x_to_y`。
//     -   它根据 `x_to_y` 的值，从事件的 `parsed_json` 中读取正确的金额字段：
//         -   如果 `x_to_y` 为 `true` (X是输入, Y是输出):
//             -   `amount_in` 从 `parsed_json["coin_x_in"]` 获取。
//             -   `amount_out` 从 `parsed_json["coin_y_out"]` 获取。
//         -   如果 `x_to_y` 为 `false` (Y是输入, X是输出):
//             -   `amount_in` 从 `parsed_json["coin_y_in"]` 获取。
//             -   `amount_out` 从 `parsed_json["coin_x_out"]` 获取。
//
// 5.  **`InterestSwapEvent::to_swap_event()` (统一格式)**:
//     -   这个方法把Interest Protocol专用的“信息卡片”(`InterestSwapEvent`)，转换成程序内部所有DEX事件都使用的标准“事件报告”格式 (`SwapEvent`)。
//     -   和一些其他协议类似，`pool` 字段被设置为 `None`，因为事件本身可能不直接包含池的ObjectID。
//
// **Interest Protocol事件的特殊性 (Peculiarity of Interest Protocol Event)**:
// -   **方向推断**: 交易方向 (X到Y，还是Y到X) 是通过检查事件类型字符串中是否包含特定的子串 ("SwapTokenX") 来间接推断的，
//     而不是像某些DEX事件那样有一个明确的布尔字段 (如 `a2b` 或 `x_for_y`)。
// -   **泛型参数**: 事件的泛型参数列表有3个，其中第0个可能是代表池子本身的类型，而第1和第2个代表交易的两种代币。
// -   **金额字段**: JSON负载中有针对 `coin_x` 和 `coin_y` 各自的输入和输出金额字段 (`coin_x_in`, `coin_y_out` 或 `coin_y_in`, `coin_x_out`)，
//     需要根据推断出的交易方向来选择正确的字段作为 `amount_in` 和 `amount_out`。

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
    types::{Protocol, SwapEvent}, // Protocol枚举 (如Interest) 和通用的SwapEvent结构
};

/// `INTEREST_SWAP_EVENT` 常量
///
/// 定义了Interest Protocol在Sui链上发出的“代币交换”事件的类型字符串的公共前缀。
/// 实际的事件类型可能会在此基础上附加泛型参数，例如 `...SwapTokenXtoY<PoolType, CoinX, CoinY>`。
pub const INTEREST_SWAP_EVENT: &str =
    "0x5c45d10c26c5fb53bfaff819666da6bc7053d2190dfa29fec311cc666ff1f4b0::core::SwapToken";

/// `InterestSwapEvent` 结构体
///
/// 用于存储从Interest Protocol的 `SwapToken` (或其变体如 `SwapTokenXtoY`) 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)] // 自动派生Debug, Clone, Deserialize trait
pub struct InterestSwapEvent {
    pub coin_in: String,    // 输入代币的规范化类型字符串
    pub coin_out: String,   // 输出代币的规范化类型字符串
    pub amount_in: u64,     // 输入代币的数量 (通常是最小单位)
    pub amount_out: u64,    // 输出代币的数量 (通常是最小单位)
}

/// 为 `InterestSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
/// 这使得可以尝试将一个通用的 `&SuiEvent` 引用转换为一个 `InterestSwapEvent`。
impl TryFrom<&SuiEvent> for InterestSwapEvent {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let event_type_str = event.type_.to_string(); // 获取事件的完整类型字符串
        // 确保事件类型以 INTEREST_SWAP_EVENT 开头。
        // Interest Protocol的SwapToken事件有三个泛型参数：
        // 第一个是池类型 (PoolType)，第二个是代币X (CoinX)，第三个是代币Y (CoinY)。
        ensure!(
            event_type_str.starts_with(INTEREST_SWAP_EVENT) && event.type_.type_params.len() == 3,
            "事件类型不匹配Interest SwapToken的要求 (Not an InterestSwapEvent: type mismatch or wrong number of type_params)"
        );

        // 通过检查事件类型字符串中是否包含 "SwapTokenX" 来判断交易方向。
        // 例如，`core::SwapTokenXtoY` 表示 X -> Y，`core::SwapTokenYtoX` 表示 Y -> X。
        let x_to_y_direction = event_type_str.contains("SwapTokenX");

        // 从泛型参数中提取 coin_x 和 coin_y 的类型。
        // 注意索引：type_params[0] 是 PoolType，type_params[1] 是 CoinX，type_params[2] 是 CoinY。
        let coin_x_type_str = event.type_.type_params[1].to_string();
        let normalized_coin_x = normalize_coin_type(&coin_x_type_str);
        let coin_y_type_str = event.type_.type_params[2].to_string();
        let normalized_coin_y = normalize_coin_type(&coin_y_type_str);

        // 根据推断出的交易方向 `x_to_y_direction`，确定实际的输入代币和输出代币。
        let (final_coin_in, final_coin_out) = if x_to_y_direction {
            (normalized_coin_x, normalized_coin_y) // X是输入, Y是输出
        } else {
            (normalized_coin_y, normalized_coin_x) // Y是输入, X是输出
        };

        // 调用下面的 `new` 方法，从事件的 `parsed_json` 内容、已确定的输入/输出代币类型和方向，
        // 来解析输入和输出金额，并创建 `InterestSwapEvent` 实例。
        Self::new(&event.parsed_json, final_coin_in, final_coin_out, x_to_y_direction)
    }
}

/// 为 `InterestSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for InterestSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        // 从 `ShioEvent` 的 `event_type` 字符串解析出 `StructTag`。
        let event_type_tag = StructTag::from_str(&event.event_type).map_err(|e| eyre!(e))?;
        // 同样检查事件类型前缀和泛型参数数量。
        ensure!(
            event.event_type.starts_with(INTEREST_SWAP_EVENT) && event_type_tag.type_params.len() == 3,
            "事件类型不匹配Interest SwapToken的要求 (Not an InterestSwapEvent: type mismatch or wrong number of type_params)"
        );

        // 判断交易方向
        let x_to_y_direction = event.event_type.contains("SwapTokenX");

        // 从 `StructTag` 中提取并规范化 coin_x 和 coin_y 的类型。
        let coin_x_type_str = event_type_tag.type_params[1].to_string();
        let normalized_coin_x = normalize_coin_type(&coin_x_type_str);
        let coin_y_type_str = event_type_tag.type_params[2].to_string();
        let normalized_coin_y = normalize_coin_type(&coin_y_type_str);

        // 确定实际的输入和输出代币类型
        let (final_coin_in, final_coin_out) = if x_to_y_direction {
            (normalized_coin_x, normalized_coin_y)
        } else {
            (normalized_coin_y, normalized_coin_x)
        };

        // 获取 `ShioEvent` 中的 `parsed_json`。
        let parsed_json_value = event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json field in ShioEvent)")?;

        // 调用 `new` 方法创建 `InterestSwapEvent` 实例。
        Self::new(parsed_json_value, final_coin_in, final_coin_out, x_to_y_direction)
    }
}

impl InterestSwapEvent {
    /// `new` 构造函数
    ///
    /// 从已解析的JSON值 (`parsed_json`)、确定的输入/输出代币类型和交易方向 (`x_to_y`) 创建 `InterestSwapEvent`。
    /// 它根据 `x_to_y` 的值从JSON中读取正确的金额字段。
    ///
    /// 参数:
    /// - `parsed_json`: 一个对 `serde_json::Value` 的引用，代表事件中包含的JSON数据。
    /// - `coin_in`: 输入代币的规范化类型字符串。
    /// - `coin_out`: 输出代币的规范化类型字符串。
    /// - `x_to_y`: 一个布尔值，`true` 表示交易方向是 X -> Y，`false` 表示 Y -> X。
    ///             (这里的X和Y对应于事件泛型参数中的 `coin_x` 和 `coin_y`)
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `InterestSwapEvent` 实例，如果JSON中缺少关键字段或解析失败则返回错误。
    pub fn new(parsed_json: &Value, coin_in: String, coin_out: String, x_to_y: bool) -> Result<Self> {
        // 根据交易方向 `x_to_y`，确定从JSON中读取哪些字段作为输入金额。
        let amount_in_val: u64 = if x_to_y { // 如果是 X -> Y
            parsed_json["coin_x_in"] // 输入金额是 coin_x_in
                .as_str()
                .ok_or_else(|| eyre!("Interest事件JSON中缺少'coin_x_in'字段 (direction XtoY)"))?
                .parse()?
        } else { // 如果是 Y -> X
            parsed_json["coin_y_in"] // 输入金额是 coin_y_in
                .as_str()
                .ok_or_else(|| eyre!("Interest事件JSON中缺少'coin_y_in'字段 (direction YtoX)"))?
                .parse()?
        };

        // 根据交易方向 `x_to_y`，确定从JSON中读取哪些字段作为输出金额。
        let amount_out_val: u64 = if x_to_y { // 如果是 X -> Y
            parsed_json["coin_y_out"] // 输出金额是 coin_y_out
                .as_str()
                .ok_or_else(|| eyre!("Interest事件JSON中缺少'coin_y_out'字段 (direction XtoY)"))?
                .parse()?
        } else { // 如果是 Y -> X
            parsed_json["coin_x_out"] // 输出金额是 coin_x_out
                .as_str()
                .ok_or_else(|| eyre!("Interest事件JSON中缺少'coin_x_out'字段 (direction YtoX)"))?
                .parse()?
        };

        Ok(Self { // 返回构造好的 InterestSwapEvent 实例
            coin_in,
            coin_out,
            amount_in: amount_in_val,
            amount_out: amount_out_val,
        })
    }

    /// `to_swap_event` 异步方法
    ///
    /// 将 `InterestSwapEvent` 转换为通用的 `SwapEvent` 枚举成员。
    /// `async` 关键字在这里可能是为了保持与其他协议 `to_swap_event` 方法签名的一致性。
    pub async fn to_swap_event(&self) -> Result<SwapEvent> {
        Ok(SwapEvent {
            protocol: Protocol::Interest, // 指明协议为Interest Protocol
            pool: None,                   // Interest Protocol的SwapToken事件不直接包含池的ObjectID，
                                          // 或者池的识别依赖于事件类型中的第一个泛型参数 (PoolType)，这里未传递。
            coins_in: vec![self.coin_in.clone()],
            coins_out: vec![self.coin_out.clone()],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

[end of crates/dex-indexer/src/protocols/interest.rs]
