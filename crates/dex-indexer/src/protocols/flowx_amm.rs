// 该文件 `flowx_amm.rs` (位于 `dex-indexer` crate 的 `protocols` 子模块下)
// 负责处理与 FlowX AMM (自动做市商) DEX 协议相关的事件和数据结构。
// FlowX Finance 同时提供 CLMM 池和传统的 AMM 池，此文件专注于其 AMM 部分。
//
// **文件顶注解释 (Explanation of the Top-level Comment)**:
// `//! There is no `pool_id` in the PairCreated events, so we can't backfill pools for FlowX AMM.`
// 这句注释指出 FlowX AMM 的 `PairCreated` (池创建) 事件中**不包含**直接的 `pool_id` (池对象ID)。
// 这意味着，仅仅通过监听 `PairCreated` 事件，索引器无法直接知道新创建的池的ID是什么。
// 这对“回填历史数据”(backfilling) 造成了困难，因为回填通常需要知道池ID才能去链上查询池的详细信息。
// 不过，从代码实现来看，`FlowxAmmPoolCreated::TryFrom<&SuiEvent>` 中，
// `pool_id` 是从事件的 `parsed_json["pair"]` 字段中获取的。这表明 `PairCreated` 事件的JSON负载中
// 确实有一个名为 `pair` 的字段可以作为池ID。
// 因此，顶注可能是指早期版本的情况，或者是指 `PairCreated` 事件的泛型参数或直接字段中没有 `pool_id`，
// 而必须通过解析 `parsed_json` 才能得到。如果 `parsed_json["pair"]` 可靠，则回填是可能的。
//
// **文件概览 (File Overview)**:
// 这个文件是 `dex-indexer` 项目中专门用来“认识”和“翻译” FlowX AMM 这个DEX发生的两种主要“事件”的：
// “有人创建了一个新的交易池 (`PairCreated`)” 和 “有人在这个池子里完成了一笔代币交换 (`Swapped`)”。
//
// **主要工作 (Main Tasks)**:
// 1.  **识别FlowX AMM的特定事件**:
//     -   `FLOWX_AMM_POOL_CREATED` 和 `FLOWX_AMM_SWAP_EVENT` 常量是这两种事件在Sui链上的“专属门牌号”。
//     -   `flowx_amm_event_filter()` 函数生成一个“过滤器”，告诉Sui节点只订阅FlowX AMM创建新池子的事件。
//
// 2.  **`FlowxAmmPoolCreated` 结构体 (新池子信息卡)**:
//     -   当监听到FlowX AMM新池子创建事件 (`PairCreated`) 时，这个结构体记录事件信息（池ID `pair`、两种代币类型 `coin_x`, `coin_y`）。
//     -   `TryFrom<&SuiEvent>` 是“填卡员”，从原始Sui事件中提取数据。
//         注意：事件中的代币类型 (`coin_x`, `coin_y`) 需要手动添加 "0x" 前缀。
//     -   `to_pool()` 方法将这张FlowX AMM专用的“信息卡”转换为通用的 `Pool` 结构。
//         这个方法还会去链上读取池对象的 `fee_rate` (手续费率) 字段，并将其存储在 `PoolExtra::FlowxAmm` 中。
//
// 3.  **`FlowxAmmSwapEvent` 结构体 (交换记录卡)**:
//     -   记录FlowX AMM交换事件 (`Swapped`) 的详细信息（输入输出代币、输入输出金额）。
//         与BlueMove类似，它也从 `coin_x`, `coin_y`, `amount_x_in`, `amount_x_out`, `amount_y_in`, `amount_y_out`
//         这些字段中判断实际的交易方向。
//     -   `TryFrom` 实现能从不同来源（`SuiEvent`, `ShioEvent`, 原始JSON `Value`）提取信息。
//     -   `to_swap_event()` 方法将其转换为通用的 `SwapEvent`。
//         一个值得注意的细节是，在 `to_swap_event` 中，`pool` 字段被设置为 `None`。
//         这可能意味着FlowX AMM的 `Swapped` 事件本身不直接包含池的ObjectID，或者在当前转换逻辑中没有传递这个信息。
//         如果需要池ID，可能需要从其他上下文（如事件的 `package_id` 或 `sender`）或通过关联 `PoolCreated` 事件来获取。
//
// **Sui区块链和DeFi相关的概念解释 (Sui Blockchain and DeFi-related Concepts)**:
// (与 abex.rs, cetus.rs 等文件中的解释类似，主要涉及DEX、事件、事件类型、JSON解析、代币类型规范化等。)
//
// -   **AMM (自动做市商 / Automated Market Maker)**:
//     FlowX AMM 采用的是传统的自动做市商模型，通常基于类似 `x*y=k` 的常数乘积或其他数学公式来确定价格和执行交换。
//     这与CLMM（集中流动性）模型不同，AMM的流动性通常分布在整个价格曲线上。
//
// -   **手续费率 (`fee_rate`)**:
//     FlowX AMM的池对象在链上存储了一个 `fee_rate` 字段。这个字段的值代表了在该池进行交易时，
//     交易者需要支付的手续费比例。例如，一个 `fee_rate` 值30可能代表0.03% (如果基数是100,000) 或0.3% (如果基数是10,000)。
//     具体换算关系取决于协议的定义。`FlowxAmmPoolCreated::to_pool()` 方法会读取这个值。

// 顶注：FlowX AMM的PairCreated事件中没有pool_id，所以不能回填池子。
// (Top-level comment: There is no `pool_id` in the PairCreated events, so we can't backfill pools for FlowX AMM.)
// (实际代码中 `parsed_json["pair"]` 被用作pool_id，所以此注释可能指早期版本或特定上下文。)
// (In actual code, `parsed_json["pair"]` is used as pool_id, so this comment might refer to an earlier version or specific context.)

// 引入 eyre 库，用于更方便的错误处理和上下文管理。
use eyre::{ensure, eyre, OptionExt, Result};
// 引入 serde 库的 Deserialize trait，用于从如JSON这样的格式反序列化数据。
use serde::Deserialize;
// 引入 serde_json 的 Value 类型，用于表示通用的JSON值。
use serde_json::Value;
// 引入 shio 库的 ShioEvent 类型，可能用于处理与MEV相关的特定事件。
use shio::ShioEvent;
// 引入 Sui SDK 中的相关类型。
use sui_sdk::{
    rpc_types::{EventFilter, SuiData, SuiEvent, SuiObjectDataOptions}, // 事件过滤器, Sui数据容器, Sui事件, 对象数据选项。
    types::base_types::ObjectID,      // ObjectID是Sui对象的唯一标识符。
    SuiClient,                        // Sui RPC客户端，用于与Sui网络交互。
};

// 从当前crate的父模块 (protocols) 引入 get_coin_decimals 函数。
use super::get_coin_decimals;
// 从当前crate的根模块引入 normalize_coin_type (规范化代币类型字符串) 函数和相关类型定义。
use crate::{
    normalize_coin_type,
    types::{Pool, PoolExtra, Protocol, SwapEvent, Token}, // 通用池结构, 协议特定附加信息, Protocol枚举, 通用交换事件, 代币信息结构。
};

/// `FLOWX_AMM_POOL_CREATED` 常量
///
/// 定义了FlowX AMM协议在Sui链上发出的“新交易对创建”事件的全局唯一类型字符串。
/// 注意：FlowX AMM称其为 `PairCreated` 而不是 `PoolCreated`。
pub const FLOWX_AMM_POOL_CREATED: &str =
    "0xba153169476e8c3114962261d1edc70de5ad9781b83cc617ecc8c1923191cae0::factory::PairCreated";

/// `FLOWX_AMM_SWAP_EVENT` 常量
///
/// 定义了FlowX AMM协议在Sui链上发出的“代币交换完成”事件的全局唯一类型字符串。
pub const FLOWX_AMM_SWAP_EVENT: &str =
    "0xba153169476e8c3114962261d1edc70de5ad9781b83cc617ecc8c1923191cae0::pair::Swapped";

/// `flowx_amm_event_filter` 函数
///
/// 创建并返回一个 `EventFilter`，专门用于订阅FlowX AMM的“新交易对创建”事件。
pub fn flowx_amm_event_filter() -> EventFilter {
    EventFilter::MoveEventType(FLOWX_AMM_POOL_CREATED.parse().unwrap()) // 解析类型字符串为StructTag
}

/// `FlowxAmmPoolCreated` 结构体
///
/// 用于存储从FlowX AMM的 `PairCreated` 事件中解析出来的具体信息。
#[derive(Debug, Clone, Deserialize)]
pub struct FlowxAmmPoolCreated {
    pub pool: ObjectID,   // 新创建的交易对（池）的ObjectID (从 "pair" 字段获取)
    pub token0: String, // 池中第一个代币 (CoinX) 的类型字符串 (已添加 "0x" 前缀)
    pub token1: String, // 池中第二个代币 (CoinY) 的类型字符串 (已添加 "0x" 前缀)
}

/// 为 `FlowxAmmPoolCreated` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for FlowxAmmPoolCreated {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        let parsed_json = &event.parsed_json; // 获取事件的JSON数据部分
        // 从JSON中提取 "pair" 字段作为池ID，并解析为 ObjectID。
        let pool_id_str = parsed_json["pair"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxAmmPoolCreated事件JSON中缺少'pair'字段 (作为pool_id)"))?;
        let pool_object_id = ObjectID::from_str(pool_id_str)?;

        // 从JSON中提取 "coin_x" 字段作为token0。
        // FlowX AMM事件中的代币类型可能不带 "0x" 前缀，这里统一添加。
        let token0_raw_str = parsed_json["coin_x"].as_str().ok_or_else(|| eyre!("FlowxAmmPoolCreated事件JSON中缺少'coin_x'字段"))?;
        let token0_formatted_str = format!("0x{}", token0_raw_str);

        // 从JSON中提取 "coin_y" 字段作为token1。
        let token1_raw_str = parsed_json["coin_y"].as_str().ok_or_else(|| eyre!("FlowxAmmPoolCreated事件JSON中缺少'coin_y'字段"))?;
        let token1_formatted_str = format!("0x{}", token1_raw_str);

        Ok(Self {
            pool: pool_object_id,
            token0: token0_formatted_str,
            token1: token1_formatted_str,
        })
    }
}

impl FlowxAmmPoolCreated {
    /// `to_pool` 异步方法
    ///
    /// 将 `FlowxAmmPoolCreated` 事件数据转换为通用的 `Pool` 结构。
    /// 此方法会异步查询代币精度，并从链上对象获取池的 `fee_rate`。
    pub async fn to_pool(&self, sui: &SuiClient) -> Result<Pool> {
        // 异步获取token0和token1的精度信息
        let token0_decimals = get_coin_decimals(sui, &self.token0).await?;
        let token1_decimals = get_coin_decimals(sui, &self.token1).await?;

        // 设置获取对象数据时的选项，需要对象的内容 (content) 来提取 fee_rate。
        let object_data_options = SuiObjectDataOptions::default().with_content();

        // 从链上获取池对象的详细数据
        let pool_sui_object_data = sui
            .read_api()
            .get_object_with_options(self.pool, object_data_options)
            .await?
            .data // 获取RpcResult中的SuiObjectResponse
            .ok_or_else(|| eyre!("FlowX AMM池对象 {} 在链上未找到或无数据", self.pool))?;

        // 从池对象的Move内容中提取 "fee_rate" 字段。
        // `fee_rate` 是一个u64值，代表手续费率。
        let fee_rate_val: u64 = pool_sui_object_data
            .content
            .ok_or_else(|| eyre!("FlowX AMM池对象 {} 没有内容字段", self.pool))?
            .try_into_move()
            .ok_or_else(|| eyre!("FlowX AMM池对象 {} 的内容不是有效的MoveObject", self.pool))?
            .fields
            .field_value("fee_rate") // 按名称查找 "fee_rate" 字段
            .ok_or_else(|| eyre!("FlowX AMM池对象 {} 中未找到'fee_rate'字段", self.pool))?
            .to_string() // 将MoveValue转换为字符串
            .parse()?; // 将字符串解析为u64

        // 创建 Token 结构列表
        let tokens_vec = vec![
            Token::new(&self.token0, token0_decimals),
            Token::new(&self.token1, token1_decimals),
        ];
        // 创建 PoolExtra::FlowxAmm 枚举成员，存储FlowX AMM特定的附加信息 (手续费率)
        let extra_data = PoolExtra::FlowxAmm { fee_rate: fee_rate_val };

        Ok(Pool {
            protocol: Protocol::FlowxAmm, // 指明协议为FlowxAmm
            pool: self.pool,             // 池的ObjectID
            tokens: tokens_vec,          // 池中代币列表
            extra: extra_data,           // 协议特定附加信息
        })
    }
}

/// `FlowxAmmSwapEvent` 结构体
///
/// 用于存储从FlowX AMM的 `Swapped` 事件中解析出来的具体交换信息。
#[derive(Debug, Clone, Deserialize)]
pub struct FlowxAmmSwapEvent {
    pub coin_in: String,    // 输入代币的规范化类型字符串
    pub coin_out: String,   // 输出代币的规范化类型字符串
    pub amount_in: u64,     // 输入代币的数量
    pub amount_out: u64,    // 输出代币的数量
}

/// 为 `FlowxAmmSwapEvent` 实现 `TryFrom<&SuiEvent>` trait。
impl TryFrom<&SuiEvent> for FlowxAmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &SuiEvent) -> Result<Self> {
        // 确保事件类型与 `FLOWX_AMM_SWAP_EVENT` 匹配。
        ensure!(
            event.type_.to_string() == FLOWX_AMM_SWAP_EVENT, // 注意：FlowX AMM的Swapped事件似乎没有泛型参数直接在类型字符串中
            "事件类型不匹配FlowX AMM SwappedEvent (Not a FlowxAmmSwapEvent)"
        );
        // 直接尝试从事件的 `parsed_json` 字段转换。
        (&event.parsed_json).try_into()
    }
}

/// 为 `FlowxAmmSwapEvent` 实现 `TryFrom<&ShioEvent>` trait。
impl TryFrom<&ShioEvent> for FlowxAmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(event: &ShioEvent) -> Result<Self> {
        ensure!(event.event_type == FLOWX_AMM_SWAP_EVENT, "事件类型不匹配FlowX AMM SwappedEvent (Not a FlowxAmmSwapEvent)");
        event.parsed_json.as_ref().ok_or_eyre("ShioEvent中缺少parsed_json字段 (Missing parsed_json in ShioEvent)")?.try_into()
    }
}

/// 为 `FlowxAmmSwapEvent` 实现 `TryFrom<&Value>` trait (从 `serde_json::Value` 转换)。
/// 这是实际的JSON解析逻辑。
/// FlowX AMM的 `Swapped` 事件JSON结构包含 `coin_x`, `coin_y`, `amount_x_in`, `amount_x_out`, `amount_y_in`, `amount_y_out` 字段。
/// 需要通过判断哪个输入金额 (`amount_x_in` 或 `amount_y_in`) 大于0来确定实际的交易方向。
impl TryFrom<&Value> for FlowxAmmSwapEvent {
    type Error = eyre::Error;

    fn try_from(parsed_json: &Value) -> Result<Self> {
        // 从JSON中提取池中两种基础代币的类型 (coin_x, coin_y)。
        let coin_x_raw_str = parsed_json["coin_x"].as_str().ok_or_else(|| eyre!("FlowxAmmSwapEvent JSON中缺少'coin_x'字段"))?;
        let normalized_coin_x = normalize_coin_type(format!("0x{}", coin_x_raw_str).as_str());

        let coin_y_raw_str = parsed_json["coin_y"].as_str().ok_or_else(|| eyre!("FlowxAmmSwapEvent JSON中缺少'coin_y'字段"))?;
        let normalized_coin_y = normalize_coin_type(format!("0x{}", coin_y_raw_str).as_str());

        // 提取与代币X和代币Y相关的输入输出金额。
        let amount_x_in_val: u64 = parsed_json["amount_x_in"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxAmmSwapEvent JSON中缺少'amount_x_in'字段"))?
            .parse()?;
        let amount_x_out_val: u64 = parsed_json["amount_x_out"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxAmmSwapEvent JSON中缺少'amount_x_out'字段"))?
            .parse()?;
        let amount_y_in_val: u64 = parsed_json["amount_y_in"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxAmmSwapEvent JSON中缺少'amount_y_in'字段"))?
            .parse()?;
        let amount_y_out_val: u64 = parsed_json["amount_y_out"]
            .as_str()
            .ok_or_else(|| eyre!("FlowxAmmSwapEvent JSON中缺少'amount_y_out'字段"))?
            .parse()?;

        // 判断实际的交易方向和金额。
        // 如果 amount_x_in > 0，则输入的是代币X，输出的是代币Y。
        let (final_coin_in, final_coin_out) = if amount_x_in_val > 0 {
            (normalized_coin_x, normalized_coin_y)
        } else {
            // 否则，输入的是代币Y，输出的是代币X。
            (normalized_coin_y, normalized_coin_x)
        };

        let (final_amount_in, final_amount_out) = if amount_x_in_val > 0 {
            (amount_x_in_val, amount_y_out_val)
        } else {
            (amount_y_in_val, amount_x_out_val)
        };

        Ok(Self {
            coin_in: final_coin_in,
            coin_out: final_coin_out,
            amount_in: final_amount_in,
            amount_out: final_amount_out,
        })
    }
}

impl FlowxAmmSwapEvent {
    /// `to_swap_event` 异步方法
    ///
    /// 将 `FlowxAmmSwapEvent` 转换为通用的 `SwapEvent` 枚举成员。
    /// 注意：此事件不直接包含池ID，因此转换后的 `SwapEvent.pool` 为 `None`。
    pub async fn to_swap_event(&self) -> Result<SwapEvent> {
        Ok(SwapEvent {
            protocol: Protocol::FlowxAmm, // 指明协议为FlowxAmm
            pool: None,                   // FlowX AMM的Swapped事件不直接提供池ID
            coins_in: vec![self.coin_in.clone()],
            coins_out: vec![self.coin_out.clone()],
            amounts_in: vec![self.amount_in],
            amounts_out: vec![self.amount_out],
        })
    }
}

[end of crates/dex-indexer/src/protocols/flowx_amm.rs]
