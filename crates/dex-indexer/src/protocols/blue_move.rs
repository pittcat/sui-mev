// 该文件 `blue_move.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 BlueMove DEX 协议相关的事件和数据结构。
// BlueMove 最初是Sui上的一个NFT市场，后来也加入了DEX功能。
// 这个文件的主要功能是：
// 1. 定义 BlueMove "池创建" (`Created_Pool_Event`) 和 "交换" (`Swap_Event`) 事件的类型字符串常量。
// 2. 提供 `blue_move_event_filter()` 函数，用于创建Sui事件订阅的过滤器，专门监听BlueMove的池创建事件。
// 3. 定义 `BlueMovePoolCreated` 结构体，用于存储从链上 `Created_Pool_Event` 事件解析出来的池创建信息。
// 4. 实现 `TryFrom<&SuiEvent>` for `BlueMovePoolCreated`。
// 5. `BlueMovePoolCreated::to_pool()` 方法，将解析出的事件数据转换为通用的 `Pool` 结构。
// 6. 定义 `BlueMoveSwapEvent` 结构体，用于存储从链上 `Swap_Event` 事件解析出来的交换信息。
// 7. 实现 `TryFrom<&SuiEvent>`、`TryFrom<&ShioEvent>` 和 `TryFrom<&Value>` for `BlueMoveSwapEvent`。
//    特别地，`TryFrom<&Value>` (即 `BlueMoveSwapEvent::new` 的实际调用者) 中包含了从事件JSON中提取
//    `token_x_in`, `amount_x_in`, `token_y_in`, `amount_y_in`, `token_x_out`, `amount_x_out`, `token_y_out`, `amount_y_out`
//    这些字段，并根据哪个输入金额 (`amount_x_in` 或 `amount_y_in`) 大于0来判断实际的交易方向。
// 8. `BlueMoveSwapEvent::to_swap_event()` 方法，将 `BlueMoveSwapEvent` 转换为通用的 `SwapEvent`。
// 9. `blue_move_related_object_ids()` 函数，返回与BlueMove协议相关的核心对象ID列表 (硬编码 + 动态派生)。
//    它还包括一个 `BLUE_MOVE_DEX_INFO` 常量，以及通过 `derive_dynamic_field_id` 从此INFO对象派生子对象ID的逻辑。
// 10. `blue_move_pool_children_ids()` 函数，用于获取特定BlueMove池的动态子对象ID，以及根据池代币类型派生的其他相关对象ID。
//     这部分逻辑比较复杂，涉及到构造 `StructTag` 和 `MoveStructLayout` 来解析动态字段。
// 11. `pool_dynamic_child_layout()` 和 `format_coin_type_for_derive()` 是上述ID派生逻辑的辅助函数。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” BlueMove 这个DEX发生的两种主要“事件”的：
// “有人创建了一个新的交易池” 和 “有人在这个池子里完成了一笔代币交换”。
// 它还负责收集与BlueMove协议本身以及特定池子相关联的一些重要的“全局对象”和“动态子对象”的ID。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别BlueMove的特定事件**:
//     -   `BLUE_MOVE_POOL_CREATED` 和 `BLUE_MOVE_SWAP_EVENT` 常量是这两种事件在Sui链上的“专属门牌号”。
//     -   `blue_move_event_filter()` 函数生成一个“过滤器”，告诉Sui节点只订阅BlueMove创建新池子的事件。
//
// 2.  **`BlueMovePoolCreated` 结构体 (新池子信息卡)**:
//     -   当监听到BlueMove新池子创建事件时，这个结构体记录事件信息（池ID、两种代币类型）。
//     -   `TryFrom<&SuiEvent>` 是“填卡员”，从原始Sui事件中提取数据。
//     -   `to_pool()` 方法将这张BlueMove专用的“信息卡”转换为通用的 `Pool` 结构，并查询代币精度。
//
// 3.  **`BlueMoveSwapEvent` 结构体 (交换记录卡)**:
//     -   记录BlueMove交换事件的详细信息（池ID、输入输出代币、输入输出金额）。
//     -   `TryFrom` 实现能从不同来源（`SuiEvent`, `ShioEvent`, 原始JSON `Value`）提取信息。
//         特别是从 `Value` 转换时，它会分析 `token_x_in`, `amount_x_in`, `token_y_in`, `amount_y_in` 等字段来判断实际的交易方向。
//     -   `to_swap_event()` 方法将其转换为通用的 `SwapEvent`。
//
// 4.  **收集相关对象ID (Collecting Related Object IDs)**:
//     -   `BLUE_MOVE_DEX_INFO`: 一个核心的BlueMove信息对象的ID。
//     -   `blue_move_related_object_ids()`: 列出了一些硬编码的BlueMove相关对象ID，并动态派生了 `BLUE_MOVE_DEX_INFO` 的一个子对象ID。
//     -   `blue_move_pool_children_ids()`: 针对特定的BlueMove池子，获取其动态字段子对象ID，
//         以及根据池中代币类型（如 `BlueMove-{CoinA}-{CoinB}-LP`）派生出的其他相关动态字段对象的ID。
//         这部分使用了 `derive_dynamic_field_id` 和 `MoveStructLayout` 等底层工具，显示了BlueMove可能广泛使用动态字段来组织其链上数据。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
//
// -   **动态字段 (Dynamic Fields)**:
//     Sui Move语言的一个特性，允许在一个对象（父对象）下动态地添加或移除其他对象（子对象），这些子对象通过一个特定的“键”与父对象关联。
//     这提供了一种灵活的方式来组织和扩展对象的数据，类似于在一个对象内部实现一个可扩展的哈希表或集合。
//     BlueMove似乎利用了动态字段来存储与主DEX Info对象或特定池对象相关联的信息（例如LP代币信息、特定交易对的状态等）。
//     `derive_dynamic_field_id` 函数就是用来根据父对象ID、键的类型标签和键的值来计算出动态字段子对象的ID。
//     `pool_dynamic_child_layout()` 和 `format_coin_type_for_derive()` 辅助了这个过程。
//
// -   **`MoveStructLayout` (Move结构布局)**:
//     当需要手动解析从链上获取的原始Move对象字节（BCS编码）时，或者在某些情况下需要知道一个Move结构体内部字段的精确布局（类型、名称、顺序）时，
//     会用到 `MoveStructLayout`。它描述了一个Move结构体的内部字段定义。
//     在 `blue_move_pool_children_ids` 中，`pool_dynamic_child_layout()` 创建了一个 `MoveStructLayout` 来帮助解析通过动态字段获取的 `Field<Wrapper<ID>, ID>` 对象的内部结构，以便提取出孙子对象的ID。

// 引入标准库的 FromStr trait (用于从字符串转换) 和 Arc (原子引用计数)。
use std::{str::FromStr, sync::Arc};

// 引入 eyre 库，用于更方便的错误处理和上下文管理。
use eyre::{bail, ensure, eyre, OptionExt, Result};
// 引入 Move 核心类型，用于解析和表示Move对象的结构和类型。
use move_core_types::{
    annotated_value::{MoveFieldLayout, MoveStruct, MoveStructLayout, MoveTypeLayout, MoveValue}, // Move值、字段布局、结构布局、类型布局
    language_storage::StructTag, // 结构标签，表示具体的Move结构体类型
};
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
    types::{base_types::ObjectID, dynamic_field::derive_dynamic_field_id, TypeTag}, // ObjectID, 动态字段ID派生函数, TypeTag。
    SuiClient, // Sui RPC客户端。
};
// 引入 Sui 核心类型中的动态字段提取函数和Object类型。
use sui_types::{dynamic_field::extract_field_from_move_struct, object::Object, Identifier};
// 引入 tracing 库的 warn! 宏，用于记录警告级别的日志。
use tracing::warn;

// 从当前crate的父模块 (protocols) 引入 get_coin_decimals 函数。
use super::get_coin_decimals;
// 从当前crate的根模块引入 normalize_coin_type 函数和相关类型定义。
use crate::{
    move_field_layout, move_struct_layout, move_type_layout_struct, normalize_coin_type, // 宏和函数，用于处理Move类型和布局
    types::{Pool, PoolExtra, Protocol, SwapEvent, Token}, // 通用类型定义
};

/// `BLUE_MOVE_POOL_CREATED` 常量
///
/// 定义了BlueMove DEX协议在Sui链上发出的“新池创建”事件的全局唯一类型字符串。
pub const BLUE_MOVE_POOL_CREATED: &str =
    "0xb24b6789e088b876afabca733bed2299fbc9e2d6369be4d1acfa17d8145454d9::swap::Created_Pool_Event";

/// `BLUE_MOVE_SWAP_EVENT` 常量
///
/// 定义了BlueMove DEX协议在Sui链上发出的“代币交换完成”事件的全局唯一类型字符串。
pub const BLUE_MOVE_SWAP_EVENT: &str =
    "0xb24b6789e088b876afabca733bed2299fbc9e2d6369be4d1acfa17d8145454d9::swap::Swap_Event";

/// `BLUE_MOVE_DEX_INFO` 常量
///
/// 定义了BlueMove协议的一个核心DEX信息对象的ObjectID字符串。
/// 这个对象可能存储了协议的全局配置或状态。
const BLUE_MOVE_DEX_INFO: &str = "0x3f2d9f724f4a1ce5e71676448dc452be9a6243dac9c5b975a588c8c867066e92";

/// `blue_move_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，专门用于订阅BlueMove的“新池创建”事件。
pub fn blue_move_event_filter() -> EventFilter {
    EventFilter::MoveEventType(BLUE_MOVE_POOL_CREATED.parse().unwrap()) // 解析类型字符串为StructTag
}

/// `BlueMovePoolCreated` 结构体
///
/// 用于存储从BlueMove的 `Created_Pool_Event` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)]
pub struct BlueMovePoolCreated {
    pub pool: ObjectID,   // 新创建的池的ObjectID
    pub token0: String, // 池中第一个代币的类型字符串 (已添加 "0x" 前缀)
    pub token1: String, // 池中第二个代币的类型字符串 (已添加 "0x" 前缀)
}

/// 为 `BlueMovePoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
/// 使得可以尝试将一个通用的 `&SuiEvent` 引用转换为 `BlueMovePoolCreated`。
impl TryFrom<&SuiEvent> for BlueMovePoolCreated {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let parsed_json = &event.parsed_json; // 获取事件的JSON数据部分
        // 从JSON中提取 "pool_id" 字段，并解析为 ObjectID。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMovePoolCreated事件JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 从JSON中提取 "token_x_name" 字段作为token0。
        // BlueMove事件中的代币类型可能不带 "0x" 前缀，这里手动添加。
        let token0_str_raw = parsed_json["token_x_name"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMovePoolCreated事件JSON中缺少'token_x_name'字段"))?;
        let token0_str_formatted = format!("0x{}", token0_str_raw); // 添加 "0x" 前缀

        // 从JSON中提取 "token_y_name" 字段作为token1。
        let token1_str_raw = parsed_json["token_y_name"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMovePoolCreated事件JSON中缺少'token_y_name'字段"))?;
        let token1_str_formatted = format!("0x{}", token1_str_raw); // 添加 "0x" 前缀

        Ok(Self {
            pool: pool_object_id,
            token0: token0_str_formatted,
            token1: token1_str_formatted,
        })
    }
}

impl BlueMovePoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `BlueMovePoolCreated` 事件数据转换为通用的 `Pool` 结构。
    /// 需要异步查询每个代币的精度 (decimals)。
    pub async fn to_pool(&self, sui: &SuiClient) -> Result<Pool> {
        // 异步获取token0和token1的精度信息
        let token0_decimals = get_coin_decimals(sui, &self.token0).await?;
        let token1_decimals = get_coin_decimals(sui, &self.token1).await?;

        // 创建 Token 结构列表
        let tokens_vec = vec![
            Token::new(&self.token0, token0_decimals),
            Token::new(&self.token1, token1_decimals),
        ];
        // BlueMove的PoolCreated事件中不直接包含额外的池信息 (如费率)，所以extra为None。
        let extra_data = PoolExtra::None;

        Ok(Pool {
            protocol: Protocol::BlueMove, // 指明协议为BlueMove
            pool: self.pool,             // 池的ObjectID
            tokens: tokens_vec,          // 池中代币列表
            extra: extra_data,           // 无特定附加信息
        })
    }
}

/// `BlueMoveSwapEvent` 结构体
///
/// 用于存储从BlueMove的 `Swap_Event` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)]
pub struct BlueMoveSwapEvent {
    pub pool: ObjectID,       // 发生交换的池的ObjectID
    pub coin_in: String,      // 输入代币的规范化类型字符串
    pub coin_out: String,     // 输出代币的规范化类型字符串
    pub amount_in: u64,       // 输入代币的数量
    pub amount_out: u64,      // 输出代币的数量
}

/// 为 `BlueMoveSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for BlueMoveSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        // 确保事件类型以 `BLUE_MOVE_SWAP_EVENT` 开头。
        // BlueMove的Swap事件泛型参数可能包含两种代币类型。
        ensure!(
            event.type_.to_string().starts_with(BLUE_MOVE_SWAP_EVENT),
            "事件类型不匹配BlueMove SwapEvent (Not a BlueMoveSwapEvent)"
        );
        // 直接尝试从事件的 `parsed_json` 字段转换。
        (&event.parsed_json).try_into()
    }
}

/// 为 `BlueMoveSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for BlueMoveSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        ensure!(
            event.event_type.starts_with(BLUE_MOVE_SWAP_EVENT),
            "事件类型不匹配BlueMove SwapEvent (Not a BlueMoveSwapEvent)"
        );
        event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json in ShioEvent)")?.try_into()
    }
}

/// 为 `BlueMoveSwapEvent` 实现 `TryFrom<&Value>` trait (从 `serde_json::Value` 转换)。
/// 这是实际的JSON解析逻辑。
/// BlueMove的Swap事件JSON结构包含 `token_x_in`, `amount_x_in`, `token_y_in`, `amount_y_in` 等字段，
/// 以及 `token_x_out`, `amount_x_out`, `token_y_out`, `amount_y_out`。
/// 需要通过判断哪个输入金额 (`amount_x_in` 或 `amount_y_in`) 大于0来确定实际的交易方向。
impl TryFrom<&Value> for BlueMoveSwapEvent {
    type Error = eyre::Error;

    fn try_from(parsed_json: &Value) -> Result<Self> {
        // 提取池ID
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 提取与代币X相关的输入输出信息
        let coin_x_in_type_raw = parsed_json["token_x_in"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'token_x_in'字段"))?;
        let amount_x_in_val: u64 = parsed_json["amount_x_in"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'amount_x_in'字段"))?
            .parse()?;

        // 提取与代币Y相关的输入输出信息
        let coin_y_in_type_raw = parsed_json["token_y_in"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'token_y_in'字段"))?;
        let amount_y_in_val: u64 = parsed_json["amount_y_in"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'amount_y_in'字段"))?
            .parse()?;

        let coin_x_out_type_raw = parsed_json["token_x_out"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'token_x_out'字段"))?;
        let amount_x_out_val: u64 = parsed_json["amount_x_out"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'amount_x_out'字段"))?
            .parse()?;

        let coin_y_out_type_raw = parsed_json["token_y_out"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'token_y_out'字段"))?;
        let amount_y_out_val: u64 = parsed_json["amount_y_out"]
            .as_str()
            .ok_or_else(|| eyre!("BlueMoveSwapEvent JSON中缺少'amount_y_out'字段"))?
            .parse()?;

        // 判断实际的交易方向
        // 如果 amount_x_in > 0，则表示输入的是代币X，输出的是代币Y。
        // 否则，输入的是代币Y，输出的是代币X。
        let (final_coin_in_type, final_coin_out_type, final_amount_in, final_amount_out) = if amount_x_in_val > 0 {
            (coin_x_in_type_raw, coin_y_out_type_raw, amount_x_in_val, amount_y_out_val)
        } else {
            (coin_y_in_type_raw, coin_x_out_type_raw, amount_y_in_val, amount_x_out_val)
        };

        // 规范化代币类型字符串 (添加 "0x" 前缀并处理SUI的特殊情况)
        let normalized_coin_in = normalize_coin_type(&format!("0x{}", final_coin_in_type));
        let normalized_coin_out = normalize_coin_type(&format!("0x{}", final_coin_out_type));

        Ok(Self {
            pool: pool_object_id,
            coin_in: normalized_coin_in,
            coin_out: normalized_coin_out,
            amount_in: final_amount_in,
            amount_out: final_amount_out,
        })
    }
}

impl BlueMoveSwapEvent {
    /// `to_swap_event` 异步方法
    ///
    /// 将 `BlueMoveSwapEvent` 转换为通用的 `SwapEvent` 枚举成员。
    pub async fn to_swap_event(&self) -> Result<SwapEvent> {
        Ok(SwapEvent {
            protocol: Protocol::BlueMove, // 指明协议为BlueMove
            pool: Some(self.pool),       // 交换发生的池的ObjectID
            coins_in: vec![self.coin_in.clone()],
            coins_out: vec![self.coin_out.clone()],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

/// `blue_move_related_object_ids` 函数
///
/// 返回与BlueMove协议本身相关的核心对象ID列表。
/// 包括一些硬编码的已知对象ID和通过动态字段派生得到的ID。
pub fn blue_move_related_object_ids() -> Vec<String> {
    let mut result_ids_str_vec = vec![
        // 以下是一些已知的BlueMove相关包ID或对象ID (可能需要根据实际部署更新)
        "0x08cd33481587d4c4612865b164796d937df13747d8c763b8a178c87e3244498f", // BlueMoveDex6 (可能是某个版本的包或核心对象)
        "0xb24b6789e088b876afabca733bed2299fbc9e2d6369be4d1acfa17d8145454d9", // BlueMoveDex (主包ID, 与事件类型中的包ID一致)
        "0x7be61b62d902f3fe78d0a5e20b81b4715a47ff06cae292db8991dfea422cf57e", // BlueMove 9 (可能是另一个相关对象)
        "0x41d5f1c14825d92c93cdae3508705cc31582c8aaaca501aaa4970054fd3b5b2d", // Version Manage (版本管理对象)
    ]
    .into_iter()
    .map(|s| s.to_string()) // 将 &str 转换为 String
    .collect::<Vec<_>>();

    // --- 动态派生与 BLUE_MOVE_DEX_INFO 相关的对象ID ---
    // `dex_related_ids()` 函数派生与 `BLUE_MOVE_DEX_INFO` 对象相关的一个子对象ID。
    result_ids_str_vec.extend(dex_related_ids());

    result_ids_str_vec
}

/// `dex_related_ids` (私有辅助函数)
///
/// 根据 `BLUE_MOVE_DEX_INFO` (父对象ID) 和一个特定的键类型及键值，派生出一个动态字段子对象的ID。
fn dex_related_ids() -> Vec<String> {
    let parent_dex_info_id = ObjectID::from_hex_literal(BLUE_MOVE_DEX_INFO).unwrap();

    // 键的类型标签: `0x2::dynamic_object_field::Wrapper<0x2::object::ID>`
    // 这表示动态字段的键是一个包装了 `ObjectID` 的结构体。
    let key_type_tag = TypeTag::from_str("0x02::dynamic_object_field::Wrapper<0x02::object::ID>").unwrap();
    // 键的值: 这里用 `parent_dex_info_id` 本身作为键的内容进行BCS序列化。
    // 这是一种常见的模式，用父对象ID作为键来查找某个特定的“主”子对象或元数据子对象。
    let key_value_bcs_bytes: Vec<u8> = bcs::to_bytes(&parent_dex_info_id).unwrap();

    // 使用 `derive_dynamic_field_id` 函数计算子对象的ID。
    let child_object_id = derive_dynamic_field_id(parent_dex_info_id, &key_type_tag, &key_value_bcs_bytes).unwrap();

    // 返回包含父对象ID和派生出的子对象ID的列表。
    vec![parent_dex_info_id, child_object_id].into_iter().map(|id| id.to_string()).collect()
}

/// `blue_move_pool_children_ids` 异步函数
///
/// 获取特定BlueMove池对象的动态子对象ID，以及根据池代币类型派生的其他相关动态字段对象的ID。
/// BlueMove似乎使用动态字段来存储与池或LP代币相关的信息。
///
/// 参数:
/// - `pool`: 一个对 `Pool` 结构体的引用，代表要检查的BlueMove池。
/// - `simulator`: 一个共享的模拟器实例，用于从链上（或缓存）获取对象数据。
///
/// 返回:
/// - `Result<Vec<String>>`: 包含所有相关子对象和派生对象ID字符串的列表。
pub async fn blue_move_pool_children_ids(pool: &Pool, simulator: Arc<dyn Simulator>) -> Result<Vec<String>> {
    let mut result_ids_str_vec = vec![]; // 用于存储结果ID字符串的向量

    let parent_pool_id = pool.pool; // 池的ObjectID作为父ID

    // --- 派生第一个子对象 (类似 `dex_related_ids` 中的逻辑) ---
    // 这个子对象可能是池的一个核心动态字段，键是池ID本身。
    let key_type_tag_for_pool_child = TypeTag::from_str("0x02::dynamic_object_field::Wrapper<0x02::object::ID>").map_err(|e| eyre!(e))?;
    let key_value_bcs_bytes_for_pool_child = bcs::to_bytes(&parent_pool_id)?;
    let child_object_id = derive_dynamic_field_id(parent_pool_id, &key_type_tag_for_pool_child, &key_value_bcs_bytes_for_pool_child)?;
    result_ids_str_vec.push(child_object_id.to_string());

    // --- 派生孙子对象 (从上面派生的子对象中进一步派生) ---
    // 这个孙子对象可能是存储在第一个子对象内部 `value` 字段的另一个ObjectID。
    // 需要定义子对象的Move结构布局来正确解析它。
    { // 使用块限制 `layout` 和 `parse_grandson_id` 的作用域
        let child_object_layout = pool_dynamic_child_layout(); // 获取预定义的子对象布局

        // 内部函数，用于从子对象 (类型为 `Field<Wrapper<ID>, ID>`) 中解析出孙子对象的ID (存储在 `value` 字段)。
        let parse_grandson_id_from_child_obj = |child_sui_object: &Object| -> Result<String> {
            let child_move_object = child_sui_object.data.try_as_move().ok_or_eyre("子对象不是有效的Move对象 (Child object is not a valid Move object)")?;
            let child_move_struct = MoveStruct::simple_deserialize(child_move_object.contents(), &child_object_layout)
                .map_err(|e| eyre!("反序列化子对象Move结构失败: {} (Failed to deserialize child object Move struct: {})", e))?;
            // `extract_field_from_move_struct` 是一个辅助函数，用于从MoveStruct中按名称提取字段值。
            let grandson_id_value = extract_field_from_move_struct(&child_move_struct, "value")
                .ok_or_eyre("在子对象中未找到'value'字段 (Missing 'value' field in child object)")?;
            match grandson_id_value {
                MoveValue::Address(sui_address_grandson_id) => Ok(sui_address_grandson_id.to_hex_literal()), // ObjectID在MoveValue中以Address形式表示
                _ => bail!("子对象中的'value'字段不是预期的Address类型 (Field 'value' in child object is not the expected Address type)"),
            }
        };

        // 获取上面派生出的第一个子对象的数据
        if let Some(child_sui_object_data) = simulator.get_object(&child_object_id).await {
            match parse_grandson_id_from_child_obj(&child_sui_object_data) {
                Ok(grandson_id_str) => result_ids_str_vec.push(grandson_id_str),
                Err(e) => {
                    // 如果解析失败（例如子对象结构不匹配或字段缺失），记录警告但继续。
                    warn!("从子对象 {} 解析孙子对象ID失败: {} (Failed to parse grandson ObjectID from child object {}: {})", child_object_id, e);
                }
            }
        }
    }

    // --- 根据池代币类型派生与 BLUE_MOVE_DEX_INFO 相关的动态字段子对象ID ---
    // BlueMove似乎使用 `BLUE_MOVE_DEX_INFO` 作为父对象，
    // 并用形如 "BlueMove-{CoinA}-{CoinB}-LP" 的字符串作为键，来存储与特定LP代币相关的信息。
    {
        let dex_info_parent_id = ObjectID::from_hex_literal(BLUE_MOVE_DEX_INFO).map_err(|e| eyre!(e))?;
        // 键的类型标签: `0x02::dynamic_object_field::Wrapper<0x01::string::String>`
        // 这表示动态字段的键是一个包装了 `Std::String` 的结构体。
        let key_type_tag_for_lp_info =
            TypeTag::from_str("0x02::dynamic_object_field::Wrapper<0x01::string::String>").map_err(|e| eyre!(e))?;

        // 确保池中有两种代币 (BlueMove通常是双币池)
        if pool.tokens.len() >= 2 {
            // 格式化代币类型，用于构成键字符串。
            // `format_coin_type_for_derive` 将代币类型转换为规范的、不含 "0x" 前缀的显示格式。
            let coin_a_formatted_str = format_coin_type_for_derive(&pool.tokens[0].token_type);
            let coin_b_formatted_str = format_coin_type_for_derive(&pool.tokens[1].token_type);
            // 构建键字符串，例如 "BlueMove-sui::SUI-some_module::TOKEN-LP"
            let key_value_str = format!("BlueMove-{}-{}-LP", coin_a_formatted_str, coin_b_formatted_str);

            // 将键字符串进行BCS序列化
            let key_value_bcs_bytes_for_lp = bcs::to_bytes(&key_value_str)?;
            // 派生动态字段子对象的ID
            let lp_info_child_id = derive_dynamic_field_id(dex_info_parent_id, &key_type_tag_for_lp_info, &key_value_bcs_bytes_for_lp)?;
            result_ids_str_vec.push(lp_info_child_id.to_string());
        }
    }

    Ok(result_ids_str_vec)
}

/// `pool_dynamic_child_layout` (私有辅助函数)
///
/// 返回一个预定义的 `MoveStructLayout`，用于描述BlueMove池的第一个动态子对象的内部结构。
/// 这个子对象的类型是 `dynamic_field::Field<Wrapper<ID>, ID>`，
/// 它包含一个名为 `value` 的字段，该字段存储了我们感兴趣的“孙子”对象的ID。
fn pool_dynamic_child_layout() -> MoveStructLayout {
    MoveStructLayout {
        type_: StructTag::from_str( // 子对象的完整类型标签
            // Field<NameType, ValueType>
            // NameType is Wrapper<ID>
            // ValueType is ID
            "0x02::dynamic_field::Field<0x02::dynamic_object_field::Wrapper<0x02::object::ID>, 0x02::object::ID>",
        )
        .unwrap(), // unwrap假设类型字符串总是有效的
        fields: Box::new(vec![ // 字段列表
            // 第一个字段: `id: UID` (Field本身的ID)
            move_field_layout!( // 使用宏简化MoveFieldLayout的创建
                "id", // 字段名
                move_type_layout_struct!(move_struct_layout!( // 字段类型是结构体 0x2::object::UID
                    StructTag::from_str("0x02::object::UID").unwrap(),
                    vec![move_field_layout!( // UID结构体内部有一个字段 "id"
                        "id",
                        move_type_layout_struct!(move_struct_layout!( // 这个内部 "id" 的类型是 0x2::object::ID
                            StructTag::from_str("0x02::object::ID").unwrap(),
                            // ID结构体内部有一个字段 "bytes" 类型是 Address (ObjectID的Move表示)
                            vec![move_field_layout!("bytes", MoveTypeLayout::Address)]
                        ))
                    )]
                ))
            ),
            // 第二个字段: `name: Wrapper<ID>` (动态字段的键)
            move_field_layout!(
                "name",
                move_type_layout_struct!(move_struct_layout!(
                    StructTag::from_str("0x02::dynamic_object_field::Wrapper<0x02::object::ID>").unwrap(),
                    // Wrapper<ID> 结构体内部有一个字段 "name" 类型是 Address (ObjectID)
                    vec![move_field_layout!("name", MoveTypeLayout::Address)]
                ))
            ),
            // 第三个字段: `value: ID` (动态字段的值，这是我们想提取的孙子对象ID)
            move_field_layout!("value", MoveTypeLayout::Address), // 类型是Address (ObjectID)
        ]),
    }
}

/// `format_coin_type_for_derive` (内联辅助函数)
///
/// 将一个完整的代币类型字符串 (例如 "0x2::sui::SUI")
/// 转换为其用于动态字段键派生的规范显示格式 (例如 "sui::SUI")。
/// 它通过 `TypeTag::to_canonical_display(false)` 实现，`false` 表示不显示地址前缀 "0x"。
#[inline]
fn format_coin_type_for_derive(coin_type: &str) -> String {
    let coin_type_tag = TypeTag::from_str(coin_type).unwrap(); // 解析为TypeTag
    format!("{}", coin_type_tag.to_canonical_display(false)) // 获取规范显示格式，不含地址前缀
}

// --- 测试模块 ---
#[cfg(test)]
mod tests {

    use mev_logger::LevelFilter; // 日志级别过滤器
    use simulator::DBSimulator;  // 数据库模拟器

    use super::*; // 导入外部模块 (blue_move.rs) 的所有公共成员

    /// `test_blue_move_pool_children_ids` 测试函数
    ///
    /// 测试 `blue_move_pool_children_ids` 函数是否能为给定的BlueMove池正确派生相关的子对象ID。
    #[tokio::test]
    async fn test_blue_move_pool_children_ids() {
        // 初始化日志，设置默认级别为INFO
        mev_logger::init_console_logger(Some(LevelFilter::INFO));

        // 创建一个示例的 Pool 结构体，代表一个已知的BlueMove池
        let pool_info = Pool {
            protocol: Protocol::BlueMove,
            // 这是一个示例池ID，实际测试时可能需要替换为测试网络上有效的BlueMove池ID
            pool: ObjectID::from_str("0xe057718861803021cb3b40ec1514b37c8f1fa36636b2dcb9de01e16009db121c").unwrap(),
            tokens: vec![ // 池中的两种代币
                Token::new("0x2::sui::SUI", 9), // SUI, 9位精度
                Token::new(
                    "0xed4504e791e1dad7bf93b41e089b4733c27f35fde505693e18186c2ba8e2e14b::suib::SUIB", // SUIB (示例), 9位精度
                    9,
                ),
            ],
            extra: PoolExtra::None, // BlueMove池创建事件不直接提供额外信息
        };

        // 创建一个DBSimulator实例用于测试 (带回退，意味着如果本地找不到对象会尝试RPC查询)
        let simulator_instance: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);

        // 调用被测试函数
        let children_ids_vec = blue_move_pool_children_ids(&pool_info, simulator_instance).await.unwrap();
        // 打印结果以供检查
        // 预期的结果会依赖于链上实际的动态字段结构，以及 `derive_dynamic_field_id` 的正确性。
        println!("为池 {} 派生的子对象ID列表: {:?}", pool_info.pool, children_ids_vec);
        // 可以在这里添加断言 (assert!) 来验证结果是否符合预期 (如果已知正确的子对象ID)。
        // 例如: assert!(children_ids_vec.contains(&"some_expected_child_id_string".to_string()));
    }
}

[end of crates/dex-indexer/src/protocols/blue_move.rs]
