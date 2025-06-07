// 该文件 `navi.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 主要负责提供与 Navi Protocol (Sui上的一个借贷协议) 相关的核心对象ID列表。
// 从文件顶部的注释 `//! Navi is used exclusively for flashloan.` 来看，
// 在当前 `dex-indexer` 或相关套利机器人的上下文中，Navi协议主要被用于其闪电贷功能。
//
// **文件概览 (File Overview)**:
// 这个文件的核心功能是 `navi_related_object_ids()` 函数。
// 这个函数返回一个包含许多硬编码的Navi协议相关的核心对象ID字符串的列表。
// 这些对象ID对于与Navi协议交互（特别是进行闪电贷操作）或为其建立索引和缓存至关重要。
//
// **主要内容 (Main Contents)**:
// 1.  **`navi_related_object_ids()` 函数**:
//     -   **硬编码的核心对象ID (Hardcoded Core Object IDs)**:
//         函数内部首先定义了一个包含多个已知Navi协议对象ID的向量。这些ID字符串代表了：
//         -   不同版本的Navi协议包ID或核心模块ID (例如，`NaviProtocol 7`, `NaviProtocol 20` 等)。
//         -   特定的资金池对象ID (`NaviPool` - 根据注释，这特指SUI的资金池)。
//         -   协议的全局配置对象ID (`NaviConfig`)。
//         -   协议的资产配置对象ID (`NaviAssetConfig`)。
//         -   协议的存储对象ID (`NaviStorage` - 可能包含用户账户数据、利率模型等)。
//         -   协议的准备金数据对象ID (`NaviReserveData`)。
//         这些ID是与Navi协议交互时经常需要引用的关键对象。
//     -   **动态派生的存储子对象ID (Dynamically Derived Storage Children IDs)**:
//         函数接下来会尝试从一个特定的父对象ID（硬编码为 `0xe6d4c6610b86ce7735ea754596d71d72d10c7980b5052fc3c8cdf8d09fea9b4b`，
//         这可能是一个Navi协议内部用于组织其存储的“表”对象或类似结构）动态地派生出一系列子对象的ID。
//         -   它假设这些子对象是通过 `u8` 类型的键（从0到19）作为动态字段（Dynamic Field）挂载在该父对象下的。
//         -   `derive_dynamic_field_id` 函数用于根据父对象ID、键的类型标签 (`TypeTag::U8`) 和键的BCS序列化值来计算出子对象的ID。
//         -   这部分逻辑表明Navi协议可能将其某些内部状态（例如不同资产的储备数据或用户账户分片）分散存储在多个动态字段对象中。
//         -   收集这些子对象ID对于确保 `DBSimulator` 能够预加载Navi协议的完整或接近完整的状态非常重要。
//     -   **合并与返回 (Merging and Returning)**:
//         最后，函数将硬编码的核心对象ID列表和动态派生的子对象ID列表合并，并返回一个包含所有这些ID字符串的 `Vec<String>`。
//         这个列表会被 `dex-indexer` 用于预加载这些对象到数据库模拟器中，以便在模拟涉及Navi闪电贷的交易时，
//         能够拥有相对完整的链上状态视图，从而提高模拟的准确性和速度。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
//
// -   **借贷协议 (Lending Protocol)**:
//     允许用户存入资产赚取利息，或抵押资产借出其他资产的DeFi协议。Navi是Sui上的一个主要借贷协议。
//
// -   **闪电贷 (Flashloan)**:
//     一种特殊的无抵押贷款，要求在同一笔原子交易内完成借款和还款。
//     文件顶部的注释表明，Navi协议在这个项目中的主要用途就是提供闪电贷资金。
//
// -   **对象ID (ObjectID)**:
//     Sui上所有数据（包括合约、代币、状态对象等）都以对象的形式存在，每个对象都有唯一的ID。
//     `navi_related_object_ids()` 函数收集的就是这些ID。
//
// -   **动态字段 (Dynamic Fields)**:
//     Sui Move语言允许在一个对象（父对象）下动态地添加键值对形式的子对象。这些子对象被称为动态字段。
//     `derive_dynamic_field_id` 函数用于计算这些动态子对象的ID。
//     Navi协议似乎使用动态字段来组织其部分存储数据（例如，用u8作为键来索引不同的数据片）。
//
// -   **TypeTag (类型标签)**:
//     在Sui中，`TypeTag` 用于在运行时表示一个具体的Move类型。当派生动态字段ID时，需要提供键的 `TypeTag`。
//     例如，`TypeTag::U8` 表示键的类型是 `u8`。
//
// -   **BCS (Binary Canonical Serialization)**:
//     Sui和Move语言使用的标准二进制序列化格式。当派生动态字段ID时，键的值需要先进行BCS序列化。
//
// **用途 (Purpose in Dex-Indexer)**:
// `dex-indexer` 的目标之一是为链上模拟（特别是套利交易模拟）提供支持。
// 为了让模拟尽可能准确和高效，模拟器（如 `DBSimulator`）需要能够访问到所有可能在交易中被引用的对象的状态。
// `navi_related_object_ids()` 函数收集的这些ID列表，会被用于：
// 1.  **预加载数据到 `DBSimulator`**: 在模拟开始前，可以将这些ID对应的链上对象数据批量获取并加载到模拟器的本地数据库中。
// 2.  **确定索引范围**: 告诉索引器哪些对象是与Navi协议核心功能相关的，可能需要被重点监控或索引其状态变化（尽管此文件本身不执行索引，而是提供ID给执行索引的组件）。
// 由于Navi主要用于闪电贷，确保其资金池 (`NAVI_POOL` 在 `bin/arb/src/defi/navi.rs` 中定义)、配置和核心存储对象被正确加载，
// 对于模拟依赖Navi闪电贷的套利路径至关重要。

// 引入Sui核心类型中的ObjectID, TypeTag和动态字段派生函数。
use sui_types::{base_types::ObjectID, dynamic_field::derive_dynamic_field_id, TypeTag};

/// `navi_related_object_ids` 函数
///
/// 返回一个包含与Navi协议相关的所有核心及部分动态派生子对象的ID字符串列表。
/// 这些ID用于在 `dex-indexer` 或相关模拟器中预加载或识别Navi协议的关键链上状态。
///
/// **注意**: 此函数不执行异步操作，它返回的是一个硬编码和基于固定逻辑派生的ID列表。
/// 如果Navi协议升级并更改了这些核心对象的ID，或者其动态字段的组织方式发生变化，
/// 这个列表可能需要手动更新。
pub fn navi_related_object_ids() -> Vec<String> {
    // 步骤 1: 定义一个包含已知Navi协议核心对象ID的向量。
    // 这些通常是协议的包ID、全局配置对象、主要资金池、存储对象等的ID。
    // 这些ID是通过查看Navi协议的部署信息或文档获得的。
    let mut result_ids_vec = vec![
        // 不同版本的Navi协议包或核心模块的ID
        "0xd899cf7d2b5db716bd2cf55599fb0d5ee38a3061e7b6bb6eebf73fa5bc4c81ca", // NaviProtocol 7
        "0x06007a2d0ddd3ef4844c6d19c83f71475d6d3ac2d139188d6b62c052e6965edd", // NaviProtocol 9
        "0x834a86970ae93a73faf4fff16ae40bdb72b91c47be585fff19a2af60a19ddca3", // NaviProtocol 20 (这个与 `bin/arb/src/defi/navi.rs` 中的 `NAVI_PROTOCOL` 常量一致)
        "0x1951eff08b3fd5bd134df6787ec9ec533c682d74277b824dbd53e440926901ad", // NaviProtocol 21
        "0xc2d49bf5e75d2258ee5563efa527feb6155de7ac6f6bf025a23ee88cd12d5a83", // NaviProtocol 22
        // Navi SUI资金池对象ID (与 `bin/arb/src/defi/navi.rs` 中的 `NAVI_POOL` 常量一致)
        "0x96df0fce3c471489f4debaaa762cf960b3d97820bd1f3f025ff8190730e958c5", // NaviPool
        // Navi 全局配置对象ID (与 `bin/arb/src/defi/navi.rs` 中的 `NAVI_CONFIG` 常量一致)
        "0x3672b2bf471a60c30a03325f104f92fb195c9d337ba58072dce764fe2aa5e2dc", // NaviConfig
        // Navi 资产配置对象ID
        "0x3dea04b6029fa398581cfac0f70f6fcf6ff4ddd9e9852b1a7374395196394de1", // NaviAsset
        // Navi 另一种资产配置对象ID
        "0x48e3820fe5cc11bd6acf0115b496070c2e9d2077938a7818a06c23d0bb33ad69", // NaviAssetConfig
        // Navi 存储对象ID (与 `bin/arb/src/defi/navi.rs` 中的 `NAVI_STORAGE` 常量一致)
        "0xbb4e2f4b6205c2e2a2db47aeb4f830796ec7c005f88537ee775986639bc442fe", // NaviStorage
        // Navi 准备金数据对象ID
        "0x9a91a751ff83ef1eb940066a60900d479cbd39c6eaccdd203632c97dedd10ce9", // NaviReserveData
    ]
    .into_iter() // 将&str数组转换为迭代器
    .map(|s| s.to_string()) // 将每个&str转换为String
    .collect::<Vec<_>>(); // 收集为Vec<String>

    // 步骤 2: 动态派生一组存储子对象的ID。
    // 这部分代码假设Navi协议使用一个特定的父对象来存储一系列子对象，
    // 这些子对象通过从0到19的u8类型的键进行索引。
    let storage_children_ids_vec = {
        let mut derived_ids_vec = vec![]; // 用于存储派生出的子对象ID

        // 硬编码的父对象ID，Navi可能用它作为某个内部“表”或集合的根。
        let parent_object_id =
            ObjectID::from_hex_literal("0xe6d4c6610b86ce7735ea754596d71d72d10c7980b5052fc3c8cdf8d09fea9b4b").unwrap();

        // 动态字段的键的类型是 u8。
        let key_type_tag = TypeTag::U8;

        // 循环遍历键值从0到19 (总共20个)。
        for i in 0..20u8 { // u8类型，所以范围是0-255，这里只取前20个
            // 将u8类型的键进行BCS序列化。
            let key_bcs_bytes = bcs::to_bytes(&i).unwrap();
            // 使用父对象ID、键的类型标签和键的BCS序列化值来派生出动态子对象的ID。
            let child_object_id = derive_dynamic_field_id(parent_object_id, &key_type_tag, &key_bcs_bytes).unwrap();
            derived_ids_vec.push(child_object_id.to_string()); // 将派生的ID字符串添加到列表中
        }

        derived_ids_vec // 返回派生的子对象ID列表
    };

    // 将动态派生的子对象ID列表追加到核心对象ID列表中。
    result_ids_vec.extend(storage_children_ids_vec);

    result_ids_vec // 返回最终合并的ID列表
}

[end of crates/dex-indexer/src/protocols/navi.rs]
