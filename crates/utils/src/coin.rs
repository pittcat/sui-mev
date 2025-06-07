// 该文件 `coin.rs` (位于 `utils` crate中) 提供了一系列与Sui代币 (Coin) 操作相关的辅助工具函数。
// 这些函数封装了与 `SuiClient` 交互以获取代币信息、格式化代币数量以及创建模拟代币对象的逻辑。
//
// **文件概览 (File Overview)**:
// 这个文件是 `utils` 工具库中专门负责处理“钱”（Sui上的代币）的“小工具箱”。
// 它提供了一些方便的功能，让程序的其他部分在需要获取账户的Gas币、特定类型的代币，
// 或者需要一个假的代币对象来进行测试/模拟时，能够更容易地操作。
//
// **主要功能 (Key Functions)**:
//
// 1.  **`get_gas_coin_refs()` 异步函数**:
//     -   **功能**: 获取指定Sui地址 (`owner`) 所拥有的所有SUI原生代币 (Gas币) 的对象引用 (`ObjectRef`) 列表。
//         `ObjectRef` 包含了对象的ID、版本号和摘要，是交易中引用对象所必需的。
//     -   **参数**:
//         -   `sui: &SuiClient`: Sui RPC客户端实例。
//         -   `owner: SuiAddress`: 要查询Gas币的账户地址。
//         -   `exclude: Option<ObjectID>`: (可选) 一个要从结果中排除的特定Gas币的ObjectID。
//             这在某些场景下（例如，不想使用某个特定的Gas币对象作为支付Gas）可能有用。
//     -   **实现**: 调用 `sui.coin_read_api().get_coins()` 方法，不指定 `coin_type` (默认为SUI)，
//         然后过滤掉被排除的ObjectID，并收集所有符合条件的Gas币的 `ObjectRef`。
//
// 2.  **`get_coins()` 异步函数**:
//     -   **功能**: 获取指定Sui地址 (`owner`) 所拥有的、特定代币类型 (`coin_type`) 且余额不小于 `min_balance` 的所有代币对象 (`Coin`) 的列表。
//         `Coin` 是Sui SDK中定义的结构体，包含了代币的ObjectID、版本、摘要、类型、余额等信息。
//     -   **实现**: 调用 `sui.coin_read_api().get_coins()` 方法，并传入 `coin_type` 和可选的游标/限制参数 (这里都用 `None`)。
//         然后对结果进行过滤，只保留余额 (`coin.balance`) 大于或等于 `min_balance` 的代币。
//
// 3.  **`get_coin()` 异步函数**:
//     -   **功能**: 与 `get_coins()` 类似，但只返回**第一个**满足条件的代币对象 (`Coin`)。
//     -   **用途**: 当你只需要一个符合特定类型和最低余额要求的代币对象，而不在乎是哪一个时，这个函数很方便。
//     -   **实现**: 调用 `get_coins()` 获取列表，然后取列表中的第一个元素。如果列表为空，则返回错误。
//
// 4.  **`mocked_sui()` 函数**:
//     -   **功能**: 创建一个“模拟的”或“假的”SUI原生代币对象 (`Object`)。
//         这个函数主要用于测试或模拟环境中，当你需要一个SUI代币对象作为输入或Gas支付，
//         但又不想或不能使用链上真实的SUI代币对象时。
//     -   **实现**: 调用 `Object::with_id_owner_gas_for_testing()` 方法。
//         它会创建一个具有固定（但通常是无效的、仅用于测试的）ObjectID (`0x...1338`)、
//         指定所有者 (`owner`) 和指定面额 (`amount`) 的SUI代币对象。
//         这个对象的 `previous_transaction` 通常会被设置为创世摘要。
//
// 5.  **`is_native_coin()` 函数**:
//     -   **功能**: 检查给定的代币类型字符串 (`coin_type`) 是否是Sui的原生代币 (SUI)。
//     -   **实现**: 直接将输入的 `coin_type` 与 `SUI_COIN_TYPE` (Sui SDK中定义的常量 "0x2::sui::SUI") 进行比较。
//
// 6.  **`format_sui_with_symbol()` 函数**:
//     -   **功能**: 将一个 `u64` 类型的SUI代币值（通常是以最小单位MIST表示）格式化为一个带 " SUI" 后缀的、
//         更易读的浮点数表示形式。
//     -   **实现**:
//         1.  定义 `one_sui = 1_000_000_000.0` (1 SUI = 10^9 MIST)。
//         2.  将输入的 `u64` 值转换为 `f64`，然后除以 `one_sui`，得到以SUI为单位的浮点数值。
//         3.  使用 `format!("{} SUI", value)` 将这个浮点数值和 " SUI" 符号组合成一个字符串。
//
// **Sui相关的概念解释 (Sui-related Concepts)**:
//
// -   **Coin Object (代币对象)**:
//     在Sui中，代币（包括SUI原生代币和其他自定义代币）是以对象的形式存在的。每个代币对象都有自己的ObjectID、版本、所有者和余额。
//     一个账户可能拥有多个同类型的代币对象，每个对象有不同的余额。
//
// -   **ObjectRef (对象引用)**:
//     当交易需要使用某个链上对象作为输入时，它通常是通过提供该对象的 `ObjectRef` 来指定的。
//     `ObjectRef` 包含对象的ID、版本号 (`SequenceNumber`) 和摘要 (`ObjectDigest`)。
//     使用 `ObjectRef` 可以确保交易引用的是特定版本的对象，这对于防止双花和保证交易的确定性非常重要。
//     Gas币在交易中就是通过 `ObjectRef` 来指定的。
//
// -   **Coin Read API (`sui.coin_read_api()`)**:
//     Sui SDK的 `SuiClient` 提供的一组RPC接口，专门用于查询与代币相关的信息，例如：
//     -   `get_coins()`: 获取指定地址拥有的特定类型（或所有类型）的代币列表。
//     -   `get_balance()`: 获取指定地址拥有的特定类型代币的总余额。
//     -   `get_coin_metadata()`: 获取特定代币类型的元数据（如精度、名称、符号等）。
//
// -   **MIST**:
//     SUI原生代币的最小单位。1 SUI = 10^9 MIST。交易的Gas计算和很多链上金额通常是以MIST为单位的。

// 引入标准库的 FromStr trait，用于从字符串转换。
use std::str::FromStr;

// 引入 eyre 库，用于更方便的错误处理和上下文管理。
use eyre::{eyre, Result};
// 引入 Sui SDK 中的 Coin (RPC返回的代币结构), SuiClient (RPC客户端), SUI_COIN_TYPE (SUI原生代币类型常量)。
use sui_sdk::{rpc_types::Coin, SuiClient, SUI_COIN_TYPE};
// 引入 Sui 核心类型库中的 ObjectID, ObjectRef, SuiAddress, Object。
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // 对象ID, 对象引用, Sui地址
    object::Object,                               // Sui对象结构
};

/// `get_gas_coin_refs` 异步函数
///
/// 获取指定Sui地址 (`owner`) 所拥有的所有SUI原生代币 (Gas币) 的对象引用 (`ObjectRef`) 列表。
///
/// 参数:
/// - `sui`: 一个对 `SuiClient` 的引用，用于与Sui网络交互。
/// - `owner`: 要查询Gas币的账户地址。
/// - `exclude`: (可选) 一个要从结果中排除的特定Gas币的ObjectID。
///
/// 返回:
/// - `Result<Vec<ObjectRef>>`: 成功则返回包含多个 `ObjectRef` 的向量，否则返回错误。
pub async fn get_gas_coin_refs(
    sui: &SuiClient,
    owner: SuiAddress,
    exclude: Option<ObjectID>,
) -> Result<Vec<ObjectRef>> {
    // 调用 Sui RPC API 的 `get_coins` 方法。
    // - `owner`: 指定查询的地址。
    // - `None` for coin_type: 表示查询所有类型的代币，但由于Gas币只能是SUI，这里实际上是获取SUI代币。
    //   （更精确的做法是传入 `Some(SUI_COIN_TYPE.to_string())`，但对于Gas币，`None` 通常也有效）。
    // - `None` for cursor: 从头开始查询。
    // - `None` for limit: 获取默认数量或所有代币。
    let coins_response = sui.coin_read_api().get_coins(owner, None, None, None).await?;

    // 从响应中提取数据，并进行处理。
    let object_refs_vec = coins_response
        .data // `data` 字段是一个 `Vec<Coin>`
        .into_iter() // 将其转换为迭代器
        .filter_map(|coin_data| { // `filter_map` 用于过滤并转换元素
            if let Some(excluded_id) = exclude { // 如果提供了要排除的ID
                if coin_data.coin_object_id == excluded_id { // 如果当前代币ID与要排除的ID相同
                    return None; // 则过滤掉这个代币 (返回None)
                }
            }
            // 对于未被排除的代币，调用其 `object_ref()` 方法获取 `ObjectRef`。
            // `Coin` 结构体应该有一个返回其 `ObjectRef` 的方法。
            Some(coin_data.object_ref())
        })
        .collect(); // 将所有有效的 `ObjectRef` 收集到一个新的向量中

    Ok(object_refs_vec)
}

/// `get_coins` 异步函数
///
/// 获取指定Sui地址 (`owner`) 所拥有的、特定代币类型 (`coin_type`)
/// 且余额不小于 `min_balance` 的所有代币对象 (`Coin`) 的列表。
///
/// 参数:
/// - `sui`: `SuiClient` 引用。
/// - `owner`: 账户地址。
/// - `coin_type`: 要查询的代币类型字符串 (例如 "0x2::sui::SUI")。
/// - `min_balance`: 要求的最小余额。
///
/// 返回:
/// - `Result<Vec<Coin>>`: 包含符合条件代币对象的向量。
pub async fn get_coins(sui: &SuiClient, owner: SuiAddress, coin_type: &str, min_balance: u64) -> Result<Vec<Coin>> {
    // 调用 Sui RPC API 的 `get_coins` 方法，这次指定了 `coin_type`。
    let coins_response = sui
        .coin_read_api()
        .get_coins(owner, Some(coin_type.to_string()), None, None)
        .await?;

    // 从响应中过滤出余额大于或等于 `min_balance` 的代币。
    let filtered_coins = coins_response
        .data
        .into_iter()
        .filter(|coin_obj| coin_obj.balance >= min_balance) // 只保留余额符合条件的
        .collect();

    Ok(filtered_coins)
}

/// `get_coin` 异步函数
///
/// 获取指定Sui地址 (`owner`) 拥有的、特定代币类型 (`coin_type`)
/// 且余额不小于 `min_balance` 的**第一个**代币对象 (`Coin`)。
///
/// 参数: (与 `get_coins` 相同)
///
/// 返回:
/// - `Result<Coin>`: 如果找到符合条件的代币，则返回该代币对象。
///   如果未找到，则返回一个包含错误信息的 `Err`。
pub async fn get_coin(sui: &SuiClient, owner: SuiAddress, coin_type: &str, min_balance: u64) -> Result<Coin> {
    // 调用上面定义的 `get_coins` 函数获取所有符合条件的代币列表。
    let coins_list = get_coins(sui, owner, coin_type, min_balance).await?;

    // 从列表中取第一个元素。
    coins_list
        .into_iter() // 将Vec转换为迭代器
        .next()      // 获取迭代器的第一个元素 (Option<Coin>)
        .ok_or_else(|| eyre!("未找到余额大于或等于 {} 的 {} 代币 (No coins with balance >= {} for type {})", min_balance, coin_type, min_balance, coin_type)) // 如果为None (列表为空)，则返回错误
}

/// `mocked_sui` 函数
///
/// 创建一个“模拟的”或“假的”SUI原生代币对象 (`Object`)。
/// 主要用于测试或模拟环境中，当你需要一个SUI代币对象但不想或不能使用链上真实对象时。
///
/// 参数:
/// - `owner`: 模拟代币的所有者地址。
/// - `amount`: 模拟代币的面额 (以MIST为单位)。
///
/// 返回:
/// - `Object`: 新创建的模拟SUI代币对象。
pub fn mocked_sui(owner: SuiAddress, amount: u64) -> Object {
    // `Object::with_id_owner_gas_for_testing` 是Sui SDK提供的测试工具函数。
    // 它创建一个具有指定ID、所有者和面额的SUI Gas币对象。
    // ObjectID::from_str(...) 用于从一个固定的十六进制字符串创建一个ObjectID。
    // 这个ID "0x...1338" 是一个任意选择的、不太可能与真实对象冲突的ID，仅用于测试。
    Object::with_id_owner_gas_for_testing(
        ObjectID::from_str("0x0000000000000000000000000000000000000000000000000000000000001338").unwrap(), // 固定的测试用ObjectID
        owner,  // 指定所有者
        amount, // 指定面额
    )
}

/// `is_native_coin` 函数
///
/// 检查给定的代币类型字符串 (`coin_type`) 是否代表Sui的原生代币 (SUI)。
///
/// 参数:
/// - `coin_type`: 要检查的代币类型字符串。
///
/// 返回:
/// - `bool`: 如果是SUI原生代币则为 `true`，否则为 `false`。
pub fn is_native_coin(coin_type: &str) -> bool {
    // `SUI_COIN_TYPE` 是Sui SDK中定义的常量，值为 "0x2::sui::SUI"。
    coin_type == SUI_COIN_TYPE
}

/// `format_sui_with_symbol` 函数
///
/// 将一个 `u64` 类型的SUI代币值（通常是以最小单位MIST表示）
/// 格式化为一个带 " SUI" 后缀的、更易读的浮点数表示形式。
///
/// 参数:
/// - `value`: 以MIST为单位的SUI数量。
///
/// 返回:
/// - `String`: 格式化后的字符串，例如 "123.45 SUI"。
pub fn format_sui_with_symbol(value: u64) -> String {
    // 1 SUI 等于 10^9 MIST。
    let one_sui_in_mist = 1_000_000_000.0; // 使用浮点数以便进行除法
    // 将输入的MIST值转换为f64，然后除以 one_sui_in_mist 得到以SUI为单位的浮点数值。
    let value_in_sui = value as f64 / one_sui_in_mist;

    // 使用 format! 宏将浮点数值和 " SUI" 符号组合成一个字符串。
    // 默认的浮点数格式化可能包含很多小数位，如果需要特定精度，可以使用格式化选项，
    // 例如 format!("{:.2} SUI", value_in_sui) 来保留两位小数。
    format!("{} SUI", value_in_sui)
}

[end of crates/utils/src/coin.rs]
