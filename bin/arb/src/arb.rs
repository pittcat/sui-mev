//! 该文件是套利机器人程序的一部分，主要负责发现和执行Sui区块链上的套利机会。
//! 套利是指利用不同市场或交易路径上的价格差异来获利。
//! 例如，如果代币A在一个去中心化交易所（DEX）的价格低于另一个DEX，
//! 机器人可以尝试在一个DEX上买入代币A，然后在另一个DEX上卖出，从而赚取差价。
//!
//! 此文件中的代码会：
//! 1. 定义如何解析命令行参数（例如，要交易的代币类型，RPC节点URL等）。
//! 2. 初始化与Sui区块链的连接。
//! 3. 初始化一个交易模拟器，用于在实际执行交易前预测交易结果。
//! 4. 实现`Arb`结构体，其中包含寻找套利机会的核心逻辑。
//! 5. 实现`TrialCtx`和`TrialResult`等辅助结构体，用于在搜索过程中管理和评估潜在的交易路径。
//! 6. 使用搜索算法（如网格搜索和黄金分割搜索）来寻找最佳的输入金额以最大化利润。
//! 7. 构建并可能执行最终的套利交易。
//!
//! 示例用法 (通过命令行运行):
//! cargo run -r --bin arb run --coin-type \
//!     "0xa8816d3a6e3136e86bc2873b1f94a15cadc8af2703c075f2d546c2ae367f4df9::ocean::OCEAN"
//! 上述命令会尝试寻找 `OCEAN` 代币的套利机会。
//! `-r` 表示以release模式运行 (性能优化)。
//! `--bin arb` 指定运行`arb`这个二进制程序。
//! `run` 是传递给`arb`程序的子命令。
//! `--coin-type` 指定要进行套利的代币的完整类型字符串。

// Rust标准库及第三方库的引入
use std::{
    fmt, // 用于格式化输出 (例如，实现 Display trait)
    str::FromStr, // 用于从字符串转换 (例如，将字符串地址转为SuiAddress)
    sync::Arc, // Atomic Reference Counting, 一种线程安全的智能指针，允许多个所有者共享数据
    time::{Duration, Instant}, // 用于处理时间和计时
};

use async_trait::async_trait; // 使得trait中的方法可以声明为异步 (async)
use clap::Parser; // 用于解析命令行参数
use eyre::{ensure, ContextCompat, Result}; // 用于错误处理, ensure! 宏用于断言条件，否则返回错误
use itertools::Itertools; // 提供了一系列有用的迭代器适配器
use object_pool::ObjectPool; // 对象池，用于复用昂贵的对象，如模拟器实例
use simulator::{HttpSimulator, SimulateCtx, Simulator}; // 交易模拟器相关的组件
use sui_sdk::SuiClientBuilder; // 用于构建Sui RPC客户端，与Sui区块链交互
use sui_types::{
    base_types::{ObjectID, ObjectRef, SuiAddress}, // Sui区块链的基本类型，如对象ID, 对象引用, Sui地址
    transaction::TransactionData, // 代表一个交易的数据结构
};
use tokio::task::JoinSet; // 用于管理一组并发的异步任务
use tracing::{debug, info, instrument, Instrument}; // 用于日志和追踪 (instrument宏会自动为函数添加追踪)
use utils::coin; // 自定义的工具模块，可能包含与代币操作相关的函数

// 当前crate (项目) 内其他模块的引入
use crate::{
    common::get_latest_epoch, // 获取最新的Sui纪元信息
    common::search::{golden_section_search_maximize, SearchGoal}, // 黄金分割搜索算法及相关trait
    defi::{Defi, Path, TradeType}, // DeFi (去中心化金融) 相关的定义，如交易路径、交易类型
    types::Source, // 定义交易来源的类型
    HttpConfig, // HTTP配置，如RPC URL
};

/// 定义了运行套利机器人时可以接受的命令行参数。
/// 使用 `clap::Parser` 宏可以自动从结构体定义生成命令行参数解析逻辑。
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// 要进行套利的代币类型 (Coin Type)。
    /// 例如: "0x2::sui::SUI" 或某个自定义代币的完整类型地址。
    #[arg(long)]
    pub coin_type: String,

    /// (可选) 指定一个特定的交易池ID (Pool ID)。
    /// 如果提供，套利搜索可能会更关注与这个池相关的路径。
    /// Pool ID通常是一个Sui对象的ID。
    #[arg(long)]
    pub pool_id: Option<String>,

    /// (可选) 交易发送者的Sui地址。
    /// 如果不提供，可能使用默认地址或从其他配置中获取。
    #[arg(
        long,
        default_value = "" // 默认值为空字符串，后续逻辑需要处理
    )]
    pub sender: String,

    /// HTTP相关的配置，例如Sui RPC节点的URL。
    /// `#[command(flatten)]` 表示将 `HttpConfig` 中的字段直接作为当前命令的参数。
    #[command(flatten)]
    pub http_config: HttpConfig,
}

/// `run` 函数是套利命令的入口点。
/// 它接收解析后的命令行参数 `args`，并执行主要的套利逻辑。
pub async fn run(args: Args) -> Result<()> { // `Result<()>` 表示函数可能返回错误 (eyre::Result)
    // 初始化日志系统。
    // `mev_logger::init_console_logger_with_directives` 用于设置日志级别和格式。
    // "arb=debug" 表示 arb 模块的日志级别为 debug。
    mev_logger::init_console_logger_with_directives(None, &["arb=debug", "dex_indexer=debug"]);

    info!("启动套利程序，参数: {:?}", args); // 记录一条信息级别的日志

    // 从参数中获取RPC URL和IPC路径 (如果IPC用于模拟器)
    let rpc_url = args.http_config.rpc_url.clone();
    let ipc_path = args.http_config.ipc_path.clone();

    // 将字符串形式的发送者地址转换为 SuiAddress 类型。
    // `SuiAddress::from_str` 是标准方法。 `map_err` 用于转换可能的错误类型。
    let sender = SuiAddress::from_str(&args.sender).map_err(|e| eyre::eyre!(e))?;

    // 创建一个交易模拟器对象池。
    // 对象池用于管理和复用模拟器实例，避免重复创建昂贵的资源。
    // HttpSimulator::new(...) 会创建一个通过HTTP与Sui节点通信的模拟器。
    // `Arc<ObjectPool<...>>` 表示这是一个线程安全共享的对象池。
    let simulator_pool = Arc::new(ObjectPool::new(1, move || { // 池中保持1个模拟器实例
        // 在新的tokio运行时中异步创建模拟器
        // 这是因为 ObjectPool 的初始化函数是同步的，而模拟器创建是异步的
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { Box::new(HttpSimulator::new(&rpc_url, &ipc_path).await) as Box<dyn Simulator> })
    }));

    // 创建 Arb 实例，这是执行套利逻辑的核心。
    let arb = Arb::new(&args.http_config.rpc_url, Arc::clone(&simulator_pool)).await?;
    
    // 创建 Sui SDK 客户端，用于与Sui区块链进行标准交互 (如获取gas币)。
    let sui = SuiClientBuilder::default().build(&args.http_config.rpc_url).await?;
    
    // 获取发送者账户的gas代币对象引用 (ObjectRef)。
    // gas代币 (通常是SUI) 用于支付交易手续费。
    // `coin::get_gas_coin_refs` 是一个辅助函数。
    let gas_coins = coin::get_gas_coin_refs(&sui, sender, None).await?;
    
    // 获取最新的Sui纪元 (epoch) 信息。纪元信息包含当前的gas价格等。
    let epoch = get_latest_epoch(&sui).await?;
    
    // 创建模拟上下文 (SimulateCtx)，包含纪元信息和可能的初始状态 (这里是空)。
    let sim_ctx = SimulateCtx::new(epoch, vec![]);
    
    // 将字符串形式的 pool_id (如果提供) 转换为 ObjectID 类型。
    // `ObjectID::from_hex_literal` 用于从十六进制字符串转换。
    let pool_id = args.pool_id.as_deref().map(ObjectID::from_hex_literal).transpose()?;

    // 调用 `find_opportunity` 方法寻找套利机会。
    let result = arb
        .find_opportunity(
            sender,         // 交易发送者地址
            &args.coin_type, // 要套利的代币类型
            pool_id,        // (可选) 特定的交易池ID
            gas_coins,      // 用于支付gas的代币对象
            sim_ctx,        // 模拟上下文
            true,           // 是否使用黄金分割搜索 (GSS) 进行优化
            Source::Public, // 交易来源，这里是公开的 (Public)
        )
        .await?;

    // 打印找到的套利机会结果。
    info!("套利结果: {:#?}", result); // `{:#?}` 是Rust的Debug格式化输出，带缩进和换行，更易读
    Ok(()) // 表示函数成功完成
}

/// `ArbResult` 结构体用于存储套利机会搜索的结果。
#[derive(Debug)] // 自动派生 Debug trait，使其可以被打印输出
pub struct ArbResult {
    pub create_trial_ctx_duration: Duration, // 创建 `TrialCtx` (尝试上下文) 所花费的时间
    pub grid_search_duration: Duration,    // 网格搜索阶段花费的时间
    pub gss_duration: Option<Duration>,      // (可选) 黄金分割搜索 (GSS) 阶段花费的时间
    pub best_trial_result: TrialResult,    // 找到的最佳尝试结果 (包含利润、路径等)
    pub cache_misses: u64,                 // 在模拟过程中缓存未命中的次数 (可能影响性能)
    pub source: Source,                    // 交易来源信息 (可能包含MEV竞价相关数据)
    pub tx_data: TransactionData,          // 构建好的最终套利交易数据，准备发送到链上
}

/// `Arb` 结构体是套利机器人的核心。
/// 它封装了与DeFi协议交互的逻辑。
pub struct Arb {
    defi: Defi, // `Defi` 结构体实例，用于处理与去中心化交易所的交互，如获取交易路径、报价等
}

impl Arb {
    /// 创建一个新的 `Arb` 实例。
    ///
    /// 参数:
    /// - `http_url`: Sui RPC节点的URL。
    /// - `simulator_pool`: 一个共享的交易模拟器对象池。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `Arb` 实例，否则返回错误。
    pub async fn new(http_url: &str, simulator_pool: Arc<ObjectPool<Box<dyn Simulator>>>) -> Result<Self> {
        // 初始化 Defi 实例，它会加载所有相关的DEX信息。
        let defi = Defi::new(http_url, simulator_pool).await?;
        Ok(Self { defi })
    }

    /// `find_opportunity` 是寻找套利机会的核心方法。
    /// 它会执行一系列搜索和模拟来找到最佳的交易路径和金额。
    ///
    /// 参数:
    /// - `sender`: 交易发送者的Sui地址。
    /// - `coin_type`: 要套利的代币类型。
    /// - `pool_id`: (可选) 特定的交易池ID，如果指定，会优先考虑涉及此池的路径。
    /// - `gas_coins`: 用于支付交易费的gas代币列表。
    /// - `sim_ctx`: 模拟上下文，包含当前纪元信息 (如gas价格)。
    /// - `use_gss`:布尔值，指示是否使用黄金分割搜索 (GSS) 来优化输入金额。
    /// - `source`: 交易来源，例如是公开的还是通过MEV竞价。
    ///
    /// 返回:
    /// - `Result<ArbResult>`: 成功则返回包含套利结果的 `ArbResult`，否则返回错误。
    #[allow(clippy::too_many_arguments)] // 允许函数有很多参数 (Rust linter有时会警告这个)
    pub async fn find_opportunity(
        &self,
        sender: SuiAddress,
        coin_type: &str,
        pool_id: Option<ObjectID>,
        gas_coins: Vec<ObjectRef>,
        sim_ctx: SimulateCtx,
        use_gss: bool, // 是否使用黄金分割搜索
        source: Source,
    ) -> Result<ArbResult> {
        // 从模拟上下文中获取当前Sui网络的gas价格。
        let gas_price = sim_ctx.epoch.gas_price;

        // 创建 `TrialCtx` (尝试上下文)。
        // `TrialCtx` 封装了单次套利尝试所需的所有信息和状态。
        // 使用 `Instant::now()` 和 `timer.elapsed()` 来计时创建过程。
        let (ctx, create_trial_ctx_duration) = {
            let timer = Instant::now();
            // `Arc` 用于在异步任务间安全地共享 `TrialCtx`。
            let ctx = Arc::new(
                TrialCtx::new(
                    self.defi.clone(), // 克隆 `Defi` 实例 (内部是 Arc，所以克隆成本低)
                    sender,
                    coin_type,
                    pool_id,
                    gas_coins.clone(), // 克隆 gas_coins 列表
                    sim_ctx.clone(),   // 克隆模拟上下文
                )
                .await?, // `?` 用于错误传播
            );
            (ctx, timer.elapsed())
        };

        // --- 网格搜索 (Grid Search) ---
        // 网格搜索是一种通过在预定义的网格点上评估函数来寻找最优解的方法。
        // 这里用于初步找到一个大致有利可图的输入金额。
        // `starting_grid` 定义了初始的输入金额单位 (例如 0.001 SUI)。
        let starting_grid = 1_000_000u64; // 对应 0.001 SUI (假设SUI有9位小数)
        let mut cache_misses = 0; // 记录模拟缓存未命中的次数

        let (mut max_trial_res, grid_search_duration) = {
            let timer = Instant::now();
            let mut joinset = JoinSet::new(); // 用于并发执行多个网格点的尝试

            // 尝试10个不同的数量级作为输入金额 (starting_grid * 10^1, ..., starting_grid * 10^10)
            for inc in 1..11 { // 从1到10
                let ctx_clone = Arc::clone(&ctx); // 克隆Arc指针，传递给异步任务
                // 计算当前网格点对应的输入金额
                let grid = starting_grid.checked_mul(10u64.pow(inc)).context("网格金额溢出")?;

                // 产生一个异步任务来评估这个输入金额
                // `in_current_span()` 使得这个任务继承当前的tracing span，方便日志追踪
                joinset.spawn(async move { ctx_clone.trial(grid).await }.in_current_span());
            }

            let mut max_trial_res = TrialResult::default(); // 初始化一个默认的最佳尝试结果
            // 等待所有网格搜索任务完成，并收集结果
            while let Some(Ok(trial_res_result)) = joinset.join_next().await {
                // `join_next()` 返回 `Result<Result<TrialResult>>`
                // 外层Result是任务执行结果，内层Result是 `trial` 函数的执行结果
                if let Ok(trial_res) = trial_res_result { // 如果 trial 函数成功
                    if trial_res.cache_misses > cache_misses {
                        cache_misses = trial_res.cache_misses;
                    }
                    // 如果当前尝试的结果优于已知的最佳结果，则更新最佳结果
                    if trial_res > max_trial_res { // TrialResult 实现了 PartialOrd (基于profit比较)
                        max_trial_res = trial_res;
                    }
                }
            }
            (max_trial_res, timer.elapsed()) // 返回找到的最佳结果和搜索耗时
        };

        // 确保网格搜索找到了至少一个有利可图的结果
        // `ensure!` 宏如果条件为false，会返回一个错误。
        ensure!(
            max_trial_res.profit > 0, // 利润必须大于0
            "缓存未命中次数: {}. 未找到有利可图的网格点。", // 错误消息
            cache_misses
        );

        // --- (可选) 黄金分割搜索 (Golden Section Search - GSS) ---
        // GSS 是一种用于在单峰函数上寻找极值的优化算法。
        // 如果 `use_gss` 为 true，则在网格搜索找到的最佳点附近使用GSS进行更精细的搜索。
        let gss_duration = if use_gss {
            let timer = Instant::now();
            // 定义GSS的搜索边界，通常在网格搜索结果的附近 (例如，0.1倍到10倍)
            let upper_bound = max_trial_res.amount_in.saturating_mul(10); // 饱和乘法，防止溢出
            let lower_bound = max_trial_res.amount_in.saturating_div(10); // 饱和除法

            let goal = TrialGoal; // 定义GSS的优化目标 (这里是 `TrialGoal` 结构体)
            // 执行黄金分割搜索
            let (_, _, trial_res) = golden_section_search_maximize(lower_bound, upper_bound, goal, &ctx).await;
            
            if trial_res.cache_misses > cache_misses {
                cache_misses = trial_res.cache_misses;
            }
            // 如果GSS找到了更好的结果，则更新 `max_trial_res`
            if trial_res > max_trial_res {
                max_trial_res = trial_res;
            }

            Some(timer.elapsed()) // 返回GSS耗时
        } else {
            None // 如果不使用GSS，则耗时为None
        };

        // 再次确保找到了有利可图的交易路径 (无论是否经过GSS)
        ensure!(
            max_trial_res.profit > 0,
            "缓存未命中次数: {}. 未找到有利可图的交易路径。",
            cache_misses
        );

        // 从最终的最佳尝试结果中解构出所需信息
        let TrialResult {
            amount_in,    // 最佳输入金额
            trade_path,   // 最佳交易路径
            profit,       // 最大利润
            ..            // 其他字段忽略 (用 `..` 表示)
        } = &max_trial_res; // 使用引用避免所有权转移

        // 更新交易来源 (Source) 信息
        // 如果来源包含截止时间 (deadline)，则记录套利机会发现的时间。
        // 这对于MEV (Miner Extractable Value) 场景可能很重要，MEV竞价通常有时间限制。
        let mut current_source = source; // 创建source的可变副本
        if current_source.deadline().is_some() {
            current_source = current_source.with_arb_found_time(utils::current_time_ms());
        }
        // 设置MEV竞价金额，例如利润的90%。这部分金额可能会支付给验证者以优先处理交易。
        // TODO: 使 bid_amount 可配置，而不是硬编码为利润的90%。
        current_source = current_source.with_bid_amount(*profit / 10 * 9); // profit是u64，整数除法

        // 使用找到的最佳参数构建最终的交易数据 (TransactionData)。
        // `TransactionData` 是Sui SDK中用于表示一笔交易的结构。
        let tx_data = self
            .defi
            .build_final_tx_data(sender, *amount_in, trade_path, gas_coins, gas_price, current_source.clone())
            .await?;

        // 返回包含所有结果的 `ArbResult`
        Ok(ArbResult {
            create_trial_ctx_duration,
            grid_search_duration,
            gss_duration,
            best_trial_result: max_trial_res, // 存储的是优化后的 max_trial_res
            cache_misses,
            source: current_source,
            tx_data,
        })
    }
}

/// `TrialCtx` (尝试上下文) 结构体。
/// 封装了执行单次套利“尝试” (trial) 所需的所有上下文信息。
/// 一次“尝试”通常是指给定一个输入金额，通过模拟找到最佳的买入和卖出路径组合，并计算利润。
pub struct TrialCtx {
    defi: Defi,                       // DeFi交互的实例
    sender: SuiAddress,               // 交易发送方地址
    coin_type: String,                // 目标代币类型
    pool_id: Option<ObjectID>,        // (可选) 特定的交易池ID
    buy_paths: Vec<Path>,             // 预先计算好的所有可能的“买入”路径
    sell_paths: Vec<Path>,            // 预先计算好的所有可能的“卖出”路径
    gas_coins: Vec<ObjectRef>,        // 用于支付gas的代币
    sim_ctx: SimulateCtx,             // 模拟上下文 (包含epoch, gas价格等)
}

impl TrialCtx {
    /// 创建一个新的 `TrialCtx` 实例。
    /// 这个过程会预先查找所有可能的买入和卖出路径。
    pub async fn new(
        defi: Defi,
        sender: SuiAddress,
        coin_type: &str,
        pool_id: Option<ObjectID>,
        gas_coins: Vec<ObjectRef>,
        sim_ctx: SimulateCtx,
    ) -> Result<Self> {
        // 查找所有可能的买入路径 (例如，用SUI买入目标代币)
        let buy_paths = defi.find_buy_paths(coin_type).await?;
        // 确保至少找到一条买入路径
        ensure!(!buy_paths.is_empty(), "未找到目标代币 {} 的买入路径", coin_type);

        // 查找所有可能的卖出路径 (例如，卖出目标代币换回SUI)
        let sell_paths = defi.find_sell_paths(coin_type).await?;
        // 确保至少找到一条卖出路径
        ensure!(!sell_paths.is_empty(), "未找到目标代币 {} 的卖出路径", coin_type);

        // 如果指定了 pool_id，则验证买入或卖出路径中至少有一条包含该池。
        // 这是为了确保如果关注某个特定池的波动，我们的路径搜索是相关的。
        if pool_id.is_some() {
            let buy_paths_contain_pool = buy_paths.iter().any(|p| p.contains_pool(pool_id));
            let sell_paths_contain_pool = sell_paths.iter().any(|p| p.contains_pool(pool_id));
            ensure!(
                buy_paths_contain_pool || sell_paths_contain_pool,
                "未找到包含指定交易池 {:?} 的路径",
                pool_id
            );
        }

        Ok(Self {
            defi,
            sender,
            coin_type: coin_type.to_string(), // 将 &str 转为 String
            pool_id,
            buy_paths,
            sell_paths,
            gas_coins,
            sim_ctx,
        })
    }

    /// `trial` 方法是核心的单次尝试逻辑。
    /// 给定一个输入金额 `amount_in` (通常是基础货币，如SUI)，它会：
    /// 1. 在所有 `buy_paths` 中找到最佳的买入路径和结果。
    /// 2. 将最佳买入路径与所有 `sell_paths` 组合，形成完整的交易路径。
    /// 3. 在这些组合路径中找到能产生最大利润的最终路径。
    ///
    /// `#[instrument]` 宏用于自动添加tracing/logging功能，方便调试。
    /// - `skip_all`: 不自动记录所有函数参数。
    /// - `fields(...)`: 自定义记录的字段。
    ///   - `in`: 记录输入金额 (格式化为SUI单位)。
    ///   - `len`: 记录买入路径的数量。
    ///   - `action`: 动态字段，在函数执行过程中可以更新 (例如，"buy", "sell")。
    #[instrument(
        name = "trial", // span的名称
        skip_all,
        fields(
            // 将amount_in (通常是u64类型的MIST) 转换为f64类型的SUI并格式化
            in = %format!("{:<15}", (amount_in as f64 / 1_000_000_000.0)), 
            len = %format!("{:<2}", self.buy_paths.len()), // 买入路径数量
            action="init" // 初始action状态
        )
    )]
    pub async fn trial(&self, amount_in: u64) -> Result<TrialResult> {
        // 更新tracing span的action字段为"buy"
        tracing::Span::current().record("action", "buy");

        let timer = Instant::now();
        // 步骤1: 找到最佳的买入路径。
        // `find_best_path_exact_in` 会模拟所有`buy_paths`，使用精确的`amount_in`，
        // 并返回结果最好的那条路径及其模拟结果。
        let best_buy_res = self
            .defi
            .find_best_path_exact_in(
                &self.buy_paths,      // 提供所有可能的买入路径
                self.sender,
                amount_in,            // 输入的SUI金额
                TradeType::Swap,      // 交易类型是普通交换 (Swap)
                &self.gas_coins,
                &self.sim_ctx,
            )
            .await?;
        let buy_elapsed = timer.elapsed(); // 记录买入阶段耗时

        let timer = Instant::now();
        // 步骤2: 将最佳买入路径与所有卖出路径组合。
        let best_buy_path = &best_buy_res.path; // 获取最佳买入路径的引用
        // 检查最佳买入路径是否包含我们关注的特定pool_id (如果设置了的话)
        let buy_path_contains_pool = best_buy_path.contains_pool(self.pool_id);

        // 遍历所有预计算的 `sell_paths`
        let trade_paths: Vec<Path> = self
            .sell_paths
            .iter()
            // `filter_map` 用于过滤不符合条件的路径并转换路径格式
            .filter_map(|sell_path_candidate| {
                // 条件1: 买入路径和卖出路径应该是互斥的 (不包含相同的交易池)，避免循环交易或无效路径。
                // 条件2: 如果指定了 `pool_id`，那么组合路径 (买入路径 或 卖出路径) 中至少有一个要包含这个 `pool_id`。
                //         如果没指定 `pool_id` (即 `self.pool_id` 为 `None`)，`contains_pool` 会返回 `true` (或应设计为如此)，
                //         使得 `buy_path_contains_pool || p.contains_pool(self.pool_id)` 变为 `true || true` (假设默认包含)。
                //         或者，更准确地说，`contains_pool(None)` 应该总是返回 `true`，表示不按特定池过滤。
                //         (需要查看 `Path::contains_pool` 的具体实现来确认 `None` 的行为)
                //         假设 `contains_pool(None)` 意味着不施加池限制，则条件变为：
                //         `best_buy_path.is_disjoint(p)` 必须为真。
                //         并且 (`buy_path_contains_pool` (如果pool_id指定了，买路径是否包含) OR `p.contains_pool(self.pool_id)` (如果pool_id指定了，卖路径是否包含))
                //         如果 `self.pool_id` 是 `None`，则 `buy_path_contains_pool` (应为true或不关心) OR `p.contains_pool(None)` (应为true)
                //         所以，核心是 `is_disjoint` 和 当 `pool_id` 有值时的包含性检查。
                if best_buy_path.is_disjoint(sell_path_candidate) && 
                   (buy_path_contains_pool || sell_path_candidate.contains_pool(self.pool_id)) {
                    // 如果满足条件，则将买入路径和卖出路径合并成一条完整的交易路径。
                    let mut full_trade_path = best_buy_path.clone(); // 克隆买入路径
                    full_trade_path.path.extend(sell_path_candidate.path.clone()); // 追加卖出路径的步骤
                    Some(full_trade_path)
                } else {
                    None // 不符合条件，则过滤掉
                }
            })
            .collect_vec(); // 收集所有有效的完整交易路径

        // 确保至少找到一条有效的组合交易路径
        ensure!(
            !trade_paths.is_empty(),
            "对于代币 {} 和交易池 {:?}，未找到有效的组合交易路径。",
            self.coin_type,
            self.pool_id
        );

        // 更新tracing span的action字段为"sell"
        tracing::Span::current().record("action", "sell");
        // 步骤3: 在所有组合的 `trade_paths` 中找到最佳的。
        // 这里的交易类型是 `Flashloan`，因为整个套利过程可以看作：
        // 1. 借入 `amount_in` 的SUI (通过闪电贷，如果DEX支持或链支持原子组合)
        // 2. 执行 `best_buy_path` (买入目标代币)
        // 3. 执行 `sell_path_candidate` (卖出目标代币换回SUI)
        // 4. 偿还闪电贷，剩余为利润。
        // 即使不是真的闪电贷，`TradeType::Flashloan` 可能在模拟时有特殊处理，例如假设起始代币是借来的。
        let best_trade_res = self
            .defi
            .find_best_path_exact_in(
                &trade_paths, // 提供所有组合的完整交易路径
                self.sender,
                amount_in,    // 初始输入金额 (可以看作闪电贷的金额)
                TradeType::Flashloan, // 交易类型
                &self.gas_coins,
                &self.sim_ctx,
            )
            .await?;
        
        let sell_elapsed = timer.elapsed(); // 记录卖出/组合阶段耗时
        // 记录调试信息，包含代币类型、最佳交易结果、买入耗时、卖出耗时
        debug!(coin_type = ?self.coin_type, result = %best_trade_res, ?buy_elapsed, ?sell_elapsed, "单次尝试结果");

        // 获取最终利润
        let profit = best_trade_res.profit();
        if profit <= 0 { // 如果利润小于等于0，则不是一个好的机会
            return Ok(TrialResult::default()); // 返回一个默认的 (通常是零利润) TrialResult
        }

        // 如果有利润，则创建一个 `TrialResult` 实例
        let result = TrialResult::new(
            &self.coin_type,
            amount_in,
            profit as u64, // 确保利润是u64
            best_trade_res.path, // 最终的最佳交易路径 (组合路径)
            best_trade_res.cache_misses, // 缓存未命中次数
        );

        Ok(result) // 返回成功的 TrialResult
    }
}

/// `TrialResult` 结构体用于存储单次套利尝试 (trial) 的结果。
#[derive(Debug, Default, Clone)] // Default 用于创建默认实例 (例如零利润)
pub struct TrialResult {
    pub coin_type: String,    // 套利的代币类型
    pub amount_in: u64,       // 输入金额 (例如，多少SUI)
    pub profit: u64,          // 产生的利润 (通常以SUI的最小单位MIST表示)
    pub trade_path: Path,     // 导致此利润的完整交易路径
    pub cache_misses: u64,    // 模拟过程中的缓存未命中次数
}

/// 为 `TrialResult` 实现 `PartialOrd` trait，使其可以进行比较。
/// 这里的比较是基于 `profit` 字段。
impl PartialOrd for TrialResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.profit.partial_cmp(&other.profit) // 比较利润大小
    }
}

/// 为 `TrialResult` 实现 `PartialEq` trait，判断两个结果是否相等。
/// 同样基于 `profit` 字段。
impl PartialEq for TrialResult {
    fn eq(&self, other: &Self) -> bool {
        self.profit == other.profit // 判断利润是否相等
    }
}

impl TrialResult {
    /// `TrialResult` 的构造函数。
    pub fn new(coin_type: &str, amount_in: u64, profit: u64, trade_path: Path, cache_misses: u64) -> Self {
        Self {
            coin_type: coin_type.to_string(),
            amount_in,
            profit,
            trade_path,
            cache_misses,
        }
    }
}

/// 为 `TrialResult` 实现 `fmt::Display` trait，使其可以被格式化为字符串输出。
/// 这主要用于日志和调试。
impl fmt::Display for TrialResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            // 输出部分关键信息，trade_path可能很长，只显示一部分摘要
            "TrialResult {{ coin_type: {}, amount_in: {}, profit: {}, trade_path: {:?} ... }}",
            self.coin_type, self.amount_in, self.profit, self.trade_path // trade_path 使用Debug格式
        )
    }
}

/// `TrialGoal` 结构体，用作黄金分割搜索 (GSS) 的目标。
/// GSS需要一个目标函数来评估每个点的值。在这里，目标是最大化 `TrialResult` 的利润。
pub struct TrialGoal;

/// 为 `TrialGoal` 实现 `SearchGoal` trait。
/// `SearchGoal` 定义了如何在GSS的每次迭代中评估一个点 (输入金额 `amount_in`)。
#[async_trait] // 因为 `evaluate` 方法是异步的
impl SearchGoal<TrialCtx, u64, TrialResult> for TrialGoal {
    /// `evaluate` 方法接收一个输入金额 `amount_in` 和 `TrialCtx` 上下文，
    /// 返回该输入金额对应的“值” (用于GSS比较，这里是利润) 以及完整的 `TrialResult`。
    ///
    /// 参数:
    /// - `amount_in`: GSS算法当前尝试的输入金额。
    /// - `ctx`: `TrialCtx` 上下文，包含了执行 `trial` 所需的一切。
    ///
    /// 返回:
    /// - `(u64, TrialResult)`: 一个元组，第一个元素是利润 (用于GSS优化)，第二个元素是完整的尝试结果。
    async fn evaluate(&self, amount_in: u64, ctx: &TrialCtx) -> (u64, TrialResult) {
        // 调用 ctx.trial(amount_in) 来获取给定输入金额的 TrialResult。
        // `unwrap_or_default()`: 如果 `trial` 方法返回错误 (例如找不到路径)，
        // 则使用一个默认的 `TrialResult` (通常是零利润)，以确保GSS可以继续进行。
        let trial_res = ctx.trial(amount_in).await.unwrap_or_default();
        (trial_res.profit, trial_res) // 返回利润和完整结果
    }
}

// --- 测试模块 ---
// `#[cfg(test)]` 属性宏表示这部分代码仅在执行 `cargo test` 时编译。
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use simulator::{DBSimulator, HttpSimulator, Simulator}; // 测试可能用到不同的模拟器实现
    use sui_types::base_types::SuiAddress;

    use super::*; // 导入外部模块 (即 `arb.rs` 本身) 的所有公共成员
    use crate::config::tests::{TEST_ATTACKER, TEST_HTTP_URL}; // 从配置中导入测试常量

    /// `test_find_best_trade_path` 是一个异步的集成测试函数。
    /// 它会模拟整个 `find_opportunity` 的流程，并检查结果。
    #[tokio::test] // 声明这是一个基于tokio运行时的异步测试
    async fn test_find_best_trade_path() {
        // 初始化日志，方便在测试输出中看到详细信息
        mev_logger::init_console_logger_with_directives(None, &["arb=debug"]);

        // 创建一个HTTP模拟器对象池 (与 `run` 函数中类似)
        let simulator_pool = Arc::new(ObjectPool::new(1, move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { Box::new(HttpSimulator::new(&TEST_HTTP_URL, &None).await) as Box<dyn Simulator> })
        }));

        let start = Instant::now(); // 开始计时

        // 设置测试参数
        let sender = SuiAddress::from_str(TEST_ATTACKER).unwrap(); // 测试用的攻击者/发送者地址
        let sui = SuiClientBuilder::default().build(TEST_HTTP_URL).await.unwrap(); // Sui客户端
        let epoch = get_latest_epoch(&sui).await.unwrap(); // 最新纪元
        let sim_ctx = SimulateCtx::new(epoch, vec![]); // 模拟上下文

        let gas_coins = coin::get_gas_coin_refs(&sui, sender, None).await.unwrap(); // Gas币
        let arb = Arb::new(TEST_HTTP_URL, Arc::clone(&simulator_pool)).await.unwrap(); // Arb实例
        // 一个已知的测试代币类型
        let coin_type = "0xce7ff77a83ea0cb6fd39bd8748e2ec89a3f41e8efdc3f4eb123e0ca37b184db2::buck::BUCK";

        // 调用 find_opportunity 寻找套利机会
        let arb_res = arb
            .find_opportunity(
                sender,
                coin_type,
                None, // 不指定特定 pool_id
                gas_coins,
                sim_ctx.clone(), // 克隆 sim_ctx
                true, // 使用 GSS
                Source::Public, // 公开来源
            )
            .await
            .unwrap(); // `unwrap()` 用于测试，如果出错测试会panic

        // 记录找到的最佳交易路径信息
        info!(?arb_res, "找到的最佳交易路径"); // ?arb_res 使用Debug格式打印

        // --- (可选) 使用不同的模拟器进行验证 ---
        // 这部分代码展示了如何使用不同的模拟器 (例如基于数据库的 DBSimulator)
        // 来验证由 HttpSimulator 找到的交易路径的有效性。
        info!("正在创建数据库模拟器 (DBSimulator)...");
        // 创建一个DBSimulator实例 (可能用于更精确或不同视角的模拟)
        let db_sim: Arc<dyn Simulator> = Arc::new(DBSimulator::new_default_slow().await);
        info!("数据库模拟器创建耗时: {:?}", start.elapsed());

        let tx_data = arb_res.tx_data; // 获取之前找到的套利交易数据
        // 重新创建一个HTTP模拟器 (也可以复用之前的，但这里显式创建用于对比)
        let http_sim: Arc<dyn Simulator> = Arc::new(HttpSimulator::new(TEST_HTTP_URL, &None).await);

        // 使用HTTP模拟器再次模拟交易
        let http_res = http_sim.simulate(tx_data.clone(), sim_ctx.clone()).await.unwrap();
        info!(?http_res, "🧀 HTTP模拟器执行结果"); // 🧀 表情符号可能是为了醒目

        // 使用数据库模拟器模拟交易
        let db_res = db_sim.simulate(tx_data, sim_ctx).await.unwrap();
        info!(?db_res, "🧀 数据库模拟器执行结果");

        // 在实际测试中，这里可能还会有断言 (assertions) 来检查 http_res 和 db_res
        // 是否符合预期，例如利润是否为正，交易是否成功等。
        // assert!(http_res.is_ok());
        // assert!(db_res.is_ok());
        // assert!(arb_res.best_trial_result.profit > 0);
    }
}
