// 该文件 `strategy.rs` (位于 `dex-indexer` crate中) 定义了 `PoolCreatedStrategy`，
// 这是一个用于发现和索引Sui链上新创建的DEX池的策略。
// 它实现了 `burberry::Strategy` trait，并与 `burberry::Engine` 集成，
// 通过定期查询Sui事件API来获取新池信息，然后将其存入数据库并更新内存缓存。
//
// **文件概览 (File Overview)**:
// 这个文件定义了 `dex-indexer` 如何去“发现新大陆”（新的DEX交易池）的策略。
// 想象一下，Sui区块链上每天都可能有人创建新的交易池，这个策略就是负责把这些新池子找出来，记录下来。
//
// **核心组件 (Core Components)**:
// 1.  **`PoolCreatedStrategy` 结构体**:
//     -   这是本文件的“主角”，它包含了执行发现策略所需的所有“工具”和“状态”。
//     -   `pool_cache: PoolCache`: 一个内存缓存，用来快速查找已经发现的池子信息。
//         (A memory cache for quick lookup of already discovered pool information.)
//     -   `db: Arc<dyn DB>`: 一个数据库连接（通过 `Arc` 共享，并实现了 `DB` trait），用来把发现的池子信息永久保存起来。
//         (A database connection (shared via `Arc` and implementing the `DB` trait) for persistently storing discovered pool information.)
//     -   `sui: SuiClient`: 一个Sui客户端，用来和Sui区块链“对话”，比如查询事件。
//         (A Sui client for "talking" with the Sui blockchain, e.g., querying events.)
//
// 2.  **`PoolCreatedStrategy::new()` 构造函数**:
//     -   用来创建一个新的 `PoolCreatedStrategy` 实例，并传入它需要的数据库连接、Sui客户端和池缓存。
//
// 3.  **`PoolCreatedStrategy::backfill_pools()` 异步方法**:
//     -   这个方法是策略的核心逻辑之一，负责“回填”历史数据。
//     -   它会遍历所有 `supported_protocols()` (在 `lib.rs` 中定义，比如Cetus, Turbos等)，
//         为每个协议启动一个并行的异步任务 (通过 `tokio::task::JoinSet`)。
//     -   每个任务都会调用 `backfill_pools_for_protocol()` 函数。
//     -   目的是尽可能地把该协议历史上所有创建过的池子都找出来并存到数据库和缓存里。
//
// 4.  **`backfill_pools_for_protocol()` 异步函数 (私有)**:
//     -   这是实际为单个协议回填池子数据的函数。
//     -   **工作流程**:
//         1.  获取该协议的事件过滤器 (`protocol.event_filter()`)，这个过滤器通常只关心该协议的“池创建事件”。
//         2.  获取该协议上次处理到的事件游标 (`cursor`)，这样可以从上次结束的地方继续，避免重复处理。
//         3.  进入一个循环，不断地通过Sui客户端的 `query_events` API 从链上查询一批新的“池创建事件”。
//             查询会从 `cursor` 开始，按时间顺序（通常是降序，即从最近的开始）获取。
//         4.  对于查询到的每一页 (`page`) 事件数据：
//             -   遍历这页里的每个Sui事件 (`event`)。
//             -   调用 `protocol.sui_event_to_pool(event, &sui).await` 将原始的Sui事件转换为统一的 `Pool` 结构。
//                 （这个转换逻辑在各个 `protocols/*.rs` 文件中定义，例如 `protocols/cetus.rs` 会把Cetus的特定事件转成 `Pool`）。
//             -   如果转换成功，就更新内存中的 `pool_cache`：
//                 -   按单个代币类型索引 (`token_pools`)。
//                 -   按交易对索引 (`token01_pools`)。
//                 -   按池ID索引 (`pool_map`)。
//             -   将转换成功的 `Pool` 对象收集到一个临时列表 `pools_to_flush` 中。
//         5.  当一页事件处理完毕后（或者处理了一定数量后），调用 `db.flush(&protocol, &pools_to_flush, new_cursor)?`
//             将这批新发现的池子数据和最新的事件游标 (`new_cursor`) 一起写入到数据库。
//         6.  如果RPC返回结果表明还有下一页 (`page.has_next_page`)，则更新游标并继续循环获取下一页数据。
//         7.  直到所有历史事件都处理完毕。
//     -   这个函数通过分页查询和游标管理，确保能够完整地、不重复地获取一个协议所有历史上创建的池。
//
// 5.  **`Strategy<Event, NoAction>` trait 实现 for `PoolCreatedStrategy`**:
//     -   这使得 `PoolCreatedStrategy` 可以被 `burberry::Engine` 用作一个标准的事件处理策略。
//     -   `name()`: 返回策略名称 "PoolCreatedStrategy"。
//     -   `sync_state()`: 在引擎启动时调用一次。这里它会调用 `self.backfill_pools().await` 来执行初始的历史数据回填。
//     -   `process_event()`: 每当引擎从收集器（比如 `QueryEventCollector`）那里收到一个 `Event` 时，这个方法就会被调用。
//         -   对于 `PoolCreatedStrategy` 来说，它响应的是 `QueryEventCollector` 定期发出的 `Event::QueryEventTrigger` (虽然事件参数 `_event` 被忽略了)。
//         -   收到触发后，它会再次调用 `self.backfill_pools().await`。这意味着它会定期地尝试从链上拉取最新的池创建事件，
//             从而实现对新创建池子的增量索引。
//         -   `NoAction` 表示这个策略本身不直接产生需要外部执行器处理的“动作”，它的主要工作是更新数据库和缓存。
//
// **工作流程总结 (Workflow Summary)**:
// 1.  `DexIndexer` 初始化时，创建 `PoolCreatedStrategy`。
// 2.  `PoolCreatedStrategy::sync_state()` 被调用，触发一次全面的 `backfill_pools()`，尽可能拉取所有协议的历史池数据。
// 3.  `burberry::Engine` 启动后，`QueryEventCollector` 会定期产生 `Event::QueryEventTrigger`。
// 4.  每次 `PoolCreatedStrategy::process_event()` 收到这个触发事件时，会再次调用 `backfill_pools()`。
// 5.  `backfill_pools()` (通过 `backfill_pools_for_protocol`) 会从上次记录的游标开始，查询Sui链上新发生的“池创建事件”。
// 6.  新发现的池子信息会被解析、存入数据库 (`db.flush()`)，并更新到内存缓存 (`pool_cache`)。
// 这样，`DexIndexer` 就能持续地维护一个相对最新的DEX池数据库和缓存，供其他部分（如套利机器人）查询。

// 引入标准库的 Arc (原子引用计数)。
use std::sync::Arc;

// 引入 burberry 框架的 Strategy trait, ActionSubmitter trait, 和 async_trait 宏。
use burberry::{async_trait, ActionSubmitter, Strategy};
// 引入 eyre 库，用于错误处理。
use eyre::Result;
// 引入 Sui SDK 中的 EventID (事件唯一标识) 和 SuiClient (Sui RPC客户端)。
use sui_sdk::{types::event::EventID, SuiClient};
// 引入 Tokio 的 JoinSet，用于管理并发的异步任务。
use tokio::task::JoinSet;
// 引入 tracing 库的日志宏。
use tracing::{debug, error, info};

// 从当前crate的根模块引入 supported_protocols (获取支持的DEX协议列表) 和 token01_key (生成交易对的规范键)。
use crate::{
    supported_protocols, token01_key,
    types::{Event, NoAction, PoolCache, Protocol}, // 从本地types模块引入Event枚举, NoAction (表示无动作), PoolCache, Protocol枚举。
    DB, // 引入DB trait (数据库接口)。
};

/// `PoolCreatedStrategy` 结构体
///
/// 实现了 `burberry::Strategy` trait，用于发现和索引新创建的DEX池。
#[derive(Clone)] // 允许克隆 PoolCreatedStrategy 实例 (内部成员使用Arc，克隆成本低)
pub struct PoolCreatedStrategy {
    pool_cache: PoolCache, // 内存中的池数据缓存，用于快速查询和更新

    db: Arc<dyn DB>, // 数据库接口的动态分发对象，用于持久化存储池数据和游标
    sui: SuiClient,  // Sui RPC客户端，用于从链上查询事件和对象数据
}

impl PoolCreatedStrategy {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `PoolCreatedStrategy` 实例。
    ///
    /// 参数:
    /// - `db`: 一个实现了 `DB` trait 的数据库实例的共享引用 (Arc)。
    /// - `sui`: 一个 `SuiClient` 实例。
    /// - `pool_cache`: 一个 `PoolCache` 实例，用于在内存中维护池数据。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `PoolCreatedStrategy` 实例。
    pub fn new(db: Arc<dyn DB>, sui: SuiClient, pool_cache: PoolCache) -> Result<Self> {
        Ok(Self { pool_cache, db, sui })
    }

    /// `backfill_pools` 异步方法
    ///
    /// 为所有支持的DEX协议执行历史数据回填。
    /// 它会为每个协议并发地启动一个 `backfill_pools_for_protocol` 任务。
    ///
    /// 返回:
    /// - `Result<()>`: 如果所有协议的回填任务都无严重错误地完成（或被处理），则返回Ok。
    pub async fn backfill_pools(&self) -> Result<()> {
        let mut joinset = JoinSet::new(); // 创建一个JoinSet来管理并发任务
        // 从数据库获取所有协议当前已处理到的事件游标
        let cursors = self.db.get_processed_cursors()?;

        // 遍历所有支持的DEX协议
        for protocol_enum_val in supported_protocols() {
            // 克隆共享资源以传递给异步任务
            let sui_client_clone = self.sui.clone();
            let db_arc_clone = Arc::clone(&self.db);
            let pool_cache_clone = self.pool_cache.clone();
            // 获取当前协议的游标 (如果存在的话)
            let cursor_for_protocol = cursors.get(&protocol_enum_val).cloned().flatten();

            // 为每个协议启动一个独立的异步任务来执行回填
            joinset.spawn(async move {
                backfill_pools_for_protocol(
                    sui_client_clone,
                    db_arc_clone,
                    protocol_enum_val,
                    cursor_for_protocol,
                    pool_cache_clone,
                )
                .await // 等待单个协议的回填完成
            });
        }

        // 等待所有并发的回填任务完成
        while let Some(task_result) = joinset.join_next().await {
            // `task_result` 是 `Result<Result<(), eyre::Error>, tokio::task::JoinError>`
            // 外层Result处理任务本身的panic或取消，内层Result处理 `backfill_pools_for_protocol` 的业务逻辑错误。
            if let Err(join_err) = task_result { // 如果任务本身执行出错 (例如panic)
                error!("回填池数据的并发任务执行失败: {:?}", join_err);
                // 根据需要，这里可以选择是否要传播错误或仅记录。
                // 如果一个协议的回填失败不应阻止其他协议，则仅记录。
            } else if let Ok(Err(backfill_err)) = task_result { // 如果任务正常完成，但业务逻辑返回错误
                error!("协议回填过程中发生错误: {:?}", backfill_err);
            }
            // 如果 `Ok(Ok(()))` 则表示单个协议回填成功，无需特殊处理。
        }

        info!("所有协议的池数据回填已完成或已尝试。");
        Ok(())
    }
}

/// 为 `PoolCreatedStrategy` 实现 `burberry::Strategy<Event, NoAction>` trait。
/// - `Event`: 此策略监听的输入事件类型 (来自 `crate::types::Event`)。
/// - `NoAction`: 此策略不产生需要外部执行器处理的动作。
#[async_trait]
impl Strategy<Event, NoAction> for PoolCreatedStrategy {
    /// `name` 方法 (来自 `Strategy` trait)
    /// 返回策略的名称。
    fn name(&self) -> &str {
        "PoolCreatedStrategy"
    }

    /// `sync_state` 方法 (来自 `Strategy` trait)
    ///
    /// 在策略首次启动或引擎要求同步状态时调用。
    /// 对于此策略，它会执行一次完整的数据回填。
    async fn sync_state(&mut self, _submitter: Arc<dyn ActionSubmitter<NoAction>>) -> Result<()> {
        info!("PoolCreatedStrategy: 正在执行初始状态同步 (数据回填)...");
        self.backfill_pools().await // 调用回填函数
    }

    /// `process_event` 方法 (来自 `Strategy` trait)
    ///
    /// 处理从引擎接收到的单个事件。
    /// 此策略主要响应由 `QueryEventCollector` 定期发出的 `Event::QueryEventTrigger`。
    /// 收到触发后，它会再次执行数据回填，以获取自上次回填以来新创建的池。
    ///
    /// 参数:
    /// - `_event`: 接收到的事件 (当前实现中忽略了事件的具体内容，总是执行回填)。
    /// - `_`: 动作提交器 (此策略不产生动作，所以忽略)。
    async fn process_event(&mut self, _event: Event, _: Arc<dyn ActionSubmitter<NoAction>>) {
        info!("PoolCreatedStrategy: 收到事件触发，开始新一轮的池数据回填...");
        if let Err(error) = self.backfill_pools().await { // 执行回填
            error!("定期池数据回填过程中发生错误: {:?}", error); // 记录错误
        }
    }
}

/// `backfill_pools_for_protocol` 异步函数 (私有)
///
/// 为单个指定的DEX协议 (`protocol`) 执行历史池数据的回填。
/// 它会从上次记录的事件游标 (`cursor`) 开始，通过Sui RPC分页查询该协议的“池创建事件”，
/// 直到处理完所有相关的历史事件。
///
/// 参数:
/// - `sui`: `SuiClient` 实例。
/// - `db`: 实现了 `DB` trait 的数据库实例的共享引用。
/// - `protocol`: 要回填数据的DEX协议。
/// - `cursor`: (可选) 上次处理到的事件ID (`EventID`)。如果为 `None`，则从头开始查询。
/// - `pool_cache`: `PoolCache` 实例，用于在发现新池时同步更新内存缓存。
///
/// 返回:
/// - `Result<()>`: 如果回填成功则返回Ok，否则返回错误。
async fn backfill_pools_for_protocol(
    sui: SuiClient,
    db: Arc<dyn DB>,
    protocol: Protocol,
    mut cursor: Option<EventID>, // 游标设为可变，因为会在循环中更新
    pool_cache: PoolCache,
) -> Result<()> {
    // 获取该协议的事件过滤器 (通常是监听特定类型的池创建事件)
    let event_filter_for_protocol = protocol.event_filter();

    debug!(target: "dex_indexer_strategy", protocol = %protocol, ?event_filter_for_protocol, initial_cursor = ?cursor, "开始为协议回填池数据...");

    // 从 PoolCache 解构出各个索引映射表，以便直接操作它们。
    // 注意：这里获取的是 `PoolCache` 的克隆副本中的 `DashMap` 的引用。
    // 对这些 `DashMap` 的修改会反映在 `pool_cache` 这个副本中，但不会直接修改
    // `PoolCreatedStrategy` 结构体中原始的 `pool_cache`，除非 `PoolCache` 内部的 `DashMap`
    // 是用 `Arc` 包裹的（当前 `PoolCache` 的定义似乎不是这样，它直接拥有 `DashMap`）。
    // 如果 `PoolCache` 实现了 `Clone` 并且其字段是 `Arc<DashMap>`，那么这里的克隆是浅拷贝，
    // 修改会影响到其他持有相同 `Arc` 的 `PoolCache` 实例。
    // 假设 `PoolCache::clone()` 行为是合理的 (例如，如果 `DashMap` 本身是 `Arc<DashMap>`，则克隆Arc；
    // 如果 `DashMap` 不是Arc包裹，则克隆 `DashMap` 本身，这里修改的是本地副本)。
    // 从 `PoolCreatedStrategy::new` 的实现看，`pool_cache` 是直接 `clone()` 传入的，
    // 所以这里的修改是针对 `backfill_pools_for_protocol` 函数作用域内的 `pool_cache` 副本。
    // 这意味着内存缓存的更新可能只在此函数或其调用者 `backfill_pools` 的作用域内有效，
    // 而不会直接更新到 `PoolCreatedStrategy` 实例中的 `pool_cache`，除非 `PoolCache` 内部巧妙地使用了共享机制。
    // **澄清**：`PoolCache` 结构体内部的 `DashMap` 字段是 `Arc<DashMap>`，所以这里的克隆是浅拷贝，
    // 对 `token_pools`, `token01_pools`, `pool_map` 的修改会反映到所有共享此 `PoolCache` 的地方。
    let PoolCache {
        token_pools,    // 按单个代币类型索引: HashMap<TokenType, HashSet<Pool>> (实际是Arc<DashMap<...>>)
        token01_pools,  // 按交易对索引: HashMap<Token01Key, HashSet<Pool>> (实际是Arc<DashMap<...>>)
        pool_map,       // 按池ID索引: DashMap<ObjectID, Pool> (实际是Arc<DashMap<...>>)
    } = pool_cache;

    // 进入循环，分页查询事件直到没有更多事件为止
    loop {
        // 通过Sui RPC查询事件。
        // `query_events` 参数：过滤器, 起始游标, 分页大小(None为默认), 是否降序(false为升序，即从旧到新)
        let current_page_of_events = sui
            .event_api()
            .query_events(event_filter_for_protocol.clone(), cursor, None, false)
            .await?; // 处理RPC错误
        debug!(target: "dex_indexer_strategy", protocol = %protocol, events_count = page.data.len(), has_next_page = page.has_next_page, "查询到一页事件");

        // 如果当前页没有数据，说明已经处理完所有相关事件，可以结束回填。
        if current_page_of_events.data.is_empty() {
            break;
        }

        let mut new_pools_in_page = vec![]; // 用于收集当前页中新发现的池
        // 遍历当前页的每个Sui事件
        for event_item in &current_page_of_events.data {
            // 尝试将通用的 `SuiEvent` 转换为特定协议的 `Pool` 结构。
            // `protocol.sui_event_to_pool` 是一个在 `Protocol` 枚举上定义的方法 (可能通过trait)，
            // 它会根据 `protocol` 的具体值调用相应协议的事件解析逻辑 (例如 `cetus::CetusPoolCreated::try_from(event)?.to_pool(&sui)`).
            match protocol.sui_event_to_pool(event_item, &sui).await {
                Ok(parsed_pool_object) => { // 如果转换成功
                    // --- 更新内存缓存 (`PoolCache`) ---
                    // 1. 按单个代币类型索引 (`token_pools`)
                    for token_info in &parsed_pool_object.tokens {
                        let token_type_key = token_info.token_type.clone();
                        // `entry(key).or_default()` 获取或插入一个空的HashSet，然后插入池的克隆。
                        token_pools.entry(token_type_key).or_default().insert(parsed_pool_object.clone());
                    }
                    // 2. 按交易对索引 (`token01_pools`)
                    for (token0_type_str, token1_type_str) in parsed_pool_object.token01_pairs() {
                        let pair_key = token01_key(&token0_type_str, &token1_type_str); // 生成规范化的交易对键
                        token01_pools.entry(pair_key).or_default().insert(parsed_pool_object.clone());
                    }
                    // 3. 按池ID索引 (`pool_map`)
                    pool_map.insert(parsed_pool_object.pool, parsed_pool_object.clone()); // DashMap可以直接插入

                    // 将成功解析的池对象添加到当前页的新池列表中，以便后续批量写入数据库。
                    new_pools_in_page.push(parsed_pool_object);
                }
                Err(parse_error) => { // 如果事件转换为Pool失败
                    error!(target: "dex_indexer_strategy", protocol = %protocol, event = ?event_item, error = ?parse_error, "无效的池创建事件或转换Pool对象失败");
                }
            }
        }
        debug!(target: "dex_indexer_strategy", protocol = %protocol, new_pools_count = new_pools_in_page.len(), current_cursor = ?cursor, "处理完一页事件，发现 {} 个新池", new_pools_in_page.len());

        // 更新游标：
        // 如果RPC结果指示有下一页，则使用返回的 `next_cursor`。
        // 否则，表示当前页是最后一页，将游标更新为当前页最后一个事件的ID。
        // 这样做是为了下一次调用 `backfill_pools_for_protocol` 时能从正确的位置继续。
        cursor = if current_page_of_events.has_next_page {
            current_page_of_events.next_cursor
        } else {
            // 获取当前页最后一个事件的ID作为新的游标
            current_page_of_events.data.last().map(|event_data| event_data.id)
        };
        // 将当前页发现的新池数据和更新后的游标刷新到数据库。
        db.flush(&protocol, &new_pools_in_page, cursor)?;

        // （可选）在处理完每一页后可以短暂休眠，以避免对RPC节点造成过大压力。
        // (Optional: Sleep briefly after processing each page to avoid overloading the RPC node.)
        // std::thread::sleep(Duration::from_secs(1)); // 例如休眠1秒 (Example: sleep for 1 second)

        // 如果没有下一页的游标（通常意味着已处理完所有事件），则退出循环。
        if cursor.is_none() && !current_page_of_events.has_next_page { // 确保在最后一页且next_cursor为None时确实退出
            break;
        }
    }

    info!(
        target: "dex_indexer_strategy",
        protocol = %protocol,
        total_pools_in_db = %db.pool_count(&protocol)?, // 从数据库获取该协议的总池数
        "协议数据回填完成。"
    );

    Ok(())
}

[end of crates/dex-indexer/src/strategy.rs]
