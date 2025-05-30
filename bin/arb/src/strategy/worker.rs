// 该文件 `worker.rs` 定义了 `Worker` 结构体及其关联逻辑。
// `Worker` 在套利机器人中扮演工作线程的角色，负责接收由主策略 (`ArbStrategy`) 分发的
// 潜在套利机会 (`ArbItem`)，并对这些机会进行详细的分析、模拟和最终的交易构建与提交。
// 每个 `Worker` 通常在独立的线程中运行，以实现并行处理。
//
// 文件概览:
// - `Worker` 结构体:
//   - `_id`: worker的标识ID (当前未使用)。
//   - `sender`: 机器人操作者的Sui地址。
//   - `arb_item_receiver`: 异步通道的接收端，用于从 `ArbStrategy` 接收 `ArbItem`。
//   - `simulator_pool`: 共享的模拟器对象池，用于在机会分析时执行交易模拟。
//   - `simulator_name`: 使用的模拟器的名称 (用于日志)。
//   - `dedicated_simulator`: (可选) 一个专用的 `ReplaySimulator` 实例，可能用于最终的dry-run或特定场景。
//   - `submitter`: 一个共享的动作提交器 (`ActionSubmitter`)，用于将最终的套利动作 (如执行交易、发送通知) 提交给引擎的执行器。
//   - `sui`: Sui SDK客户端实例，用于与链交互 (如获取最新的Gas币)。
//   - `arb`: 一个共享的 `Arb` 实例，包含了核心的套利机会发现逻辑 (`find_opportunity`)。
// - `Worker::run()`: worker的主循环。它不断地从 `arb_item_receiver` 通道接收任务，并调用 `handle_arb_item` 处理。
// - `Worker::handle_arb_item()`: 处理单个 `ArbItem` 的核心逻辑：
//   1. 调用 `arbitrage_one_coin` 函数，使用 `Arb` 实例来分析该 `ArbItem` 是否真的存在有利可图的套利机会。
//   2. 如果找到机会 (`ArbResult`)，则调用 `dry_run_tx_data` 对最终构建的交易数据进行一次“预演”模拟。
//      这一步是为了确保在最新的链状态下交易仍然有利可图，并且可以更新交易中对象引用的版本。
//   3. 根据机会的来源 (`Source`)，决定是提交一个公开交易 (`Action::ExecutePublicTx`) 还是一个Shio MEV竞价 (`Action::ShioSubmitBid`)。
//   4. 通过 `submitter` 提交相应的动作。
//   5. 构建并通过 `submitter` 提交Telegram通知消息。
//   6. 如果配置了专用模拟器，则通知它（可能表示与其相关的机会被发现，需要它更频繁地更新状态）。
// - `Worker::dry_run_tx_data()`: 在提交实际交易前，使用最新的对象版本（通过 `fix_object_refs` 更新）
//   对交易数据进行最后一次模拟，以确认其有效性和盈利能力。
// - `Worker::fix_object_refs()`: 更新交易数据中的Gas支付对象 (`GasData.payment`) 为最新的对象引用。
//   这对于防止因Gas币版本陈旧导致的交易失败很重要。
// - `arbitrage_one_coin()` (独立异步函数): 封装了调用 `Arb::find_opportunity` 的逻辑，
//   并处理其结果（记录成功或失败的日志）。
//
// 工作流程:
// 1. `ArbStrategy` 将一个 `ArbItem` 发送到异步通道。
// 2. `Worker::run()` 中的某个worker实例从通道接收到该 `ArbItem`。
// 3. `Worker::handle_arb_item()` 被调用。
// 4. `arbitrage_one_coin()` 执行核心套利机会查找。
//    - `Arb::find_opportunity()` 被调用，它会：
//      - 查找买入和卖出路径。
//      - 进行网格搜索和黄金分割搜索以优化输入金额。
//      - 模拟最佳路径以计算利润。
//      - 返回 `ArbResult` (包含最佳路径、利润、构建好的 `TransactionData` 等)。
// 5. 如果 `arbitrage_one_coin` 返回了有利可图的 `ArbResult`:
//    a. `dry_run_tx_data()` 对 `ArbResult.tx_data` 进行最终模拟。
//       - `fix_object_refs()` 更新Gas币版本。
//       - 使用专用模拟器或池中模拟器执行模拟。
//       - 检查模拟结果是否成功且盈利。
//    b. 根据 `ArbResult.source` 确定是提交普通交易还是Shio竞价。
//    c. 通过 `submitter` 提交动作。
//    d. 发送Telegram通知。

// 引入标准库及第三方库
use std::{
    sync::Arc,                       // 原子引用计数
    time::{Duration, Instant},      // 时间处理
};

use burberry::ActionSubmitter;    // `burberry`引擎框架中的动作提交器trait
use eyre::{bail, ensure, Context, OptionExt, Result}; // 错误处理库
use object_pool::ObjectPool;      // 对象池 (用于模拟器)
use simulator::{ReplaySimulator, SimulateCtx, Simulator}; // 各种模拟器和模拟上下文
use sui_json_rpc_types::SuiTransactionBlockEffectsAPI; // 用于访问交易效果API的trait
use sui_sdk::SuiClient;           // Sui SDK客户端
use sui_types::{
    base_types::{ObjectID, SuiAddress}, // Sui基本类型
    object::Owner,                      // 对象所有者类型
    transaction::{GasData, TransactionData, TransactionDataAPI}, // Sui交易数据和相关API
};
use tracing::{error, info, instrument}; // 日志库
use utils::coin; // 外部 `utils` crate的代币工具 (例如获取最新Gas币)

// 从当前crate的其他模块引入
use crate::{
    arb::{Arb, ArbResult}, // 套利计算核心逻辑和结果类型
    common::notification::new_tg_messages, // 构建Telegram通知消息的函数
    types::{Action, Source}, // 自定义的Action和Source枚举
};

use super::arb_cache::ArbItem; // 从父模块(strategy)的 `arb_cache` 子模块引入 `ArbItem`

/// `Worker` 结构体
///
/// 负责处理从主策略分发的单个套利机会。
pub struct Worker {
    pub _id: usize, // Worker的ID (当前未使用，用_前缀表示)
    pub sender: SuiAddress, // 机器人操作者的Sui地址，用于构建交易

    pub arb_item_receiver: async_channel::Receiver<ArbItem>, // 异步通道的接收端，用于接收ArbItem

    pub simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>, // 共享的模拟器对象池
    pub simulator_name: String, // 使用的模拟器名称 (用于日志)

    pub dedicated_simulator: Option<Arc<ReplaySimulator>>, // (可选) 专用的回放模拟器

    pub submitter: Arc<dyn ActionSubmitter<Action>>, // 共享的动作提交器，用于提交最终的Action
    pub sui: SuiClient,                             // Sui SDK客户端实例 (用于获取最新Gas币等)
    pub arb: Arc<Arb>,                              // 共享的Arb实例 (套利计算核心)
}

impl Worker {
    /// `run` 方法 (Worker的主循环)
    ///
    /// 此方法在一个独立的Tokio运行时中执行 (通过 `#[tokio::main]` 宏)。
    /// 它不断地从 `arb_item_receiver` 通道等待并接收 `ArbItem`，
    /// 然后调用 `handle_arb_item` 来处理每个接收到的项目。
    ///
    /// 返回:
    /// - `Result<()>`: 如果通道关闭或发生不可恢复的错误，则返回Err。正常情况下此循环会一直运行。
    #[tokio::main] // 使用独立的tokio运行时执行此异步函数 (通常在std::thread中调用)
    pub async fn run(mut self) -> Result<()> {
        loop { // 无限循环以持续处理任务
            tokio::select! { // 同时等待多个异步操作
                // 等待从通道接收 ArbItem
                arb_item_result = self.arb_item_receiver.recv() => { // recv() 是异步的
                    // `context()` 来自 `eyre`，用于在错误路径上添加上下文信息。
                    let received_arb_item = arb_item_result.context("接收ArbItem时通道发生错误或已关闭")?;
                    // 调用 handle_arb_item 处理接收到的套利机会
                    if let Err(error) = self.handle_arb_item(received_arb_item).await {
                        // 如果处理失败，记录错误。循环会继续，尝试处理下一个机会。
                        error!(?error, "处理ArbItem失败");
                    }
                }
                // `else` 分支在 `tokio::select!` 中，如果没有其他分支立即就绪，则执行。
                // 在这里，如果 `recv()` 之外没有其他分支，`else` 分支通常在通道关闭后被触发。
                // 或者，如果select!的语义是至少需要一个分支，那么在只有一个分支时，
                // else分支可能在通道为空但未关闭时短暂轮询，但这取决于select!的具体行为。
                // 此处 `else` 意味着如果 `recv()` 返回 `Err` (通道关闭) 之外的情况导致select!退出，
                // （例如，如果未来添加了其他分支如 `shutdown_receiver.recv()`），
                // 则认为发生了未定义行为。
                // 对于当前只有一个 `recv()` 分支的情况，`recv()` 返回 `Err` 时，上面的 `context` 会处理。
                // 如果 `select!` 因其他原因（理论上不应发生）没有选择 `recv()` 分支，则进入此 `else`。
                else => bail!("策略通道发生未定义行为 (例如，所有发送者已drop，通道关闭)"),
            }
        }
    }

    /// `handle_arb_item` 方法
    ///
    /// 处理单个套利机会 (`ArbItem`) 的核心逻辑。
    ///
    /// `#[instrument]` 宏用于自动为这个函数创建一个追踪span。
    /// - `skip_all`: 不自动记录所有函数参数。
    /// - `fields(...)`: 自定义记录的字段。
    ///   - `coin`: 套利机会相关的代币名称 (取类型路径的最后一部分)。
    ///   - `tx`: 触发机会的原始交易摘要。
    ///
    /// 参数:
    /// - `arb_item`: 要处理的套利机会。
    ///
    /// 返回:
    /// - `Result<()>`: 如果处理成功则返回Ok，否则返回错误。
    #[instrument(skip_all, fields(coin = %arb_item.coin.split("::").nth(2).unwrap_or(&arb_item.coin), tx = %arb_item.tx_digest))]
    pub async fn handle_arb_item(&mut self, arb_item: ArbItem) -> Result<()> {
        // 解构 ArbItem 以获取其字段
        let ArbItem {
            coin,
            pool_id,
            tx_digest: original_trigger_tx_digest, // 触发此机会的原始交易摘要
            sim_ctx, // 与此机会相关的模拟上下文
            source,  // 机会来源 (Public, Shio等)
        } = arb_item;

        // 步骤 1: 调用 `arbitrage_one_coin` 进行核心的套利机会分析和查找。
        // `self.arb.clone()` 克隆Arc指针，`sim_ctx.clone()` 克隆模拟上下文。
        // `use_gss = false` 表示在这次分析中不使用黄金分割搜索来优化输入金额。
        // （GSS可能在 `Arb::find_opportunity` 的更早阶段，如网格搜索后，已经被调用过了，
        //  或者对于worker的快速处理，暂时禁用以提高速度）。
        if let Some((arb_result_found, time_elapsed_for_arb)) = arbitrage_one_coin(
            Arc::clone(&self.arb),
            self.sender,
            &coin,
            pool_id,
            sim_ctx.clone(), // 初始的模拟上下文 (可能基于触发交易后的状态)
            false,           // 是否在此阶段使用黄金分割搜索 (GSS)
            source.clone(),  // 克隆Source，因为后续可能修改它 (例如更新bid_amount)
        )
        .await // arbitrage_one_coin 是异步的
        {
            // 如果找到了有利可图的套利机会 (arb_result_found)

            // 步骤 2: 对找到的套利交易所构建的 `TransactionData` 进行最终的 "dry run" (预演模拟)。
            // `dry_run_tx_data` 会使用最新的对象版本（特别是Gas币）再次模拟，以确保交易仍然有效且有利可图。
            // `sim_ctx.clone()` 使用与机会发现时相同的（或基于其的）模拟上下文。
            let final_tx_data = match self.dry_run_tx_data(arb_result_found.tx_data.clone(), sim_ctx.clone()).await {
                Ok(tx_data) => tx_data, // dry run成功，获取到最终的TransactionData
                Err(error) => {
                    // 如果dry run失败，记录错误并中止对此机会的处理。
                    error!(arb_result = ?arb_result_found, ?error, "最终交易数据的Dry Run失败");
                    return Ok(()); // 返回Ok表示此item处理完毕（尽管未成功套利）
                }
            };

            // 获取最终套利交易的摘要
            let arb_tx_digest = *final_tx_data.digest(); // `digest()` 返回 `&TransactionDigest`

            // 步骤 3: 根据机会来源 (`arb_result_found.source`) 确定要提交的动作类型。
            let action_to_submit = match arb_result_found.source {
                Source::Shio { bid_amount, .. } => {
                    // 如果机会来自Shio，则创建一个 `Action::ShioSubmitBid`。
                    // `bid_amount` 此时应该是 `ArbResult` 中计算出的实际竞价金额。
                    // `original_trigger_tx_digest` 是Shio机会对应的原始机会交易摘要。
                    Action::ShioSubmitBid((final_tx_data, bid_amount, original_trigger_tx_digest))
                }
                _ => {
                    // 对于其他来源 (如Public)，则创建一个 `Action::ExecutePublicTx`。
                    Action::ExecutePublicTx(final_tx_data)
                }
            };

            // 步骤 4: 通过 `submitter` 提交动作给引擎的执行器处理。
            self.submitter.submit(action_to_submit);

            // 步骤 5: 构建并提交Telegram通知消息。
            let telegram_messages = new_tg_messages(
                original_trigger_tx_digest, // 原始触发交易的摘要
                arb_tx_digest,              // 我们构建的套利交易的摘要
                &arb_result_found,          // 套利结果详情
                time_elapsed_for_arb,       // 套利分析耗时
                &self.simulator_name,       // 使用的模拟器名称
            );
            for tg_msg in telegram_messages {
                self.submitter.submit(tg_msg.into()); // 将Message转换为Action::NotifyViaTelegram并提交
            }

            // 步骤 6: (可选) 如果配置了专用回放模拟器，则通知它。
            // 这可能用于指示一个与回放模拟器当前状态相关的机会已被发现，
            // 提示回放模拟器可能需要更频繁地更新其内部状态或进行特定分析。
            if let Some(dedicated_sim_ref) = &self.dedicated_simulator {
                // `update_notifier.send(())` 发送一个空消息作为通知信号。
                // `.unwrap()` 处理发送失败的情况 (如果通道关闭则可能panic)。
                dedicated_sim_ref.update_notifier.send(()).await.unwrap();
            }
        }
        // 如果 `arbitrage_one_coin` 没有找到机会 (返回None)，则此 `handle_arb_item` 调用结束。
        Ok(())
    }

    /// `dry_run_tx_data` 方法 (私有辅助异步函数)
    ///
    /// 对给定的 `TransactionData` 进行最终的模拟（预演）。
    /// 主要目的：
    /// 1. 使用最新的对象引用（特别是Gas币）更新交易数据。
    /// 2. 在接近实际执行的环境中再次确认交易是否成功且有利可图。
    ///
    /// 参数:
    /// - `tx_data`: 从 `ArbResult` 中获取的、已构建好的套利交易数据。
    /// - `sim_ctx`: 用于此次模拟的上下文。
    ///
    /// 返回:
    /// - `Result<TransactionData>`: 如果dry run成功且有利可图，则返回（可能已更新Gas币引用的）`TransactionData`。
    ///   否则返回错误。
    async fn dry_run_tx_data(&self, tx_data: TransactionData, sim_ctx: SimulateCtx) -> Result<TransactionData> {
        // 步骤 1: 修复/更新交易数据中的对象引用，特别是Gas币。
        // `fix_object_refs` 会获取最新的Gas币引用并替换掉 `tx_data` 中的旧引用。
        let mut tx_data_fixed_gas = self.fix_object_refs(tx_data).await?; // 可变绑定以允许修改

        // 步骤 2: 执行模拟。
        // 优先使用专用回放模拟器 (如果配置了)，否则从模拟器池中获取一个实例。
        let simulation_response = if let Some(dedicated_sim_ref) = &self.dedicated_simulator {
            dedicated_sim_ref.simulate(tx_data_fixed_gas.clone(), sim_ctx).await? // 克隆tx_data用于模拟
        } else {
            self.simulator_pool.get().simulate(tx_data_fixed_gas.clone(), sim_ctx).await?
        };

        // 步骤 3: 检查模拟结果的状态。
        let status = &simulation_response.effects.status();
        ensure!(status.is_ok(), "Dry run模拟结果状态非成功: {:?}", status);

        // 步骤 4: 检查余额变更，确认机器人操作者 (`self.sender`) 的余额有所增加。
        // 这是一种简单的盈利检查。
        // `find()` 遍历余额变更列表，找到属于操作者的那条记录。
        let balance_change_event = &simulation_response
            .balance_changes
            .iter() // 改为iter()避免消耗
            .find(|bc| bc.owner == Owner::AddressOwner(self.sender))
            .ok_or_eyre("Dry run结果中未找到操作者的余额变更记录")?; // 如果找不到则返回错误

        // 确保余额变化量大于0 (即有利润)。
        // 注意：`bc.amount` 是 `i128`，可以为负。
        // 这里的检查 `bc.amount > 0` 对于 SUI->SUI 套利是正确的（净增加）。
        // 如果套利目标是其他代币，或者 `bc.amount` 代表的是目标代币的净增加，也可能适用。
        // 但如果套利是 SUI -> TokenX，然后期望 TokenX 增加，而 SUI 减少，
        // 那么需要检查特定 TokenX 的余额变化。
        // 此处假设 `bc.amount > 0` 是一个通用的盈利指标，或者特指SUI余额增加。
        ensure!(balance_change_event.amount > 0, "Dry run后操作者余额未增加或反而减少: {:?}", balance_change_event);

        // 如果所有检查通过，返回（可能已更新Gas币引用的）`TransactionData`。
        Ok(tx_data_fixed_gas)
    }

    /// `fix_object_refs` 方法 (私有辅助异步函数)
    ///
    /// 更新 `TransactionData` 中的Gas支付对象 (`GasData.payment`) 为当前账户最新的可用Gas币引用。
    /// 这是因为Gas币对象在每次使用后其版本都会改变，如果使用旧版本的Gas币引用会导致交易失败。
    ///
    /// 参数:
    /// - `tx_data`: 要修复的原始 `TransactionData`。
    ///
    /// 返回:
    /// - `Result<TransactionData>`: 更新了Gas币引用的 `TransactionData`。
    async fn fix_object_refs(&self, mut tx_data: TransactionData) -> Result<TransactionData> { // tx_data设为可变
        // 从链上获取当前发送者账户 (`self.sender`) 最新的Gas币对象引用列表。
        // `None` 作为第三个参数给 `get_gas_coin_refs` 可能表示不排除任何特定对象ID。
        let latest_gas_coins = coin::get_gas_coin_refs(&self.sui, self.sender, None).await?;

        // 获取对 `tx_data` 中 `GasData` 部分的可变引用，并更新其 `payment` 字段。
        let gas_data_mut_ref: &mut GasData = tx_data.gas_data_mut();
        gas_data_mut_ref.payment = latest_gas_coins;

        Ok(tx_data) // 返回修改后的 tx_data
    }
}

/// `arbitrage_one_coin` (独立异步函数)
///
/// 封装了对单个代币进行套利机会发现的核心逻辑。
/// 它调用 `Arb::find_opportunity` 来执行实际的路径搜索和模拟。
///
/// 参数:
/// - `arb_instance`: 共享的 `Arb` 实例。
/// - `attacker_address`: 机器人操作者的Sui地址。
/// - `coin_type_str`: 要分析的代币类型字符串。
/// - `pool_id_option`: (可选) 与机会相关的特定交易池ID。
/// - `sim_ctx`: 用于此次分析的模拟上下文。
/// - `use_gss`: 是否在 `find_opportunity` 中使用黄金分割搜索。
/// - `source`: 此机会的来源。
///
/// 返回:
/// - `Option<(ArbResult, Duration)>`: 如果找到有利可图的机会，则返回Some元组，包含 `ArbResult` 和分析耗时。
///   否则返回None。
async fn arbitrage_one_coin(
    arb_instance: Arc<Arb>,
    attacker_address: SuiAddress,
    coin_type_str: &str,
    pool_id_option: Option<ObjectID>,
    sim_ctx: SimulateCtx,
    use_gss: bool,
    source: Source,
) -> Option<(ArbResult, Duration)> {
    let start_time = Instant::now(); // 记录开始时间
    // 调用 Arb 实例的 find_opportunity 方法
    let arb_result_outcome = arb_instance
        .find_opportunity(
            attacker_address,
            coin_type_str,
            pool_id_option,
            vec![], // Gas币列表为空，因为 `find_opportunity` 内部的模拟可能使用模拟Gas或不直接构建最终交易
            sim_ctx,
            use_gss,
            source,
        )
        .await;

    match arb_result_outcome {
        Ok(found_arb_result) => {
            // 如果成功找到机会
            info!(
                elapsed = ?start_time.elapsed(), // 总耗时
                elapsed.ctx_creation = ?found_arb_result.create_trial_ctx_duration, // TrialCtx创建耗时
                elapsed.grid_search = ?found_arb_result.grid_search_duration,  // 网格搜索耗时
                elapsed.gss = ?found_arb_result.gss_duration,                // GSS耗时
                cache_misses = ?found_arb_result.cache_misses,               // 缓存未命中次数
                coin = %coin_type_str,                                       // 代币类型
                "💰 发现可盈利机会: {:?}",                                    // 日志消息
                &found_arb_result.best_trial_result                          // 最佳尝试结果
            );
            Some((found_arb_result, start_time.elapsed())) // 返回结果和总耗时
        }
        Err(error) => {
            // 如果没有找到机会或发生错误
            let time_elapsed_on_failure = start_time.elapsed();
            // 根据耗时决定日志格式 (如果耗时较长，使用更醒目的红色标记)
            if time_elapsed_on_failure > Duration::from_secs(1) {
                info!(elapsed = ?time_elapsed_on_failure, %coin_type_str, "🥱 \x1b[31m未发现机会 (No opportunity): {error:#}\x1b[0m");
            } else {
                info!(elapsed = ?time_elapsed_on_failure, %coin_type_str, "🥱 未发现机会 (No opportunity): {error:#}");
            }
            None // 返回None
        }
    }
}
