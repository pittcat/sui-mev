// 该文件 `mod.rs` 是 `strategy` 模块的根文件，定义了套利机器人的核心策略逻辑。
// `ArbStrategy` 结构体是主要的策略实现，它负责接收不同来源的事件，
// 分析这些事件以发现潜在的套利机会，管理这些机会的缓存，并将它们分发给工作线程进行详细处理和可能的执行。
//
// 文件概览:
// 1. 声明子模块: `arb_cache` (套利机会缓存) 和 `worker` (处理套利机会的工作线程逻辑)。
// 2. `ArbStrategy` 结构体:
//    - `sender`: 机器人的Sui地址。
//    - `arb_item_sender`: 一个异步通道 (`async_channel::Sender`)，用于将套利机会 (`ArbItem`) 发送给工作线程。
//    - `arb_cache`: `ArbCache` 实例，用于存储和管理待处理的套利机会。
//    - `recent_arbs`: 一个双端队列 (`VecDeque`)，用于跟踪最近处理过的套利机会（基于代币），以避免短期内重复处理。
//    - `max_recent_arbs`: `recent_arbs` 队列的最大长度。
//    - `simulator_pool`: 共享的模拟器对象池，供工作线程使用。
//    - `own_simulator`: 策略自身可能用于某些特定模拟（如解析事件中的对象）。
//    - `rpc_url`, `workers`, `sui`, `epoch`, `dedicated_simulator`: 其他配置和状态。
// 3. `ArbStrategy::new()`: 构造函数，初始化策略状态。
// 4. 事件处理方法:
//    - `on_new_tx_effects()`: 处理公开交易的执行效果 (`SuiTransactionBlockEffects`) 和相关事件 (`SuiEvent`)。
//      从中解析出可能影响价格的代币和交易池，并将其作为套利机会存入 `arb_cache`。
//    - `on_new_tx()`: (当前为空实现) 原本可能用于处理传入的原始 `TransactionData`。
//    - `on_new_shio_item()`: 处理来自Shio MEV协议的事件 (`ShioItem`)。
//      从中解析潜在机会、构建模拟上下文（包括覆盖对象、设置Gas价格），并存入 `arb_cache`。
// 5. 辅助方法:
//    - `parse_involved_coin_pools()`: 从 `SuiEvent` 列表中解析出相关的代币对和池ID。
//    - `get_potential_opportunity()`: 从 `ShioItem` 中提取潜在机会（代币对、池ID）和需要覆盖的对象状态。
//    - `get_latest_epoch()`: 获取并缓存最新的Sui纪元信息。
// 6. `burberry::Strategy` trait 实现:
//    - `name()`: 返回策略名称。
//    - `sync_state()`: 初始化状态，主要是创建工作线程和用于分发 `ArbItem` 的异步通道。
//      每个工作线程会运行一个 `Worker` 实例。
//    - `process_event()`: 接收引擎分发的事件，调用相应的 `on_new_...` 方法处理，
//      然后从 `arb_cache` 中取出有效的套利机会，通过通道发送给工作线程。
//      同时管理 `recent_arbs` 队列和清理过期的缓存条目。
// 7. `run_in_tokio!` 宏: 一个工具宏，用于在当前线程或新的Tokio当前线程运行时中执行异步代码块。
//    这在从同步上下文（如 `std::thread::spawn` 的闭包）调用异步代码时非常有用。
//
// 核心逻辑流程:
// 1. `sync_state` (在引擎启动时调用): 创建 `N` 个工作线程 (`Worker`)。每个worker监听一个共享的异步通道 (`arb_item_receiver`)。
// 2. 当外部事件 (如新交易、Shio事件) 到达 `ArbStrategy::process_event` 时:
//    a. 事件被分发到相应的 `on_new_...` 方法进行初步解析。
//    b. 解析出的潜在套利机会 (代币、池ID、模拟上下文等) 被存入 `arb_cache`。
//       `arb_cache` 会处理机会的去重（基于代币）、更新和过期。
//    c. `process_event` 尝试从 `arb_cache` 中弹出有效的、最新的、未过期的套利机会 (`ArbItem`)。
//    d. 如果弹出的机会对应的代币不在 `recent_arbs` 队列中（或机会来自Shio，不受此限制），
//       则将该 `ArbItem` 通过异步通道发送给等待的工作线程。
//    e. 更新 `recent_arbs` 队列，并清理 `arb_cache` 中的过期条目。
// 3. 工作线程 (`Worker`) 从通道接收到 `ArbItem` 后，会进行详细的套利分析：
//    a. 发现针对 `ArbItem.coin` 的所有可能交易路径。
//    b. 使用模拟器池中的模拟器评估这些路径的盈利能力。
//    c. 如果找到有利可图的套利机会，则构建交易并提交给执行器 (`ActionSubmitter`)。

// 引入标准库及第三方库
use std::{
    collections::{HashSet, VecDeque}, // HashSet用于存储唯一元素, VecDeque用于实现LRU队列 (recent_arbs)
    str::FromStr,                    // 用于从字符串转换 (例如TransactionDigest)
    sync::Arc,                       // 原子引用计数
    time::Duration,                  // 时间处理
};

use arb_cache::{ArbCache, ArbItem}; // 从子模块 `arb_cache` 引入
use async_channel::Sender;        // 异步通道的发送端，用于策略向worker分发任务
use burberry::ActionSubmitter;    // `burberry`引擎框架中的动作提交器trait，用于策略向执行器提交动作
use dex_indexer::types::Protocol; // DEX协议类型
use eyre::{ensure, eyre, Result}; // 错误处理库
use fastcrypto::encoding::{Base64, Encoding}; // Base64编解码 (用于处理ShioObject中的BCS数据)
use object_pool::ObjectPool;      // 对象池 (用于模拟器)
use rayon::prelude::*;            // Rayon库，用于数据并行处理 (这里用于ShioObject的解析)
use shio::{ShioItem, ShioObject};  // Shio MEV协议相关的类型
use simulator::{ReplaySimulator, SimEpoch, SimulateCtx, Simulator}; // 各种模拟器和模拟上下文
use sui_json_rpc_types::{SuiEvent, SuiTransactionBlockEffects, SuiTransactionBlockEffectsAPI}; // Sui RPC类型
use sui_sdk::{SuiClient, SuiClientBuilder}; // Sui SDK客户端
use sui_types::{
    base_types::{MoveObjectType, ObjectID, SuiAddress}, // Sui基本类型
    committee::ProtocolVersion,                         // 协议版本 (用于MoveObject反序列化)
    digests::TransactionDigest,                         // 交易摘要
    object::{MoveObject, Object, Owner, OBJECT_START_VERSION}, // Sui对象相关类型
    supported_protocol_versions::{Chain, ProtocolConfig}, // 协议配置 (用于MoveObject反序列化)
    transaction::{InputObjectKind, ObjectReadResult, TransactionData}, // 交易和对象读取相关类型
};
use tokio::{
    runtime::{Builder, Handle, RuntimeFlavor}, // Tokio运行时相关，用于 `run_in_tokio!` 宏
    task::JoinSet,                             // Tokio任务集合，用于并发执行异步任务
};
use tracing::{debug, error, info, instrument, warn}; // 日志库
use worker::Worker;                                  // 从子模块 `worker` 引入

// 从当前crate的其他模块引入
use crate::{
    arb::Arb, // 套利计算核心逻辑 (在 `bin/arb/src/arb.rs` 中定义)
    common::get_latest_epoch, // 获取最新Sui纪元信息的函数
    types::{Action, Event, Source}, // 自定义的Action, Event, Source枚举
};

/// `ArbStrategy` 结构体
///
/// 套利策略的核心实现。
pub struct ArbStrategy {
    sender: SuiAddress,                         // 机器人操作者的Sui地址，用于构建和发送交易
    arb_item_sender: Option<Sender<ArbItem>>,   // (可选) 异步通道的发送端，用于将ArbItem发送给工作线程
                                                // 在 `sync_state` 中初始化
    arb_cache: ArbCache,                        // 套利机会缓存实例

    recent_arbs: VecDeque<String>,              // 最近处理过的套利机会的代币列表 (用于避免短期重复)
    max_recent_arbs: usize,                     // `recent_arbs` 队列的最大长度

    simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>, // 共享的模拟器对象池 (供worker使用)
    own_simulator: Arc<dyn Simulator>,          // 策略自身使用的模拟器 (例如，解析事件时获取对象信息)
    rpc_url: String,                            // Sui RPC节点的URL
    workers: usize,                             // 启动的工作线程数量
    sui: SuiClient,                             // Sui SDK客户端实例 (用于获取epoch等链上信息)
    epoch: Option<SimEpoch>,                    // (可选) 缓存的当前Sui纪元信息
    dedicated_simulator: Option<Arc<ReplaySimulator>>, // (可选) 专用的回放模拟器
}

impl ArbStrategy {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `ArbStrategy` 实例。
    ///
    /// 参数:
    /// - `attacker`: 机器人的Sui地址。
    /// - `simulator_pool`: 共享的模拟器对象池。
    /// - `own_simulator`: 策略自身使用的模拟器。
    /// - `recent_arbs_capacity`: `recent_arbs` 队列的容量。
    /// - `rpc_url`: Sui RPC URL。
    /// - `workers_count`: 要启动的工作线程数量。
    /// - `dedicated_simulator`: (可选的) 专用回放模拟器。
    pub async fn new(
        attacker: SuiAddress,
        simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>,
        own_simulator: Arc<dyn Simulator>,
        recent_arbs_capacity: usize, // 参数名修改为 capacity 更清晰
        rpc_url: &str,
        workers_count: usize, // 参数名修改为 count 更清晰
        dedicated_simulator: Option<Arc<ReplaySimulator>>,
    ) -> Self {
        // 创建Sui客户端并获取初始的纪元信息
        let sui_client = SuiClientBuilder::default().build(rpc_url).await.unwrap();
        let initial_epoch = get_latest_epoch(&sui_client).await.unwrap();

        Self {
            sender: attacker,
            arb_item_sender: None, // 在 sync_state 中初始化
            arb_cache: ArbCache::new(Duration::from_secs(5)), // 套利机会在缓存中保留5秒
            recent_arbs: VecDeque::with_capacity(recent_arbs_capacity), // 初始化指定容量的双端队列
            max_recent_arbs: recent_arbs_capacity,
            simulator_pool,
            own_simulator,
            rpc_url: rpc_url.to_string(),
            workers: workers_count,
            sui: sui_client,
            epoch: Some(initial_epoch),
            dedicated_simulator,
        }
    }

    /// `on_new_tx` 方法 (处理原始交易数据)
    ///
    /// 当前为空实现。理论上，这里可以处理从某些来源（如私有中继）获取到的原始、未上链的 `TransactionData`。
    /// 逻辑可能包括：
    /// 1. 模拟这个 `tx`。
    /// 2. 从模拟结果中解析出交易效果和事件。
    /// 3. 调用 `on_new_tx_effects` 或类似逻辑，将潜在机会加入 `arb_cache`。
    #[instrument(name = "on-new-tx", skip_all, fields(tx = %tx.digest()))] // 追踪此方法，记录交易摘要
    async fn on_new_tx(&self, tx: TransactionData) -> Result<()> {
        // TODO: 实现处理原始交易数据的逻辑
        // 1. 模拟交易: self.own_simulator.simulate(tx, sim_ctx).await?
        // 2. 解析模拟结果中的effects和events
        // 3. 调用 self.on_new_tx_effects(sim_effects, sim_events).await? (可能需要可变self)
        //    或者直接从中提取coin_pools并加入arb_cache
        warn!("on_new_tx 方法尚未完全实现，接收到交易: {:?}", tx.digest());
        Ok(())
    }

    /// `on_new_tx_effects` 方法 (处理已上链交易的效果)
    ///
    /// 当接收到新的公开交易执行效果 (`SuiTransactionBlockEffects`) 和相关事件 (`SuiEvent`) 时调用此方法。
    ///
    /// 步骤:
    /// 1. 调用 `parse_involved_coin_pools` 从事件中解析出可能受影响的代币和交易池ID。
    /// 2. 如果没有解析到相关的代币/池，则直接返回。
    /// 3. 获取最新的Sui纪元信息，并创建一个新的模拟上下文 (`SimulateCtx`)。
    /// 4. 对于每个解析到的 (代币, 池ID) 对，将其作为套利机会 (`Source::Public`) 插入到 `arb_cache` 中。
    #[instrument(name = "on-new-tx-effects", skip_all, fields(tx = %tx_effects.transaction_digest()))]
    async fn on_new_tx_effects(&mut self, tx_effects: SuiTransactionBlockEffects, events: Vec<SuiEvent>) -> Result<()> {
        // 从事件中解析出相关的 (代币, 池ID) 组合
        let coin_pools_set = self.parse_involved_coin_pools(events).await;
        if coin_pools_set.is_empty() { // 如果没有解析到，则无需进一步处理
            return Ok(());
        }

        let transaction_digest = tx_effects.transaction_digest(); // 获取交易摘要
        let latest_epoch = self.get_latest_epoch().await?; // 获取最新的纪元信息
        // 创建模拟上下文，初始时不包含特定的对象覆盖 (空vec![])
        // 因为这是公开交易，我们通常基于最新的链状态进行模拟。
        let sim_ctx_for_public_event = SimulateCtx::new(latest_epoch, vec![]);

        // 将每个解析到的 (代币, 池ID) 对作为套利机会插入缓存
        for (coin, pool_id) in coin_pools_set {
            self.arb_cache.insert(
                coin,
                pool_id,
                *transaction_digest, // 交易摘要
                sim_ctx_for_public_event.clone(), // 克隆模拟上下文
                Source::Public, // 标记来源为公开交易
            );
        }
        Ok(())
    }

    /// `on_new_shio_item` 方法 (处理Shio MEV竞价事件)
    ///
    /// 当接收到来自Shio协议的新事件 (`ShioItem`) 时调用此方法。
    /// Shio事件通常代表一个MEV机会，可能包含一个“机会交易”及其相关的对象状态。
    ///
    /// 步骤:
    /// 1. 调用 `get_potential_opportunity` 从 `ShioItem` 中解析出潜在的套利机会
    ///    (受影响的代币/池，以及需要覆盖的链上对象状态)。
    /// 2. 如果没有解析到机会，则直接返回。
    /// 3. 获取最新的Sui纪元信息。
    /// 4. 创建一个模拟上下文 (`SimulateCtx`)，其中包含了从 `ShioItem` 中提取的、需要覆盖的对象状态，
    ///    并设置Gas价格与机会交易的Gas价格一致（这对于准确模拟MEV场景很重要）。
    /// 5. 对于每个解析到的 (代币, 池ID) 对，将其作为套利机会 (`Source::Shio`) 插入到 `arb_cache` 中。
    ///    `Source::Shio` 会包含机会交易的摘要、竞价金额（初始为0）、截止时间等MEV相关信息。
    #[instrument(name = "on-new-shio-item", skip_all, fields(tx = %shio_item.tx_digest()))]
    async fn on_new_shio_item(&mut self, shio_item: ShioItem) -> Result<()> {
        // 从ShioItem中解析潜在机会：(受影响的代币池集合, 需要在模拟中覆盖的对象状态列表)
        let (coin_pools_set, override_objects_for_sim) =
            match self.get_potential_opportunity(&shio_item).await {
                Some(potential_opportunity_data) => potential_opportunity_data,
                None => return Ok(()), // 如果没有解析到机会，则返回
            };

        // 将ShioItem中的交易摘要字符串转换为 TransactionDigest 类型
        let opportunity_tx_digest = TransactionDigest::from_str(shio_item.tx_digest()).map_err(|e| eyre!(e))?;
        let latest_epoch = self.get_latest_epoch().await?; // 获取最新纪元

        // 创建模拟上下文，使用从ShioItem中提取的 `override_objects_for_sim`。
        // 这允许模拟器在模拟时使用这些特定的对象版本，而不是最新的链上版本。
        let mut sim_ctx_for_shio = SimulateCtx::new(latest_epoch, override_objects_for_sim);
        // **重要**: 对于MEV机会，模拟时应使用机会交易的Gas价格，以准确评估利润。
        sim_ctx_for_shio.with_gas_price(shio_item.gas_price());

        // 构建 `Source::Shio`，包含MEV竞价相关信息
        let shio_source = Source::Shio {
            opp_tx_digest: opportunity_tx_digest, // 机会交易的摘要
            bid_amount: 0, // 初始竞价金额设为0，后续由Worker计算和更新
            start: utils::current_time_ms(), // 记录处理开始时间
            // 将ShioItem提供的截止时间稍微提前一点 (20ms)，为最终的dry_run和网络延迟留出时间。
            deadline: shio_item.deadline_timestamp_ms().saturating_sub(20), // 使用saturating_sub防止下溢
            arb_found: 0, // 套利机会发现时间戳，稍后更新
        };

        // 将每个解析到的 (代币, 池ID) 对作为套利机会插入缓存
        for (coin, pool_id) in coin_pools_set {
            self.arb_cache.insert(
                coin,
                pool_id,
                opportunity_tx_digest, // 使用机会交易的摘要
                sim_ctx_for_shio.clone(),
                shio_source, // 标记来源为Shio
            );
        }
        Ok(())
    }

    /// `parse_involved_coin_pools` 方法 (私有辅助函数)
    ///
    /// 从一列 `SuiEvent` 中解析出所有涉及到的 (代币类型, 可选池ID) 的唯一组合。
    /// 它会并发地处理每个事件，尝试将其转换为一个已知的DEX协议的 `SwapEvent`，
    /// 然后从 `SwapEvent` 中提取相关的代币和池ID。
    ///
    /// 参数:
    /// - `events`: `SuiEvent` 的向量。
    ///
    /// 返回:
    /// - `HashSet<(String, Option<ObjectID>)>`: 包含所有唯一 (代币, 池ID) 对的集合。
    async fn parse_involved_coin_pools(&self, events: Vec<SuiEvent>) -> HashSet<(String, Option<ObjectID>)> {
        let mut join_set = JoinSet::new(); // 用于并发处理事件

        for event in events {
            // 克隆 `own_simulator` 的Arc指针，传递给异步任务。
            // `own_simulator` 可能用于从事件数据中获取缺失的对象信息以正确解析SwapEvent。
            let own_simulator_clone = Arc::clone(&self.own_simulator);
            join_set.spawn(async move { // 为每个事件创建一个异步任务
                // 尝试将 SuiEvent 转换为已知的协议类型 (Protocol枚举)
                if let Ok(protocol_type) = Protocol::try_from(&event) {
                    // 尝试将特定协议的 SuiEvent 转换为通用的 SwapEvent 结构
                    if let Ok(swap_event_data) = protocol_type.sui_event_to_swap_event(&event, own_simulator_clone).await {
                        // `involved_coin_one_side()` 可能返回交易对中的一个代币（例如，非SUI的那一个），
                        // 或者一个代表性的代币，用于触发后续的套利路径搜索。
                        // `pool_id()` 返回该交换发生的池的ID。
                        return Some((swap_event_data.involved_coin_one_side(), swap_event_data.pool_id()));
                    }
                }
                None // 如果转换失败，则返回None
            });
        }

        // 收集所有并发任务的结果
        let mut coin_pools_set = HashSet::new();
        while let Some(task_result) = join_set.join_next().await { // 等待下一个任务完成
            if let Ok(Some((coin, pool_id))) = task_result { // 如果任务成功且返回Some(...)
                coin_pools_set.insert((coin, pool_id));
            }
        }
        coin_pools_set // 返回收集到的唯一 (代币, 池ID) 对
    }

    /// `get_potential_opportunity` 方法 (私有辅助函数)
    ///
    /// 从一个 `ShioItem` (代表MEV机会) 中解析出潜在的套利机会信息。
    /// 包括：受影响的 (代币, 池ID) 对，以及需要在模拟中覆盖的链上对象状态。
    ///
    /// 参数:
    /// - `shio_item`: Shio MEV机会事件。
    ///
    /// 返回:
    /// - `Option<(HashSet<(String, Option<ObjectID>)>, Vec<ObjectReadResult>)>`:
    ///   如果成功解析出机会，则返回Some元组，包含 (代币池集合, 需覆盖的对象列表)。否则返回None。
    async fn get_potential_opportunity(
        &self,
        shio_item: &ShioItem,
    ) -> Option<(HashSet<(String, Option<ObjectID>)>, Vec<ObjectReadResult>)> {
        // 从ShioItem中获取相关的Sui事件列表
        let events = shio_item.events();
        if events.is_empty() { // 如果没有事件，则无法判断机会
            return None;
        }

        // 与 `parse_involved_coin_pools` 类似，并发解析事件以获取代币池信息。
        let mut join_set = JoinSet::new();
        for event in events { // 注意：这里 `events` 是 `&Vec<SuiEvent>`，所以 `event` 是 `&SuiEvent`
            let own_simulator_clone = Arc::clone(&self.own_simulator);
            // 需要克隆event或者将其生命周期传递给异步块，这里直接用 `event.clone()`
            let event_clone = event.clone();
            join_set.spawn(async move {
                if let Ok(protocol_type) = Protocol::try_from(&event_clone) {
                    // `shio_event_to_swap_event` 用于处理ShioItem中特定格式的事件
                    if let Ok(swap_event_data) = protocol_type.shio_event_to_swap_event(&event_clone, own_simulator_clone).await {
                        return Some((swap_event_data.involved_coin_one_side(), swap_event_data.pool_id()));
                    }
                }
                None
            });
        }

        let mut involved_coin_pools_set = HashSet::new();
        while let Some(task_result) = join_set.join_next().await {
            if let Ok(Some((coin, pool_id))) = task_result {
                involved_coin_pools_set.insert((coin, pool_id));
            }
        }

        if involved_coin_pools_set.is_empty() { // 如果没有解析到相关的代币池，则返回None
            return None;
        }

        // 从ShioItem中获取机会交易的摘要
        let opportunity_tx_digest = TransactionDigest::from_str(shio_item.tx_digest()).ok()?; // .ok()? 将Result转为Option

        // 从ShioItem中获取由机会交易创建或修改的对象列表 (`created_mutated_objects`)。
        // 这些对象及其状态需要在模拟时被“覆盖”，以确保模拟环境与机会发生时的链状态一致。
        // 使用Rayon进行并行处理，提高效率。
        let override_objects_for_sim: Vec<ObjectReadResult> = shio_item
            .created_mutated_objects() // 返回 `&Vec<ShioObject>`
            .par_iter() // 并行迭代器
            .filter_map(|shio_obj_ref| new_object_read_result(opportunity_tx_digest, shio_obj_ref).ok()) // 转换为 ObjectReadResult
            .collect();

        Some((involved_coin_pools_set, override_objects_for_sim))
    }

    /// `get_latest_epoch` 方法 (可变self，用于更新缓存的epoch)
    ///
    /// 获取最新的Sui纪元信息 (`SimEpoch`)。
    /// 它首先检查缓存的 `self.epoch` 是否仍然有效 (通过 `is_stale()`，可能比较时间戳)。
    /// 如果缓存有效，则直接返回缓存值。否则，从Sui RPC节点获取最新的纪元信息，
    /// 更新缓存，然后返回。
    async fn get_latest_epoch(&mut self) -> Result<SimEpoch> {
        if let Some(cached_epoch) = self.epoch {
            if !cached_epoch.is_stale() { // 假设 SimEpoch 有 is_stale() 方法
                return Ok(cached_epoch);
            } else {
                // 缓存的epoch已过期，清除它以便重新获取
                self.epoch = None;
            }
        }

        // 从Sui客户端获取最新的纪元信息
        let latest_epoch = get_latest_epoch(&self.sui).await?;
        self.epoch = Some(latest_epoch); // 更新缓存
        Ok(latest_epoch)
    }
}

/// `new_object_read_result` (私有辅助函数)
///
/// 将从 `ShioObject` (Shio协议定义的对象表示) 转换为Sui标准的 `ObjectReadResult`。
/// `ObjectReadResult` 是模拟器在设置覆盖对象时所需的格式。
///
/// 参数:
/// - `tx_digest`: 产生这些对象的交易的摘要 (用于设置 `Object::previous_transaction`)。
/// - `shio_obj`: 从 `ShioItem` 中获取的 `&ShioObject`。
///
/// 返回:
/// - `Result<ObjectReadResult>`: 转换后的 `ObjectReadResult`。
fn new_object_read_result(tx_digest: TransactionDigest, shio_obj: &ShioObject) -> Result<ObjectReadResult> {
    // ShioObject 应该代表一个Move对象
    ensure!(
        shio_obj.data_type() == "moveObject",
        "无效的数据类型: {}，期望为 moveObject",
        shio_obj.data_type()
    );

    let object_id = ObjectID::from_hex_literal(&shio_obj.id)?; // 解析ObjectID

    // 从ShioObject的字段构建 `MoveObject`
    let move_object_instance = {
        // 解析Move对象的具体类型 (例如 "0x2::coin::Coin<0x2::sui::SUI>")
        let object_concrete_type: MoveObjectType = serde_json::from_str(&shio_obj.object_type)?;
        let has_public_transfer = shio_obj.has_public_transfer(); // 对象是否有公共转移能力
        let version = OBJECT_START_VERSION; // 对于新创建或覆盖的对象，版本通常从 OBJECT_START_VERSION 开始
                                            // 或者需要从 ShioObject 获取更准确的版本信息（如果提供）
        // 将Base64编码的BCS对象内容解码为字节
        let object_bcs_contents = Base64::decode(&shio_obj.object_bcs)?;
        // 获取当前链的协议配置 (这里硬编码为Mainnet，可能需要根据实际环境调整)
        let protocol_config = ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Mainnet);
        // `MoveObject::new_from_execution` 是一个unsafe函数，用于从执行结果（如模拟或RPC）
        // 创建MoveObject。它假设输入数据是有效的。
        unsafe {
            MoveObject::new_from_execution(
                object_concrete_type,
                has_public_transfer,
                version, // **注意**: 版本可能需要从ShioObject获取，如果它代表的是已存在对象的修改而非全新对象。
                         // 如果是覆盖，版本应与被覆盖对象的版本匹配或更高。
                         // OBJECT_START_VERSION (1) 通常用于全新创建的对象。
                object_bcs_contents,
                &protocol_config,
            )?
        }
    };

    // 解析对象的所有者信息 (Owner)
    let owner_info = serde_json::from_value::<Owner>(shio_obj.owner.clone())?; // shio_obj.owner 是 serde_json::Value
    let previous_transaction_digest = tx_digest; // 将对象的 `previous_transaction` 设置为产生它的交易的摘要

    // 创建 `Object` 实例
    let sui_object_instance = Object::new_move(move_object_instance, owner_info.clone(), previous_transaction_digest);

    // 根据所有者类型确定 `InputObjectKind`
    // 这对于后续在PTB中使用此对象作为输入是必要的。
    let input_object_kind = match owner_info {
        Owner::Shared { initial_shared_version } => InputObjectKind::SharedMoveObject {
            id: object_id,
            initial_shared_version,
            mutable: true, // 假设在模拟中这些共享对象可能被修改
        },
        _ => InputObjectKind::ImmOrOwnedMoveObject(sui_object_instance.compute_object_reference()), // 对于私有对象或不可变对象
    };

    // 将 `InputObjectKind` 和对象数据 (`sui_object_instance.into()`) 包装成 `ObjectReadResult`。
    Ok(ObjectReadResult::new(input_object_kind, sui_object_instance.into()))
}


/// `run_in_tokio!` 宏
///
/// 一个工具宏，用于确保给定的异步代码块 (`$code`) 在一个Tokio运行时上下文中执行。
/// 这在从同步代码（如 `std::thread::spawn` 的闭包）中调用异步函数时非常有用。
///
/// 逻辑:
/// 1. 尝试获取当前线程的Tokio运行时句柄 (`Handle::try_current()`)。
/// 2. 如果成功获取句柄:
///    a. 检查运行时类型 (`runtime_flavor()`):
///       - 如果是 `CurrentThread` 类型: 在一个新的标准库线程作用域 (`std::thread::scope`) 内，
///         创建一个新的 `CurrentThread` Tokio运行时，并在其中 `block_on` 执行异步代码。
///         这是为了避免在已有的 `CurrentThread` 运行时内再次 `block_on` 可能导致的问题。
///       - 如果是其他类型 (如 `MultiThread`): 使用 `tokio::task::block_in_place` 结合句柄的 `block_on`
///         来执行异步代码。`block_in_place` 允许在异步任务中执行阻塞代码而不会阻塞整个线程池。
/// 3. 如果获取句柄失败 (表示当前不在Tokio运行时上下文中):
///    创建一个新的 `CurrentThread` Tokio运行时，并在其中 `block_on` 执行异步代码。
#[macro_export]
macro_rules! run_in_tokio {
    ($code:expr) => { // `$code` 是要执行的异步表达式 (例如 `async { ... }` 或一个返回Future的函数调用)
        match Handle::try_current() { // 尝试获取当前Tokio运行时句柄
            Ok(handle) => match handle.runtime_flavor() { // 获取成功，检查运行时类型
                RuntimeFlavor::CurrentThread => std::thread::scope(move |s| { // 如果是单线程运行时
                    s.spawn(move || { // 创建一个新线程来搭建新的单线程运行时
                        Builder::new_current_thread() // Tokio单线程运行时构建器
                            .enable_all() // 启用所有Tokio特性 (如IO, time)
                            .build()
                            .unwrap()
                            .block_on(async move { $code.await }) // 在新运行时中阻塞执行异步代码
                    })
                    .join() // 等待新线程完成
                    .unwrap() // 处理线程join的Result
                }),
                _ => tokio::task::block_in_place(move || { // 如果是多线程运行时
                    handle.block_on(async move { $code.await }) // 使用block_in_place在当前线程阻塞执行
                }),
            },
            Err(_) => { // 如果当前不在Tokio运行时中
                Builder::new_current_thread() // 创建一个新的单线程运行时
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async move { $code.await }) // 在新运行时中阻塞执行异步代码
            }
        }
    };
}


/// 为 `ArbStrategy` 实现 `burberry::Strategy<Event, Action>` trait。
/// `burberry::Strategy` 是 `burberry` 引擎框架的核心trait之一，定义了策略的行为。
/// - `Event`: 策略处理的输入事件类型。
/// - `Action`: 策略产生的输出动作类型。
#[burberry::async_trait] // 因为 `sync_state` 和 `process_event` 是异步的
impl burberry::Strategy<Event, Action> for ArbStrategy {
    /// `name` 方法 (来自 `Strategy` trait)
    /// 返回策略的名称。
    fn name(&self) -> &str {
        "ArbStrategy"
    }

    /// `sync_state` 方法 (来自 `Strategy` trait)
    ///
    /// 在策略首次启动或引擎要求同步状态时调用。
    /// 主要用于初始化策略的内部状态，特别是与并发处理相关的部分，如创建工作线程和通道。
    ///
    /// 参数:
    /// - `submitter`: 一个共享的动作提交器 (`Arc<dyn ActionSubmitter<Action>>`)，
    ///   工作线程将通过它提交产生的套利动作 (如执行交易、发送通知)。
    ///
    /// 返回:
    /// - `Result<()>`: 如果初始化成功则返回Ok。
    async fn sync_state(&mut self, submitter: Arc<dyn ActionSubmitter<Action>>) -> Result<()> {
        // 防止重复初始化
        if self.arb_item_sender.is_some() {
            panic!("ArbStrategy状态已同步过 (arb_item_sender 已存在)!");
        }

        // 创建一个无界异步通道 (`async_channel`)，用于将 `ArbItem` 从策略主逻辑分发给工作线程。
        // `arb_item_sender` 是发送端，`arb_item_receiver` 是接收端。
        let (sender_channel_end, receiver_channel_end) = async_channel::unbounded();
        self.arb_item_sender = Some(sender_channel_end); // 保存发送端到策略实例中

        let attacker_address = self.sender; // 机器人的Sui地址
        let rpc_url_clone = self.rpc_url.clone(); // 克隆RPC URL以传递给线程

        let num_workers_to_spawn = self.workers; // 要创建的工作线程数量
        info!("准备启动 {} 个工作线程来处理套利机会...", num_workers_to_spawn);

        // 创建一个临时的MPSC (多生产者单消费者) 通道，用于等待所有工作线程完成其初始化。
        // 主线程将是消费者，每个工作线程在初始化完毕后会发送一个信号。
        let (init_signal_sender, mut init_signal_receiver) = tokio::sync::mpsc::channel(num_workers_to_spawn);

        // 启动指定数量的工作线程
        for worker_id in 0..num_workers_to_spawn {
            debug!(worker.id = worker_id, "正在启动工作线程...");

            // 为每个工作线程克隆所需的共享资源:
            let worker_arb_item_receiver = receiver_channel_end.clone(); // 克隆通道接收端
            let worker_action_submitter = Arc::clone(&submitter);      // 克隆动作提交器的Arc指针
            let worker_sui_client = SuiClientBuilder::default().build(&rpc_url_clone).await?; // 每个worker拥有独立的SuiClient
            let worker_rpc_url = rpc_url_clone.clone();
            let worker_init_signal_sender = init_signal_sender.clone(); // 克隆初始化信号发送端
            let worker_simulator_pool_for_arb = Arc::clone(&self.simulator_pool); // 用于Arb实例的模拟器池
            let worker_simulator_pool_for_worker = Arc::clone(&self.simulator_pool); // 用于Worker自身的模拟器池
            // 获取一个模拟器名称用于日志 (假设池中所有模拟器名称相同)
            let simulator_name_for_worker = worker_simulator_pool_for_arb.get().name().to_string();
            let worker_dedicated_simulator = self.dedicated_simulator.clone(); // 克隆专用模拟器

            // 创建并启动一个新的标准库线程来运行每个worker。
            // 使用 `std::thread` 而不是 `tokio::spawn` 可能是为了给每个worker一个完全独立的OS线程，
            // 或者因为worker内部的某些逻辑可能不完全是异步的，或者为了更好地控制线程属性。
            let _ = std::thread::Builder::new()
                .stack_size(128 * 1024 * 1024) // 设置较大的栈空间 (128 MB)，以防复杂计算或深递归
                .name(format!("worker-{}", worker_id)) // 为线程命名，方便调试
                .spawn(move || { // `move`关键字将所有权移入闭包
                    // 在新线程内，我们可能需要一个新的Tokio运行时来执行worker的异步逻辑。
                    // `run_in_tokio!`宏处理了这个问题。

                    // 初始化 `Arb` 实例 (套利计算核心)
                    // `Arb::new` 是异步的，所以用 `run_in_tokio!` 执行。
                    let arb_instance = Arc::new(
                        run_in_tokio!({ Arb::new(&worker_rpc_url, worker_simulator_pool_for_arb) })
                            .unwrap(), // unwrap处理Arb::new的Result
                    );

                    // 发送初始化完成信号给主线程
                    run_in_tokio!(worker_init_signal_sender.send(())).unwrap(); // unwrap处理send的Result

                    // 创建 `Worker` 实例
                    let worker_instance = Worker {
                        _id: worker_id, // 未使用的ID，用_前缀
                        sender: attacker_address,
                        arb_item_receiver: worker_arb_item_receiver,
                        simulator_pool: worker_simulator_pool_for_worker,
                        simulator_name: simulator_name_for_worker,
                        submitter: worker_action_submitter,
                        sui: worker_sui_client,
                        arb: arb_instance,
                        dedicated_simulator: worker_dedicated_simulator,
                    };
                    // 运行worker的主循环。如果worker panic，则使整个程序panic。
                    worker_instance.run().unwrap_or_else(|e| panic!("工作线程 {} panic: {:?}", worker_id, e));
                });
        }
        // 删除 `init_signal_sender` 的原始副本，确保当所有克隆副本被drop后，接收端能感知到通道关闭。
        drop(init_signal_sender);

        // 等待所有工作线程发送初始化完成信号。
        for i in 0..num_workers_to_spawn {
            // `recv().await` 会阻塞，直到从通道接收到一个消息或通道关闭。
            // `expect` 用于处理 `Option`，如果接收到 `None` (表示通道已关闭且没有更多消息)，则panic。
            init_signal_receiver.recv().await.expect(&format!("工作线程 {} 初始化失败或通道意外关闭", i) );
        }

        info!("所有工作线程已成功启动!");
        Ok(())
    }

    /// `process_event` 方法 (来自 `Strategy` trait)
    ///
    /// 处理从引擎接收到的单个事件。
    ///
    /// 参数:
    /// - `event`: 要处理的 `Event` 枚举实例。
    /// - `_submitter`: 动作提交器 (在此方法中未使用，用 `_` 前缀表示，因为动作提交通常由worker完成)。
    async fn process_event(&mut self, event: Event, _submitter: Arc<dyn ActionSubmitter<Action>>) {
        // 根据事件类型，调用相应的处理函数。
        let processing_result = match event {
            Event::PublicTx(tx_effects, events) => self.on_new_tx_effects(tx_effects, events).await,
            Event::PrivateTx(tx_data) => self.on_new_tx(tx_data).await,
            Event::Shio(shio_item) => self.on_new_shio_item(shio_item).await,
        };
        // 如果事件处理过程中发生错误，则记录错误并返回。
        if let Err(error) = processing_result {
            error!(?error, "处理事件失败");
            return;
        }

        // --- 从 ArbCache 中提取套利机会并发送给工作线程 ---
        // 确保 `arb_item_sender` 已经被初始化 (在 `sync_state` 中完成)。
        let sender_channel = self.arb_item_sender.as_ref().unwrap();
        let current_channel_len = sender_channel.len(); // 获取通道中当前排队的任务数量

        // 如果通道中的任务数量小于一个阈值 (例如10)，则尝试从缓存中提取更多任务填充通道。
        // 这是为了保持工作线程有足够的任务处理，同时避免通道过度拥塞。
        if current_channel_len < 10 {
            let num_items_to_send = 10 - current_channel_len; // 计算需要发送的任务数量
            for _ in 0..num_items_to_send {
                if let Some(arb_item_to_process) = self.arb_cache.pop_one() { // 从缓存中弹出一个有效的机会
                    // 检查此机会的代币是否在 `recent_arbs` 队列中 (短期内已处理过)。
                    // Shio来源的机会不受此限制，总是尝试处理。
                    if !self.recent_arbs.contains(&arb_item_to_process.coin) || arb_item_to_process.source.is_shio() {
                        let coin_name = arb_item_to_process.coin.clone(); // 克隆代币名称以备后用
                        // 异步发送 `ArbItem` 到工作线程通道。
                        // `.unwrap()` 处理发送失败的情况 (如果通道关闭则可能panic)。
                        sender_channel.send(arb_item_to_process).await.unwrap();

                        // 将此代币加入 `recent_arbs` 队列。
                        self.recent_arbs.push_back(coin_name);
                        // 如果队列超出最大容量，则从队首移除最旧的条目。
                        if self.recent_arbs.len() > self.max_recent_arbs {
                            self.recent_arbs.pop_front();
                        }
                    }
                } else {
                    // 如果 `arb_cache.pop_one()` 返回 `None`，表示缓存中没有更多可处理的机会。
                    break; // 跳出填充循环
                }
            }
        } else {
            // 如果通道已满或接近满，则记录警告。
            warn!("套利机会通道积压，当前长度: {}", current_channel_len);
        }

        // --- 清理 ArbCache 和 recent_arbs 中过期的条目 ---
        // 从 `arb_cache` 中移除所有已过期的条目。
        let expired_coin_names_from_cache = self.arb_cache.remove_expired();
        // 对于每个从缓存中移除的过期代币，如果它也存在于 `recent_arbs` 队列中，则将其从队列中移除。
        // 这是为了确保 `recent_arbs` 不会阻止处理一个在队列中但其缓存条目已因过期而被移除的代币的新机会。
        for expired_coin_name in expired_coin_names_from_cache {
            if let Some(position_in_recent_arbs) = self.recent_arbs.iter().position(|x| x == &expired_coin_name) {
                self.recent_arbs.remove(position_in_recent_arbs); // 从队列中移除
            }
        }
    }
}
