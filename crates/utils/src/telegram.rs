// 该文件 `telegram.rs` (位于 `utils` crate中) 定义了一系列与Telegram消息发送相关的常量。
// 这些常量主要用于存储Telegram机器人的API令牌 (Token) 以及不同Telegram聊天或话题 (Thread) 的ID。
// 在应用程序中（例如套利机器人），当需要发送通知、警报或状态更新到Telegram时，会使用这些常量来指定
// 通过哪个机器人发送以及发送到哪个聊天或话题。
//
// **文件概览 (File Overview)**:
// 这个文件是 `utils` 工具库中的“Telegram通讯录”。
// 它列出了一些重要的“联系方式”，方便程序的其他部分给指定的Telegram用户或群组发送消息。
//
// **核心内容 (Key Contents)**:
// -   **机器人令牌 (Bot Token)**:
//     -   `R2D2_TELEGRAM_BOT_TOKEN`: 这可能是一个特定Telegram机器人的API令牌。
//         每个Telegram机器人都有一个由BotFather (Telegram官方的机器人管理机器人) 生成的唯一令牌，
//         应用程序使用此令牌来授权自己通过该机器人发送消息。
//         "R2D2" 可能是一个内部代号或机器人名称。
//
// -   **聊天/群组ID (Chat/Group IDs)**:
//     -   `CHAT_MONEY_PRINTER`: 这可能是一个Telegram群组或频道的ID，用于接收与“赚钱”（Money Printer，通常指盈利的套利操作）相关的通知。
//         ID可以是数字（对于私有群组/频道）或以 "@" 开头的用户名（对于公开群组/频道）。
//
// -   **话题/线程ID (Topic/Thread IDs)**:
//     Telegram群组可以开启“话题”（Topics/Threads）功能，允许在同一个群组内创建多个子聊天分区，使讨论更有条理。
//     如果目标群组启用了话题，发送消息时可以指定一个 `message_thread_id` 来将消息发送到特定的话题中。
//     -   `CHAT_MONEY_PRINTER_THREAD_ERROR_REPORT`: 可能指向 `CHAT_MONEY_PRINTER` 群组内一个专门用于报告程序错误或问题的“错误报告”话题。
//     -   `CHAT_MONEY_PRINTER_THREAD_WALLET_WATCHER`: 可能指向一个用于接收“钱包监控”（例如，重要地址的余额变动、大额交易等）相关通知的话题。
//     -   `CHAT_MONEY_PRINTER_THREAD_TEST`: 可能指向一个用于发送测试消息或进行机器人功能测试的话题。
//
// -   **特定用户聊天ID (Specific User Chat ID)**:
//     -   `BIG_DICK_BOY_CHAT_ID`: 这很可能是一个特定Telegram用户的个人聊天ID，或者一个私有聊天的ID。
//         "BIG_DICK_BOY" 是一个占位符或内部戏称，实际使用时会替换为真实的、用于接收某些特殊或私人通知的用户ID。
//
// **重要提示 (IMPORTANT NOTE)**:
// **所有这些常量的值在当前代码中都被设置为空字符串 `""`。**
// 这是出于安全和隐私考虑，避免将真实的API令牌和聊天ID硬编码到公开的代码库中。
// 在实际部署和使用此应用程序之前，开发者**必须**：
// 1.  创建一个或获取现有Telegram机器人的API令牌，并替换 `R2D2_TELEGRAM_BOT_TOKEN` 的值。
// 2.  获取目标Telegram群组/频道/用户的正确聊天ID和话题ID，并替换其他相应常量的值。
// 这些值通常通过与Telegram机器人API交互或在Telegram客户端中查找来获得。
// 如果这些常量未被正确配置，任何尝试通过Telegram发送消息的功能都将失败。

/// `R2D2_TELEGRAM_BOT_TOKEN` 常量
///
/// 用于授权应用程序通过名为 "R2D2" (或类似代号) 的Telegram机器人发送消息的API令牌。
/// **注意**: 此值必须由开发者在部署前替换为真实的机器人Token。
pub const R2D2_TELEGRAM_BOT_TOKEN: &str = "";

/// `CHAT_MONEY_PRINTER` 常量
///
/// 目标Telegram聊天或群组的ID，用于接收与盈利操作 ("Money Printer") 相关的通知。
/// **注意**: 此值必须由开发者在部署前替换为真实的聊天ID。
pub const CHAT_MONEY_PRINTER: &str = "";

/// `CHAT_MONEY_PRINTER_THREAD_ERROR_REPORT` 常量
///
/// 在 `CHAT_MONEY_PRINTER` 群组中，用于接收错误报告的特定话题（Thread）的ID。
/// **注意**: 如果群组未开启话题功能，或不需要发送到特定话题，此值可能为空或不使用。
/// 如果使用，必须替换为真实的话题ID。
pub const CHAT_MONEY_PRINTER_THREAD_ERROR_REPORT: &str = "";

/// `CHAT_MONEY_PRINTER_THREAD_WALLET_WATCHER` 常量
///
/// 在 `CHAT_MONEY_PRINTER` 群组中，用于接收钱包监控相关通知的特定话题（Thread）的ID。
/// **注意**: 同上，按需配置。
pub const CHAT_MONEY_PRINTER_THREAD_WALLET_WATCHER: &str = "";

/// `CHAT_MONEY_PRINTER_THREAD_TEST` 常量
///
/// 在 `CHAT_MONEY_PRINTER` 群组中，用于接收测试消息的特定话题（Thread）的ID。
/// **注意**: 同上，按需配置。
pub const CHAT_MONEY_PRINTER_THREAD_TEST: &str = "";

/// `BIG_DICK_BOY_CHAT_ID` 常量
///
/// 一个特定用户的Telegram聊天ID，可能用于接收私人或重要的直接通知。
/// "BIG_DICK_BOY" 是一个占位符名称。
/// **注意**: 此值必须由开发者在部署前替换为真实的个人用户或私聊的聊天ID。
pub const BIG_DICK_BOY_CHAT_ID: &str = "";

[end of crates/utils/src/telegram.rs]
