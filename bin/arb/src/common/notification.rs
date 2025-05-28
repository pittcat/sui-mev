// 该文件负责格式化套利结果并通过Telegram发送通知。
// 当套利机器人发现一个有利可图的交易机会并成功执行（或模拟执行）后，
// 这个模块会构建一个易于阅读的消息，包含利润、交易路径、相关链接等信息，
// 然后将这些消息发送到预设的Telegram群组或频道。
//
// 文件概览:
// 1. 定义了一些常量，如Telegram机器人的Token、群组ID和不同利润等级的话题ID。
//    (注意：代码中的实际Token和ID已被清空，实际使用时需要填入真实值。)
// 2. 实现了一个核心函数 `new_tg_messages`，它接收套利结果和相关信息，
//    然后生成一个或多个 `Message` 对象 (来自 `burberry` 库) 以便发送。
// 3. 消息内容会根据利润大小发送到不同的Telegram话题 (thread) 中，
//    例如高利润和低利润的消息可以分开处理。
// 4. 消息格式使用Markdown，并包含指向Sui浏览器（如SuiVision）的链接，方便用户查看详情。

// 引入标准库及第三方库
use std::{fmt::Write, time::Duration}; // `fmt::Write` 用于构建字符串, `Duration` 用于表示时间间隔

use burberry::executor::telegram_message::{escape, Message, MessageBuilder}; // `burberry`库中用于Telegram消息处理的组件
                                                                            // `escape` 用于转义Markdown特殊字符
                                                                            // `Message` 代表一条消息
                                                                            // `MessageBuilder` 用于构建Message对象
use sui_types::digests::TransactionDigest; // Sui区块链的交易摘要 (哈希) 类型
use utils::{coin, link, telegram}; // 自定义的 `utils` 工具库，可能包含：
                                   // `coin`: 代币格式化工具 (例如，将MIST转换为SUI并添加符号)
                                   // `link`: 生成Sui浏览器链接的工具
                                   // `telegram`: Telegram相关的常量或辅助函数

use crate::{arb::ArbResult, BUILD_VERSION}; // 从当前项目中引入:
                                            // `arb::ArbResult`: 套利结果的结构体
                                            // `BUILD_VERSION`: 当前程序的构建版本号 (可能在编译时设置)

// --- Telegram机器人和群组相关的常量 ---
// 注意：这些常量的值在提供的代码中是空字符串。在实际部署时，需要替换为真实的Token和ID。
// 获取这些值通常需要：
// 1. SUI_ARB_BOT_TOKEN: 通过Telegram的BotFather创建一个机器人来获取Token。
// 2. GROUP_SUI_ARB: Telegram群组的ID。可以是公开群组的用户名 (如 "@mygroup") 或私有群组的数字ID (通常为负数)。
// 3. THREAD_LOW_PROFIT / THREAD_HIGH_PROFIT: Telegram群组中特定话题 (topic/thread) 的ID。
//    如果群组启用了话题功能，可以将不同类型的通知发送到不同的话题中。
const SUI_ARB_BOT_TOKEN: &str = ""; // 套利机器人专用的Telegram Bot Token
const GROUP_SUI_ARB: &str = "";     // 套利结果通知的目标Telegram群组ID
const THREAD_LOW_PROFIT: &str = ""; // 用于发送低利润通知的群组话题ID
const THREAD_HIGH_PROFIT: &str = "";// 用于发送高利润通知的群组话题ID

/// `new_tg_messages` 函数用于根据套利结果创建一个或多个Telegram消息。
///
/// 参数:
/// - `digest`: 原始交易的摘要 (TransactionDigest)，可能是触发套利检查的交易。
/// - `arb_digest`: 套利交易本身的摘要 (如果已执行或模拟)。
/// - `res`: 一个对 `ArbResult` 结构体的引用，包含了套利机会的详细信息 (最佳路径、利润等)。
/// - `elapsed`: 套利机会发现和处理所花费的总时间。
/// - `simulator_name`: 使用的模拟器的名称 (例如 "HttpSimulator", "DbSimulator")。
///
/// 返回:
/// - `Vec<Message>`: 一个包含多个 `Message` 对象的向量。每个 `Message` 对象都准备好被发送。
///   这里通常会为不同的Telegram群组或机器人创建不同的消息实例。
pub fn new_tg_messages(
    digest: TransactionDigest,     // 触发检查的原始交易哈希
    arb_digest: TransactionDigest, // 套利交易的哈希
    res: &ArbResult,               // 套利结果的引用
    elapsed: Duration,             // 总耗时
    simulator_name: &str,          // 模拟器名称
) -> Vec<Message> {
    // 初始化一个可变字符串 `msg`，预分配4096字节容量，用于构建消息内容。
    // Telegram消息长度有限制，但4096通常足够大部分情况。
    let mut msg = String::with_capacity(4096);
    // 获取套利结果中最佳尝试的详细信息
    let trade_res = &res.best_trial_result;

    // --- 构建消息内容 ---
    // 使用 `write!` 宏向 `msg` 字符串中写入格式化的文本。
    // `unwrap()` 用于处理 `write!` 可能返回的 `std::fmt::Result` 错误。
    // 在生产代码中，更稳健的做法是处理这个Result而不是直接unwrap。

    // 1. 利润信息
    // `escape()` 用于转义Markdown中的特殊字符，防止格式错乱。
    // `coin::format_sui_with_symbol()` 将u64类型的利润值 (通常是MIST) 格式化为带 "SUI" 符号的字符串。
    write!(
        msg,
        r#"*利润*: `{profit}`

"#, // `*...*` 表示Markdown加粗, `\`...\`` 表示代码块
        profit = escape(&coin::format_sui_with_symbol(trade_res.profit)),
    )
    .unwrap();

    // 2. 交易摘要链接、代币信息、输入金额
    // `link::tx()` 生成指向Sui区块链浏览器中对应交易详情页的链接。
    // `link::coin()` 生成指向代币信息页的链接 (如果适用)。
    write!(
        msg,
        r#"*交易哈希 (Digest)*: {scan_link}
*套利交易哈希 (Arb Digest)*: {arb_scan_link}
*代币 (Coin)*: {coin}
*输入金额 (Amount In)*: {amount_in}
*交易路径 (Path)*:
"#,
        scan_link = link::tx(&digest, None), // 原始交易链接
        arb_scan_link = link::tx(&arb_digest, None), // 套利交易链接
        coin = link::coin(&trade_res.coin_type, None), // 套利操作的代币链接
        amount_in = escape(&coin::format_sui_with_symbol(trade_res.amount_in)), // 输入金额
    )
    .unwrap();

    // 3. 详细交易路径 (Path)
    // 遍历套利结果中的交易路径 (`trade_res.trade_path.path`)，其中每个元素 `dex` 代表路径中的一步 (例如一个DEX池)。
    for (i, dex) in trade_res.trade_path.path.iter().enumerate() {
        // 为每个DEX步骤创建一个标签，包含协议名称、输入代币和输出代币类型。
        // 例如: "Cetus(SUI-USDC)"
        let tag = format!("{}({}-{})", dex.protocol(), dex.coin_in_type(), dex.coin_out_type());
        // 将每个步骤格式化为有序列表项，并链接到DEX对象的浏览器页面。
        // `link::object()` 生成对象链接。
        writeln!( // `writeln!` 会在末尾添加换行符
            msg,
            r#" {i}\. {dex}"#, // 例如: " 0. Cetus(SUI-USDC)" (链接形式)
            i = i + 1, // 路径步骤从1开始编号
            dex = link::object(dex.object_id(), Some(escape(&tag))) // DEX对象链接，显示为tag
        )
        .unwrap();
    }

    // 4. 时间消耗信息
    writeln!(msg, "*总耗时 (Elapsed)*: {}", escape(&format!("{:?}", elapsed))).unwrap();
    writeln!(
        msg,
        "*TrialCtx创建耗时*: {}",
        escape(&format!("{:?}", res.create_trial_ctx_duration))
    )
    .unwrap();
    writeln!(
        msg,
        "*网格搜索耗时 (Grid Search)*: {}",
        escape(&format!("{:?}", res.grid_search_duration))
    )
    .unwrap();
    writeln!(msg, "*GSS耗时*: {}", escape(&format!("{:?}", res.gss_duration))).unwrap(); // GSS = Golden Section Search

    // 5. 缓存未命中次数
    writeln!(msg, "*缓存未命中 (Cache Misses)*: {}", res.cache_misses).unwrap();

    // 6. 模拟器名称和交易来源信息
    writeln!(msg, "\n*{}*", simulator_name,).unwrap(); // 换行后打印模拟器名称
    writeln!(msg, "*{}*", escape(res.source.to_string().as_str())).unwrap(); // 交易来源 (例如 Public, MEV)

    // 7. 程序版本号
    write!(msg, "*版本 (Version)*: `{version}`", version = BUILD_VERSION).unwrap();

    // (用于调试) 打印构建好的消息内容到控制台
    println!("构建的Telegram消息: {}", msg);

    // --- 根据利润选择不同的Telegram话题 (Thread ID) ---
    // 这是一个示例逻辑：如果利润大于1 SUI (1_000_000_000 MIST)，则认为是高利润。
    // 注意：`125670` 和 `telegram::CHAT_MONEY_PRINTER_THREAD_TEST` 是硬编码的ID，
    // 实际应使用前面定义的常量 `THREAD_HIGH_PROFIT` 和 `THREAD_LOW_PROFIT` 或从配置读取。
    // 下面的代码块对此进行了修正。

    // 消息1: 发送到 R2D2_TELEGRAM_BOT (可能是开发者或内部测试用的机器人)
    let r2d2_thread_id = if trade_res.profit > 1_000_000_000 { // 假设1 SUI = 10^9 MIST
        // 对于高利润，可以指定一个特定的话题ID
        // 例如: "125670" // 这是一个示例ID，实际应从配置或常量获取
        // 如果没有特定高利润话题，可以和低利润使用相同话题或不指定 (发到主聊天)
        telegram::CHAT_MONEY_PRINTER_THREAD_PROFIT // 假设这是高利润话题ID
    } else {
        telegram::CHAT_MONEY_PRINTER_THREAD_TEST // 低利润话题ID
    };

    let msg1 = MessageBuilder::new()
        .bot_token(telegram::R2D2_TELEGRAM_BOT_TOKEN) // 使用 R2D2 机器人的Token
        .chat_id(telegram::CHAT_MONEY_PRINTER)      // 发送到 CHAT_MONEY_PRINTER 群组
        .thread_id(r2d2_thread_id)                  // 根据利润选择的话题ID
        .text(msg.clone())                          // 消息内容 (克隆一份)
        .disable_link_preview(true)                 // 禁用链接预览，保持消息简洁
        .build();                                   // 构建Message对象

    // 消息2: 发送到 SUI_ARB_BOT (可能是公开或更广泛的套利通知机器人)
    let sui_arb_thread_id = if trade_res.profit > 1_000_000_000 { // 1 SUI
        THREAD_HIGH_PROFIT // 使用之前定义的常量
    } else {
        THREAD_LOW_PROFIT
    };

    // 检查常量是否为空，如果为空，可能不发送这条消息或发送到主聊天（通过不设置thread_id）
    let msg2 = if SUI_ARB_BOT_TOKEN.is_empty() || GROUP_SUI_ARB.is_empty() {
        None // 如果Token或群组ID未配置，则不创建此消息
    } else {
        let mut builder = MessageBuilder::new()
            .bot_token(SUI_ARB_BOT_TOKEN)
            .chat_id(GROUP_SUI_ARB)
            .text(msg.clone()) // 消息内容
            .disable_link_preview(true);
        
        // 只有当thread_id不为空时才设置，否则Telegram API可能会报错
        if !sui_arb_thread_id.is_empty() {
            builder = builder.thread_id(sui_arb_thread_id);
        }
        Some(builder.build())
    };
    
    // 将构建好的消息收集到一个向量中。
    // `filter_map` 用于过滤掉 `None` 值 (例如当msg2未创建时)。
    vec![Some(msg1), msg2].into_iter().filter_map(|m| m).collect()
}
