// 该文件 `config.rs` 用于存放应用程序的全局配置参数和常量。
// 将配置信息集中存放在一个文件中，有助于管理和修改，同时也使得其他模块可以方便地引用这些值。
//
// 文件概览:
// 1. `GAS_BUDGET`: 定义了Sui交易中愿意支付的最大Gas单位数量。
// 2. `MAX_SQRT_PRICE_X64`, `MIN_SQRT_PRICE_X64`: 定义了价格平方根的允许范围，
//    这通常与某些DeFi协议（如集中流动性做市商 AMM）中的价格表示相关。
//    X64表示这个值是Q64.64定点数表示，即小数点后有64位二进制精度。
// 3. `pegged_coin_types()`: 一个函数，返回一个包含多种“锚定币”或主流币种类型字符串的集合 (HashSet)。
//    这些币种在套利逻辑中可能需要特殊处理，或者作为交易对的基础币。
// 4. `tests` 模块 (仅在测试时编译): 包含用于测试的特定常量，例如测试RPC节点的URL和测试账户地址。
//    (注意：这些测试常量的值在提供的代码中是空字符串，实际测试时需要填充。)

use std::collections::HashSet; // 引入标准库中的 HashSet 数据结构，用于存储一组唯一的元素。

use sui_sdk::SUI_COIN_TYPE; // 从 `sui_sdk` 中引入 `SUI_COIN_TYPE` 常量，
                           // 它代表了Sui原生代币 (SUI) 的官方类型字符串。

/// `GAS_BUDGET` 常量
///
/// 定义了在Sui区块链上执行交易时，愿意设置的Gas预算上限。
/// Gas是Sui网络中用于支付交易执行和存储费用的单位。
/// 设置一个合理的Gas预算很重要：
/// - 太低：交易可能因为Gas不足而失败。
/// - 太高：虽然交易会执行，但如果实际消耗远低于预算，未用部分会退还。
///         然而，这也表示账户需要有足够的余额来覆盖这个预算。
/// 这里的 `10_000_000_000` (即100亿 MIST，或 10 SUI) 是一个相对较大的预算，
/// 可能用于覆盖复杂的多步套利交易。1 SUI = 10^9 MIST.
pub const GAS_BUDGET: u64 = 10_000_000_000;

/// `MAX_SQRT_PRICE_X64` 常量
///
/// 定义了价格平方根 (sqrt_price) 的最大允许值，采用X64表示法 (Q64.64定点数)。
/// 在一些DeFi协议中，特别是集中流动性AMM（如Uniswap V3及其变种），价格不是直接存储的，
/// 而是存储其平方根，这有助于进行某些数学运算并提高精度。
/// X64格式意味着这个 `u128` 类型的整数实际上表示一个小数，其中整数部分占高64位，小数部分占低64位。
/// 这个最大值通常对应于一个代币相对于另一个代币的极高价格。
pub const MAX_SQRT_PRICE_X64: u128 = 79226673515401279992447579055;

/// `MIN_SQRT_PRICE_X64` 常量
///
/// 定义了价格平方根 (sqrt_price) 的最小允许值，同样采用X64表示法。
/// 这通常对应于一个代币相对于另一个代币的极低价格。
/// 这两个常量（MAX和MIN）共同定义了价格表示的有效范围。
pub const MIN_SQRT_PRICE_X64: u128 = 4295048016;

/// `pegged_coin_types` 函数
///
/// 返回一个包含多种“锚定币”（pegged coins）或主流币种的类型字符串的 `HashSet<&'static str>`。
/// “锚定币”是指其价值与某种法定货币（如美元）或其他资产锚定的加密货币，例如USDC, USDT。
/// 也可能包含主流的非锚定币，如WETH (Wrapped ETH)。
///
/// 在套利逻辑中，这些币种可能具有以下特点或用途：
/// - 作为报价货币或基础货币（例如，很多交易对都是以USDC或SUI计价）。
/// - 它们的流动性通常较好。
/// - 某些套利策略可能专门围绕这些币种设计。
///
/// `HashSet` 用于存储这些类型字符串，确保每个类型只出现一次，并且可以快速查找。
/// `&'static str` 表示这些字符串字面量在整个程序运行期间都有效 (存储在程序的只读数据段)。
///
/// 返回:
/// - `HashSet<&'static str>`: 一个包含代币类型字符串的集合。
pub fn pegged_coin_types() -> HashSet<&'static str> {
    HashSet::from_iter([ // `from_iter` 从一个迭代器创建HashSet
        // SUI 原生代币
        SUI_COIN_TYPE, // "0x2::sui::SUI"

        // --- 各种版本的稳定币 (主要是USDC, USDT) ---
        // 注意：在Sui或其他区块链上，同一种稳定币（如USDC）可能由不同的发行方或通过不同的桥接方式引入，
        // 因此它们在链上会有不同的对象类型地址。下面列出的是一些已知的USDC和USDT的类型。

        // Wormhole USDC (来自以太坊的USDC，通过Wormhole桥接)
        "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN",
        
        // Wormhole USDT (来自以太坊的USDT，通过Wormhole桥接)
        "0xc060006111016b8a020ad5b33834984a437aaa7d3c74c18e09a95d48aceab08c::coin::COIN",
        
        // Wormhole WETH (来自以太坊的Wrapped ETH，通过Wormhole桥接)
        "0xaf8cd5edc19c4512f4259f0bee101a40d41ebed738ade5874359610ef8eeced5::coin::COIN",
        
        // Celer USDC (另一种桥接的USDC)
        // "0xb231fcda8bbddb31f2ef02e6161444aec64a514e2c89279584ac9806ce9cf037::coin::COIN",
        // 上面这个地址在注释中提到是USDC，但实际的类型名可能是通用的 `coin::COIN`。
        // 为了更精确，通常会使用更具体的类型路径，如果发行者定义了的话。
        // 例如下面这个：
        "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC", // 这是一个更明确的USDC类型

        // Bucket Protocol的稳定币 BUCK
        // Bucket Protocol 是Sui上的一个去中心化借贷和稳定币协议。
        "0xce7ff77a83ea0cb6fd39bd8748e2ec89a3f41e8efdc3f4eb123e0ca37b184db2::buck::BUCK",
    ])
}

// --- 测试模块 (`tests`) ---
// `#[cfg(test)]` 属性宏表示这部分代码仅在执行 `cargo test` 命令时编译和包含。
// 这个模块通常用于存放单元测试、集成测试相关的辅助函数或常量。
#[cfg(test)]
pub mod tests {
    // `TEST_HTTP_URL` 常量
    //
    // 定义了用于测试的Sui RPC节点的HTTP URL。
    // 在实际测试时，这里应该填入一个可用的测试网 (testnet) 或开发网 (devnet) RPC节点的地址。
    // 例如: "https://fullnode.testnet.sui.io:443"
    // 将其设为空字符串意味着在没有正确配置的情况下，依赖此常量的测试可能会失败或无法运行。
    pub const TEST_HTTP_URL: &str = ""; // 示例: "https://fullnode.devnet.sui.io:443"

    // `TEST_ATTACKER` 常量
    //
    // 定义了用于测试的攻击者（或交易发送者）的Sui账户地址。
    // 这个账户在测试环境中需要有足够的Gas代币 (SUI) 和其他测试代币来执行交易。
    // 地址是一个十六进制字符串。
    // 例如: "0xabcdef1234567890..." (一个有效的Sui地址)
    // 同样，空字符串表示未配置。
    pub const TEST_ATTACKER: &str = ""; // 示例: "0xyour_test_sui_address_hex_string"
}
