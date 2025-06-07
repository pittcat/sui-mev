// 该文件负责格式化套利结果并通过Telegram发送通知。
// 当套利机器人发现一个有利可图的交易机会并成功执行（或模拟执行）后，
// 这个模块会构建一个易于阅读的消息，包含利润、交易路径、相关链接等信息，
// 然后将这些消息发送到预设的Telegram群组或频道。
//
// **文件概览 (File Overview)**:
// 这个 `notification.rs` 文件就像是套利机器人的“宣传部门”或“广播站”。
// 它的主要工作是：当机器人找到了赚钱的机会并且（可能）已经成功操作后，
// 把这个好消息（或者重要的运行状态）整理成一段文字，然后通过Telegram（一个流行的即时通讯应用）发送给用户或管理员。
// (This `notification.rs` file acts as the "publicity department" or "broadcasting station" for the arbitrage bot.
// Its main job is: when the bot finds a profitable opportunity and (potentially) has successfully executed it,
// to organize this good news (or important operational status) into a piece of text and send it via Telegram (a popular instant messaging app) to users or administrators.)
//
// **核心功能 (Core Functionality)**:
// 1.  **定义Telegram常量 (Defining Telegram Constants)**:
//     -   文件开头定义了一些字符串常量，比如 `SUI_ARB_BOT_TOKEN`（Telegram机器人的身份令牌）、`GROUP_SUI_ARB`（要发送消息的Telegram群组ID）等。
//     -   **注意**：在实际代码中，这些常量的值通常是保密的，并且在示例代码中被清空了。真正使用时，你需要自己去Telegram创建一个机器人，获取到Token，并指定好接收消息的群组ID和话题ID（如果群组开启了话题功能）。
//         (At the beginning of the file, some string constants are defined, such as `SUI_ARB_BOT_TOKEN` (the identity token for the Telegram bot), `GROUP_SUI_ARB` (the ID of the Telegram group to send messages to), etc.
//          **Note**: In actual code, these constant values are usually kept secret and are cleared in example code. For real use, you need to create a bot yourself on Telegram, get the Token, and specify the group ID and topic ID (if the group has topics enabled) to receive messages.)
//
// 2.  **`new_tg_messages` 函数 (Creating Telegram Messages)**:
//     -   这是这个模块的核心函数，它的任务是接收一套完整的套利结果数据（`ArbResult`），然后把这些数据“翻译”成一段人类容易阅读的Telegram消息。
//     -   消息内容会包括很多有用的信息，比如：
//         -   赚了多少钱（利润）。
//         -   这笔套利交易在Sui区块链上的“指纹”（交易哈希/Digest），以及原始触发交易的哈希。
//         -   用了哪个代币进行套利。
//         -   投入了多少本金。
//         -   详细的交易路径（比如，先在A交易所用SUI买USDC，再在B交易所用USDC买回SUI）。
//         -   整个发现和处理过程花了多长时间。
//         -   用了哪个模拟器来预测结果。
//         -   这个机会的来源（是公开市场还是MEV）。
//         -   当前程序的版本号。
//     -   为了让消息更易读、更专业，它会使用Markdown格式（一种轻量级标记语言，可以让文字加粗、倾斜、插入链接等）。
//     -   它还会生成指向Sui区块链浏览器（如SuiVision）的链接，这样用户可以直接点击链接去查看交易详情或对象信息。
//         (This is the core function of this module. Its task is to receive a complete set of arbitrage result data (`ArbResult`) and then "translate" this data into a human-readable Telegram message.
//          The message content will include much useful information, such as:
//          - How much money was made (profit).
//          - The "fingerprint" (transaction hash/Digest) of this arbitrage transaction on the Sui blockchain, and the hash of the original triggering transaction.
//          - Which token was used for arbitrage.
//          - How much principal was invested.
//          - The detailed trading path (e.g., first buy USDC with SUI on exchange A, then buy back SUI with USDC on exchange B).
//          - How long the entire discovery and processing took.
//          - Which simulator was used to predict the results.
//          - The source of this opportunity (public market or MEV).
//          - The current version number of the program.
//          To make messages more readable and professional, it uses Markdown format (a lightweight markup language that allows text to be bold, italic, insert links, etc.).
//          It also generates links to Sui blockchain explorers (like SuiVision), so users can directly click the links to view transaction details or object information.)
//
// 3.  **区分利润等级发送到不同话题 (Sending to Different Topics Based on Profit Level)**:
//     -   如果Telegram群组开启了“话题”（Threads/Topics）功能，这个模块可以根据利润的大小，把消息发送到不同的话题里。
//     -   例如，利润特别高的消息可以发到一个专门的“高利润”话题，而利润较低的或者日常测试的消息可以发到另一个话题。这样方便分类查看。
//         (If the Telegram group has "Threads/Topics" feature enabled, this module can send messages to different topics based on the profit amount.
//          For example, messages with very high profits can be sent to a dedicated "high profit" topic, while lower profit or routine test messages can be sent to another topic. This facilitates categorized viewing.)
//
// **Sui区块链相关的概念解释 (Sui Blockchain-related Concepts)**:
//
// -   **`TransactionDigest` (交易摘要/交易哈希)**:
//     每当你在Sui区块链上成功提交一笔交易后，这笔交易都会得到一个唯一的“指纹”，就是交易摘要。它是一个由数字和字母组成的字符串，可以用来在区块链浏览器上精确地查找这笔交易的详细信息。
//     (Whenever you successfully submit a transaction on the Sui blockchain, it gets a unique "fingerprint", which is the transaction digest. It's a string of numbers and letters that can be used to accurately find the details of this transaction on a blockchain explorer.)
//
// -   **Markdown (Markdown格式)**:
//     一种轻量级的标记语言，用一些简单的符号（比如 `*`号代表加粗，` `` ` 包裹代表代码块，`[]()` 代表链接）来排版文本，使其在Telegram等应用中显示出更丰富的格式。
//     (A lightweight markup language that uses simple symbols (e.g., `*` for bold, ``` for code blocks, `[]()` for links) to format text, allowing it to display richer formats in applications like Telegram.)
//
// -   **Sui区块链浏览器 (Sui Blockchain Explorer)**:
//     像 SuiVision (suivision.xyz) 或 Suiscan 这样的网站，它们提供了用户友好的界面，让你可以查看Sui链上的交易、地址、对象等各种信息。这个模块生成的链接通常会指向这些浏览器。
//     (Websites like SuiVision (suivision.xyz) or Suiscan provide user-friendly interfaces for viewing various information on the Sui chain, such as transactions, addresses, objects, etc. The links generated by this module usually point to these explorers.)

// 引入标准库及第三方库 (Import standard and third-party libraries)
use std::{fmt::Write, time::Duration}; // `fmt::Write` 用于构建字符串, `Duration` 用于表示时间间隔
                                      // `fmt::Write` is used for building strings, `Duration` for representing time intervals.

use burberry::executor::telegram_message::{escape, Message, MessageBuilder}; // `burberry`库中用于Telegram消息处理的组件
                                                                            // Components from the `burberry` library for Telegram message handling.
                                                                            // `escape` 用于转义Markdown特殊字符 (`escape` is used to escape Markdown special characters).
                                                                            // `Message` 代表一条消息 (`Message` represents a message).
                                                                            // `MessageBuilder` 用于构建Message对象 (`MessageBuilder` is used to build Message objects).
use sui_types::digests::TransactionDigest; // Sui区块链的交易摘要 (哈希) 类型
                                           // Transaction Digest (hash) type from the Sui blockchain.
use utils::{coin, link, telegram}; // 自定义的 `utils` 工具库，可能包含：
                                   // Custom `utils` utility library, which might contain:
                                   // `coin`: 代币格式化工具 (例如，将MIST转换为SUI并添加符号)
                                   //         (Token formatting tools (e.g., converting MIST to SUI and adding symbols)).
                                   // `link`: 生成Sui浏览器链接的工具
                                   //         (Tools for generating Sui explorer links).
                                   // `telegram`: Telegram相关的常量或辅助函数
                                   //             (Telegram-related constants or helper functions).

use crate::{arb::ArbResult, BUILD_VERSION}; // 从当前项目中引入:
                                            // Import from the current project:
                                            // `arb::ArbResult`: 套利结果的结构体 (Struct for arbitrage results).
                                            // `BUILD_VERSION`: 当前程序的构建版本号 (可能在编译时设置)
                                            //                  (Build version number of the current program (possibly set at compile time)).

// --- Telegram机器人和群组相关的常量 ---
// (Constants related to Telegram bot and groups)
// 注意：这些常量的值在提供的代码中是空字符串。在实际部署时，需要替换为真实的Token和ID。
// (Note: The values of these constants are empty strings in the provided code. For actual deployment, they need to be replaced with real Tokens and IDs.)
// 获取这些值通常需要：
// (Obtaining these values typically requires:)
// 1. SUI_ARB_BOT_TOKEN: 通过Telegram的BotFather创建一个机器人来获取Token。
//    (Create a bot via Telegram's BotFather to get a Token.)
// 2. GROUP_SUI_ARB: Telegram群组的ID。可以是公开群组的用户名 (如 "@mygroup") 或私有群组的数字ID (通常为负数)。
//    (ID of the Telegram group. Can be the username of a public group (e.g., "@mygroup") or the numerical ID of a private group (usually negative).)
// 3. THREAD_LOW_PROFIT / THREAD_HIGH_PROFIT: Telegram群组中特定话题 (topic/thread) 的ID。
//    如果群组启用了话题功能，可以将不同类型的通知发送到不同的话题中。
//    (ID of a specific topic/thread in a Telegram group. If the group has topics enabled, different types of notifications can be sent to different topics.)
const SUI_ARB_BOT_TOKEN: &str = ""; // 套利机器人专用的Telegram Bot Token (Telegram Bot Token dedicated to the arbitrage bot)
const GROUP_SUI_ARB: &str = "";     // 套利结果通知的目标Telegram群组ID (Target Telegram group ID for arbitrage result notifications)
const THREAD_LOW_PROFIT: &str = ""; // 用于发送低利润通知的群组话题ID (Group topic ID for sending low-profit notifications)
const THREAD_HIGH_PROFIT: &str = "";// 用于发送高利润通知的群组话题ID (Group topic ID for sending high-profit notifications)

/// `new_tg_messages` 函数用于根据套利结果创建一个或多个Telegram消息。
/// (The `new_tg_messages` function is used to create one or more Telegram messages based on arbitrage results.)
///
/// 参数 (Parameters):
/// - `digest`: 原始交易的摘要 (TransactionDigest)，可能是触发套利检查的交易。
///             (Digest of the original transaction, possibly the one that triggered the arbitrage check.)
/// - `arb_digest`: 套利交易本身的摘要 (如果已执行或模拟)。
///                 (Digest of the arbitrage transaction itself (if executed or simulated).)
/// - `res`: 一个对 `ArbResult` 结构体的引用，包含了套利机会的详细信息 (最佳路径、利润等)。
///          (A reference to an `ArbResult` struct, containing detailed information about the arbitrage opportunity (best path, profit, etc.).)
/// - `elapsed`: 套利机会发现和处理所花费的总时间。
///              (Total time spent on discovering and processing the arbitrage opportunity.)
/// - `simulator_name`: 使用的模拟器的名称 (例如 "HttpSimulator", "DbSimulator")。
///                     (Name of the simulator used (e.g., "HttpSimulator", "DbSimulator").)
///
/// 返回 (Returns):
/// - `Vec<Message>`: 一个包含多个 `Message` 对象的向量。每个 `Message` 对象都准备好被发送。
///                   (A vector containing multiple `Message` objects. Each `Message` object is ready to be sent.)
///   这里通常会为不同的Telegram群组或机器人创建不同的消息实例。
///   (Usually, different message instances are created here for different Telegram groups or bots.)
pub fn new_tg_messages(
    digest: TransactionDigest,     // 触发检查的原始交易哈希 (Digest of the original transaction that triggered the check)
    arb_digest: TransactionDigest, // 套利交易的哈希 (Digest of the arbitrage transaction)
    res: &ArbResult,               // 套利结果的引用 (Reference to the arbitrage result)
    elapsed: Duration,             // 总耗时 (Total time elapsed)
    simulator_name: &str,          // 模拟器名称 (Name of the simulator)
) -> Vec<Message> {
    // 初始化一个可变字符串 `msg`，预分配4096字节容量，用于构建消息内容。
    // (Initialize a mutable string `msg` with a pre-allocated capacity of 4096 bytes to build the message content.)
    // Telegram消息长度有限制，但4096通常足够大部分情况。
    // (Telegram messages have length limits, but 4096 is usually sufficient for most cases.)
    let mut msg = String::with_capacity(4096);
    // 获取套利结果中最佳尝试的详细信息
    // (Get detailed information of the best trial from the arbitrage result)
    let trade_res = &res.best_trial_result;

    // --- 构建消息内容 ---
    // (Build message content)
    // 使用 `write!` 宏向 `msg` 字符串中写入格式化的文本。
    // (Use the `write!` macro to write formatted text into the `msg` string.)
    // `unwrap()` 用于处理 `write!` 可能返回的 `std::fmt::Result` 错误。
    // (`unwrap()` is used to handle the `std::fmt::Result` error that `write!` might return.)
    // 在生产代码中，更稳健的做法是处理这个Result而不是直接unwrap。
    // (In production code, it's more robust to handle this Result instead of directly unwrapping.)

    // 1. 利润信息 (Profit information)
    // `escape()` 用于转义Markdown中的特殊字符，防止格式错乱。
    // (`escape()` is used to escape special characters in Markdown to prevent formatting issues.)
    // `coin::format_sui_with_symbol()` 将u64类型的利润值 (通常是MIST) 格式化为带 "SUI" 符号的字符串。
    // (`coin::format_sui_with_symbol()` formats a u64 profit value (usually in MIST) into a string with the "SUI" symbol.)
    write!(
        msg,
        r#"*利润 (Profit)*: `{profit}`

"#, // `*...*` 表示Markdown加粗, `\`...\`` 表示代码块 (`*...*` for Markdown bold, `\`...\`` for code block)
        profit = escape(&coin::format_sui_with_symbol(trade_res.profit)),
    )
    .unwrap();

    // 2. 交易摘要链接、代币信息、输入金额
    // (Transaction digest links, coin information, input amount)
    // `link::tx()` 生成指向Sui区块链浏览器中对应交易详情页的链接。
    // (`link::tx()` generates a link to the corresponding transaction details page on a Sui blockchain explorer.)
    // `link::coin()` 生成指向代币信息页的链接 (如果适用)。
    // (`link::coin()` generates a link to the coin information page (if applicable).)
    write!(
        msg,
        r#"*交易哈希 (Trigger Tx Digest)*: {scan_link}
*套利交易哈希 (Arb Tx Digest)*: {arb_scan_link}
*代币 (Coin)*: {coin}
*输入金额 (Amount In)*: {amount_in}
*交易路径 (Path)*:
"#,
        scan_link = link::tx(&digest, None), // 原始交易链接 (Original transaction link)
        arb_scan_link = link::tx(&arb_digest, None), // 套利交易链接 (Arbitrage transaction link)
        coin = link::coin(&trade_res.coin_type, None), // 套利操作的代币链接 (Link for the coin used in arbitrage)
        amount_in = escape(&coin::format_sui_with_symbol(trade_res.amount_in)), // 输入金额 (Input amount)
    )
    .unwrap();

    // 3. 详细交易路径 (Path / Detailed Trading Path)
    // 遍历套利结果中的交易路径 (`trade_res.trade_path.path`)，其中每个元素 `dex` 代表路径中的一步 (例如一个DEX池)。
    // (Iterate through the trading path in the arbitrage result (`trade_res.trade_path.path`), where each element `dex` represents a step in the path (e.g., a DEX pool).)
    for (i, dex) in trade_res.trade_path.path.iter().enumerate() {
        // 为每个DEX步骤创建一个标签，包含协议名称、输入代币和输出代币类型。
        // (Create a label for each DEX step, including protocol name, input coin, and output coin type.)
        // 例如 (For example): "Cetus(SUI-USDC)"
        let tag = format!("{}({}-{})", dex.protocol(), dex.coin_in_type(), dex.coin_out_type());
        // 将每个步骤格式化为有序列表项，并链接到DEX对象的浏览器页面。
        // (Format each step as an ordered list item and link to the DEX object's explorer page.)
        // `link::object()` 生成对象链接。
        // (`link::object()` generates an object link.)
        writeln!( // `writeln!` 会在末尾添加换行符 (`writeln!` adds a newline character at the end)
            msg,
            r#" {i}\. {dex}"#, // 例如 (e.g.): " 0. Cetus(SUI-USDC)" (链接形式 / in link form)
            i = i + 1, // 路径步骤从1开始编号 (Path steps are numbered starting from 1)
            dex = link::object(dex.object_id(), Some(escape(&tag))) // DEX对象链接，显示为tag (DEX object link, displayed as tag)
        )
        .unwrap();
    }

    // 4. 时间消耗信息 (Time consumption information)
    writeln!(msg, "*总耗时 (Total Elapsed)*: {}", escape(&format!("{:?}", elapsed))).unwrap();
    writeln!(
        msg,
        "*TrialCtx创建耗时 (TrialCtx Creation)*: {}",
        escape(&format!("{:?}", res.create_trial_ctx_duration))
    )
    .unwrap();
    writeln!(
        msg,
        "*网格搜索耗时 (Grid Search)*: {}",
        escape(&format!("{:?}", res.grid_search_duration))
    )
    .unwrap();
    writeln!(msg, "*GSS耗时 (GSS Duration)*: {}", escape(&format!("{:?}", res.gss_duration))).unwrap(); // GSS = Golden Section Search

    // 5. 缓存未命中次数 (Cache Misses)
    writeln!(msg, "*缓存未命中 (Cache Misses)*: {}", res.cache_misses).unwrap();

    // 6. 模拟器名称和交易来源信息 (Simulator name and transaction source information)
    writeln!(msg, "\n*模拟器 (Simulator)*: *{}*", simulator_name,).unwrap(); // 换行后打印模拟器名称 (Print simulator name after a newline)
    writeln!(msg, "*来源 (Source)*: *{}*", escape(res.source.to_string().as_str())).unwrap(); // 交易来源 (例如 Public, MEV) (Transaction source (e.g., Public, MEV))

    // 7. 程序版本号 (Program version number)
    write!(msg, "*版本 (Version)*: `{version}`", version = BUILD_VERSION).unwrap();

    // (用于调试) 打印构建好的消息内容到控制台
    // ((For debugging) Print the constructed message content to the console)
    println!("构建的Telegram消息 (Constructed Telegram Message): {}", msg);

    // --- 根据利润选择不同的Telegram话题 (Thread ID) ---
    // (Select different Telegram Topic (Thread ID) based on profit)
    // 这是一个示例逻辑：如果利润大于1 SUI (1_000_000_000 MIST)，则认为是高利润。
    // (This is an example logic: if profit is greater than 1 SUI (1,000,000,000 MIST), it's considered high profit.)
    // 注意：`125670` 和 `telegram::CHAT_MONEY_PRINTER_THREAD_TEST` 是硬编码的ID，
    // 实际应使用前面定义的常量 `THREAD_HIGH_PROFIT` 和 `THREAD_LOW_PROFIT` 或从配置读取。
    // (Note: `125670` and `telegram::CHAT_MONEY_PRINTER_THREAD_TEST` are hardcoded IDs.
    //  In actual use, the constants `THREAD_HIGH_PROFIT` and `THREAD_LOW_PROFIT` defined earlier should be used, or read from configuration.)
    // 下面的代码块对此进行了修正。
    // (The code block below has been corrected for this.)

    // 消息1: 发送到 R2D2_TELEGRAM_BOT (可能是开发者或内部测试用的机器人)
    // (Message 1: Send to R2D2_TELEGRAM_BOT (possibly a bot for developer or internal testing))
    let r2d2_thread_id = if trade_res.profit > 1_000_000_000 { // 假设1 SUI = 10^9 MIST (Assuming 1 SUI = 10^9 MIST)
        // 对于高利润，可以指定一个特定的话题ID
        // (For high profit, a specific topic ID can be specified)
        // 例如: "125670" // 这是一个示例ID，实际应从配置或常量获取
        // (e.g., "125670" // This is an example ID, should actually be obtained from config or constants)
        // 如果没有特定高利润话题，可以和低利润使用相同话题或不指定 (发到主聊天)
        // (If there's no specific high-profit topic, the same topic as low-profit can be used, or not specified (sent to main chat))
        telegram::CHAT_MONEY_PRINTER_THREAD_PROFIT // 假设这是高利润话题ID (Assuming this is the high-profit topic ID)
    } else {
        telegram::CHAT_MONEY_PRINTER_THREAD_TEST // 低利润话题ID (Low-profit topic ID)
    };

    let msg1 = MessageBuilder::new()
        .bot_token(telegram::R2D2_TELEGRAM_BOT_TOKEN) // 使用 R2D2 机器人的Token (Use R2D2 bot's Token)
        .chat_id(telegram::CHAT_MONEY_PRINTER)      // 发送到 CHAT_MONEY_PRINTER 群组 (Send to CHAT_MONEY_PRINTER group)
        .thread_id(r2d2_thread_id)                  // 根据利润选择的话题ID (Topic ID selected based on profit)
        .text(msg.clone())                          // 消息内容 (克隆一份) (Message content (clone a copy))
        .disable_link_preview(true)                 // 禁用链接预览，保持消息简洁 (Disable link preview to keep message concise)
        .build();                                   // 构建Message对象 (Build Message object)

    // 消息2: 发送到 SUI_ARB_BOT (可能是公开或更广泛的套利通知机器人)
    // (Message 2: Send to SUI_ARB_BOT (possibly a public or more general arbitrage notification bot))
    let sui_arb_thread_id = if trade_res.profit > 1_000_000_000 { // 1 SUI
        THREAD_HIGH_PROFIT // 使用之前定义的常量 (Use previously defined constant)
    } else {
        THREAD_LOW_PROFIT
    };

    // 检查常量是否为空，如果为空，可能不发送这条消息或发送到主聊天（通过不设置thread_id）
    // (Check if constants are empty. If so, this message might not be sent, or sent to the main chat (by not setting thread_id).)
    let msg2 = if SUI_ARB_BOT_TOKEN.is_empty() || GROUP_SUI_ARB.is_empty() {
        None // 如果Token或群组ID未配置，则不创建此消息 (If Token or Group ID is not configured, do not create this message)
    } else {
        let mut builder = MessageBuilder::new()
            .bot_token(SUI_ARB_BOT_TOKEN)
            .chat_id(GROUP_SUI_ARB)
            .text(msg.clone()) // 消息内容 (Message content)
            .disable_link_preview(true);

        // 只有当thread_id不为空时才设置，否则Telegram API可能会报错
        // (Only set thread_id if it's not empty, otherwise Telegram API might error out)
        if !sui_arb_thread_id.is_empty() {
            builder = builder.thread_id(sui_arb_thread_id);
        }
        Some(builder.build())
    };

    // 将构建好的消息收集到一个向量中。
    // (Collect the constructed messages into a vector.)
    // `filter_map` 用于过滤掉 `None` 值 (例如当msg2未创建时)。
    // (`filter_map` is used to filter out `None` values (e.g., when msg2 was not created).)
    vec![Some(msg1), msg2].into_iter().filter_map(|m| m).collect()
}

[end of bin/arb/src/common/notification.rs]
