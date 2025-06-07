// 该文件 `babyswap.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 BabySwap DEX 协议相关的事件。
// BabySwap 是 Sui 生态中的一个去中心化交易所。
// 这个文件的主要功能是：
// 1. 定义 BabySwap 交换事件的类型字符串 (`BABY_SWAP_EVENT`)。
// 2. 定义 `BabySwapEvent` 结构体，用于存储从链上事件解析出来的 BabySwap 交换事件的关键信息。
// 3. 实现 `TryFrom<&SuiEvent>` 和 `TryFrom<&ShioEvent>` for `BabySwapEvent`，
//    使得可以从通用的 `SuiEvent` 或 `ShioEvent` 转换为特定于 BabySwap 的 `BabySwapEvent` 结构。
// 4. `BabySwapEvent::new()` 构造函数，根据事件的JSON数据和池中两种代币的类型 (coin_x, coin_y)
//    来确定实际的输入输出方向和金额，并创建 `BabySwapEvent`。
// 5. `BabySwapEvent::to_swap_event()` 方法，将 `BabySwapEvent` 转换为一个更通用的 `SwapEvent` 枚举成员，
//    这是 `dex-indexer` 用来统一表示不同DEX协议交换事件的内部标准格式。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“翻译” BabySwap 这个去中心化交易所（DEX）发生的“代币交换事件”的。
// 当用户在 BabySwap 上用代币A换了代币B，Sui区块链上会记录一个“事件”。这个文件里的代码就是负责把这个原始的链上事件信息，
// 提取成我们程序自己能看懂的、标准化的格式。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别BabySwap的交换事件**:
//     -   `BABY_SWAP_EVENT` 常量定义了BabySwap交换事件在Sui链上特有的“名字”（类型字符串）。
//
// 2.  **`BabySwapEvent` 结构体 (信息卡片)**:
//     -   这个结构体像一张专门为BabySwap交换事件设计的“信息卡片”。
//     -   上面记录了这次交换具体是什么（`coin_in`, `coin_out`）和多少（`amount_in`, `amount_out`）。
//
// 3.  **转换逻辑 (Translation Logic)**:
//     -   `TryFrom<&SuiEvent>` 和 `TryFrom<&ShioEvent>`: 这两个“翻译机”负责检查收到的事件是不是真的是BabySwap的交换事件。
//         如果是，它们会从事件的泛型参数中提取出池子里的两种基础代币类型（`coin_x`, `coin_y`），
//         然后调用 `BabySwapEvent::new()` 来进一步处理。
//         BabySwap的事件似乎有三个泛型参数，前两个是池中的代币类型 (CoinX, CoinY)，第三个可能是LP代币类型。
//
// 4.  **`BabySwapEvent::new()` (创建卡片 - 包含方向判断)**:
//     -   这个构造函数比较特别。BabySwap的原始事件数据 (`parsed_json`) 可能同时提供了 `x_in`, `x_out`, `y_in`, `y_out` 四个字段。
//         这意味着它记录了代币X流入/流出的数量，以及代币Y流入/流出的数量。
//     -   `new()` 函数通过判断 `x_in` 是否为0来确定交易方向：
//         -   如果 `x_in` 为0，说明代币X没有作为输入（即它是输出），那么实际的输入就是代币Y (`coin_y`)，输入金额是 `y_in`；
//             对应的输出就是代币X (`coin_x`)，输出金额是 `x_out`。
//         -   否则 (如果 `x_in` 不为0)，说明代币X是输入，输入金额是 `x_in`；
//             对应的输出就是代币Y (`coin_y`)，输出金额是 `y_out`。
//     -   这样，它就能准确地填充 `BabySwapEvent` 中的 `coin_in`, `coin_out`, `amount_in`, `amount_out` 字段。
//
// 5.  **`BabySwapEvent::to_swap_event()` (统一格式)**:
//     -   这个方法把BabySwap专用的“信息卡片”(`BabySwapEvent`)，转换成程序内部所有DEX事件都使用的标准“事件报告”格式 (`SwapEvent`)。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
// (与 abex.rs 文件中的解释类似，这里不再重复。主要区别在于BabySwap事件的字段结构和泛型参数数量。)
// -   **DEX (去中心化交易所)**
// -   **事件 (Event)**
// -   **事件类型字符串 (Event Type String)**
// -   **`StructTag` (结构标签)**
// -   **`parsed_json` (已解析的JSON)**
// -   **`normalize_coin_type` (代币类型规范化)**
//
// **BabySwap事件的特殊性 (Peculiarity of BabySwap Event)**:
// BabySwap的 `EventSwap` 事件结构 (`parsed_json` 包含 `x_in`, `x_out`, `y_in`, `y_out`) 表明它可能采用了
// 一种记录双向流量的方式，或者其底层池子实现与某些单向流量记录的DEX不同。
// `BabySwapEvent::new()` 中的逻辑是关键，它通过检查哪个输入量 (`x_in` 或 `y_in`) 不为零来确定实际的交易方向。
// 另外，其事件类型 `PackageID::liquidity_pool::EventSwap<CoinX, CoinY, LpCoin>` 包含三个泛型参数，
// 前两个是交易对的代币，第三个可能是该池的LP代币类型，这在解析时需要注意。

// 引入标准库的 FromStr trait，用于从字符串转换。
use std::str::FromStr;

// 引入 eyre 库，用于更方便的错误处理和上下文管理。
use eyre::{ensure, eyre, OptionExt, Result};
// 引入 Move 核心类型中的 StructTag，用于解析和表示Move结构体的类型信息，特别是泛型参数。
use move_core_types::language_storage::StructTag;
// 引入 serde 和 serde_json，用于JSON数据的序列化和反序列化。
use serde::Deserialize;
use serde_json::Value;
// 引入 shio 库的 ShioEvent 类型，这可能是与MEV相关的特定事件格式。
use shio::ShioEvent;
// 引入 Sui SDK 中的 SuiEvent 类型，代表Sui链上的标准事件。
use sui_sdk::rpc_types::SuiEvent;

// 从当前crate的根模块或utils模块引入 normalize_coin_type 函数和 Protocol、SwapEvent 类型。
use crate::{
    normalize_coin_type, // 用于将代币类型字符串规范化的函数
    types::{Protocol, SwapEvent}, // Protocol枚举 (如BabySwap) 和通用的SwapEvent结构
};

/// `BABY_SWAP_EVENT` 常量
///
/// 定义了BabySwap DEX协议在Sui链上发出的“交换完成”事件的全局唯一类型字符串。
/// 格式为 `PackageID::ModuleName::EventStructName<GenericTypeParams...>`。
/// 这个字符串用于从Sui事件流中准确识别出BabySwap的交换事件。
pub const BABY_SWAP_EVENT: &str =
    "0x227f865230dd4fc947321619f56fee37dc7ac582eb22e3eab29816f717512d9d::liquidity_pool::EventSwap";

/// `BabySwapEvent` 结构体
///
/// 用于存储从BabySwap的 `EventSwap` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)] // 自动派生Debug, Clone, Deserialize trait
pub struct BabySwapEvent {
    pub coin_in: String,    // 输入代币的规范化类型字符串
    pub coin_out: String,   // 输出代币的规范化类型字符串
    pub amount_in: u64,     // 输入代币的数量 (通常是最小单位)
    pub amount_out: u64,    // 输出代币的数量 (通常是最小单位)
}

/// 为 `BabySwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
/// 这使得可以尝试将一个通用的 `&SuiEvent` 引用转换为一个 `BabySwapEvent`。
impl TryFrom<&SuiEvent> for BabySwapEvent {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let event_type_str = event.type_.to_string(); // 获取事件的完整类型字符串
        // 确保事件类型以 BABY_SWAP_EVENT 开头。
        // BabySwap的 `EventSwap` 事件有三个泛型参数：`CoinX`, `CoinY`, `LPCoin` (LP代币类型)。
        // 我们关心的是前两个代币类型。
        ensure!(
            event_type_str.starts_with(BABY_SWAP_EVENT) && event.type_.type_params.len() == 3,
            "事件类型不匹配BabySwap EventSwap的要求 (Not a BabySwapEvent: type mismatch or wrong number of type_params)"
        );

        // 提取第一个泛型参数作为代币X (coin_x) 的类型，并规范化。
        let coin_x_type_str = event.type_.type_params[0].to_string();
        let normalized_coin_x = normalize_coin_type(&coin_x_type_str);
        // 提取第二个泛型参数作为代币Y (coin_y) 的类型，并规范化。
        let coin_y_type_str = event.type_.type_params[1].to_string();
        let normalized_coin_y = normalize_coin_type(&coin_y_type_str);
        // 第三个泛型参数是LP代币类型，这里不直接使用。

        // 调用下面的 `new` 方法，从事件的 `parsed_json` 内容和已提取的 coin_x, coin_y 类型创建 `BabySwapEvent` 实例。
        Self::new(&event.parsed_json, normalized_coin_x, normalized_coin_y)
    }
}

/// 为 `BabySwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
/// 这使得可以尝试将一个 `&ShioEvent` (可能是MEV相关的事件封装) 转换为 `BabySwapEvent`。
impl TryFrom<&ShioEvent> for BabySwapEvent {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &ShioEvent) -> Result<Self> {
        // 从 `ShioEvent` 的 `event_type` 字符串解析出 `StructTag`。
        let event_type_tag = StructTag::from_str(&event.event_type).map_err(|e| eyre!(e))?;
        // 同样检查事件类型和泛型参数数量。
        ensure!(
            event.event_type.starts_with(BABY_SWAP_EVENT) && event_type_tag.type_params.len() == 3,
            "事件类型不匹配BabySwap EventSwap的要求 (Not a BabySwapEvent: type mismatch or wrong number of type_params)"
        );

        // 从 `StructTag` 中提取并规范化 coin_x 和 coin_y 的类型。
        let coin_x_type_str = event_type_tag.type_params[0].to_string();
        let normalized_coin_x = normalize_coin_type(&coin_x_type_str);
        let coin_y_type_str = event_type_tag.type_params[1].to_string();
        let normalized_coin_y = normalize_coin_type(&coin_y_type_str);

        // 获取 `ShioEvent` 中的 `parsed_json`。
        let parsed_json_value = event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json field in ShioEvent)")?;

        // 调用 `new` 方法创建 `BabySwapEvent` 实例。
        Self::new(parsed_json_value, normalized_coin_x, normalized_coin_y)
    }
}

impl BabySwapEvent {
    /// `new` 构造函数
    ///
    /// 从已解析的JSON值 (`parsed_json`) 和池中两种基础代币类型 (`coin_x`, `coin_y`) 创建 `BabySwapEvent`。
    /// BabySwap的事件JSON结构包含 `x_in`, `x_out`, `y_in`, `y_out` 字段，
    /// 需要根据这些字段的值来判断实际的交易方向 (是X换Y还是Y换X)。
    ///
    /// 参数:
    /// - `parsed_json`: 一个对 `serde_json::Value` 的引用，代表事件中包含的JSON数据。
    /// - `coin_x`: 池中代币X的规范化类型字符串。
    /// - `coin_y`: 池中代币Y的规范化类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `BabySwapEvent` 实例，如果JSON中缺少关键字段或解析失败则返回错误。
    pub fn new(parsed_json: &Value, coin_x: String, coin_y: String) -> Result<Self> {
        // 从JSON中提取 x_in, x_out, y_in, y_out 字段的值。
        // 这些字段记录了代币X和代币Y分别流入和流出池子的数量。
        let x_in_amount: u64 = parsed_json["x_in"] // 访问 "x_in" 字段
            .as_str() // 期望为字符串形式的数字
            .ok_or_else(|| eyre!("BabySwap事件JSON中缺少'x_in'字段"))?
            .parse()?; // 解析为u64

        let x_out_amount: u64 = parsed_json["x_out"]
            .as_str()
            .ok_or_else(|| eyre!("BabySwap事件JSON中缺少'x_out'字段"))?
            .parse()?;

        let y_in_amount: u64 = parsed_json["y_in"]
            .as_str()
            .ok_or_else(|| eyre!("BabySwap事件JSON中缺少'y_in'字段"))?
            .parse()?;

        let y_out_amount: u64 = parsed_json["y_out"]
            .as_str()
            .ok_or_else(|| eyre!("BabySwap事件JSON中缺少'y_out'字段"))?
            .parse()?;

        // 判断交易方向：
        // 如果 x_in 为 0，意味着代币X没有作为输入（即它是输出或无变动），那么实际的输入是代币Y。
        // 此时，coin_in = coin_y, amount_in = y_in_amount, coin_out = coin_x, amount_out = x_out_amount。
        // 否则，如果 x_in 不为0，意味着代币X是输入。
        // 此时，coin_in = coin_x, amount_in = x_in_amount, coin_out = coin_y, amount_out = y_out_amount。
        let (final_coin_in, final_coin_out, final_amount_in, final_amount_out) = if x_in_amount == 0 {
            // Y是输入, X是输出
            (coin_y, coin_x, y_in_amount, x_out_amount)
        } else {
            // X是输入, Y是输出
            (coin_x, coin_y, x_in_amount, y_out_amount)
        };

        Ok(Self { // 返回构造好的 BabySwapEvent 实例
            coin_in: final_coin_in,
            coin_out: final_coin_out,
            amount_in: final_amount_in,
            amount_out: final_amount_out,
        })
    }

    /// `to_swap_event` 异步方法
    ///
    /// 将 `BabySwapEvent` 转换为通用的 `SwapEvent` 枚举成员。
    /// `async` 关键字在这里可能是为了保持与其他协议 `to_swap_event` 方法签名的一致性，
    /// 尽管当前实现中没有 `.await` 调用。
    pub async fn to_swap_event(&self) -> Result<SwapEvent> {
        Ok(SwapEvent {
            protocol: Protocol::BabySwap, // 指明协议为BabySwap
            pool: None,                   // BabySwap事件通常不直接包含池的ObjectID，所以为None。
                                          // 池的识别可能需要通过其他方式，如订阅特定池的事件或从交易效果中关联。
            coins_in: vec![self.coin_in.clone()],   // 输入代币列表 (只包含一个)
            coins_out: vec![self.coin_out.clone()], // 输出代币列表 (只包含一个)
            amounts_in: vec![self.amount_in],       // 输入金额列表
            amounts_out: vec![self.amount_out],     // 输出金额列表
        })
    }
}

[end of crates/dex-indexer/src/protocols/babyswap.rs]
