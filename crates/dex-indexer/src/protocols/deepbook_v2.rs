// 该文件 `deepbook_v2.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 Sui 官方的 DeepBook V2 协议相关的事件和数据结构。
// DeepBook V2 是一个基于中央限价订单簿 (CLOB) 的去中心化交易所。
// 这个文件的主要功能是：
// 1. 定义 DeepBook V2 "池创建" (`PoolCreated`) 事件的类型字符串常量。
// 2. 提供 `deepbook_v2_event_filter()` 函数，用于创建Sui事件订阅的过滤器，专门监听DeepBook V2的池创建事件。
// 3. 定义 `DeepbookV2PoolCreated` 结构体，用于存储从链上 `PoolCreated` 事件解析出来的具体订单簿（池）创建信息，
//    例如池ID、基础资产类型 (base_asset)、报价资产类型 (quote_asset)、吃单手续费率 (taker_fee_rate)、
//    挂单返利费率 (maker_rebate_rate)、价格精度 (tick_size) 和数量精度 (lot_size)。
// 4. 实现 `TryFrom<&SuiEvent>` for `DeepbookV2PoolCreated`，使得可以从通用的 `SuiEvent`
//    转换为特定于 DeepBook V2 的 `DeepbookV2PoolCreated` 结构。
// 5. `DeepbookV2PoolCreated::to_pool()` 方法，将解析出的事件数据转换为一个更通用的 `Pool` 结构，
//    这个 `Pool` 结构是 `dex-indexer` 用来统一表示不同DEX协议池信息的内部标准格式。
//    它会异步查询基础资产和报价资产的代币精度，并将DeepBook特有的参数（如费率、tick/lot size）存储在 `PoolExtra::DeepbookV2` 中。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” Sui 官方的 DeepBook V2 这个订单簿交易所发生的“新订单簿创建事件”的。
// DeepBook V2 和AMM（自动做市商）不一样，它更像传统的股票交易所，买家和卖家通过提交限价单来买卖。
// 每个交易对（比如SUI/USDC）都有一个自己的订单簿，这个文件就是处理当一个新的交易对（即一个新的订单簿或“池子”）被创建时的事件。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别DeepBook V2的新池创建事件**:
//     -   `DEEPBOOK_V2_POOL_CREATED` 常量定义了这种事件在Sui链上特有的“名字”。
//     -   `deepbook_v2_event_filter()` 函数会生成一个“过滤器”，告诉Sui节点：“我只对DeepBook V2创建新订单簿（池子）的事件感兴趣。”
//
// 2.  **`DeepbookV2PoolCreated` 结构体 (新订单簿信息卡)**:
//     -   当监听到一个新的DeepBook V2订单簿被创建的事件时，这个结构体就负责像填一张“新订单簿信息卡”一样，
//         把事件里的重要信息（比如订单簿的ID、基础资产是什么、报价资产是什么、交易手续费怎么收、价格和数量的最小变动单位是多少等）都记录下来。
//     -   `TryFrom<&SuiEvent>` 部分就是“填卡员”，负责从原始的Sui事件中读取数据并填到这张卡上。
//         注意：DeepBook事件中，资产类型可能是通过 `name` 字段给出的，而不是直接的类型字符串，所以需要特殊处理并加上 "0x" 前缀。
//
// 3.  **`DeepbookV2PoolCreated::to_pool()` (统一格式)**:
//     -   这个方法把DeepBook V2专用的“信息卡” (`DeepbookV2PoolCreated`)，转换成一张我们程序内部所有DEX池子（包括订单簿和AMM池）通用的标准“池子信息表”（`Pool`结构）。
//     -   在转换过程中，它还会：
//         -   去链上查询基础资产和报价资产这两种代币分别是几位小数的（精度）。
//         -   把DeepBook特有的参数（吃单手续费率、挂单返利费率、tick_size, lot_size）存到一个叫做 `PoolExtra::DeepbookV2` 的附加信息区里。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
//
// -   **中央限价订单簿 (CLOB / Central Limit Order Book)**:
//     一种交易机制，买卖双方提交带有特定价格和数量的订单。买单（bids）和卖单（asks）按价格优先、时间优先的原则排列在订单簿上。
//     当一个买单的价格高于或等于一个卖单的价格时，就会发生撮合成交。
//     DeepBook V2 是Sui上的官方CLOB实现。
//
// -   **基础资产 (Base Asset) / 报价资产 (Quote Asset)**:
//     在交易对中，例如 BTC/USD，BTC是基础资产，USD是报价资产。价格表示为一个单位的基础资产值多少报价资产。
//     DeepBook V2的 `PoolCreated` 事件会明确指出哪个是基础资产，哪个是报价资产。
//
// -   **吃单手续费率 (Taker Fee Rate)**:
//     当一个用户提交的订单（通常是市价单，或能立即与订单簿上现有订单成交的限价单）“吃掉”了订单簿上的流动性时，
//     该用户需要支付的手续费的费率。
//
// -   **挂单返利费率 (Maker Rebate Rate)**:
//     当一个用户提交的限价单进入订单簿并且没有立即成交，而是为市场“提供”了流动性时，
//     如果这个订单最终被其他人的“吃单”成交了，那么这个“挂单”的用户可能会收到一部分费用返还（rebate）作为奖励。
//     这个费率就是返还的比例。有些交易所可能会设置负的挂单费率，即直接给挂单者补贴。
//
// -   **Tick Size (价格精度 / 最小价格变动单位)**:
//     订单簿上允许的最小价格差异。例如，如果tick size是0.01，那么价格可以是1.00, 1.01, 1.02，但不能是1.005。
//     它决定了订单簿上价格档位的密集程度。
//
// -   **Lot Size (数量精度 / 最小数量变动单位)**:
//     订单簿上允许的最小交易数量（或其倍数）。例如，如果lot size是100，那么你可以下单买卖100, 200, 300个单位的资产，但不能是150个。
//     它决定了订单簿上数量的粒度。

// 引入 eyre 库，用于更方便的错误处理和上下文管理。
use eyre::{eyre, Result};
// 引入 serde 库的 Deserialize trait，用于从如JSON这样的格式反序列化数据。
use serde::Deserialize;
// 引入 Sui SDK 中的相关类型。
use sui_sdk::{
    rpc_types::{EventFilter, SuiEvent}, // EventFilter用于订阅事件，SuiEvent代表链上事件。
    types::base_types::ObjectID,      // ObjectID是Sui对象的唯一标识符。
    SuiClient,                        // Sui RPC客户端，用于与Sui网络交互。
};

// 从当前crate的父模块 (protocols) 引入 get_coin_decimals 函数。
use super::get_coin_decimals;
// 从当前crate的根模块引入 Pool, PoolExtra, Protocol, Token 类型定义。
use crate::types::{Pool, PoolExtra, Protocol, Token};

/// `DEEPBOOK_V2_POOL_CREATED` 常量
///
/// 定义了DeepBook V2协议在Sui链上发出的“新池创建”（即新订单簿创建）事件的全局唯一类型字符串。
pub const DEEPBOOK_V2_POOL_CREATED: &str = "0xdee9::clob_v2::PoolCreated";

/// `deepbook_v2_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，专门用于订阅DeepBook V2的“新池创建”事件。
pub fn deepbook_v2_event_filter() -> EventFilter {
    EventFilter::MoveEventType(DEEPBOOK_V2_POOL_CREATED.parse().unwrap()) // 解析类型字符串为StructTag
}

/// `DeepbookV2PoolCreated` 结构体
///
/// 用于存储从DeepBook V2的 `PoolCreated` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)] // 自动派生常用trait
pub struct DeepbookV2PoolCreated {
    #[serde(rename = "pool_id")] // 将JSON中的 "pool_id" 字段映射到此结构体的 `pool` 字段
    pub pool: ObjectID,        // 新创建的订单簿（池）的ObjectID
    pub base_asset: String,    // 基础资产的类型字符串 (已添加 "0x" 前缀)
    pub quote_asset: String,   // 报价资产的类型字符串 (已添加 "0x" 前缀)
    pub taker_fee_rate: u64,   // 吃单手续费率 (通常是一个大整数，需要根据协议规范换算)
    pub maker_rebate_rate: u64,// 挂单返利费率 (同上)
    pub tick_size: u64,        // 价格的最小变动单位 (价格精度)
    pub lot_size: u64,         // 数量的最小变动单位 (数量精度)
}

/// 为 `DeepbookV2PoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
/// 这使得可以尝试将一个通用的 `&SuiEvent` 引用转换为 `DeepbookV2PoolCreated`。
impl TryFrom<&SuiEvent> for DeepbookV2PoolCreated {
    type Error = eyre::Error; // 定义转换失败时返回的错误类型

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let parsed_json = &event.parsed_json; // 获取事件的JSON数据部分

        // 从JSON中提取 "pool_id" 字段，并解析为 ObjectID。
        let pool_id_str = parsed_json["pool_id"]
            .as_str()
            .ok_or_else(|| eyre!("DeepBookV2PoolCreated事件JSON中缺少'pool_id'字段"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 从JSON中提取基础资产类型。DeepBook事件中资产类型在 "name" 字段下。
        let base_asset_obj = parsed_json["base_asset"]
            .as_object() // "base_asset" 本身是一个JSON对象
            .ok_or_else(|| eyre!("DeepBookV2PoolCreated事件JSON中缺少'base_asset'对象"))?;
        let base_asset_name_str = base_asset_obj["name"] // 从该对象中取 "name" 字段
            .as_str()
            .ok_or_else(|| eyre!("'base_asset'对象中缺少'name'字段"))?;
        let base_asset_formatted = format!("0x{}", base_asset_name_str); // 添加 "0x" 前缀

        // 类似地提取报价资产类型。
        let quote_asset_obj = parsed_json["quote_asset"]
            .as_object()
            .ok_or_else(|| eyre!("DeepBookV2PoolCreated事件JSON中缺少'quote_asset'对象"))?;
        let quote_asset_name_str = quote_asset_obj["name"]
            .as_str()
            .ok_or_else(|| eyre!("'quote_asset'对象中缺少'name'字段"))?;
        let quote_asset_formatted = format!("0x{}", quote_asset_name_str);

        // 从JSON中提取各种费率和精度参数 (taker_fee_rate, maker_rebate_rate, tick_size, lot_size)。
        // 这些值在JSON中通常是字符串形式的数字，需要先转为字符串再解析为u64。
        let taker_fee_rate_val: u64 = parsed_json["taker_fee_rate"]
            .as_str()
            .ok_or_else(|| eyre!("DeepBookV2PoolCreated事件JSON中缺少'taker_fee_rate'字段"))?
            .parse()?; // 将字符串解析为u64

        let maker_rebate_rate_val: u64 = parsed_json["maker_rebate_rate"]
            .as_str()
            .ok_or_else(|| eyre!("DeepBookV2PoolCreated事件JSON中缺少'maker_rebate_rate'字段"))?
            .parse()?;

        let tick_size_val: u64 = parsed_json["tick_size"]
            .as_str()
            .ok_or_else(|| eyre!("DeepBookV2PoolCreated事件JSON中缺少'tick_size'字段"))?
            .parse()?;

        let lot_size_val: u64 = parsed_json["lot_size"]
            .as_str()
            .ok_or_else(|| eyre!("DeepBookV2PoolCreated事件JSON中缺少'lot_size'字段"))?
            .parse()?;

        // 返回构造好的 DeepbookV2PoolCreated 实例
        Ok(Self {
            pool: pool_object_id,
            base_asset: base_asset_formatted,
            quote_asset: quote_asset_formatted,
            taker_fee_rate: taker_fee_rate_val,
            maker_rebate_rate: maker_rebate_rate_val,
            tick_size: tick_size_val,
            lot_size: lot_size_val,
        })
    }
}

impl DeepbookV2PoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `DeepbookV2PoolCreated` 事件数据转换为通用的 `Pool` 结构。
    /// 这个过程需要异步查询基础资产和报价资产的代币精度。
    ///
    /// 参数:
    /// - `sui`: 一个对 `SuiClient` 的引用，用于查询代币精度。
    ///
    /// 返回:
    /// - `Result<Pool>`: 转换后的 `Pool` 对象。
    pub async fn to_pool(&self, sui: &SuiClient) -> Result<Pool> {
        // 异步获取基础资产和报价资产的精度信息
        let base_asset_decimals = get_coin_decimals(sui, &self.base_asset).await?;
        let quote_asset_decimals = get_coin_decimals(sui, &self.quote_asset).await?;

        // 创建 Token 结构列表，DeepBook V2的池总是包含基础资产和报价资产。
        // 顺序通常是 (基础资产, 报价资产)。
        let tokens_vec = vec![
            Token::new(&self.base_asset, base_asset_decimals),
            Token::new(&self.quote_asset, quote_asset_decimals),
        ];

        // 创建 PoolExtra::DeepbookV2 枚举成员，存储DeepBook V2特有的附加信息
        let extra_data = PoolExtra::DeepbookV2 {
            taker_fee_rate: self.taker_fee_rate,
            maker_rebate_rate: self.maker_rebate_rate,
            tick_size: self.tick_size,
            lot_size: self.lot_size,
        };

        // 返回构造好的 Pool 对象
        Ok(Pool {
            protocol: Protocol::DeepbookV2, // 指明协议为DeepbookV2
            pool: self.pool,               // 订单簿（池）的ObjectID
            tokens: tokens_vec,            // 池中代币列表 (基础资产, 报价资产)
            extra: extra_data,             // 协议特定附加信息
        })
    }
}

[end of crates/dex-indexer/src/protocols/deepbook_v2.rs]
