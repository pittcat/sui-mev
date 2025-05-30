// 该文件 `arb_cache.rs` 定义了一个名为 `ArbCache` 的特殊缓存结构。
// 这个缓存用于管理和调度潜在的套利机会 (`ArbItem`)。
// 套利机会通常由某个事件（如价格波动）触发，并与特定的代币、交易池和链上状态相关。
// `ArbCache` 的设计目标是：
// 1. 存储这些套利机会。
// 2. 确保每个代币（或代币+池组合）在缓存中只有一个最新的条目。
// 3. 允许条目在一段时间后自动过期。
// 4. 提供一种方式来按某种优先级（例如，最先过期的或最新插入的）提取条目进行处理。
//
// 文件概览:
// - `ArbItem` 结构体: 代表一个具体的套利机会，包含了代币名称、可选的池ID、触发该机会的交易摘要、
//   相关的模拟上下文 (`SimulateCtx`) 和事件来源 (`Source`)。
// - `ArbEntry` 结构体: 存储在内部HashMap中的值，除了`ArbItem`的大部分信息外，还包含了一个
//   `generation` (代数/版本号) 和 `expires_at` (过期时间点)。`generation` 用于处理陈旧数据。
// - `HeapItem` 结构体: 存储在二叉堆 (`BinaryHeap`) 中的元素。它包含了过期时间、代数、代币名称和池ID。
//   `BinaryHeap` 用于高效地找到下一个要过期的条目。
//   - `Ord` 和 `PartialOrd` trait的实现确保了 `BinaryHeap` 表现为最小堆（基于 `expires_at`），
//     即堆顶总是最先过期的元素。
// - `ArbCache` 结构体: 核心的缓存实现。
//   - `map`: 一个 `HashMap<String, ArbEntry>`，键是代币名称，值是 `ArbEntry`。用于快速查找和更新。
//   - `heap`: 一个 `BinaryHeap<HeapItem>`，用于管理条目的过期。
//   - `generation_counter`: 一个全局的代数计数器，每次插入或更新条目时递增，用于区分新旧数据。
//   - `expiration_duration`: 条目的过期时长。
// - `ArbCache` 的方法:
//   - `new()`: 构造函数。
//   - `insert()`: 插入或更新一个套利机会。如果已存在，则更新其代数和过期时间。
//   - `get()`: (已标记为 `dead_code`) 根据代币名称获取一个机会的摘要和模拟上下文。
//   - `remove_expired()`: 移除所有已过期的条目。
//   - `pop_one()`: 从缓存中弹出一个有效的、未过期的、最新的套利机会进行处理。
//
// 数据结构和算法:
// - HashMap: 用于O(1)平均时间复杂度的插入、删除和查找。
// - BinaryHeap: 用于O(log N)时间复杂度的插入和删除最小/最大元素（这里是最小过期时间）。
// - Generation Counter: 一种处理并发或更新时数据一致性的方法。每个条目都有一个代数，
//   只有当堆中条目的代数与图中条目的代数匹配时，才认为它是最新的。
//   这有助于处理这种情况：一个旧的条目可能仍在堆中，但其对应代币的数据已被更新。

// 引入标准库及第三方库
use std::{
    cmp::Ordering, // 用于自定义排序 (实现 Ord trait)
    collections::{BinaryHeap, HashMap}, // 二叉堆 (优先队列) 和哈希图
    time::{Duration, Instant}, // 时间处理：时长和即时时间点
};

use simulator::SimulateCtx; // 模拟上下文，包含了执行模拟所需的环境信息 (如epoch, gas价格)
use sui_types::{base_types::ObjectID, digests::TransactionDigest}; // Sui基本类型：对象ID, 交易摘要

use crate::types::Source; // 从当前crate的 `types` 模块引入 `Source` 枚举 (表示事件来源)

/// `ArbItem` 结构体
///
/// 代表一个具体的套利机会，将从缓存中提取出来供工作线程处理。
pub struct ArbItem {
    pub coin: String,                   // 相关的代币类型字符串 (例如 "0x2::sui::SUI")
    pub pool_id: Option<ObjectID>,      // (可选) 相关的交易池对象ID。如果机会与特定池相关。
    pub tx_digest: TransactionDigest,   // 触发此套利机会的原始交易的摘要 (哈希)
    pub sim_ctx: SimulateCtx,           // 与此机会相关的模拟上下文 (例如，基于触发交易后的链状态)
    pub source: Source,                 // 此套利机会的来源 (例如，公开交易、私有交易、Shio事件)
}

impl ArbItem {
    /// `new` 构造函数
    ///
    /// 从代币名称、可选的池ID和 `ArbEntry` 创建一个新的 `ArbItem`。
    pub fn new(coin: String, pool_id: Option<ObjectID>, entry: ArbEntry) -> Self {
        Self {
            coin, // 直接使用传入的coin
            pool_id,
            tx_digest: entry.digest,     // 从ArbEntry获取交易摘要
            sim_ctx: entry.sim_ctx,      // 从ArbEntry获取模拟上下文
            source: entry.source,        // 从ArbEntry获取来源
        }
    }
}

/// `ArbEntry` 结构体
///
/// 存储在 `ArbCache` 内部 `HashMap` 中的值。
/// 它代表了一个套利机会的内部表示，包含了比 `ArbItem` 更多的管理信息。
pub struct ArbEntry {
    digest: TransactionDigest,   // 触发交易的摘要
    sim_ctx: SimulateCtx,        // 模拟上下文
    generation: u64,             // 代数/版本号，用于处理陈旧数据
    expires_at: Instant,         // 此条目的过期时间点
    source: Source,              // 事件来源
}

/// `HeapItem` 结构体
///
/// 存储在 `ArbCache` 内部 `BinaryHeap` 中的元素。
/// 用于根据过期时间对套利机会进行排序。
#[derive(Eq, PartialEq)] // 为 `HeapItem` 派生 `Eq` 和 `PartialEq` trait，这是 `Ord` 的要求
struct HeapItem {
    expires_at: Instant,         // 过期时间点
    generation: u64,             // 代数，与 `ArbEntry` 中的 `generation` 对应
    coin: String,                // 代币类型字符串
    pool_id: Option<ObjectID>,   // (可选) 相关的交易池对象ID
}

/// 为 `HeapItem` 实现 `Ord` trait，以定义其在二叉堆中的排序行为。
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Rust的 `BinaryHeap` 默认是一个最大堆 (max-heap)，即堆顶是最大的元素。
        // 我们希望堆顶是最先过期 (`expires_at` 最小) 的元素，即实现一个最小堆。
        // 因此，我们需要反转 `expires_at` 的比较结果。
        // `self.expires_at.cmp(&other.expires_at)`: 比较过期时间。
        // `.then(self.generation.cmp(&other.generation))`: 如果过期时间相同，则比较代数。
        //   (这里的次要排序标准可能影响不大，因为主要依赖 `expires_at`)
        // `.reverse()`: 反转最终的比较结果，将最大堆行为转变为最小堆行为。
        self.expires_at
            .cmp(&other.expires_at) // 先按过期时间升序 (因为reverse后会变成降序，所以原始比较是升序)
            .then(self.generation.cmp(&other.generation)) // 如果过期时间相同，再按代数升序
            .reverse() // 反转，使得 `expires_at` 最小的元素具有最高的优先级 (在堆顶)
    }
}

/// 为 `HeapItem` 实现 `PartialOrd` trait。
/// `Ord` trait 要求类型也必须实现 `PartialOrd` 和 `Eq` (已通过 `#[derive(Eq)]` 实现)。
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other)) // 直接调用 `self.cmp(other)`
    }
}

/// `ArbCache` 结构体
///
/// 管理套利机会 (`ArbItem`) 的缓存，支持唯一性、重新排序和定时过期。
pub struct ArbCache {
    map: HashMap<String, ArbEntry>, // 键为代币名称 (String)，值为 `ArbEntry`。用于快速访问和更新。
    heap: BinaryHeap<HeapItem>,     // 一个最小堆 (基于 `expires_at`)，存储 `HeapItem`。用于高效管理过期。
    generation_counter: u64,        // 全局代数计数器，用于标记每个条目的新鲜度。
    expiration_duration: Duration,  // 条目从插入开始的有效时长。
}

impl ArbCache {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `ArbCache` 实例。
    ///
    /// 参数:
    /// - `expiration_duration`: 新条目在缓存中的默认过期时长。
    pub fn new(expiration_duration: Duration) -> Self {
        Self {
            map: HashMap::new(),
            heap: BinaryHeap::new(),
            generation_counter: 0, // 初始代数为0
            expiration_duration,
        }
    }

    /// `insert` 方法
    ///
    /// 插入或更新一个套利机会到缓存中。
    /// - 如果代币已存在于缓存中，此方法会更新其条目，使用新的代数和过期时间。
    ///   旧的条目（在堆中）会因为代数不匹配而变为“陈旧”(stale)，并最终被清理。
    ///
    /// 参数:
    /// - `coin`: 相关代币的类型字符串。
    /// - `pool_id`: (可选) 相关交易池的ObjectID。
    /// - `digest`: 触发此机会的交易摘要。
    /// - `sim_ctx`: 与此机会相关的模拟上下文。
    /// - `source`: 此机会的来源。
    pub fn insert(
        &mut self,
        coin: String,
        pool_id: Option<ObjectID>,
        digest: TransactionDigest,
        sim_ctx: SimulateCtx,
        source: Source,
    ) {
        let now = Instant::now(); // 获取当前时间
        self.generation_counter += 1; // 递增全局代数计数器
        let current_generation = self.generation_counter; // 获取当前代数
        let new_expires_at = now + self.expiration_duration; // 计算新的过期时间点

        // 步骤 1: 在 `map` 中插入或更新条目。
        // `map.insert` 会返回旧值 (如果存在)，但这里我们不关心它。
        self.map.insert(
            coin.clone(), // 克隆 `coin` 字符串作为键 (因为 `coin` 后面也用于 `HeapItem`)
            ArbEntry {
                digest,
                sim_ctx,
                generation: current_generation, // 使用新的代数
                expires_at: new_expires_at,   // 使用新的过期时间
                source,
            },
        );

        // 步骤 2: 在 `heap` 中插入新的 `HeapItem`。
        // 这个新条目代表了 `coin` 的最新版本。
        // 旧的 `HeapItem` (如果之前存在对应 `coin` 的话) 不会被直接从堆中移除，
        // 但它会因为代数不匹配而在后续的 `pop_one` 或 `remove_expired` 中被识别为陈旧并忽略。
        self.heap.push(HeapItem {
            expires_at: new_expires_at,
            generation: current_generation,
            coin, // 移动 `coin` 字符串的所有权到 `HeapItem`
            pool_id,
        });
    }

    /// `get` 方法 (已标记为 `#[allow(dead_code)]`，表示当前代码中可能未使用)
    ///
    /// 根据代币名称尝试从缓存中获取一个套利机会的交易摘要和模拟上下文。
    /// 注意：这个方法只检查 `map`，不检查代数或过期状态，可能返回陈旧或已过期的信息。
    /// 如果需要获取有效的、可处理的条目，应使用 `pop_one`。
    #[allow(dead_code)] // 允许存在未使用的代码
    pub fn get(&self, coin: &str) -> Option<(TransactionDigest, SimulateCtx)> {
        self.map.get(coin).map(|entry| (entry.digest, entry.sim_ctx.clone()))
    }

    /// `remove_expired` 方法
    ///
    /// 定期调用此方法以从缓存中移除已过期的条目。
    /// 它会检查堆顶的元素：
    /// - 如果堆顶元素的代数与 `map` 中对应条目的代数不匹配，则该堆顶元素是陈旧的，直接从堆中移除。
    /// - 如果代数匹配，则检查其过期时间。如果已过期，则从 `map` 和 `heap` 中都移除，并记录其代币名称。
    /// - 如果堆顶元素既不陈旧也未过期，则停止清理，因为堆中剩余的元素都具有更晚的过期时间。
    ///
    /// 返回:
    /// - `Vec<String>`: 一个包含所有被移除的过期代币名称的向量。
    pub fn remove_expired(&mut self) -> Vec<String> {
        let mut expired_coin_names = Vec::new(); // 存储已过期并移除的代币名称
        let now = Instant::now(); // 获取当前时间

        // `heap.peek()` 查看堆顶元素而不移除它。
        while let Some(top_heap_item) = self.heap.peek() {
            // 检查 `map` 中是否存在对应的 `coin` 条目
            if let Some(map_entry) = self.map.get(&top_heap_item.coin) {
                // 比较代数，看堆中的条目是否是 `map` 中该 `coin` 的最新版本
                if map_entry.generation != top_heap_item.generation {
                    // 陈旧条目 (stale): `map` 中已有更新的条目，此堆条目已无效。
                    // 直接从堆中移除此陈旧条目。
                    self.heap.pop(); // 移除堆顶
                    continue; // 继续检查下一个堆顶元素
                }
                // 代数匹配，说明堆顶条目是 `map` 中该 `coin` 的当前条目。
                // 检查是否已过期。
                if map_entry.expires_at <= now {
                    // 已过期: 从 `map` 和 `heap` 中都移除。
                    expired_coin_names.push(top_heap_item.coin.clone()); // 记录代币名称
                    self.map.remove(&top_heap_item.coin); // 从map中移除
                    self.heap.pop(); // 从heap中移除
                } else {
                    // 未过期且非陈旧: 堆顶元素是有效的且未过期。
                    // 由于是最小堆（按过期时间排序），后续元素过期时间更晚，所以无需继续检查。
                    break;
                }
            } else {
                // `map` 中不存在该 `coin` 的条目，说明堆中的这个 `HeapItem` 是陈旧的
                // (可能其对应的 `map` 条目已被更新的 `insert` 操作覆盖后，又因过期被 `pop_one` 或之前的 `remove_expired` 清理)。
                self.heap.pop(); // 直接从堆中移除
            }
        }
        expired_coin_names // 返回被移除的过期代币列表
    }

    /// `pop_one` 方法
    ///
    /// 从缓存中弹出一个有效的、未过期的、且是当前最新版本的套利机会 (`ArbItem`)。
    /// 这个方法会持续从堆顶弹出元素，直到找到一个满足以下所有条件的条目：
    /// 1. 在 `map` 中存在对应的条目。
    /// 2. 堆中条目的代数 (`generation`) 与 `map` 中条目的代数匹配 (非陈旧)。
    /// 3. `map` 中条目的过期时间 (`expires_at`) 晚于当前时间 (未过期)。
    ///
    /// 一旦找到这样的条目，它会从 `map` 中移除 (表示已被取出处理)，并转换为 `ArbItem` 返回。
    /// 如果堆顶条目不满足条件 (陈旧或已过期)，则会根据情况清理 `map` 和 `heap`，并继续尝试下一个。
    ///
    /// 返回:
    /// - `Option<ArbItem>`: 如果找到有效的套利机会，则返回 `Some(ArbItem)`，否则返回 `None`。
    pub fn pop_one(&mut self) -> Option<ArbItem> {
        let now = Instant::now(); // 获取当前时间
        // 循环，直到找到一个有效条目或堆为空
        while let Some(top_heap_item) = self.heap.pop() { // 从堆顶弹出元素并获取所有权
            // 检查 `map` 中是否存在对应的 `coin` 条目
            if let Some(map_entry_ref) = self.map.get(&top_heap_item.coin) { // 先获取引用
                // 比较代数，确保是最新版本
                if map_entry_ref.generation == top_heap_item.generation {
                    // 是当前 `map` 中的条目 (非陈旧)
                    if map_entry_ref.expires_at > now {
                        // 未过期: 这是我们要找的有效条目。
                        // 从 `map` 中移除它 (因为要被处理了)，并返回 `ArbItem`。
                        // `self.map.remove()` 返回的是 `Option<ArbEntry>`，我们确定它存在，所以 `unwrap()`。
                        let owned_map_entry = self.map.remove(&top_heap_item.coin).unwrap();
                        return Some(ArbItem::new(top_heap_item.coin, top_heap_item.pool_id, owned_map_entry));
                    } else {
                        // 已过期: 虽然是最新版本，但已过期。从 `map` 中移除它。
                        self.map.remove(&top_heap_item.coin);
                        // 继续循环，尝试下一个堆顶元素。
                    }
                } else {
                    // 陈旧条目 (stale in heap): `map` 中已有更新的条目 (具有更高的 `generation`)。
                    // 这个 `top_heap_item` 是旧版本的，直接忽略，不需要修改 `map`。
                    // (因为 `map` 中的 `map_entry_ref.generation` 会大于 `top_heap_item.generation`)
                    // 继续循环。
                }
            } else {
                // `map` 中不存在该 `coin` 的条目: 说明堆中的这个 `HeapItem` 是陈旧的，
                // 并且其对应的 `map` 条目可能已被其他操作（如另一个 `pop_one` 调用或 `remove_expired`）移除了。
                // 直接忽略，继续循环。
                continue;
            }
        }
        // 如果循环结束 (堆为空) 仍未找到有效条目，则返回 `None`。
        None
    }
}
