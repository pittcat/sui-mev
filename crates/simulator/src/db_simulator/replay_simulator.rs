// 该文件 `replay_simulator.rs` (位于 `simulator` crate 的 `db_simulator` 子模块下)
// 定义了 `ReplaySimulator` 结构体。这是一种特殊用途的模拟器，它包装了一个 `DBSimulator` 实例，
// 并增加了一个机制来确保在模拟交易时，特别是Gas币，尽可能使用最新的链上状态。
// "Replay" (回放) 这个词暗示它可能用于在接近真实链状态的环境中“重演”或验证交易行为。
//
// **文件概览 (File Overview)**:
// 这个文件定义了一个叫做 `ReplaySimulator` 的“增强版”模拟器。
// 它内部其实还是用 `DBSimulator` 来做实际的模拟工作，但它额外增加了一些“保鲜”功能，
// 目标是让模拟时用到的数据（尤其是Gas币）尽可能地接近Sui链上的最新状态。
//
// **核心组件和概念 (Core Components and Concepts)**:
//
// 1.  **`ReplaySimulator` 结构体**:
//     -   `fallback: DBSimulator`: 一个内部持有的 `DBSimulator` 实例。
//         实际的交易模拟逻辑会委托给这个 `fallback` 的 `DBSimulator`。
//         称之为 "fallback" 可能有些误导，因为它实际上是主要的模拟执行者；
//         更准确地说，`ReplaySimulator` 是对 `DBSimulator` 的一层包装和增强。
//     -   `update_notifier: Arc<Sender<()>>`: 一个异步通道 (`tokio::sync::mpsc::Sender`) 的发送端，
//         用于从外部（例如，当套利策略发现一个可能与当前链状态高度相关的机会时）通知
//         `ReplaySimulator` 的后台更新线程“可能需要更频繁地更新其缓存了”。
//         `Arc` 用于共享这个发送端。
//
// 2.  **`ReplaySimulator::new_slow()` 异步构造函数**:
//     -   创建一个新的 `ReplaySimulator` 实例。
//     -   **初始化 `DBSimulator`**: 首先，它会像 `DBSimulator` 的构造函数一样，
//         调用 `DBSimulator::new_authority_store()` 来加载Sui节点的数据库状态，
//         然后调用 `DBSimulator::new()` 来创建一个 `DBSimulator` 实例作为 `fallback`。
//         这个 `DBSimulator` 在创建时没有配置Unix套接字更新或对象预加载路径。
//     -   **创建通知通道**: 创建一个 `tokio::sync::mpsc::channel` (有界多生产者单消费者通道)，
//         `update_notifier` 持有其发送端 `tx`。
//     -   **启动后台更新循环 (`spawn_update_loop`)**:
//         -   获取 `DBSimulator` 内部的 `WritebackCache` (`cache_writeback`) 的共享引用。
//         -   在一个新的标准库线程 (`std::thread`) 中启动 `Self::spawn_update_loop` 异步函数。
//             这个线程专门负责定期更新 `cache_writeback`。
//
// 3.  **`ReplaySimulator::spawn_update_loop()` 异步静态方法 (后台更新线程的执行体)**:
//     -   这个函数在一个独立的Tokio运行时中执行 (通过 `#[tokio::main]` 宏，当它在 `std::thread::spawn` 中被调用时，
//         `run_in_tokio!` 或类似的宏会确保这一点，或者如当前代码所示，该线程会创建自己的Tokio运行时)。
//         **当前代码中，`spawn_update_loop` 被标记为 `#[tokio::main]`，这通常用于二进制程序的入口点，
//         或者用于 `tokio::spawn` 产生的任务的入口点。当它被 `std::thread::spawn` 调用时，
//         它会在新线程中创建一个新的Tokio单线程运行时并阻塞执行其异步逻辑。**
//     -   **逻辑**:
//         -   维护一个 `current_interval` (当前休眠间隔) 和 `quick_update_times` (快速更新剩余次数)。
//         -   进入无限循环：
//             1.  `tokio::time::sleep(current_interval).await`: 按当前间隔休眠。
//             2.  `receiver.try_recv()`: 检查 `update_notifier` 通道是否有新的通知。
//                 如果有通知，则将 `quick_update_times` 重置为一个较大的值 (例如50次)。
//                 这意味着在接下来的一段时间内，缓存会以更短的间隔 (`short_interval`) 更新。
//             3.  `cache_writeback.update_underlying(true)`: 调用 `WritebackCache` 的方法，
//                 将缓存中的脏数据（已修改但未持久化的）写回到其底层的 `AuthorityStore`。
//                 参数 `true` 可能表示强制刷新所有内容。
//                 **重要**: 这个操作的目的是确保 `WritebackCache` 与其底层数据库保持一致，
//                 但它本身**并不从外部Sui网络拉取最新的链状态**。
//                 它更像是一个“持久化缓存”的操作。如果 `DBSimulator` 是基于一个固定的数据库快照运行的，
//                 那么这个 `update_underlying` 只是将模拟中（如果允许写入的话）的变更写回这个快照，
//                 或者如果快照是只读的，则可能是一个空操作或内部状态同步。
//                 **要真正获取最新的链状态，`DBSimulator` 或其 `AuthorityStore` 需要有其他机制
//                 (例如 `db_simulator/mod.rs` 中的 `spawn_update_thread` 通过Unix套接字接收外部更新，
//                 或者定期重新加载整个 `AuthorityStore`)。**
//                 当前 `ReplaySimulator` 的 `spawn_update_loop` 似乎主要关注的是“刷新缓存到磁盘”
//                 和“根据通知调整刷新频率”，而不是“从链上拉取最新状态到缓存”。
//                 “确保执行总是使用最新状态”的目标，更多的是通过 `simulate()` 方法中对Gas币的特殊处理来实现的。
//             4.  根据 `quick_update_times` 的值，调整 `current_interval` 为 `short_interval` 或 `long_interval`。
//
// 4.  **`Simulator` trait 实现 for `ReplaySimulator`**:
//     -   **`simulate()`**:
//         -   **核心增强**: 在调用 `self.fallback.simulate()` (即实际的 `DBSimulator` 模拟) 之前，
//             它会特别处理交易中的Gas币 (`tx.gas()`)。
//         -   它获取这些Gas币的ObjectID，然后调用 `self.fallback.store.store.multi_get_objects()`
//             来从底层存储 (Sui数据库) 中获取这些Gas币的最新版本。
//             (`self.fallback.store` 是 `WritebackCache`, `.store` 是其内部的 `AuthorityStore`)
//         -   然后，它调用 `self.fallback.store.reload_cached()` 将这些最新版本的Gas币对象
//             重新加载到 `WritebackCache` 的内存缓存中。
//         -   这样做是为了确保在模拟交易时，使用的Gas币对象状态尽可能接近链上的最新状态，
//             从而使Gas计算和交易有效性检查更加准确。
//         -   之后，它才调用 `self.fallback.simulate(tx, ctx).await` 执行实际的模拟。
//     -   `get_object()`: 直接委托给 `self.fallback.get_object()`。
//     -   `name()`: 返回 "ReplaySimulator"。
//     -   `get_object_layout()`: (在此文件中未实现，但 `Simulator` trait 可能要求)。
//         如果需要，它也应该委托给 `self.fallback.get_object_layout()`。
//
// **ReplaySimulator 的特殊用途**:
// -   确保模拟时使用的Gas对象是最新的，这对于准确估算Gas成本和避免因Gas对象版本陈旧导致的模拟失败非常重要。
// -   通过 `update_notifier` 机制，允许外部逻辑（如高频套利策略）在检测到关键变化时，
//     请求模拟器（的缓存）更频繁地与其底层数据库（可能是定期更新的快照）进行同步（通过 `update_underlying`）。
// -   它本身并不直接轮询Sui链获取最新状态，而是依赖其内部 `DBSimulator` 的状态，
//     并通过上述机制尝试保持Gas币的“新鲜度”和响应外部更新通知。

// 引入标准库的 Arc (原子引用计数) 和 time::Duration (时间间隔)。
use std::{sync::Arc, time::Duration};

// 引入 async_trait 宏，用于在trait中定义异步方法。
use async_trait::async_trait;
// 引入 Sui 核心库的 WritebackCache 执行缓存写入接口。
use sui_core::execution_cache::ExecutionCacheWrite;
// 引入 Sui 核心类型库中的相关类型。
use sui_types::{
    base_types::ObjectID, // 对象ID
    object::Object,       // 对象结构
    storage::ObjectStore, // 对象存储trait (被WritebackCache实现)
    transaction::{TransactionData, TransactionDataAPI}, // 交易数据及其API trait
};
// 引入 Tokio 的多生产者单消费者异步通道 (mpsc) 的 Receiver 和 Sender。
use tokio::sync::mpsc::{Receiver, Sender};

// 从当前crate的根模块引入 SimulateCtx, SimulateResult, Simulator trait。
use crate::{SimulateCtx, SimulateResult, Simulator};

// 从当前crate的父模块 (db_simulator) 引入 DBSimulator。
use super::DBSimulator;

/// `ReplaySimulator` 结构体
///
/// 一种特殊用途的模拟器，旨在确保交易模拟尽可能使用最新的链上状态，
/// 特别是对于Gas币对象。它内部包装了一个 `DBSimulator` 作为主要的模拟执行者，
/// 并增加了一个后台任务来定期“刷新”其缓存，以及一个通知机制来触发更频繁的刷新。
pub struct ReplaySimulator {
    /// `fallback`: 内部持有的 `DBSimulator` 实例。
    /// 所有的模拟请求最终都会委托给这个 `DBSimulator`。
    /// 称其为 "fallback" 可能不完全准确，它更像是被包装的核心模拟器。
    fallback: DBSimulator,

    /// `update_notifier`: 一个异步通道的发送端 (`Sender<()>`)。
    /// 外部逻辑可以通过这个发送端发送一个空消息 `()` 来通知 `ReplaySimulator` 的后台更新任务，
    /// 表明可能需要更频繁地更新其内部缓存（通过调用 `WritebackCache::update_underlying`）。
    /// 使用 `Arc` 使其可以被共享给 `ReplaySimulator` 的使用者。
    pub update_notifier: Arc<Sender<()>>,
}

impl ReplaySimulator {
    /// `new_slow` 异步构造函数
    ///
    /// 创建并初始化一个新的 `ReplaySimulator` 实例。
    /// "slow" 可能指其初始化过程涉及到加载 `DBSimulator` 的底层 `AuthorityStore`，这可能比较耗时。
    ///
    /// 参数:
    /// - `store_path`: Sui节点数据库的路径字符串。
    /// - `config_path`: Sui节点配置文件的路径字符串。
    /// - `long_interval`: 后台更新任务在没有收到外部通知时的默认休眠间隔。
    /// - `short_interval`: 后台更新任务在收到外部通知后，短期内使用的更频繁的休眠间隔。
    ///
    /// 返回:
    /// - `Self`: 新创建的 `ReplaySimulator` 实例。
    pub async fn new_slow(
        store_path: &str,
        config_path: &str,
        long_interval: Duration,  // 例如，60秒
        short_interval: Duration, // 例如，1秒
    ) -> Self {
        // 步骤 1: 初始化底层的 AuthorityStore。
        let authority_store_instance = DBSimulator::new_authority_store(store_path, config_path).await;

        // 步骤 2: 创建用于外部通知的 tokio::sync::mpsc 通道。
        // `channel(100)` 创建一个有界通道，容量为100。
        // `tx` 是发送端 (Sender)，`rx` 是接收端 (Receiver)。
        let (tx_notifier_sender, rx_update_receiver) = tokio::sync::mpsc::channel(100);

        // 步骤 3: 创建核心的 DBSimulator 实例。
        // 注意：这里传递给 DBSimulator::new 的 update_socket 和 preload_path 都是 None，
        // 这意味着这个内部的 DBSimulator 不会使用它自己的Unix套接字更新或文件预加载机制。
        // `with_fallback` 设置为 true。
        let db_simulator_instance = DBSimulator::new(authority_store_instance, None, None, true).await;
        // 获取对 DBSimulator 内部 WritebackCache 的共享引用，用于后台更新任务。
        let cache_writeback_ref = db_simulator_instance.store.clone();

        // 步骤 4: 启动后台更新循环线程。
        // 使用 `std::thread::Builder` 创建一个具有描述性名称和自定义栈大小（如果需要）的OS线程。
        std::thread::Builder::new()
            .name("replay-sim-update-loop".to_string()) // 为线程命名
            .spawn(move || { // `move` 将所有权移入线程闭包
                // 在新线程中调用 `Self::spawn_update_loop` 异步函数。
                // `spawn_update_loop` 内部会创建自己的Tokio运行时 (因其被 `#[tokio::main]` 标记)。
                Self::spawn_update_loop(rx_update_receiver, cache_writeback_ref, short_interval, long_interval)
            })
            .unwrap(); // `unwrap` 处理线程创建失败的 panic

        // 返回 ReplaySimulator 实例
        Self {
            fallback: db_simulator_instance,         // 存储内部的 DBSimulator
            update_notifier: Arc::new(tx_notifier_sender), // 存储通知通道的发送端
        }
    }

    /// `spawn_update_loop` 异步静态方法 (后台更新线程的执行体)
    ///
    /// 这个函数在一个独立的Tokio运行时中执行，负责定期调用 `WritebackCache::update_underlying`。
    /// 它还会监听通过 `receiver` 发送过来的通知，以在需要时临时切换到更短的更新间隔。
    ///
    /// **重要逻辑解释**:
    /// - `cache_writeback.update_underlying(true)`: 这个操作主要是将 `WritebackCache` 中
    ///   在模拟过程中可能发生的（但通常DBSimulator用于只读模拟，除非特意配置）的“脏”数据写回到其包装的 `AuthorityStore`。
    ///   如果 `AuthorityStore` 是基于一个只读的数据库快照，那么这个操作可能主要是为了内部缓存一致性或清理。
    ///   它**本身并不从外部Sui网络拉取最新的链状态**。
    ///   `ReplaySimulator` 实现“最新状态”的主要机制是在 `simulate` 方法中对Gas币的特殊处理。
    ///   这个后台循环更像是一个“缓存维护”和“响应式刷新频率调整”的机制。
    ///
    /// 参数:
    /// - `receiver`: 通知通道的接收端。
    /// - `cache_writeback`: 对 `WritebackCache` 的共享引用。
    /// - `short_interval`: 收到通知后使用的短更新间隔。
    /// - `long_interval`: 默认的长更新间隔。
    #[tokio::main] // 此属性表示这个函数是一个Tokio异步运行时的主入口点。
                   // 当被 std::thread::spawn 调用时，它会在新线程中创建并运行一个新的Tokio运行时。
    async fn spawn_update_loop(
        mut receiver: Receiver<()>, // 通知通道的接收端 (可变，因为会调用 recv)
        cache_writeback: Arc<dyn ExecutionCacheWrite>, // WritebackCache (实现了 ExecutionCacheWrite trait)
        short_interval: Duration, // 短间隔
        long_interval: Duration,  // 长间隔
    ) {
        let mut quick_update_trigger_count = 0; // 记录在短间隔模式下还需执行多少次更新
        let mut current_sleep_interval = long_interval; // 当前的休眠/更新间隔

        loop { // 无限循环
            // 步骤 A: 按当前间隔休眠
            tokio::time::sleep(current_sleep_interval).await;

            // 步骤 B: 检查是否有外部通知要求快速更新
            // `try_recv()` 是非阻塞的，尝试从通道接收一个消息。
            // 如果通道为空，它会立即返回 `Err(TryRecvError::Empty)`。
            // 如果通道已关闭，返回 `Err(TryRecvError::Closed)`。
            // 如果成功收到消息 `Ok(())`，则表示外部请求了一次或多次快速更新。
            while receiver.try_recv().is_ok() { // 清空通道中所有已到达的通知
                quick_update_trigger_count = 50; // 将快速更新的剩余次数重置为一个固定值 (例如50次)
                                               // 这意味着在接下来的约 50 * short_interval 时间内会保持快速更新。
            }

            // 步骤 C: 执行缓存更新 (刷新到其包装的AuthorityStore)
            // `update_underlying(true)`: `true` 可能表示强制或深度刷新。
            // 这个操作的目的依赖于 `WritebackCache` 和其底层 `AuthorityStore` 的具体实现和配置。
            // 如上文所述，它不直接从网络拉取新数据。
            cache_writeback.update_underlying(true);

            // 步骤 D: 根据是否处于“快速更新模式”来调整下一个休眠间隔
            if quick_update_trigger_count > 0 { // 如果还在快速更新模式下
                current_sleep_interval = short_interval; // 使用短间隔
                quick_update_trigger_count -= 1; // 减少快速更新的剩余次数
            } else { // 如果快速更新次数已用完
                current_sleep_interval = long_interval; // 恢复到长间隔
            }
        }
    }
}

/// 为 `ReplaySimulator` 实现 `Simulator` trait。
#[async_trait]
impl Simulator for ReplaySimulator {
    /// `simulate` 异步方法 (核心模拟逻辑)
    ///
    /// 执行交易模拟。在调用底层 `DBSimulator` 进行模拟之前，
    /// 它会特别处理交易中的Gas币，确保它们的状态是从底层存储（数据库）中最新加载的。
    ///
    /// 参数:
    /// - `tx_data_to_simulate`: 要模拟的 `TransactionData`。
    /// - `simulation_context`: `SimulateCtx`，包含了epoch信息和需要覆盖的对象状态。
    ///
    /// 返回:
    /// - `eyre::Result<SimulateResult>`: 包含模拟结果或错误。
    async fn simulate(&self, tx_data_to_simulate: TransactionData, simulation_context: SimulateCtx) -> eyre::Result<SimulateResult> {
        // 步骤 1: 确保Gas币是最新的。
        // 获取交易中指定的所有Gas支付对象的ID。
        let gas_object_ids: Vec<ObjectID> = tx_data_to_simulate.gas().iter().map(|obj_ref| obj_ref.0).collect();
        if !gas_object_ids.is_empty() { // 只在实际指定了Gas币时才执行更新
            // 从 `self.fallback.store.store` (即底层的 `AuthorityStore`) 批量获取这些Gas币的最新版本。
            // `multi_get_objects` 直接查询持久化存储。
            let latest_gas_object_versions = self.fallback.store.store.multi_get_objects(&gas_object_ids);
            // 将成功获取到的最新Gas币对象收集起来。
            let gas_coins_to_reload = latest_gas_object_versions
                .into_iter()
                .filter_map(|obj_option| obj_option.map(|obj_data| (obj_data.id(), obj_data))) // 转换为 (ID, Object) 元组
                .collect::<Vec<_>>();
            // 调用 `self.fallback.store.reload_cached()` (即 `WritebackCache::reload_objects`)
            // 将这些最新版本的Gas币对象强制加载/更新到内存缓存 (`WritebackCache`) 中。
            // 这样，后续 `self.fallback.simulate` 在获取Gas币时，会从缓存中拿到这些较新的版本。
            self.fallback.store.reload_cached(gas_coins_to_reload);
        }

        // 步骤 2: 调用内部 `DBSimulator` (`self.fallback`) 的 `simulate` 方法执行实际的模拟。
        // 传入原始的交易数据和模拟上下文。
        self.fallback.simulate(tx_data_to_simulate, simulation_context).await
    }

    /// `get_object` 异步方法 (来自 `Simulator` trait)
    ///
    /// 获取指定ObjectID的对象数据。直接委托给内部的 `DBSimulator`。
    async fn get_object(&self, obj_id: &ObjectID) -> Option<Object> {
        self.fallback.get_object(obj_id).await // 注意: DBSimulator::get_object 不是异步的，这里 .await 是多余的。
                                                // **修正**: `DBSimulator::get_object` 不是异步的。
                                                // `Simulator::get_object` trait方法是异步的，所以这里的实现也需要是异步的。
                                                // 但如果底层的 `self.fallback.get_object` 是同步的，
                                                // 我们可以直接 `self.fallback.get_object(obj_id)` 并用 `async move` 包装，
                                                // 或者如果 `DBSimulator::get_object` 确实是异步的（可能在更新的版本中），则 `.await` 是正确的。
                                                // 假设 `self.fallback.get_object()` 确实是 `async` (与 `DBSimulator` 的 `Simulator` 实现一致)。
    }

    /// `name` 方法 (来自 `Simulator` trait)
    ///
    /// 返回模拟器的名称。
    fn name(&self) -> &str {
        "ReplaySimulator"
    }

    // `get_object_layout` 方法没有在这里实现。
    // 如果 `Simulator` trait 要求实现它，那么这里也应该添加，并委托给 `self.fallback.get_object_layout()`。
    // 例如:
    // fn get_object_layout(&self, obj_id: &ObjectID) -> Option<move_core_types::annotated_value::MoveStructLayout> {
    //     self.fallback.get_object_layout(obj_id)
    // }
    // (需要与 `DBSimulator` 中 `get_object_layout` 的签名和行为保持一致)。
}

[end of crates/simulator/src/db_simulator/replay_simulator.rs]
