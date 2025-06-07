// 该文件 `mod.rs` 是 `dex-indexer` crate 中 `protocols` 子模块的入口文件。
// 它负责：
// 1. 声明 `protocols` 模块下的所有具体协议实现子模块 (如 abex, cetus, turbos 等)。
// 2. 定义一些在各个协议实现中可能都会用到的通用辅助函数和宏。
//
// **文件概览 (File Overview)**:
// 这个 `mod.rs` 文件是 `protocols` 文件夹的总入口和“公共工具房”。
// `protocols` 文件夹里的每一个 `.rs` 文件（比如 `abex.rs`, `cetus.rs`）都是针对某一个特定DEX协议的事件解析器。
// 这个 `mod.rs` 文件：
// -   首先，通过 `pub mod abex;` 这样的语句，把所有这些特定协议的解析器都“注册”到 `protocols` 模块下，
//     这样 `dex-indexer` crate 的其他部分（比如 `lib.rs`）就可以通过 `protocols::abex` 这样的路径来使用它们了。
// -   其次，它提供了一些通用的“小工具”，这些工具可能被多个不同的协议解析器共用，避免重复写代码。
//
// **主要包含的通用工具 (Key Utility Functions and Macros)**:
//
// 1.  **`SUI_RPC_NODE` 常量**:
//     -   一个字符串常量，用于存储Sui RPC节点的URL。**注意：在当前代码中，它被设置为空字符串 `""`。**
//         这意味着如果任何依赖此常量的代码（比如下面的 `get_children_ids` 或测试）在没有正确配置环境变量或默认值的情况下运行，
//         将会失败。实际使用时，这个值需要被设置为一个有效的Sui RPC节点地址。
//
// 2.  **`get_coin_decimals()` 异步函数 (带缓存)**:
//     -   **功能**: 获取指定代币类型的精度（即小数点位数）。例如，SUI是9位精度。
//     -   **实现**:
//         -   它首先尝试通过Sui RPC的 `get_coin_metadata` API来获取官方的代币元数据，从中提取精度。
//         -   如果RPC调用失败或者元数据中没有精度信息，它会**回退到使用 `blockberry::get_coin_decimals()` 函数**
//             (这表明 `blockberry.rs` 文件中定义的Blockberry API客户端被用作备用数据源)。
//         -   **缓存**: 使用 `#[cached]` 宏。这意味着一旦某个代币的精度被成功获取，结果就会被缓存起来。
//             下次再请求同一个代币的精度时，会直接从缓存返回，避免重复的RPC或API调用，提高效率。
//             缓存的键是代币类型字符串。
//
// 3.  **`get_pool_coins_type()` 异步函数 (带缓存)**:
//     -   **功能**: 根据一个DEX池的ObjectID，获取该池中交易对的两种代币的类型字符串。
//     -   **实现**:
//         -   通过Sui RPC的 `get_object_with_options` API获取池对象的类型信息 (只需要类型，不需要完整内容)。
//         -   从返回的对象类型字符串中解析出 `StructTag` (结构标签)。
//         -   从 `StructTag` 的泛型参数列表 (`type_params`) 中提取前两个类型作为代币A和代币B的类型。
//             （这里假设池对象的类型定义中，前两个泛型参数总是代表交易对的两种代币）。
//         -   对提取出的代币类型字符串进行规范化 (使用 `normalize_coin_type`)。
//         -   **缓存**: 同样使用 `#[cached]` 宏，缓存键是池的ObjectID字符串。
//
// 4.  **`get_coin_in_out_v2!` 宏**:
//     -   **功能**: 这是一个声明宏 (declarative macro)，用于根据池的ObjectID、一个实现了 `Simulator` trait 的provider，
//         以及一个布尔值 `a2b` (表示交易方向是否为A到B)，来获取一个交易对的实际输入代币类型和输出代币类型。
//     -   **实现**:
//         -   它异步地从 `provider` (模拟器) 获取池对象的Move对象数据。
//         -   解析该Move对象的类型，并提取其泛型参数（期望至少有两个，代表代币A和代币B）。
//         -   根据传入的 `a2b` 参数，决定返回 `(coin_a, coin_b)` 还是 `(coin_b, coin_a)`。
//     -   **用途**: 这个宏主要被各个协议的 `SwapEvent::to_swap_event_v2()` 方法使用，
//         因为Swap事件本身可能只提供一个代币类型或不提供代币类型，需要从池对象状态中推断。
//         使用 `Simulator` 而不是 `SuiClient` 使得它在纯模拟环境或测试中更易用。
//
// 5.  **`get_children_ids()` 异步函数**:
//     -   **功能**: 获取指定父对象ID的所有动态子对象的ID列表。
//     -   **实现**: 使用 `SuiClient` 的 `get_dynamic_fields` API 进行分页查询，收集所有子对象的ID。
//     -   **用途**: 主要用于 `pool_ids.rs` 工具，或者在索引特定协议（如Cetus, Kriya CLMM, FlowX CLMM）的池时，
//         需要发现与主池对象关联的所有动态子对象（例如ticks, positions, bitmaps等）。
//     -   **注意**: 此函数直接创建了一个新的 `SuiClient` 实例，RPC URL依赖 `SUI_RPC_NODE` 常量。
//         在生产环境中，通常会复用已有的客户端实例。
//
// 6.  **`move_field_layout!`, `move_type_layout_struct!`, `move_struct_layout!` 宏**:
//     -   这些是声明宏，用于以更简洁的方式构建 `MoveFieldLayout`, `MoveTypeLayout::Struct`, 和 `MoveStructLayout` 实例。
//     -   这些布局结构在需要手动解析Move对象内容或与动态字段交互时非常有用，例如在 `blue_move.rs` 的
//         `pool_dynamic_child_layout()` 函数中被用来定义一个复杂的嵌套结构布局。
//
// **Sui相关的概念解释 (Sui-related Concepts)**:
//
// -   **`TypeTag` 和 `StructTag`**:
//     `TypeTag` 是Sui类型系统在运行时表示任何Move类型（包括基本类型、向量、结构体等）的枚举。
//     `StructTag` 是 `TypeTag::Struct` 的内部表示，它详细描述了一个结构体类型，包括其定义的地址（包ID）、模块名、结构体名以及任何泛型参数。
//     这些对于从事件的类型字符串或对象类型中准确解析出代币类型至关重要。
//
// -   **`SuiObjectDataOptions`**:
//     当通过RPC向Sui节点请求对象数据时，可以使用这个选项结构来指定你希望返回哪些信息。
//     例如，你可以只请求对象的类型 (`with_type()`)，或者请求对象的完整内容 (`with_content()`)，或者同时请求所有者信息、BCS序列化内容等。
//     精细控制请求内容可以减少不必要的数据传输，提高效率。
//
// -   **动态字段 (Dynamic Fields)**:
//     见 `blue_move.rs` 或 `cetus.rs` 中的解释。`get_children_ids()` 函数就是用来获取这些动态字段的。

// --- 声明 protocols 模块下的所有子模块 ---
// 每个 pub mod 对应一个协议的实现文件。
pub mod abex;        // ABEX DEX 协议
pub mod aftermath;   // Aftermath Finance 协议
pub mod babyswap;    // BabySwap DEX 协议
pub mod blue_move;   // BlueMove (可能涉及NFT市场和DEX功能)
pub mod cetus;       // Cetus CLMM DEX 协议
pub mod deepbook_v2; // Sui官方DeepBook V2订单簿协议
pub mod flowx_amm;   // FlowX AMM (传统自动做市商) 协议
pub mod flowx_clmm;  // FlowX CLMM (集中流动性) 协议
pub mod interest;    // Interest Protocol (可能为借贷或生息相关)
pub mod kriya_amm;   // KriyaDEX AMM 协议
pub mod kriya_clmm;  // KriyaDEX CLMM 协议
pub mod navi;        // Navi 借贷协议
pub mod suiswap;     // SuiSwap DEX 协议
pub mod turbos;      // Turbos Finance CLMM DEX 协议

// 引入标准库的 FromStr (从字符串转换) 和 Arc (原子引用计数)。
use std::str::FromStr;
use std::sync::Arc;

// 引入 cached 宏，用于函数结果的缓存。
use cached::proc_macro::cached;
// 引入 eyre 库，用于错误处理。
use eyre::{bail, ensure, eyre, Result};

// 引入 Sui SDK 中的相关类型。
use sui_sdk::{
    rpc_types::SuiObjectDataOptions, // 用于指定获取对象时需要哪些数据
    types::{base_types::ObjectID, TypeTag}, // ObjectID (对象ID), TypeTag (Move类型标签)
    SuiClient, SuiClientBuilder,       // Sui RPC客户端和构建器
};

// 从当前crate的 blockberry 模块引入 (用于备用获取代币精度)。
use crate::blockberry;
// 从当前crate的根模块引入 normalize_coin_type 函数。
use crate::normalize_coin_type;

/// `SUI_RPC_NODE` 常量
///
/// 定义了Sui RPC节点的URL。
/// **重要**: 当前值为空字符串。实际使用时，必须将其设置为一个有效的Sui RPC节点URL，
/// 例如 "https://fullnode.mainnet.sui.io:443" 或通过环境变量配置。
/// 依赖此常量的函数 (如 `get_children_ids` 和一些测试) 在此值未正确设置时会失败。
pub const SUI_RPC_NODE: &str = "";

/// `get_coin_decimals` 异步函数 (带缓存)
///
/// 获取指定Sui代币类型的精度 (小数点位数)。
/// 它首先尝试通过Sui RPC的 `get_coin_metadata` API获取。如果失败或未找到，
/// 则回退到使用 `blockberry::get_coin_decimals` 作为备用数据源。
/// 使用 `#[cached]` 宏对结果进行缓存，以提高后续调用的性能。
///
/// 参数:
/// - `sui`: 一个对 `SuiClient` 的引用。
/// - `coin_type`: 要查询的代币类型字符串 (例如 "0x2::sui::SUI")。
///
/// 返回:
/// - `Result<u8>`: 成功则返回代币的精度，否则返回错误。
#[cached(key = "String", convert = r##"{ coin_type.to_string() }"##, result = true)]
pub async fn get_coin_decimals(sui: &SuiClient, coin_type: &str) -> Result<u8> {
    // 调用Sui RPC API获取代币元数据
    let coin_metadata_result = sui.coin_read_api().get_coin_metadata(coin_type.into()).await;
    if let Ok(Some(metadata)) = coin_metadata_result {
        // 如果成功获取到元数据并且元数据非空，则返回其`decimals`字段。
        return Ok(metadata.decimals);
    }

    // 如果RPC调用失败或未返回元数据，则回退到Blockberry API。
    // 记录一个警告，表明正在使用回退机制。
    tracing::warn!("Sui RPC get_coin_metadata 未找到代币 {} 的精度，尝试从Blockberry获取...", coin_type);
    match blockberry::get_coin_decimals(coin_type).await {
        Ok(decimals) => Ok(decimals),
        Err(e) => Err(e), // 如果Blockberry也失败，则返回其错误
    }
}

/// `get_pool_coins_type` 异步函数 (带缓存)
///
/// 根据DEX池的ObjectID，获取该池中交易对的两种代币的类型字符串 (CoinA, CoinB)。
/// 假设池对象的类型定义中，前两个泛型参数代表了交易对的代币类型。
///
/// 参数:
/// - `sui`: 一个对 `SuiClient` 的引用。
/// - `pool_id`: DEX池的ObjectID。
///
/// 返回:
/// - `Result<(String, String)>`: 成功则返回一个元组 `(coin_a_type, coin_b_type)`，否则返回错误。
#[cached(key = "String", convert = r##"{ pool_id.to_string() }"##, result = true)]
pub async fn get_pool_coins_type(sui: &SuiClient, pool_id: ObjectID) -> Result<(String, String)> {
    // 设置RPC请求选项，只请求对象的类型信息 (`with_type()`)，不需要完整内容。
    let object_data_options = SuiObjectDataOptions::default().with_type();
    // 通过Sui RPC获取池对象的信息
    let object_response = sui
        .read_api()
        .get_object_with_options(pool_id, object_data_options)
        .await? // 处理RPC错误
        .into_object()?; // 从响应中提取SuiObjectData，如果对象不存在或获取失败则返回错误

    // 从对象数据中获取其Move类型字符串 (例如 "0xPKG::module::Pool<0xTOKEN_A::A, 0xTOKEN_B::B>")
    let pool_type_str = object_response.object_type().map_err(|e| eyre!(e))?.to_string();
    // 将类型字符串解析为 `TypeTag` 枚举
    let type_tag =
        TypeTag::from_str(&pool_type_str).map_err(|_| eyre!("无效的池类型字符串: {}, 对象ID: {}", pool_type_str, pool_id))?;
    // 从 `TypeTag` 中提取 `StructTag` (如果它是一个结构体类型)
    let struct_tag_info = match type_tag {
        TypeTag::Struct(s_box) => *s_box, // 解引用Box获取StructTag
        _ => bail!("池类型 {} 不是一个结构体类型, 对象ID: {}", pool_type_str, pool_id), // 如果不是结构体则报错
    };

    // 确保结构体至少有两个泛型类型参数 (代表两种代币)
    ensure!(
        struct_tag_info.type_params.len() >= 2,
        "池类型 {} 的泛型参数少于2个, 对象ID: {}",
        pool_type_str,
        pool_id
    );

    // 提取前两个泛型参数作为代币A和代币B的类型字符串
    let coin_a_type = struct_tag_info.type_params[0].to_string();
    let normalized_coin_a = normalize_coin_type(&coin_a_type); // 规范化代币类型
    let coin_b_type = struct_tag_info.type_params[1].to_string();
    let normalized_coin_b = normalize_coin_type(&coin_b_type); // 规范化代币类型

    Ok((normalized_coin_a, normalized_coin_b))
}

/// `get_coin_in_out_v2!` 宏
///
/// 一个声明宏，用于根据池的ObjectID、一个实现了 `Simulator` trait 的provider，
/// 以及一个布尔值 `a2b` (表示交易方向是否为A到B)，来获取一个交易对的实际输入代币类型和输出代币类型。
///
/// 参数:
/// - `$pool`: 池的ObjectID (`ObjectID` 类型)。
/// - `$provider`: 实现了 `Simulator` trait 的对象 (例如 `Arc<dyn Simulator>`)。
/// - `$a2b`: 布尔值，`true` 表示交易方向是从池的第一个泛型代币到第二个，`false` 反之。
///
/// 返回:
/// - `Result<(String, String)>`: 成功则返回一个元组 `(coin_in_type, coin_out_type)`。
///   如果获取对象失败、对象不是Move对象、或类型参数不符合预期，则返回错误。
#[macro_export] // 将宏导出，使其在crate内部其他地方可用
macro_rules! get_coin_in_out_v2 {
    ($pool_id:expr, $simulator_provider:expr, $is_a_to_b:expr) => {{ // 宏的匹配模式和代码块
        // 从模拟器获取池对象的内部数据 (`obj_inner`)
        let object_data_inner = $simulator_provider
            .get_object(&$pool_id) // 调用模拟器的 get_object 方法
            .await // 等待异步操作完成
            .ok_or_else(|| eyre!("使用模拟器未能找到对象ID: {}", $pool_id))? // 如果对象未找到，则返回错误
            .into_inner(); // 获取 ObjectReadResult 中的 Object

        // 尝试将对象数据转换为 MoveObject
        let move_object_data = object_data_inner
            .data
            .try_as_move() // 获取 SuiMoveObject (如果对象是Move对象)
            .ok_or_else(|| eyre!("对象 {} 不是一个有效的Move对象", $pool_id))?; // 如果不是Move对象，则返回错误

        // 获取Move对象的类型参数列表
        let type_parameters = move_object_data.type_().type_params(); // type_() 返回 &StructTag
        // 提取第一个泛型参数作为代币A的类型
        let coin_a_type_str = match type_parameters.first() { // 获取第一个类型参数 (Option<&TypeTag>)
            Some(sui_sdk::types::TypeTag::Struct(struct_type_tag)) => { // 如果是Struct类型
                // 将StructTag格式化为 "0xADDRESS::module::name" 的形式并规范化
                $crate::normalize_coin_type(&format!("0x{}::{}::{}", struct_type_tag.address, struct_type_tag.module, struct_type_tag.name))
            }
            _ => return Err(eyre!("池 {} 缺少第一个泛型类型参数或类型不正确", $pool_id)), // 如果缺少或类型不符则报错
        };

        // 提取第二个泛型参数作为代币B的类型
        let coin_b_type_str = match type_parameters.get(1) { // 获取第二个类型参数
            Some(sui_sdk::types::TypeTag::Struct(struct_type_tag)) => {
                $crate::normalize_coin_type(&format!("0x{}::{}::{}", struct_type_tag.address, struct_type_tag.module, struct_type_tag.name))
            }
            _ => return Err(eyre!("池 {} 缺少第二个泛型类型参数或类型不正确", $pool_id)),
        };

        // 根据 `$is_a_to_b` 参数决定实际的输入和输出代币类型
        if $is_a_to_b {
            (coin_a_type_str, coin_b_type_str) // A是输入, B是输出
        } else {
            (coin_b_type_str, coin_a_type_str) // B是输入, A是输出
        }
    }};
}

/// `get_children_ids` 异步函数
///
/// 获取指定父对象ID的所有动态子对象的ID列表。
/// **注意**: 此函数直接创建一个新的 `SuiClient` 实例，并依赖 `SUI_RPC_NODE` 常量。
/// 在高频调用场景下，应考虑复用客户端实例。
///
/// 参数:
/// - `parent_object_id`: 父对象的ObjectID。
///
/// 返回:
/// - `Result<Vec<String>>`: 包含所有子对象ID字符串的向量。
pub async fn get_children_ids(parent_object_id: ObjectID) -> Result<Vec<String>> {
    // 创建SuiClient (如果SUI_RPC_NODE为空，这里会panic)
    let sui_client = SuiClientBuilder::default().build(SUI_RPC_NODE).await.unwrap();
    let mut next_page_cursor = None; // 用于分页查询的游标
    let mut children_ids_vec = vec![]; // 存储所有子对象ID

    loop { // 循环直到获取所有分页数据
        // 调用Sui RPC API的 `get_dynamic_fields` 方法获取一页动态字段信息
        let dynamic_fields_page = sui_client
            .read_api()
            .get_dynamic_fields(parent_object_id, next_page_cursor, None) // None表示默认分页大小
            .await?;
        next_page_cursor = dynamic_fields_page.next_cursor; // 更新游标以获取下一页
        // 从当前页数据中提取所有子对象的ID，并转换为字符串格式
        let current_page_children_ids = dynamic_fields_page
            .data
            .iter()
            .map(|field_info| field_info.object_id.to_string());
        children_ids_vec.extend(current_page_children_ids); // 追加到总列表
        if !dynamic_fields_page.has_next_page { // 如果没有下一页数据，则退出循环
            break;
        }
    }

    Ok(children_ids_vec)
}

// --- 用于简化 MoveStructLayout 创建的声明宏 ---
// 这些宏使得定义Move结构体的字段和类型布局更加简洁易读。

/// `move_field_layout!` 宏
///
/// 创建一个 `MoveFieldLayout` 实例。
///
/// 用法: `move_field_layout!("field_name", field_type_layout)`
#[macro_export]
macro_rules! move_field_layout {
    ($field_name_literal:literal, $field_layout_expr:expr) => { // $name是字段名的字符串字面量, $layout是字段类型的MoveTypeLayout表达式
        move_core_types::annotated_value::MoveFieldLayout {
            name: move_core_types::identifier::Identifier::new($field_name_literal).unwrap(), // 从字符串创建Identifier
            layout: $field_layout_expr, // 字段的类型布局
        }
    };
}

/// `move_type_layout_struct!` 宏
///
/// 创建一个 `MoveTypeLayout::Struct` 实例。
///
/// 用法: `move_type_layout_struct!(move_struct_layout_instance)`
#[macro_export]
macro_rules! move_type_layout_struct {
    ($move_struct_layout_expr:expr) => { // $struct是MoveStructLayout的表达式
        move_core_types::annotated_value::MoveTypeLayout::Struct(Box::new($move_struct_layout_expr)) // 将MoveStructLayout包装在Box中
    };
}

/// `move_struct_layout!` 宏
///
/// 创建一个 `MoveStructLayout` 实例。
///
/// 用法: `move_struct_layout!(struct_tag_instance, vec_of_field_layouts)`
#[macro_export]
macro_rules! move_struct_layout {
    ($struct_type_tag_expr:expr, $fields_vec_expr:expr) => { // $type_是StructTag表达式, $fields是MoveFieldLayout的Vec表达式
        move_core_types::annotated_value::MoveStructLayout {
            type_: $struct_type_tag_expr, // 结构体的类型标签 (StructTag)
            fields: Box::new($fields_vec_expr), // 结构体字段布局的向量 (包装在Box中)
        }
    };
}

// --- 测试模块 ---
#[cfg(test)]
mod tests {
    use std::sync::Arc; // 原子引用计数

    use simulator::{DBSimulator, Simulator}; // 数据库模拟器和模拟器trait
    use sui_sdk::SuiClientBuilder; // Sui客户端构建器
    use sui_types::base_types::SuiAddress; // Sui地址类型

    use super::*; // 导入外部模块 (protocols::mod.rs) 的所有公共成员
    use crate::tests::TEST_HTTP_URL; // 从crate根的tests模块导入测试用RPC URL

    /// `test_get_coin_decimals` 测试函数
    ///
    /// 测试 `get_coin_decimals` 函数是否能正确获取代币精度。
    #[tokio::test]
    async fn test_get_coin_decimals() {
        // 如果 TEST_HTTP_URL 为空，则打印警告并跳过测试，以防CI环境因缺少配置而失败。
        if TEST_HTTP_URL.is_empty() {
            println!("警告: TEST_HTTP_URL为空，test_get_coin_decimals 将跳过。请在本地环境中配置有效的RPC URL进行测试。");
            return;
        }
        let sui_client = SuiClientBuilder::default().build(TEST_HTTP_URL).await.unwrap();
        // 使用一个已知的Cally代币类型进行测试
        let decimals_result = get_coin_decimals(
            &sui_client,
            "0x19bb4ac89056993bd6f76ddfcd4b152b41c0fda25d3f01b343e98af29756b150::cally::CALLY",
        )
        .await
        .unwrap();
        assert_eq!(decimals_result, 6, "Cally代币的精度应为6"); // Cally代币已知有6位精度
    }

    /// `test_get_pool_coins_type` 测试函数
    ///
    /// 测试 `get_pool_coins_type` 函数是否能正确获取池中两种代币的类型。
    #[tokio::test]
    async fn test_get_pool_coins_type() {
        if TEST_HTTP_URL.is_empty() {
            println!("警告: TEST_HTTP_URL为空，test_get_pool_coins_type 将跳过。");
            return;
        }
        let sui_client = SuiClientBuilder::default().build(TEST_HTTP_URL).await.unwrap();
        // 使用一个已知的池ID进行测试
        let pool_object_id: ObjectID = "0x863d838561f4e82b9dbf54a4634fbd7018ac118f5c64fb34aceb1fc0b5882b0a"
            .parse()
            .unwrap();

        let (coin_a_type_str, coin_b_type_str) = get_pool_coins_type(&sui_client, pool_object_id).await.unwrap();
        // 断言获取到的代币类型是否与预期一致
        assert_eq!(
            coin_a_type_str,
            "0x92baf7a2dcb487f54a3f8f0f7ffee6dd07517f1b94b05e89355995a371b7df35::xec::XEC", // 预期的代币A类型
            "获取的代币A类型不匹配"
        );
        assert_eq!(coin_b_type_str, "0x2::sui::SUI", "获取的代币B类型应为SUI"); // 预期的代币B类型
    }

    /// `test_debug_object_info` 测试函数
    ///
    /// 用于调试目的，获取并打印指定对象的详细信息及其Move布局。
    /// 测试命令示例: `cargo test --package dex-indexer --lib -- protocols::tests::test_debug_object_info --exact --show-output`
    #[tokio::test]
    async fn test_debug_object_info() {
        // 一个示例对象ID，用于调试。实际测试时可能需要替换为有效的对象ID。
        let object_id_to_debug =
            ObjectID::from_hex_literal("0x0fea99ed9c65068638963a81587c3b8cafb71dc38c545319f008f7e9feb2b5f8").unwrap();

        // 使用DBSimulator获取对象信息。如果本地数据库没有该对象，`new_test(true)` 可能允许回退到RPC查询。
        let simulator_instance: Arc<dyn Simulator> = Arc::new(DBSimulator::new_test(true).await);
        let object_data = simulator_instance.get_object(&object_id_to_debug).await.unwrap();
        println!("🔥 对象详细信息: {:?}", object_data); // 打印对象数据
        let object_layout = simulator_instance.get_object_layout(&object_id_to_debug).unwrap();
        println!("🧀 对象布局信息: {:?}", object_layout); // 打印对象布局
    }

    /// `test_debug_child_objects` 测试函数
    ///
    /// 用于调试目的，获取并打印指定地址拥有的所有对象。
    #[tokio::test]
    async fn test_debug_child_objects() {
        if TEST_HTTP_URL.is_empty() {
            println!("警告: TEST_HTTP_URL为空，test_debug_child_objects 将跳过。");
            return;
        }
        let sui_client = SuiClientBuilder::default().build(TEST_HTTP_URL).await.unwrap();
        // 一个示例Sui地址，用于查询其拥有的对象。
        let owner_address = SuiAddress::from_str("0x577f358f93a323a91766d98681acf0b60fc85415189860c0832872a2d8f18d19").unwrap();
        // 调用Sui RPC API的 `get_owned_objects` 方法
        let owned_objects_response = sui_client.read_api().get_owned_objects(owner_address, None, None, None).await.unwrap();
        println!("🧀 地址 {} 拥有的对象: {:?}", owner_address, owned_objects_response); // 打印结果
    }
}

[end of crates/dex-indexer/src/protocols/mod.rs]
