// 该文件 `shio.rs` 实现了与 Shio 协议交互的逻辑。
// 从函数名 `submit_bid` 和模块名 `auctioneer` (拍卖师) 来看，Shio 协议似乎与某种拍卖机制相关，
// 很可能用于 MEV (Miner Extractable Value, 矿工可提取价值) 场景下的优先权竞价 (Priority Gas Auction - PGA) 或其他类型的拍卖。
// 用户（例如套利机器人）可以通过 `submit_bid` 函数提交一个出价 (bid)，以期获得某些优势，
// 例如交易的优先打包权或从某个机会中捕获价值。
//
// 文件概览:
// 1. 定义了与 Shio 协议相关的常量，主要是其合约包ID (`SHIO`)。
// 2. `GLOBAL_STATES`: 一个静态的 `OnceCell<Vec<ObjectArg>>`，用于缓存一组 Shio "全局状态" 对象的 `ObjectArg`。
//    Shio 协议可能将其状态分布在多个对象上，或者提供多个入口点与之交互。
//    `shio::SHIO_GLOBAL_STATES` (从外部 `shio` crate 导入) 似乎预定义了这些状态对象的ID和版本。
// 3. `Shio` 结构体: 代表与 Shio 拍卖协议交互的实例。
//    - 它持有一组 `global_states` (作为 `ObjectArg`)。
//    - `state_idx`: 一个原子计数器 (`AtomicUsize`)，用于轮流选择使用哪个 `global_state` 对象进行下一次出价。
//      这种轮询机制可能用于负载均衡或避免单个状态对象的瓶颈。
// 4. `new()` 方法: `Shio` 结构体的构造函数，负责异步初始化并缓存 `global_states`。
// 5. `submit_bid()` 方法: 将提交出价的操作添加到现有的交易上下文 (`TradeCtx`) 中。
//    它会调用 Shio 合约的 `auctioneer::submit_bid` 函数。
// 6. `next_state()` 方法: 一个私有辅助函数，用于从 `global_states` 列表中轮询选择下一个状态对象。
//
// Sui/MEV概念:
// - MEV (Miner Extractable Value): 矿工（或在PoS中为验证者）通过其在交易排序、打包和区块提议过程中的能力，
//   可以提取的超出标准区块奖励和交易手续费的额外价值。套利、清算、抢先交易 (front-running) 等都是MEV的来源。
// - PGA (Priority Gas Auction): 优先Gas拍卖。一种常见的MEV机制，用户通过支付更高的Gas价格（或直接出价）
//   来竞争其交易被优先打包的权利。Shio协议可能实现了类似PGA的机制。
// - Auctioneer (拍卖师): 在Shio合约中，`auctioneer` 模块可能负责管理拍卖逻辑，如接收出价、确定赢家等。
// - Global States (全局状态对象): Shio协议可能将其核心状态（如当前的最高出价、拍卖轮次等）存储在一个或多个全局共享对象中。
//   使用多个状态对象并轮询访问它们，可能是为了提高并发处理能力或扩展性。

// 引入标准库及第三方库
use std::{
    str::FromStr, // FromStr用于从字符串转换 (例如ObjectID)
    sync::{atomic::AtomicUsize, Arc}, // AtomicUsize用于线程安全的原子计数器, Arc原子引用计数
};

use eyre::{ensure, eyre, Result}; // 错误处理库
use shio::SHIO_GLOBAL_STATES; // 从外部 `shio` crate 导入预定义的全局状态对象ID和版本列表
                             // `shio` crate 可能是 Shio 协议官方提供的客户端库或常量定义。
use sui_sdk::SUI_COIN_TYPE; // SUI原生代币的类型字符串 ("0x2::sui::SUI")
use sui_types::{
    base_types::{ObjectID, SequenceNumber}, // Sui对象ID, 对象版本号 (SequenceNumber)
    transaction::{Argument, Command, ObjectArg}, // Sui交易构建相关类型
    Identifier, TypeTag, // Sui标识符 (用于模块名、函数名), 类型标签
};
use tokio::sync::OnceCell; // Tokio异步单次初始化单元，用于全局缓存 `GLOBAL_STATES`

use super::TradeCtx; // 从父模块(defi)引入 `TradeCtx` (交易上下文)

// Shio协议核心合约包ID (Package ID)
const SHIO: &str = "0x1889977f0fb56ae730e7bda8e8e32859ce78874458c74910d36121a81a615123";

// 用于缓存 Shio 全局状态对象列表 (作为 ObjectArg) 的静态 `OnceCell`。
// `OnceCell` 确保这些状态对象只被从 `SHIO_GLOBAL_STATES` 初始化一次。
static GLOBAL_STATES: OnceCell<Vec<ObjectArg>> = OnceCell::const_new();

/// `Shio` 结构体
///
/// 代表与Shio拍卖协议进行交互的实例。
#[derive(Clone)] // 允许克隆 Shio 实例 (Arc成员使得克隆成本低)
pub struct Shio {
    global_states: Vec<ObjectArg>, // 存储一组Shio全局状态对象的ObjectArg
    state_idx: Arc<AtomicUsize>,   // 原子计数器，用于轮询选择下一个global_state
}

impl Shio {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `Shio` 实例。
    /// 它会异步初始化（如果尚未初始化）全局的 `GLOBAL_STATES` 列表。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `Shio` 实例。
    pub async fn new() -> Result<Self> {
        // 获取或初始化全局的 Shio 全局状态对象列表。
        // `get_or_init` 接收一个异步闭包，在 `GLOBAL_STATES` 首次被访问时执行。
        let global_states_vec = GLOBAL_STATES
            .get_or_init(|| async {
                // 从外部 `shio` crate 导入的 `SHIO_GLOBAL_STATES` 是一个元组数组 `(id_str, version_u64)`。
                // 将这些预定义的ID和版本转换为 `ObjectArg::SharedObject` 类型。
                SHIO_GLOBAL_STATES
                    .iter()
                    .map(|(id_str, version_u64)| ObjectArg::SharedObject {
                        id: ObjectID::from_str(id_str).unwrap(), // 从字符串解析ObjectID
                        initial_shared_version: SequenceNumber::from_u64(*version_u64), // 对象版本
                        mutable: true, // 假设这些全局状态对象在交易中是可变的
                    })
                    .collect::<Vec<_>>() // 收集为 Vec<ObjectArg>
            })
            .await // 等待初始化完成
            .clone(); // 克隆 Vec<ObjectArg> (因为 OnceCell::get_or_init 返回引用)

        // 初始化原子计数器 `state_idx`，用于轮询 `global_states_vec`。
        let state_idx_atomic = Arc::new(AtomicUsize::new(0));

        Ok(Self {
            global_states: global_states_vec,
            state_idx: state_idx_atomic,
        })
    }

    /// `submit_bid` 方法
    ///
    /// 将提交Shio出价的操作添加到现有的交易上下文 (`TradeCtx`) 中。
    /// 调用Shio合约的 `auctioneer::submit_bid` 函数。
    ///
    /// 参数:
    /// - `ctx`: 可变的交易上下文 (`&mut TradeCtx`)，用于构建PTB。
    /// - `coin_bid_arg`: 代表用于支付出价的SUI代币的 `Argument`。
    ///                   这个Coin对象应该包含至少 `bid_amount` 的面额。
    /// - `bid_amount`: 出价的金额 (以SUI的最小单位MIST计)。
    ///
    /// 返回:
    /// - `Result<()>`: 如果成功将命令添加到PTB则返回Ok，否则返回错误。
    pub fn submit_bid(&self, ctx: &mut TradeCtx, coin_bid_arg: Argument, bid_amount: u64) -> Result<()> {
        // 确保出价金额大于0。
        ensure!(bid_amount > 0, "出价金额 (bid_amount) 必须大于0");

        // 构建调用Shio合约方法所需的信息
        let package_id = ObjectID::from_hex_literal(SHIO)?; // Shio合约包ID
        let module_name = Identifier::new("auctioneer").map_err(|e| eyre!(e))?; // `auctioneer`模块
        let function_name = Identifier::new("submit_bid").map_err(|e| eyre!(e))?; // `submit_bid`函数

        // --- 构建调用参数列表 ---
        // 1. `s: &mut GlobalState` (或类似的类型): Shio的全局状态对象。
        //    通过 `next_state()` 方法轮询选择一个状态对象。
        let global_state_arg = ctx.obj(self.next_state()).map_err(|e| eyre!(e))?;
        
        // 2. `bid_amount: u64`: 出价金额。
        let bid_amount_arg = ctx.pure(bid_amount).map_err(|e| eyre!(e))?;
        
        // 3. `fee: Balance<SUI>` (或 `Coin<SUI>`): 用于支付出价的代币。
        //    合约的 `submit_bid` 函数可能接收一个 `Balance<SUI>` 对象。
        //    `ctx.coin_into_balance` 会将传入的 `Coin<SUI>` (coin_bid_arg) 转换为 `Balance<SUI>`。
        //    注意：如果 `coin_bid_arg` 的面额大于 `bid_amount`，这里可能需要先用 `split_coin`
        //    分割出精确等于 `bid_amount` 的Coin对象，然后再转换为Balance。
        //    当前的实现是将整个 `coin_bid_arg` 转换为Balance，合约内部会处理金额。
        //    假设 `submit_bid` 合约能正确处理传入的 `Balance` 中的金额。
        let sui_coin_type_tag = TypeTag::from_str(SUI_COIN_TYPE).unwrap();
        let fee_balance_arg = ctx.coin_into_balance(coin_bid_arg, sui_coin_type_tag)?;
        
        let call_arguments = vec![global_state_arg, bid_amount_arg, fee_balance_arg];

        // Shio的 `submit_bid` 函数通常没有泛型类型参数 (因为它处理的是SUI出价)。
        let type_arguments = vec![];

        // 向PTB中添加Move调用命令
        ctx.command(Command::move_call(package_id, module_name, function_name, type_arguments, call_arguments));

        Ok(())
    }

    /// `next_state` (私有辅助函数)
    ///
    /// 从 `global_states` 列表中轮询选择下一个状态对象的 `ObjectArg`。
    /// 使用原子计数器 `state_idx` 来确保在并发环境下的正确轮询。
    ///
    /// 返回:
    /// - `ObjectArg`: 下一个要使用的全局状态对象的 `ObjectArg`。
    fn next_state(&self) -> ObjectArg {
        // `fetch_add` 原子地增加计数器并返回增加前的值。
        // `Ordering::Relaxed` 表示此原子操作不需要与其他内存操作建立同步关系，
        // 对于简单的计数器轮询是足够的。
        let mut current_idx = self.state_idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        // 如果索引超出了 `global_states` 列表的范围，则重置为0。
        if current_idx >= self.global_states.len() {
            current_idx = 0;
            // `store` 原子地将计数器设置为新值。
            // 将下一个索引设置为1（如果列表非空），以避免立即再次使用索引0（如果其他线程也可能重置）。
            // 或者简单地设置为0，然后下一次fetch_add会得到0，再加1。
            // 如果严格轮询，应该在 current_idx = 0 之后，将 state_idx 设置为 1 (如果len > 0)。
            // 当前实现：如果越界，下次从0开始，再下次从1开始。
            self.state_idx.store(1, std::sync::atomic::Ordering::Relaxed); 
        }

        // 返回选定索引处的 `ObjectArg`。
        // 注意：这里直接使用 `self.global_states[current_idx]`，是安全的，因为 `current_idx`
        // 在越界后被重置为0，且 `global_states` 列表在初始化后通常不会改变长度。
        self.global_states[current_idx].clone() // 克隆ObjectArg (通常是浅拷贝)
    }
}
