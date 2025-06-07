// 该文件 `abex.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 ABEX DEX 协议相关的事件。
// ABEX 是 Sui 生态中的一个去中心化交易所。
// 这个文件的主要功能是：
// 1. 定义 ABEX Swap 事件的类型字符串 (`ABEX_SWAP_EVENT`)。
// 2. 定义 `AbexSwapEvent` 结构体，用于存储从链上事件解析出来的 ABEX 交换事件的关键信息，
//    如输入输出代币类型、输入输出金额。
// 3. 实现 `TryFrom<&SuiEvent>` 和 `TryFrom<&ShioEvent>` for `AbexSwapEvent`，
//    使得可以从通用的 `SuiEvent` (Sui SDK 定义的事件类型) 或 `ShioEvent` (可能是MEV相关的事件类型)
//    转换为特定于 ABEX 的 `AbexSwapEvent` 结构。
// 4. `AbexSwapEvent::new()` 构造函数，用于从解析后的JSON数据和代币类型创建 `AbexSwapEvent`。
// 5. `AbexSwapEvent::to_swap_event()` 方法，将 `AbexSwapEvent` 转换为一个更通用的 `SwapEvent` 枚举成员。
//    `SwapEvent` (可能在 `types.rs` 中定义) 是 `dex-indexer` 用来统一表示不同DEX协议交换事件的内部标准格式。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“翻译” ABEX 这个去中心化交易所（DEX）发生的“代币交换事件”的。
// 当有人在 ABEX 上用代币A换了代币B，Sui区块链上会记录一个“事件”。这个文件里的代码就是负责把这个原始的链上事件信息，
// 提取成我们程序自己能看懂的、标准化的格式。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别ABEX的交换事件**:
//     -   `ABEX_SWAP_EVENT` 常量定义了ABEX交换事件在Sui链上特有的“名字”（类型字符串）。
//         就像每个公司的每种表格都有一个独一无二的编号一样。
//
// 2.  **`AbexSwapEvent` 结构体 (信息卡片)**:
//     -   这个结构体像一张专门为ABEX交换事件设计的“信息卡片”。
//     -   上面记录了这次交换具体是什么（`coin_in`, `coin_out`）和多少（`amount_in`, `amount_out`）。
//
// 3.  **转换逻辑 (Translation Logic)**:
//     -   `TryFrom<&SuiEvent>` 和 `TryFrom<&ShioEvent>` 这两个部分是“翻译机”。
//         -   `SuiEvent` 是Sui官方SDK给我们的原始事件格式。
//         -   `ShioEvent` 可能是从一个叫做Shio的MEV（矿工可提取价值）相关的服务那里得到的事件格式。
//         -   这两个“翻译机”负责检查收到的事件是不是真的是ABEX的交换事件，如果是，就把里面的关键信息（比如哪两种币，各多少数量）填到 `AbexSwapEvent` 这张“信息卡片”上。
//
// 4.  **`AbexSwapEvent::new()` (创建卡片)**:
//     -   这是一个辅助小工具，用来根据已经解析好的JSON数据和代币类型，快速创建一张 `AbexSwapEvent` “信息卡片”。
//
// 5.  **`AbexSwapEvent::to_swap_event()` (统一格式)**:
//     -   这个方法把ABEX专用的“信息卡片”(`AbexSwapEvent`)，转换成一个更通用的、我们程序内部所有DEX事件都使用的标准“事件报告”格式 (`SwapEvent`)。
//         这样做的好处是，程序其他部分在处理不同DEX的交换事件时，可以用同样的方式来处理，不用为每个DEX都写一套单独的逻辑。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
//
// -   **DEX (去中心化交易所 / Decentralized Exchange)**:
//     一个允许用户直接与其他用户（或者与一个自动化的流动性池）进行点对点加密货币交易的平台，无需传统金融中介（如银行或券商）。ABEX 就是这样一个平台。
//
// -   **事件 (Event)**:
//     在Sui这样的智能合约平台上，当合约中的某些重要操作被执行时（例如，一次成功的代币交换），合约可以发出一个“事件”。
//     这个事件会被记录在区块链上，外部程序（比如我们的索引器）可以监听到这些事件，并据此做出反应。
//     `SuiEvent` 是Sui SDK中用来表示这种链上事件的标准数据结构。
//
// -   **事件类型字符串 (Event Type String)**:
//     每个Sui事件都有一个全局唯一的类型字符串，它通常的格式是 `PackageID::ModuleName::EventStructName`。
//     例如，`0xceab...::market::Swapped` 就是ABEX协议的 `market` 模块中定义的 `Swapped` 事件的类型。
//     通过这个类型字符串，我们可以准确地识别出某个事件属于哪个协议的哪种操作。
//
// -   **`StructTag` (结构标签)**:
//     这是Move语言（Sui的智能合约语言）在运行时表示一个具体结构体类型的方式。它包含了定义该结构体的地址（PackageID）、模块名、结构体名以及任何泛型类型参数。
//     在处理事件时，我们经常需要从事件的类型字符串中解析出 `StructTag`，以便提取出泛型参数（比如交换涉及的代币类型）。
//
// -   **`parsed_json` (已解析的JSON)**:
//     `SuiEvent` 对象中有一个 `parsed_json` 字段，它包含了事件发出时合约自定义的、与该事件相关的数据，这些数据以JSON格式存储。
//     例如，对于一个交换事件，`parsed_json` 里面可能就有 `source_amount` (输入金额) 和 `dest_amount` (输出金额) 这样的字段。
//
// -   **`normalize_coin_type` (代币类型规范化)**:
//     这是一个辅助函数（可能定义在 `dex-indexer` crate的根模块或 `utils` 模块）。
//     它的作用是将Sui代币的类型字符串转换为一个统一的、规范的格式。
//     例如，SUI代币本身可能有多种表示方式（如带前导零的完整地址，或简写如 "0x2::sui::SUI"），这个函数会把它们都统一成官方标准的那一种。
//     这对于后续的比较和作为HashMap的键等操作非常重要。

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
    types::{Protocol, SwapEvent}, // Protocol枚举 (如Abex, Cetus等) 和通用的SwapEvent结构
};

/// `ABEX_SWAP_EVENT` 常量
///
/// 定义了ABEX DEX协议在Sui链上发出的“交换完成”事件的全局唯一类型字符串。
/// 格式通常是 `PackageID::ModuleName::EventStructName`。
/// 这个字符串用于从一堆Sui事件中准确识别出哪些是ABEX的交换事件。
pub const ABEX_SWAP_EVENT: &str = "0xceab84acf6bf70f503c3b0627acaff6b3f84cee0f2d7ed53d00fa6c2a168d14f::market::Swapped";

/// `AbexSwapEvent` 结构体
///
/// 用于存储从ABEX的 `Swapped` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)] // 自动派生Debug, Clone, Deserialize trait
                                     // Debug: 方便调试打印。
                                     // Clone: 允许创建此结构体的副本。
                                     // Deserialize: 允许从如JSON这样的格式反序列化填充此结构体 (虽然这里主要是手动填充)。
pub struct AbexSwapEvent {
    pub coin_in: String,    // 输入代币的规范化类型字符串
    pub coin_out: String,   // 输出代币的规范化类型字符串
    pub amount_in: u64,     // 输入代币的数量 (通常是最小单位)
    pub amount_out: u64,    // 输出代币的数量 (通常是最小单位)
}

/// 为 `AbexSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
/// 这使得我们可以尝试将一个通用的 `&SuiEvent` 引用转换为一个 `AbexSwapEvent`。
/// 转换过程包括：
/// 1. 检查事件类型是否匹配 `ABEX_SWAP_EVENT`。
/// 2. 检查事件类型是否包含两个泛型参数 (代表两种代币)。
/// 3. 提取并规范化这两个泛型代币类型。
/// 4. 从事件的 `parsed_json` 字段中解析出输入和输出金额。
impl TryFrom<&SuiEvent> for AbexSwapEvent {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let event_type_str = event.type_.to_string(); // 获取事件的完整类型字符串
        // 确保事件类型以 ABEX_SWAP_EVENT 开头，并且事件的泛型参数列表 (event.type_.type_params) 恰好有两个元素。
        // 这两个泛型参数通常代表了交换中涉及的两种代币类型。
        ensure!(
            event_type_str.starts_with(ABEX_SWAP_EVENT) && event.type_.type_params.len() == 2,
            "事件类型不匹配ABEX SwapEvent的要求 (Not an AbexSwapEvent: type mismatch or wrong number of type_params)" // 如果不匹配，则返回错误
        );

        // 提取第一个泛型参数作为输入代币类型，并进行规范化。
        let coin_in_type_str = event.type_.type_params[0].to_string();
        let normalized_coin_in = normalize_coin_type(&coin_in_type_str);
        // 提取第二个泛型参数作为输出代币类型，并进行规范化。
        let coin_out_type_str = event.type_.type_params[1].to_string();
        let normalized_coin_out = normalize_coin_type(&coin_out_type_str);

        // 调用下面的 `new` 方法，从事件的 `parsed_json` 内容和已提取的代币类型创建 `AbexSwapEvent` 实例。
        Self::new(&event.parsed_json, normalized_coin_in, normalized_coin_out)
    }
}

/// 为 `AbexSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
/// 这使得我们可以尝试将一个 `&ShioEvent` (可能是MEV相关的事件封装) 转换为 `AbexSwapEvent`。
/// 转换逻辑与 `TryFrom<&SuiEvent>` 类似，但数据来源是 `ShioEvent` 的字段。
impl TryFrom<&ShioEvent> for AbexSwapEvent {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &ShioEvent) -> Result<Self> {
        // 从 `ShioEvent` 的 `event_type` 字符串解析出 `StructTag`。
        // `StructTag` 提供了对Move结构体类型（包括其泛型参数）的结构化访问。
        let event_type_tag = StructTag::from_str(&event.event_type).map_err(|e| eyre!(e))?;
        // 确保事件类型以 ABEX_SWAP_EVENT 开头，并且泛型参数数量为2。
        ensure!(
            event.event_type.starts_with(ABEX_SWAP_EVENT) && event_type_tag.type_params.len() == 2,
            "事件类型不匹配ABEX SwapEvent的要求 (Not an AbexSwapEvent: type mismatch or wrong number of type_params)"
        );

        // 从 `StructTag` 中提取并规范化输入和输出代币类型。
        let coin_in_type_str = event_type_tag.type_params[0].to_string();
        let normalized_coin_in = normalize_coin_type(&coin_in_type_str);
        let coin_out_type_str = event_type_tag.type_params[1].to_string();
        let normalized_coin_out = normalize_coin_type(&coin_out_type_str);

        // `ShioEvent` 中的 `parsed_json` 是一个 `Option<Value>`，
        // 使用 `ok_or_eyre` 确保它存在，否则返回错误。
        let parsed_json_value = event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json field in ShioEvent)")?;

        // 调用 `new` 方法创建 `AbexSwapEvent` 实例。
        Self::new(parsed_json_value, normalized_coin_in, normalized_coin_out)
    }
}

impl AbexSwapEvent {
    /// `new` 构造函数
    ///
    /// 从已解析的JSON值 (`parsed_json`) 和规范化的输入/输出代币类型字符串创建 `AbexSwapEvent`。
    ///
    /// 参数:
    /// - `parsed_json`: 一个对 `serde_json::Value` 的引用，代表事件中包含的JSON数据。
    /// - `coin_in`: 输入代币的规范化类型字符串。
    /// - `coin_out`: 输出代币的规范化类型字符串。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `AbexSwapEvent` 实例，如果JSON中缺少关键字段或解析失败则返回错误。
    pub fn new(parsed_json: &Value, coin_in: String, coin_out: String) -> Result<Self> {
        // 从JSON中提取 "source_amount" 字段，期望它是一个字符串，然后将其解析为 u64 类型的输入金额。
        // `ok_or_else` 用于在字段缺失或类型不匹配时提供自定义错误。
        // `parse::<u64>()?` 将字符串解析为u64，`?` 用于传播可能的解析错误。
        let amount_in: u64 = parsed_json["source_amount"] // 访问JSON字段
            .as_str() // 尝试将其转换为字符串切片
            .ok_or_else(|| eyre!("ABEX事件JSON中缺少'source_amount'字段或其非字符串类型 (Missing 'source_amount' field or not a string in ABEX event JSON)"))?
            .parse()?; // 解析字符串为u64

        // 类似地，提取 "dest_amount" 字段作为输出金额。
        let amount_out: u64 = parsed_json["dest_amount"]
            .as_str()
            .ok_or_else(|| eyre!("ABEX事件JSON中缺少'dest_amount'字段或其非字符串类型 (Missing 'dest_amount' field or not a string in ABEX event JSON)"))?
            .parse()?;

        Ok(Self { // 返回构造好的 AbexSwapEvent 实例
            coin_in,
            coin_out,
            amount_in,
            amount_out,
        })
    }

    /// `to_swap_event` 异步方法
    ///
    /// 将 `AbexSwapEvent` 转换为通用的 `SwapEvent` 枚举成员。
    /// `SwapEvent` 是 `dex-indexer` 内部用于统一表示不同协议交换事件的格式。
    ///
    /// 返回:
    /// - `Result<SwapEvent>`: 转换后的 `SwapEvent::Abex`。
    ///   (注意: 当前实现不是异步的，但标记为 `async fn` 可能是为了与其他协议的 `to_swap_event` 保持签名一致，
    ///    或者为未来可能需要异步操作（如查询额外信息）做准备。)
    ///   (实际上，根据当前代码，`async` 关键字在这里不是必需的，因为没有 `.await` 调用。)
    pub async fn to_swap_event(&self) -> Result<SwapEvent> {
        Ok(SwapEvent {
            protocol: Protocol::Abex, // 指明协议为Abex
            pool: None,               // ABEX的Swap事件通常不直接包含池的ObjectID，所以这里是None。
                                      // 池的识别可能需要依赖代币对或其他上下文信息。
            coins_in: vec![self.coin_in.clone()],   // 输入代币列表 (只包含一个)
            coins_out: vec![self.coin_out.clone()], // 输出代币列表 (只包含一个)
            amounts_in: vec![self.amount_in],       // 输入金额列表
            amounts_out: vec![self.amount_out],     // 输出金额列表
        })
    }
}

[end of crates/dex-indexer/src/protocols/abex.rs]
