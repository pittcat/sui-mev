// 该文件 `override_cache.rs` (位于 `simulator` crate 的 `db_simulator` 子模块下)
// 定义了 `OverrideCache` 结构体。这个缓存层在 `DBSimulator` 中扮演着关键角色，
// 允许在模拟交易时，使用一组预设的对象状态来“覆盖”从底层数据库（如 `WritebackCache`）读取到的状态。
// 这对于模拟特定场景（例如，基于某个MEV机会发生后的链状态进行模拟）或在不实际修改底层数据库的情况下
// 测试交易对某些对象状态的依赖性非常有用。
//
// **文件概览 (File Overview)**:
// 这个文件是 `DBSimulator` 的一个“自定义状态层”或“沙盒层”。
// 想象一下，`DBSimulator` 有一个大的“真实世界”数据库（通过 `WritebackCache` 访问）。
// 但有时，在模拟一个特定的交易时，我们想假设某些对象处于特定的状态，而不是它们在“真实世界”数据库中的当前状态。
// `OverrideCache` 就是用来实现这个“假设”的。
//
// **核心组件 (Core Components)**:
// 1.  **`ret_latest_clock_obj!` 宏**:
//     -   一个简单的声明宏，用于在需要时动态创建一个表示当前最新时间的Sui `Clock` 对象。
//     -   Sui的 `Clock` 对象 (ObjectID `0x6`) 是一个共享对象，其 `timestamp_ms` 字段会随时间更新。
//     -   在模拟中，如果 `OverrideCache` 被请求提供 `Clock` 对象，这个宏会生成一个新的 `Clock` 对象，
//         其时间戳设置为当前的系统时间。这确保了模拟中的时间依赖操作（如检查截止时间）能使用一个相对“实时”的时间。
//
// 2.  **`OverrideCache` 结构体**:
//     -   `fallback: Option<Arc<WritebackCache>>`: (可选的) 后备缓存/存储。
//         如果 `OverrideCache` 在其自身的 `overrides` 列表中找不到某个对象，并且 `fallback` 是 `Some`，
//         它会尝试从这个后备的 `WritebackCache` (通常代表更广泛的数据库状态) 中读取该对象。
//         如果 `fallback` 是 `None`，则 `OverrideCache` 只会使用其 `overrides` 列表中的对象。
//     -   `overrides: Vec<ObjectReadResult>`: 一个向量，存储了所有明确要在此缓存层中覆盖的对象及其状态。
//         `ObjectReadResult` 包含了对象的ID、版本、内容以及其作为输入对象的种类 (`InputObjectKind`)。
//         这个列表通常在创建 `DBSimulator` 或 `SimulateCtx` 时根据特定场景（如MEV机会、模拟的Gas币、模拟的借入代币）提供。
//     -   `versioned_cache: RwLock<BTreeMap<(ObjectID, SequenceNumber), Object>>`:
//         一个线程安全的、读写锁保护的BTreeMap，用于缓存从 `fallback` 存储中读取到的特定版本的对象。
//         键是 `(ObjectID, SequenceNumber)` 元组，值是 `Object`。
//         当通过 `fallback` 读取对象时，这些对象会被存入此缓存，以便后续对同一版本对象的快速访问，
//         减少对 `fallback` 存储的重复查询。`RwLock` 允许多个读取者或单个写入者。
//
// 3.  **`OverrideCache::new()` 构造函数**:
//     -   创建一个新的 `OverrideCache` 实例。
//
// 4.  **对象获取方法**:
//     -   `get_override(&self, object_id: &ObjectID) -> Option<ObjectReadResult>`:
//         专门从此缓存的 `overrides` 列表中查找具有指定 `object_id` 的对象。
//         特别处理 `SUI_CLOCK_OBJECT_ID`：如果请求的是时钟对象，它会使用 `ret_latest_clock_obj!` 宏动态生成一个最新的时钟对象并返回。
//     -   `get_override_object(&self, object_id: &ObjectID) -> Option<Object>`:
//         与 `get_override` 类似，但只返回 `Object` 本身 (如果找到且不是删除标记)。
//     -   `get_versioned_object_for_comparison(&self, object_id: &ObjectID, version: SequenceNumber) -> Option<Object>`:
//         从 `versioned_cache` (即通过fallback读取并缓存的对象) 中查找特定版本的对象。
//
// 5.  **Sui存储相关Trait的实现**:
//     `OverrideCache` 实现了一系列Sui存储相关的trait，使其可以像一个标准的Sui对象存储一样被执行引擎使用。
//     这些trait包括 `BackingPackageStore`, `ChildObjectResolver`, `ObjectStore`, `ParentSync`, 和 `ObjectCacheRead`。
//     -   **`ObjectCacheRead` 的实现是核心**:
//         -   `get_package_object()`: 获取Move包对象。优先查 `overrides`，然后查 `fallback`。
//         -   `get_object()`: 获取最新版本的对象。优先查 `overrides`。如果未找到，且有 `fallback`，则从 `fallback` 读取，
//             并将结果存入 `versioned_cache` 以备后用。
//         -   `get_latest_object_ref_or_tombstone()`: 获取最新对象的引用或删除标记。逻辑与 `get_object` 类似。
//         -   `get_latest_object_or_tombstone()`: 获取最新对象或删除标记 (返回 `ObjectOrTombstone` 枚举)。
//             特别处理在 `overrides` 中被标记为已删除的共享对象：它会尝试从 `fallback` 中获取该对象被删除前的版本作为“墓碑”的基础。
//         -   `get_object_by_key()`: 获取指定ID和版本的对象。优先查 `overrides` (并检查版本)，然后查 `fallback`。
//         -   其他方法如 `multi_get_objects_by_key`, `object_exists_by_key`, `find_object_lt_or_eq_version` 等，
//             也大多遵循“先查 `overrides`，再查 `fallback` (如果存在)”的模式。
//         -   `get_lock()`, `get_sui_system_state_object_unsafe()`, `get_bridge_object_unsafe()`: 这些方法通常直接委托给 `fallback` (如果存在)，
//             因为 `OverrideCache` 主要关注对象内容的覆盖，而不是锁或系统级单例对象的管理。
//         -   日志记录: 在 `fallback` 查找发生时，会用 `warn!` 记录一条 "override missing" 的日志，这有助于调试缓存未命中的情况。
//
// **工作流程和用途**:
// 1.  在 `DBSimulator::simulate()` 方法中，会根据 `SimulateCtx` 提供的 `override_objects` 列表和可选的 `fallback` (即 `DBSimulator` 的主 `WritebackCache`) 来创建一个 `OverrideCache` 实例。
// 2.  这个 `OverrideCache` 实例随后被传递给Sui的执行引擎 (`self.executor.execute_transaction_to_effects()`) 作为对象存储 (`ObjectStore` / `ObjectCacheRead`)。
// 3.  当执行引擎在模拟交易过程中需要读取某个对象时，它会调用 `OverrideCache` 实现的 `ObjectCacheRead` 方法。
// 4.  `OverrideCache` 会首先检查其 `overrides` 列表中是否有这个对象：
//     -   如果找到了，并且版本等条件满足，就返回这个被覆盖的对象状态。
//     -   如果 `overrides` 中没有，或者不满足条件，并且 `OverrideCache` 配置了 `fallback` 存储，
//         它就会尝试从 `fallback` 存储中读取该对象。从 `fallback` 读取到的对象会被存入 `versioned_cache`。
// 5.  这样，执行引擎就能在一个混合了“真实数据库状态”和“用户指定覆盖状态”的环境中进行模拟。
//
// **关键点**:
// -   **状态隔离与定制**: `OverrideCache` 使得模拟可以在一个高度定制的、与主数据库状态部分隔离的环境中进行。
// -   **MEV模拟**: 对于MEV场景，一个常见的做法是：获取某个“机会交易”执行后的所有状态变更（对象创建、修改、删除），
//     将这些变更作为 `override_objects` 提供给 `OverrideCache`，然后在这个“假设的未来状态”下模拟自己的套利交易。
// -   **动态时钟**: 对时钟对象的特殊处理确保了模拟中的时间敏感操作（如检查deadline）能使用当前的真实时间。
// -   **性能**: 通过 `versioned_cache` 缓存从fallback读取的对象，减少对底层存储的重复访问。

// 引入标准库的相关模块
use std::{
    collections::BTreeMap, // BTreeMap 用于 versioned_cache，提供有序的键值存储
    sync::{Arc, RwLock},   // Arc 用于共享所有权，RwLock 用于读写锁保护 versioned_cache
    time::{SystemTime, UNIX_EPOCH}, // 用于获取当前系统时间，为Clock对象提供时间戳
};

// 引入Sui核心库和SDK中的相关类型
use sui_core::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore, // Epoch特定的存储接口
    authority::SuiLockResult,                                     // 获取对象锁的结果类型
    execution_cache::{ObjectCacheRead, WritebackCache},          // 执行缓存的读取接口和写回缓存类型
};
use sui_types::{
    base_types::{ObjectID, ObjectRef, SequenceNumber, VersionNumber}, // 基本ID和版本号类型
    bridge::Bridge, // Bridge对象类型 (用于Sui桥)
    clock::Clock,   // Clock对象类型 (Sui系统时钟)
    committee::EpochId, // Epoch ID类型
    digests::TransactionDigest, // 交易摘要类型
    error::{SuiError, SuiResult, UserInputError}, // Sui错误类型定义
    id::{ID, UID}, // 通用ID和UID类型 (用于构造Clock对象ID)
    messages_checkpoint::CheckpointSequenceNumber, // 检查点序列号
    object::{Data, MoveObject, Object, ObjectInner, Owner, OBJECT_START_VERSION}, // 对象核心结构和所有者类型
    storage::{MarkerValue, ObjectKey, ObjectOrTombstone, PackageObject}, // 存储相关的枚举和结构
    sui_system_state::SuiSystemState, // Sui系统状态对象类型
    transaction::ObjectReadResultKind, // 对象读取结果的种类 (Object, DeletedSharedObject等)
    SUI_CLOCK_OBJECT_ID, // Sui时钟对象的固定ObjectID ("0x6")
};
use sui_types::{
    storage::{BackingPackageStore, ChildObjectResolver, ObjectStore, ParentSync}, // 存储相关的trait
    transaction::ObjectReadResult, // 对象读取结果的封装类型
};
// 引入tracing库的warn!宏，用于记录警告日志
use tracing::warn;

/// `ret_latest_clock_obj!` 宏
///
/// 一个声明宏，用于动态创建一个表示当前最新时间的Sui `Clock` 对象 (`Object`类型)。
/// 当 `OverrideCache` 被请求提供 `SUI_CLOCK_OBJECT_ID` 时，会调用此宏。
///
/// 宏的实现:
/// - 创建一个 `ObjectInner` 结构，其字段填充如下：
///   - `owner`: 设置为共享对象，初始共享版本为 `OBJECT_START_VERSION`。
///   - `data`: 设置为 `Data::Move`，其中包含一个 `MoveObject`。
///     - `MoveObject::type_`: 设置为 `Clock::type_().into()`，即 `0x2::clock::Clock` 的结构标签。
///     - `has_public_transfer`: `false`，Clock对象通常不可直接转移。
///     - `version`: `OBJECT_START_VERSION`。这可能是一个简化，因为真实的Clock对象版本会递增。
///                  但在模拟覆盖时，使用一个固定的起始版本通常是可接受的，
///                  因为我们主要关心其 `timestamp_ms` 字段。
///     - `contents`: `Clock` 结构体的BCS序列化字节。
///       - `id`: Clock对象的ID (UID结构，内部ID为 `SUI_CLOCK_OBJECT_ID`)。
///       - `timestamp_ms`: 通过 `SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()` 获取当前的Unix时间戳（毫秒）。
/// - `previous_transaction`: 设置为创世交易摘要。
/// - `storage_rebate`: 设置为0。
/// - 最后将 `ObjectInner` 转换为 `Object` 类型。
macro_rules! ret_latest_clock_obj {
    () => {{ // 宏开始
        let clock_object_inner = ObjectInner { // 创建ObjectInner实例
            owner: Owner::Shared { // Clock对象是共享的
                initial_shared_version: OBJECT_START_VERSION, // 初始共享版本
            },
            data: Data::Move(MoveObject { // 对象数据是Move对象
                type_: Clock::type_().into(), // 类型是 0x2::clock::Clock
                has_public_transfer: false,  // 不可公开转移
                version: OBJECT_START_VERSION, // 版本号 (对于模拟覆盖，使用起始版本可能足够)
                contents: bcs::to_bytes(&Clock { // Clock结构体的BCS序列化内容
                    id: UID { // Clock对象的UID字段
                        id: ID { // UID内部的ID字段
                            bytes: SUI_CLOCK_OBJECT_ID, // 固定为Sui时钟对象的ID ("0x6")
                        },
                    },
                    timestamp_ms: { // 时间戳字段
                        // 获取当前系统时间，并转换为自UNIX纪元以来的毫秒数
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
                    },
                })
                .unwrap(), // bcs序列化假设不会失败
            }),
            previous_transaction: TransactionDigest::genesis_marker(), // 上一个交易摘要设为创世块标记
            storage_rebate: 0, // 存储回扣设为0
        };
        clock_object_inner.into() // 将ObjectInner转换为Object类型并作为宏的结果
    }}; // 宏结束
}

/// `OverrideCache` 结构体
///
/// 一个缓存层，用于在Sui交易模拟时提供特定版本的对象状态，以覆盖从底层存储中读取到的状态。
/// 它支持一个可选的后备存储 (`fallback`)，当覆盖列表中没有找到对象时，可以从后备存储中读取。
pub struct OverrideCache {
    /// `fallback`: (可选的) 后备缓存/存储，通常是一个 `WritebackCache` 实例，
    /// 代表更广泛的数据库状态。如果为 `None`，则此缓存只使用 `overrides` 列表中的对象。
    pub fallback: Option<Arc<WritebackCache>>,
    /// `overrides`: 一个向量，存储了所有明确要在此缓存层中覆盖的对象及其状态 (`ObjectReadResult`)。
    /// 这个列表在模拟开始前提供，包含了特定场景所需的对象版本。
    pub overrides: Vec<ObjectReadResult>,

    /// `versioned_cache`: 一个线程安全的BTreeMap，用于缓存从 `fallback` 存储中读取到的特定版本的对象。
    /// 键是 `(ObjectID, SequenceNumber)` 元组，值是 `Object`。
    /// 使用 `RwLock`允许多个读取者或单个写入者，以保证并发安全。
    pub versioned_cache: RwLock<BTreeMap<(ObjectID, SequenceNumber), Object>>,
}

impl OverrideCache {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `OverrideCache` 实例。
    ///
    /// 参数:
    /// - `fallback`: 可选的后备 `WritebackCache`。
    /// - `overrides`: 用于覆盖的对象列表。
    pub fn new(fallback: Option<Arc<WritebackCache>>, overrides: Vec<ObjectReadResult>) -> Self {
        Self {
            fallback,
            overrides,
            versioned_cache: RwLock::new(BTreeMap::new()), // 初始化空的BTreeMap和RwLock
        }
    }

    /// `get_override` 方法
    ///
    /// 从 `self.overrides` 列表中查找具有指定 `object_id` 的对象。
    /// 特殊处理Sui时钟对象ID (`SUI_CLOCK_OBJECT_ID`)：总是返回一个动态生成的、表示当前时间的Clock对象。
    ///
    /// 返回:
    /// - `Option<ObjectReadResult>`: 如果在覆盖列表中找到对象，或请求的是时钟对象，则返回 `Some`，否则 `None`。
    pub fn get_override(&self, object_id: &ObjectID) -> Option<ObjectReadResult> {
        // 特殊处理Sui时钟对象
        if object_id == &SUI_CLOCK_OBJECT_ID {
            return Some(ObjectReadResult {
                input_object_kind: sui_types::transaction::InputObjectKind::SharedMoveObject { // 时钟是共享对象
                    id: SUI_CLOCK_OBJECT_ID,
                    initial_shared_version: OBJECT_START_VERSION, // 使用起始版本
                    mutable: true, // 尽管内容动态生成，但作为共享对象通常标记为可变以允许交易引用
                },
                object: ObjectReadResultKind::Object(ret_latest_clock_obj!()), // 调用宏生成最新的Clock对象数据
            });
        }

        // 遍历 `overrides` 列表，查找匹配 `object_id` 的条目。
        // `.cloned()` 是因为 `find` 返回的是引用，而我们需要返回一个拥有的 `ObjectReadResult`。
        self.overrides.iter().find(|o| o.id() == *object_id).cloned()
    }

    /// `get_override_object` 方法
    ///
    /// 从 `self.overrides` 列表中查找对象并返回其 `Object` 数据。
    /// 如果对象在覆盖列表中被标记为已删除，则返回 `None`。
    pub fn get_override_object(&self, object_id: &ObjectID) -> Option<Object> {
        match self.get_override(object_id) { // 调用上面的 get_override 方法
            Some(read_result) => match read_result.object {
                // 如果是实际的对象数据，则返回它
                ObjectReadResultKind::Object(object_data) => Some(object_data),
                // 如果是删除标记 (共享对象删除或交易取消)，则返回None
                ObjectReadResultKind::DeletedSharedObject(_, _) => None,
                ObjectReadResultKind::CancelledTransactionSharedObject(_) => unreachable!("不应在覆盖对象中出现CancelledTransactionSharedObject"),
            },
            _ => None, // 如果 get_override 返回None，则这里也返回None
        }
    }

    /// `get_versioned_object_for_comparison` 方法
    ///
    /// 从 `self.versioned_cache` (通过fallback读取并缓存的对象) 中查找特定版本的对象。
    /// 这个缓存主要用于 `ExecutedDB` 在计算余额变更等场景时，需要精确版本对象进行比较。
    pub fn get_versioned_object_for_comparison(&self, object_id: &ObjectID, version: SequenceNumber) -> Option<Object> {
        self.versioned_cache // 获取对 RwLock<BTreeMap> 的引用
            .read()          // 获取读锁 (如果写锁被持有则阻塞)。unwrap() 处理锁中毒 (poisoned) 的情况。
            .unwrap()
            .get(&(*object_id, version)) // 在 BTreeMap 中查找键 (ObjectID, SequenceNumber)
            .cloned() // 如果找到，克隆 Object 并返回 Option<Object>
    }
}

/// 为 `OverrideCache` 实现 `BackingPackageStore` trait。
/// `BackingPackageStore` 定义了如何获取Move包对象。
impl BackingPackageStore for OverrideCache {
    fn get_package_object(&self, package_id: &ObjectID) -> SuiResult<Option<PackageObject>> {
        // 直接委托给下面 `ObjectCacheRead` trait 中实现的同名方法。
        ObjectCacheRead::get_package_object(self, package_id)
    }
}

/// 为 `OverrideCache` 实现 `ChildObjectResolver` trait。
/// `ChildObjectResolver` 定义了如何读取子对象和检查对象接收状态，这对于处理动态字段和接收的对象很重要。
impl ChildObjectResolver for OverrideCache {
    /// `read_child_object` 方法
    ///
    /// 读取指定父对象下的、版本号小于等于 `child_version_upper_bound` 的最新子对象。
    /// 并验证该子对象确实属于指定的父对象。
    fn read_child_object(
        &self,
        parent_object_id: &ObjectID,
        child_object_id: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> SuiResult<Option<sui_types::object::Object>> {
        // 使用 `find_object_lt_or_eq_version` (在 ObjectCacheRead 中实现) 查找子对象。
        let Some(child_sui_object) = self.find_object_lt_or_eq_version(*child_object_id, child_version_upper_bound) else {
            return Ok(None); // 如果找不到符合版本的子对象，则返回Ok(None)
        };

        // 验证子对象的所有者是否确实是指定的父对象。
        let expected_parent_id = *parent_object_id;
        if child_sui_object.owner != Owner::ObjectOwner(expected_parent_id.into()) {
            // 如果所有者不匹配，则返回错误，表示无效的子对象访问。
            let sui_error = SuiError::InvalidChildObjectAccess {
                object: *child_object_id,
                given_parent: expected_parent_id,
                actual_owner: child_sui_object.owner.clone(),
            };
            return Err(sui_error);
        }
        Ok(Some(child_sui_object)) // 返回找到并验证通过的子对象
    }

    /// `get_object_received_at_version` 方法
    ///
    /// 获取在特定版本 `receive_object_at_version` 被指定 `owner` 接收的 `receiving_object_id` 对象。
    /// 这个方法用于验证对象接收的逻辑，确保对象在正确的版本被接收，并且之前未被接收过。
    fn get_object_received_at_version(
        &self,
        owner_address: &ObjectID, // 接收者的ObjectID (通常是账户地址，但参数类型是ObjectID)
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        _epoch_id: EpochId, // 当前epoch ID (在此实现中未使用，但ObjectCacheRead的接口需要)
    ) -> SuiResult<Option<sui_types::object::Object>> {
        // 使用 `get_object_by_key` (在 ObjectCacheRead 中实现) 获取指定ID和版本的对象。
        let Some(received_sui_object) =
            ObjectCacheRead::get_object_by_key(self, receiving_object_id, receive_object_at_version)
        else {
            return Ok(None); // 如果在该版本找不到对象，则返回Ok(None)
        };

        // 检查对象的所有者是否与期望的接收者匹配。
        // 并且检查此对象在此版本是否已经被接收过 (通过 `have_received_object_at_version`)。
        // 这两个检查中任何一个失败，都应视为对象在该版本不可被接收，返回Ok(None)。
        // 这是为了防止重放攻击或不一致的状态。
        if received_sui_object.owner != Owner::AddressOwner((*owner_address).into()) // 转换为SuiAddress进行比较
            || self.have_received_object_at_version(receiving_object_id, receive_object_at_version, _epoch_id)
        {
            return Ok(None);
        }

        Ok(Some(received_sui_object)) // 如果所有检查通过，返回该对象
    }
}

/// 为 `OverrideCache` 实现 `ObjectStore` trait。
/// `ObjectStore` 是一个更通用的对象读取接口。
impl ObjectStore for OverrideCache {
    /// `get_object` 方法: 获取最新版本的对象。
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        ObjectCacheRead::get_object(self, object_id) // 委托给 ObjectCacheRead 实现
    }

    /// `get_object_by_key` 方法: 获取指定ID和版本的对象。
    fn get_object_by_key(&self, object_id: &ObjectID, version: VersionNumber) -> Option<Object> {
        ObjectCacheRead::get_object_by_key(self, object_id, version) // 委托给 ObjectCacheRead 实现
    }
}

/// 为 `OverrideCache` 实现 `ParentSync` trait。
/// `ParentSync` 用于在处理共享对象时，获取其父对象（如果它是子对象）的最新版本信息。
/// 这对于确保共享对象操作的一致性很重要。
impl ParentSync for OverrideCache {
    /// `get_latest_parent_entry_ref_deprecated` 方法 (已弃用)
    ///
    /// 这个方法在较新版本的Sui中已被弃用或不再需要。
    /// 当前实现直接panic，表明不应被调用。
    fn get_latest_parent_entry_ref_deprecated(&self, _object_id: ObjectID) -> Option<ObjectRef> {
        panic!("在OverrideCache中不应调用 get_latest_parent_entry_ref_deprecated (won't be called in new version)")
    }
}

/// 为 `OverrideCache` 实现 `ObjectCacheRead` trait。
/// `ObjectCacheRead` 是Sui执行层用于从缓存中读取对象和包的核心接口。
/// 这是 `OverrideCache` 最主要和最复杂的trait实现。
impl ObjectCacheRead for OverrideCache {
    /// `get_package_object` 方法: 获取Move包对象。
    fn get_package_object(&self, id: &ObjectID) -> SuiResult<Option<PackageObject>> {
        // 步骤 1: 首先检查 `self.overrides` 覆盖列表中是否有此包ID。
        if let Some(override_read_result) = self.get_override(id) {
            match override_read_result.object {
                // 如果找到且是实际的对象数据，则将其包装为 `PackageObject` 并返回。
                ObjectReadResultKind::Object(object_data) => return Ok(Some(PackageObject::new(object_data))),
                // 如果在覆盖列表中被标记为已删除，则返回Ok(None)。
                ObjectReadResultKind::DeletedSharedObject(_, _) => return Ok(None),
                // 不应出现CancelledTransactionSharedObject
                ObjectReadResultKind::CancelledTransactionSharedObject(_) => {
                    unreachable!("覆盖对象不应是CancelledTransactionSharedObject状态")
                }
            }
        }

        // 步骤 2: 如果覆盖列表中未找到，则记录一个警告，并尝试从 `self.fallback` (后备存储) 中获取。
        warn!("❗️ [OverrideCache::get_package_object] 覆盖缓存未命中，尝试从后备存储获取包: {:?}", id);
        if let Some(ref fallback_store) = self.fallback {
            fallback_store.get_package_object(id) // 调用后备存储的同名方法
        } else {
            Ok(None) // 如果没有后备存储，则返回Ok(None)
        }
    }

    /// `force_reload_system_packages` 方法: 强制重新加载系统包。
    /// 这个方法通常在系统包升级后由外部调用，以确保缓存中的系统包是最新的。
    /// 此实现直接委托给后备存储 (如果存在)。
    fn force_reload_system_packages(&self, system_package_ids: &[ObjectID]) {
        if let Some(ref fallback_store) = self.fallback {
            fallback_store.force_reload_system_packages(system_package_ids);
        }
    }

    /// `get_object` 方法: 获取最新版本的对象。
    fn get_object(&self, id: &ObjectID) -> Option<Object> {
        // 步骤 1: 检查覆盖列表。
        if let Some(override_read_result) = self.get_override(id) {
            match override_read_result.object {
                ObjectReadResultKind::Object(object_data) => return Some(object_data),
                ObjectReadResultKind::DeletedSharedObject(_, _) => return None,
                ObjectReadResultKind::CancelledTransactionSharedObject(_) => {
                    unreachable!("覆盖对象不应是CancelledTransactionSharedObject状态")
                }
            }
        }

        // 步骤 2: 如果覆盖未命中，记录警告，并尝试从后备存储获取。
        warn!("❗️ [OverrideCache::get_object] 覆盖缓存未命中，尝试从后备存储获取对象: {:?}", id);
        if let Some(ref fallback_store) = self.fallback {
            let object_from_fallback = fallback_store.get_object(id); // 从后备存储读取
            if let Some(obj_data) = object_from_fallback.clone() { // 如果成功读取到
                // 将从后备存储读取到的对象存入 `self.versioned_cache`，以便后续快速访问相同版本的对象。
                // 获取写锁来修改 `versioned_cache`。
                self.versioned_cache.write().unwrap().insert((*id, obj_data.version()), obj_data);
            }
            object_from_fallback // 返回从后备存储读取的结果
        } else {
            None // 没有后备存储，则返回None
        }
    }

    /// `get_latest_object_ref_or_tombstone` 方法: 获取最新对象的引用或删除标记。
    fn get_latest_object_ref_or_tombstone(&self, object_id: ObjectID) -> Option<ObjectRef> {
        // 步骤 1: 尝试从覆盖列表中直接获取对象。
        if let Some(override_object_data) = self.get_override_object(&object_id) {
            // 如果在覆盖列表中找到对象 (且不是删除标记)，则计算并返回其ObjectRef。
            return Some(override_object_data.compute_object_reference());
        }
        // 注意：如果 `get_override_object` 返回 `None`，可能是因为对象在覆盖列表中被标记为删除，
        // 或者根本不在覆盖列表中。

        // 步骤 2: 如果覆盖未命中或在覆盖中已删除，则记录警告并尝试从后备存储获取。
        // 对于已删除的情况，也需要查询后备存储，因为后备存储中可能仍然有该对象被删除前的最后一个版本信息，
        // 这对于获取正确的“墓碑”(Tombstone) ObjectRef 很重要。
        warn!(
            "❗️ [OverrideCache::get_latest_object_ref_or_tombstone] 覆盖缓存未命中或对象已在覆盖中删除，尝试后备存储: {:?}",
            object_id
        );
        if let Some(ref fallback_store) = self.fallback {
            fallback_store.get_latest_object_ref_or_tombstone(object_id)
        } else {
            None
        }
    }

    /// `get_latest_object_or_tombstone` 方法: 获取最新对象或删除标记 (返回 `ObjectOrTombstone` 枚举)。
    fn get_latest_object_or_tombstone(&self, object_id: ObjectID) -> Option<(ObjectKey, ObjectOrTombstone)> {
        // 步骤 1: 检查覆盖列表。
        if let Some(override_read_result) = self.get_override(&object_id) {
            match override_read_result.object {
                ObjectReadResultKind::Object(object_data) => {
                    // 如果在覆盖列表中找到对象，则返回其ObjectKey和ObjectOrTombstone::Object。
                    return Some((
                        ObjectKey::from(object_data.compute_object_reference()),
                        ObjectOrTombstone::Object(object_data),
                    ));
                }
                ObjectReadResultKind::DeletedSharedObject(_, _) => {
                    // 如果在覆盖列表中对象被标记为删除 (DeletedSharedObject):
                    // 尝试从后备存储中获取该对象被删除前的最后一个版本，作为墓碑的基础。
                    // `fallback.as_ref()?` 如果没有fallback则返回None。
                    let undeleted_object_from_fallback = match self.fallback.as_ref()?.get_object(&object_id) {
                        Some(obj_data) => obj_data,
                        None => return None, // 如果后备存储中也找不到，则无法构造墓碑，返回None。
                                             // 这种情况理论上不常见，除非对象从未存在过或DB状态不一致。
                    };
                    // 使用从后备存储获取的对象的引用来构造墓碑。
                    let object_ref_for_tombstone = undeleted_object_from_fallback.compute_object_reference();
                    return Some((ObjectKey::from(&object_ref_for_tombstone), ObjectOrTombstone::Tombstone(object_ref_for_tombstone)));
                }
                ObjectReadResultKind::CancelledTransactionSharedObject(_) => {
                    unreachable!("覆盖对象不应是CancelledTransactionSharedObject状态")
                }
            }
        }

        // 步骤 2: 如果覆盖未命中，记录警告并尝试从后备存储获取。
        warn!("❗️ [OverrideCache::get_latest_object_or_tombstone] 覆盖缓存未命中，尝试后备存储: {:?}", object_id);
        if let Some(ref fallback_store) = self.fallback {
            fallback_store.get_latest_object_or_tombstone(object_id)
        } else {
            None
        }
    }

    /// `get_object_by_key` 方法: 获取指定ID和版本的对象。
    fn get_object_by_key(&self, object_id: &ObjectID, version: SequenceNumber) -> Option<Object> {
        // 步骤 1: 检查覆盖列表。
        if let Some(override_read_result) = self.get_override(object_id) {
            match override_read_result.object {
                ObjectReadResultKind::Object(object_data) => {
                    // 如果找到对象，还需检查其版本是否与请求的版本匹配。
                    if object_data.version() == version {
                        return Some(object_data);
                    }
                    // 如果版本不匹配，则不从此覆盖条目返回 (后续可能从fallback获取)。
                }
                ObjectReadResultKind::DeletedSharedObject(_, _) => return None, // 如果在覆盖中已删除，则直接返回None。
                ObjectReadResultKind::CancelledTransactionSharedObject(_) => {
                    unreachable!("覆盖对象不应是CancelledTransactionSharedObject状态")
                }
            }
        }

        // 步骤 2: 如果覆盖未命中或版本不匹配，记录警告并尝试从后备存储获取。
        warn!(
            "❗️ [OverrideCache::get_object_by_key] 覆盖缓存未命中或版本不匹配，尝试后备存储: {:?}, 版本: {:?}",
            object_id, version
        );
        if let Some(ref fallback_store) = self.fallback {
            fallback_store.get_object_by_key(object_id, version)
        } else {
            None
        }
    }

    /// `multi_get_objects_by_key` 方法: 批量获取多个指定ID和版本的对象。
    /// 简单地循环调用单个 `get_object_by_key` 方法。
    fn multi_get_objects_by_key(&self, object_keys: &[ObjectKey]) -> Vec<Option<Object>> {
        let mut result_vec = vec![];
        for object_key_item in object_keys {
            // `(self as &dyn ObjectCacheRead)` 是为了明确调用的是 `ObjectCacheRead` trait中定义的 `get_object_by_key`。
            result_vec.push((self as &dyn ObjectCacheRead).get_object_by_key(&object_key_item.0, object_key_item.1));
        }
        result_vec
    }

    /// `object_exists_by_key` 方法: 检查指定ID和版本的对象是否存在。
    fn object_exists_by_key(&self, object_id: &ObjectID, version: SequenceNumber) -> bool {
        // 调用 `get_object_by_key`，如果返回 `Some(_)` 则表示存在。
        let Some(_) = (self as &dyn ObjectCacheRead).get_object_by_key(object_id, version) else {
            return false;
        };
        true
    }

    /// `multi_object_exists_by_key` 方法: 批量检查多个对象是否存在。
    fn multi_object_exists_by_key(&self, object_keys: &[ObjectKey]) -> Vec<bool> {
        let mut result_vec = vec![];
        for object_key_item in object_keys {
            result_vec.push((self as &dyn ObjectCacheRead).object_exists_by_key(&object_key_item.0, object_key_item.1));
        }
        result_vec
    }

    /// `find_object_lt_or_eq_version` 方法: 查找ID匹配且版本号小于或等于给定`version`的最新对象。
    /// 这个实现比较简单，只考虑了 `get_object` (获取最新版本) 的情况。
    /// 如果最新版本符合条件，则返回。
    /// **注意**: 这个实现可能不完全符合 `find_object_lt_or_eq_version` 的完整语义，
    /// 如果 `OverrideCache` 中覆盖的对象版本不是最新的，或者需要从历史版本中查找，则此逻辑可能不足。
    /// 它更像是 `get_object_le_version`。
    fn find_object_lt_or_eq_version(&self, object_id: ObjectID, version: SequenceNumber) -> Option<Object> {
        // 获取该ID的最新（可能是覆盖的）对象。
        let latest_object_data = (self as &dyn ObjectCacheRead).get_object(&object_id)?;
        // 如果其版本小于或等于请求的版本，则返回它。
        if latest_object_data.version() <= version {
            return Some(latest_object_data);
        }
        None // 否则，表示最新版本也大于请求版本，所以没有符合条件的对象。
    }

    /// `get_lock` 方法: 获取对象的锁状态。
    /// 此操作通常与共享对象的并发控制相关，直接委托给后备存储。
    fn get_lock(&self, obj_ref: ObjectRef, epoch_store: &AuthorityPerEpochStore) -> SuiLockResult {
        self.fallback
            .as_ref() // 获取对 Option<Arc<WritebackCache>> 中 Arc 的引用
            .ok_or_else(|| SuiError::Unknown("OverrideCache: 获取锁时后备存储未配置".to_string()))? // 如果没有后备存储，则返回错误
            .get_lock(obj_ref, epoch_store) // 调用后备存储的 get_lock
    }

    /// `_get_live_objref` 方法 (内部使用): 获取一个存活对象的最新ObjectRef。
    /// 如果对象在覆盖列表中被标记为删除，则返回错误。
    fn _get_live_objref(&self, object_id: ObjectID) -> SuiResult<ObjectRef> {
        // 步骤 1: 检查覆盖列表。
        if let Some(override_read_result) = self.get_override(&object_id) {
            match override_read_result.object {
                ObjectReadResultKind::Object(object_data) => {
                    return Ok(object_data.compute_object_reference()); // 返回存活对象的引用
                }
                ObjectReadResultKind::DeletedSharedObject(_, _) => {
                    // 如果在覆盖中已删除，则返回对象未找到错误。
                    return Err(SuiError::UserInputError {
                        error: UserInputError::ObjectNotFound {
                            object_id,
                            version: None, // 版本未知或不适用
                        },
                    });
                }
                ObjectReadResultKind::CancelledTransactionSharedObject(_) => {
                    unreachable!("覆盖对象不应是CancelledTransactionSharedObject状态")
                }
            }
        }

        // 步骤 2: 如果覆盖未命中，记录警告并委托给后备存储。
        warn!("❗️ [OverrideCache::_get_live_objref] 覆盖缓存未命中，尝试后备存储: {:?}", object_id);
        if let Some(ref fallback_store) = self.fallback {
            fallback_store._get_live_objref(object_id)
        } else {
            // 没有后备存储，则返回错误。
            Err(SuiError::Unknown("OverrideCache: _get_live_objref 时后备存储未配置".to_string()))
        }
    }

    /// `check_owned_objects_are_live` 方法: 检查一组私有对象引用是否都指向存活的对象。
    fn check_owned_objects_are_live(&self, owned_object_refs: &[ObjectRef]) -> SuiResult {
        for owned_obj_ref_item in owned_object_refs {
            // 对每个对象引用，调用 get_object_by_key 检查其是否存在于当前缓存（包括覆盖和后备）中。
            if (self as &dyn ObjectCacheRead)
                .get_object_by_key(&owned_obj_ref_item.0, owned_obj_ref_item.1) // 0是ID, 1是Version
                .is_none() // 如果返回None (即对象在该版本不存在)
            {
                // 则返回错误，表明对象版本不可用于消费。
                return Err(UserInputError::ObjectVersionUnavailableForConsumption {
                    provided_obj_ref: *owned_obj_ref_item,
                    current_version: owned_obj_ref_item.1, // 虽然这里叫current_version，但实际是请求的版本
                }
                .into());
            }
        }
        Ok(()) // 所有对象都检查通过
    }

    /// `get_sui_system_state_object_unsafe` 方法: 不安全地获取Sui系统状态对象。
    /// 直接委托给后备存储。
    fn get_sui_system_state_object_unsafe(&self) -> SuiResult<SuiSystemState> {
        self.fallback
            .as_ref()
            .ok_or_else(|| SuiError::Unknown("OverrideCache: 获取Sui系统状态时后备存储未配置".to_string()))?
            .get_sui_system_state_object_unsafe()
    }

    /// `get_bridge_object_unsafe` 方法: 不安全地获取Sui桥对象。
    /// 直接委托给后备存储。
    fn get_bridge_object_unsafe(&self) -> SuiResult<Bridge> {
        self.fallback
            .as_ref()
            .ok_or_else(|| SuiError::Unknown("OverrideCache: 获取Bridge对象时后备存储未配置".to_string()))?
            .get_bridge_object_unsafe()
    }

    /// `get_marker_value` 方法: 获取共享对象的标记值 (例如，用于检查是否已在当前epoch接收过)。
    /// TODO: 当前实现直接委托给后备存储，可能需要考虑 `overrides` 中的状态。
    fn get_marker_value(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> Option<MarkerValue> {
        // 理想情况下，如果 `overrides` 中有此对象的更高版本或删除标记，可能会影响此结果。
        // 但当前简单地委托给后备存储。
        self.fallback.as_ref()?.get_marker_value(object_id, version, epoch_id)
    }

    /// `get_latest_marker` 方法: 获取共享对象的最新标记值。
    /// TODO: 当前实现直接委托给后备存储。
    fn get_latest_marker(&self, object_id: &ObjectID, epoch_id: EpochId) -> Option<(SequenceNumber, MarkerValue)> {
        self.fallback.as_ref()?.get_latest_marker(object_id, epoch_id)
    }

    /// `get_highest_pruned_checkpoint` 方法: 获取已修剪的最高检查点序列号。
    /// 直接委托给后备存储，或者如果没有后备存储则返回默认值。
    fn get_highest_pruned_checkpoint(&self) -> CheckpointSequenceNumber {
        if let Some(ref fallback_store) = self.fallback {
            fallback_store.get_highest_pruned_checkpoint()
        } else {
            CheckpointSequenceNumber::default() // 返回0
        }
    }
}

[end of crates/simulator/src/db_simulator/override_cache.rs]
