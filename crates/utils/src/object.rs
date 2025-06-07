// 该文件 `object.rs` (位于 `utils` crate中) 提供了一系列与Sui链上对象 (Object)
// 特别是 MoveStruct (Move结构体) 内容提取相关的辅助工具函数。
// 当从链上获取到一个已反序列化的Move对象 (`MoveStruct`) 后，其内部字段是以 `MoveValue` 枚举的形式存在的。
// 这个文件中的函数就是为了方便地从 `MoveStruct` 中按字段名提取特定类型的 `MoveValue`，
// 并将其转换为更具体的Rust原生类型 (如 `ObjectID`, `u64`, `Vec<u64>` 等)。
// 它还提供了一个函数 `shared_obj_arg` 来根据 `Object` 的所有者信息创建 `ObjectArg::SharedObject`。
//
// **文件概览 (File Overview)**:
// 这个文件是 `utils` 工具库中专门负责“解读Sui对象内部构造”的“小工具箱”。
// Sui上的对象（比如一个DEX的交易池对象）是用Move语言定义的，当程序读取这些对象时，
// 它们的内容（字段和值）会以一种叫做 `MoveStruct` 的通用格式表示。
// 这个文件里的函数就是帮助我们从这个 `MoveStruct` 中准确地拿出我们想要的特定字段的值，
// 并且确保这个值的类型是我们期望的。
//
// **核心功能 (Key Functions)**:
//
// 1.  **`extract_*_from_move_struct()` 系列函数**:
//     -   **通用模式**:
//         -   这些函数都接收一个 `&MoveStruct` (对Move结构体的引用) 和一个 `field_name: &str` (要提取的字段名) 作为参数。
//         -   它们首先调用 `sui_types::dynamic_field::extract_field_from_move_struct()` (一个Sui官方库提供的函数)
//             来尝试从 `MoveStruct` 中按名称提取出对应的 `MoveValue`。
//             如果找不到该字段，则返回一个包含 "field not found" 信息的错误。
//         -   然后，它们使用 `match` 语句来检查提取到的 `MoveValue` 是否是期望的类型变体
//             (例如，`MoveValue::Struct` for `extract_struct_from_move_struct`,
//             `MoveValue::Address` for `extract_object_id_from_move_struct`,
//             `MoveValue::U64` for `extract_u64_from_move_struct` 等)。
//         -   如果类型匹配，则提取并返回内部的值 (可能需要克隆或转换)。
//         -   如果类型不匹配，则使用 `bail!` 宏 (来自 `eyre` 库) 返回一个描述期望类型的错误。
//     -   **具体函数列表**:
//         -   `extract_struct_from_move_struct()`: 提取一个嵌套的 `MoveStruct`。
//         -   `extract_vec_from_move_struct()`: 提取一个 `Vec<MoveValue>` (Move向量)。
//         -   `extract_object_id_from_move_struct()`: 提取一个 `MoveValue::Address` 并将其转换为 `ObjectID`。
//         -   `extract_struct_array_from_move_struct()`: 提取一个 `Vec<MoveStruct>` (Move结构体向量)。
//             它会遍历 `MoveValue::Vector` 中的每个元素，并确保每个元素都是 `MoveValue::Struct`。
//         -   `extract_u128_from_move_struct()`: 提取 `u128` 类型的值。
//         -   `extract_u64_from_move_struct()`: 提取 `u64` 类型的值。
//         -   `extract_u32_from_move_struct()`: 提取 `u32` 类型的值。
//         -   `extract_bool_from_move_struct()`: 提取 `bool` 类型的值。
//         -   `extract_u64_vec_from_move_struct()`: 提取 `Vec<u64>`。
//         -   `extract_u128_vec_from_move_struct()`: 提取 `Vec<u128>`。
//
// 2.  **`shared_obj_arg(obj: &Object, mutable: bool) -> ObjectArg` 函数**:
//     -   **功能**: 根据一个给定的Sui `Object` 实例及其期望的可变性 (`mutable`)，
//         创建一个用于可编程交易块 (PTB) 的 `ObjectArg::SharedObject`。
//     -   **实现**:
//         1.  检查输入对象 `obj` 的所有者信息 (`obj.owner()`)。
//         2.  如果对象是共享的 (`Owner::Shared { initial_shared_version }`)，
//             则使用其 `initial_shared_version`。
//         3.  如果对象不是共享的 (例如，它是私有的或不可变的)，则默认使用 `SequenceNumber::from_u64(0)`
//             作为初始共享版本。**这可能是一个简化或默认行为，因为非共享对象通常不直接用作 `ObjectArg::SharedObject`。
//             然而，如果一个对象即将被共享，或者在某些特殊构造PTB的场景下，可能会有这种用法。
//             更常见的做法是，对于非共享对象，会使用 `ObjectArg::ImmOrOwnedObject`。
//             这个函数专门用于创建 `SharedObject` 类型的 `ObjectArg`，所以调用者应确保传入的对象确实是（或将被视为）共享对象。**
//         4.  使用对象的ID (`obj.id()`)、获取到的初始共享版本和传入的 `mutable` 标志
//             来构造并返回一个 `ObjectArg::SharedObject`。
//
// **用途 (Purpose in Project)**:
// -   **简化对象解析**: 当 `dex-indexer` 或套利机器人的其他部分从Sui链上获取了对象数据 (通常是 `Object` 类型，
//     其 `data` 字段是 `Data::Move`，包含一个 `MoveObject`，而 `MoveObject::contents` 是BCS编码的字节)，
//     并且这些内容已经被反序列化为 `MoveStruct` 后，这些工具函数就非常有用。
//     它们提供了一种类型安全的方式来访问 `MoveStruct` 中的特定字段，而无需手动编写繁琐的 `match` 语句和类型转换逻辑。
// -   **构建交易参数**: `shared_obj_arg` 函数简化了为PTB准备共享对象参数的过程。
//
// **MoveValue 枚举**:
// `MoveValue` 是 `move_core_types` 中定义的一个枚举，它可以表示Move语言中的任何值类型，例如：
// `U8`, `U16`, `U32`, `U64`, `U128`, `U256`, `Bool`, `Address`, `Signer`,
// `Vector(Vec<MoveValue>)`, `Struct(MoveStruct)` 等。
// 当从链上读取Move对象的字段时，得到的就是这些 `MoveValue`。

// 引入 eyre 库，用于更方便的错误处理和上下文管理。
use eyre::{bail, OptionExt, Result}; // bail! 宏用于快速返回错误，OptionExt 提供了 ok_or_eyre 等方法。
// 引入 Move 核心类型中的 MoveStruct (表示已反序列化的Move结构体) 和 MoveValue (表示Move中的任何值)。
use move_core_types::annotated_value::{MoveStruct, MoveValue};
// 引入 Sui 核心类型库中的相关类型。
use sui_types::{
    base_types::{ObjectID, SequenceNumber}, // 对象ID, 对象版本号 (SequenceNumber 通常用于共享对象版本或对象元数据版本)
    dynamic_field::extract_field_from_move_struct, // 从MoveStruct中按名称提取字段值的辅助函数
    object::{Object, Owner},                // Sui对象结构 (Object) 和所有者类型 (Owner)
    transaction::ObjectArg,                 // 用于构建可编程交易块(PTB)时表示对象参数的枚举
};

/// `extract_struct_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个嵌套的 `MoveStruct`。
///
/// 参数:
/// - `move_struct`: 对源 `MoveStruct` 的引用。
/// - `field_name`: 要提取的字段的名称。
///
/// 返回:
/// - `Result<MoveStruct>`: 如果成功提取并且字段类型为 `MoveValue::Struct`，则返回克隆的嵌套 `MoveStruct`。
///   否则返回错误 (例如字段未找到或类型不匹配)。
pub fn extract_struct_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<MoveStruct> {
    // 调用Sui提供的辅助函数尝试从 `move_struct` 中提取名为 `field_name` 的字段。
    // `ok_or_eyre` 会在 `Option` 为 `None` 时返回一个包含 "field not found" 信息的错误。
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;

    // 匹配提取到的 `MoveValue` 的类型。
    match move_value {
        MoveValue::Struct(nested_move_struct) => Ok(nested_move_struct.clone()), // 如果是Struct类型，则克隆并返回
        _ => bail!("字段 '{}' 的类型不是预期的Struct，实际类型为: {:?}", field_name, move_value.type_name()), // 否则返回类型不匹配的错误
    }
}

/// `extract_vec_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `Vec<MoveValue>` (Move向量)。
pub fn extract_vec_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<Vec<MoveValue>> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::Vector(move_vec) => Ok(move_vec.clone()), // 如果是Vector类型，则克隆并返回
        _ => bail!("字段 '{}' 的类型不是预期的Vector，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `extract_object_id_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `MoveValue::Address`，并将其转换为 `ObjectID`。
/// 在Move中，ObjectID 通常以 `address` 类型表示。
pub fn extract_object_id_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<ObjectID> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::Address(account_address) => Ok(ObjectID::from_address(*account_address)), // 如果是Address类型，则转换为ObjectID并返回
        _ => bail!("字段 '{}' 的类型不是预期的Address (ObjectID)，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `extract_struct_array_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `Vec<MoveStruct>` (Move结构体向量)。
/// 它会检查向量中的每个元素是否都是 `MoveValue::Struct`。
pub fn extract_struct_array_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<Vec<MoveStruct>> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::Vector(move_vector) => {
            // 遍历向量中的每个 `MoveValue`
            let structs_vec = move_vector
                .iter()
                .map(|val| match val { // 确保每个元素都是Struct类型
                    MoveValue::Struct(inner_move_struct) => Ok(inner_move_struct.clone()),
                    _ => bail!("向量字段 '{}' 中的元素类型不是预期的Struct，实际类型为: {:?}", field_name, val.type_name()),
                })
                .collect::<Result<Vec<_>>>()?; // 将Result<Vec<MoveStruct>>收集起来，如果任何元素转换失败则整个Result为Err

            Ok(structs_vec)
        }
        _ => bail!("字段 '{}' 的类型不是预期的Vector (用于Struct数组)，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `extract_u128_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `u128` 类型的值。
pub fn extract_u128_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<u128> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::U128(u_val) => Ok(*u_val), // 如果是U128类型，则解引用并返回
        _ => bail!("字段 '{}' 的类型不是预期的U128，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `extract_u64_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `u64` 类型的值。
pub fn extract_u64_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<u64> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::U64(u_val) => Ok(*u_val), // 如果是U64类型，则解引用并返回
        _ => bail!("字段 '{}' 的类型不是预期的U64，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `extract_u32_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `u32` 类型的值。
pub fn extract_u32_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<u32> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::U32(u_val) => Ok(*u_val), // 如果是U32类型，则解引用并返回
        _ => bail!("字段 '{}' 的类型不是预期的U32，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `extract_bool_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `bool` 类型的值。
pub fn extract_bool_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<bool> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::Bool(b_val) => Ok(*b_val), // 如果是Bool类型，则解引用并返回
        _ => bail!("字段 '{}' 的类型不是预期的Bool，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `extract_u64_vec_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `Vec<u64>`。
/// 它会检查向量中的每个元素是否都是 `MoveValue::U64`。
pub fn extract_u64_vec_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<Vec<u64>> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::Vector(move_vector) => {
            let u64_values = move_vector
                .iter()
                .map(|val| match val { // 确保每个元素都是U64类型
                    MoveValue::U64(u_val_inner) => Ok(*u_val_inner),
                    _ => bail!("向量字段 '{}' 中的元素类型不是预期的U64，实际类型为: {:?}", field_name, val.type_name()),
                })
                .collect::<Result<Vec<_>>>()?; // 收集为Result<Vec<u64>>
            Ok(u64_values)
        }
        _ => bail!("字段 '{}' 的类型不是预期的Vector (用于u64数组)，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `extract_u128_vec_from_move_struct` 函数
///
/// 从一个给定的 `MoveStruct` 中按字段名提取一个 `Vec<u128>`。
/// 它会检查向量中的每个元素是否都是 `MoveValue::U128`。
pub fn extract_u128_vec_from_move_struct(move_struct: &MoveStruct, field_name: &str) -> Result<Vec<u128>> {
    let move_value = extract_field_from_move_struct(move_struct, field_name).ok_or_eyre(format!("字段 '{}' 在MoveStruct中未找到", field_name))?;
    match move_value {
        MoveValue::Vector(move_vector) => {
            let u128_values = move_vector
                .iter()
                .map(|val| match val { // 确保每个元素都是U128类型
                    MoveValue::U128(u_val_inner) => Ok(*u_val_inner),
                    _ => bail!("向量字段 '{}' 中的元素类型不是预期的U128，实际类型为: {:?}", field_name, val.type_name()),
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(u128_values)
        }
        _ => bail!("字段 '{}' 的类型不是预期的Vector (用于u128数组)，实际类型为: {:?}", field_name, move_value.type_name()),
    }
}

/// `shared_obj_arg` 函数
///
/// 根据一个Sui `Object` 实例及其期望的可变性 (`mutable`)，
/// 创建一个用于可编程交易块 (PTB) 的 `ObjectArg::SharedObject`。
///
/// 参数:
/// - `obj`: 对要作为共享对象引用的 `Object` 的引用。
/// - `mutable`: 一个布尔值，指示此共享对象在交易中是否需要以可变方式引用。
///
/// 返回:
/// - `ObjectArg`: 构造好的 `ObjectArg::SharedObject`。
pub fn shared_obj_arg(obj: &Object, mutable: bool) -> ObjectArg {
    // 获取对象的初始共享版本号。
    // `obj.owner()` 返回对象的所有者信息。
    let initial_shared_version_val = match obj.owner() {
        Owner::Shared { initial_shared_version } => *initial_shared_version, // 如果是共享对象，则使用其 initial_shared_version
        _ => SequenceNumber::from_u64(0), // 如果对象不是共享的 (例如私有或不可变)，
                                          // 则默认使用版本号0。这可能是一个简化处理，
                                          // 因为非共享对象通常不直接作为 SharedObject 类型的 ObjectArg。
                                          // 调用此函数时，调用者应确保传入的对象确实是（或将被视为）共享对象。
                                          // 如果一个对象即将被共享，它的初始共享版本可能确实从0或1开始。
    };

    // 构建并返回 ObjectArg::SharedObject
    ObjectArg::SharedObject {
        id: obj.id(), // 对象的ID
        initial_shared_version: initial_shared_version_val, // 共享对象的初始版本号
        mutable, // 是否可变引用
    }
}

[end of crates/utils/src/object.rs]
