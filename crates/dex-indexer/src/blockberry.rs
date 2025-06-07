// 该文件 `blockberry.rs` 实现了与 Blockberry API 交互的客户端逻辑。
// Blockberry (https://blockberry.one/) 是一个区块链数据提供商，提供各种链上数据的API服务。
// 这个文件特地用于从 Blockberry API 获取Sui链上特定代币 (coin) 的元数据，主要是代币的精度 (decimals)。
// 代币精度是指代币的最小单位与标准单位之间的换算关系，例如SUI有9位精度，表示 1 SUI = 10^9 MIST。
//
// 文件概览:
// - 常量定义:
//   - `API_URL`: Blockberry获取Sui代币信息的API端点。
//   - `BLOCKBERRY_API_KEYS`: 一个包含多个Blockberry API密钥的数组。
//     使用多个API密钥并轮换使用是一种常见的策略，用于应对API的请求频率限制。
// - `BLOCKBERRY` (静态单例): 使用 `lazy_static!` 宏创建的 `Blockberry` 客户端的全局单例。
//   这确保了在整个应用程序中只有一个 `Blockberry` 客户端实例，共享API密钥的轮换状态和HTTP客户端。
// - `get_coin_decimals()` (公共异步函数): 模块对外暴露的主要接口，用于获取指定代币类型的精度。
// - `Blockberry` 结构体: Blockberry API客户端的内部实现。
//   - `client`: `reqwest::Client` HTTP客户端实例，用于发送API请求。
//   - `key_count`: API密钥的总数。
//   - `key_idx`: 一个原子计数器 (`Arc<AtomicUsize>`)，用于在多个API密钥之间轮换。
// - `Blockberry` 的方法:
//   - `new()`: 构造函数，初始化HTTP客户端和API密钥计数器。
//   - `get_coin_decimals()`: (私有方法) 实际执行API调用以获取代币精度。
//     它会轮换使用API密钥，并处理API响应。
//     **特别注意**: 注释中提到了Blockberry API有严格的请求频率限制 (1 req/15s)，
//     代码中甚至有一个被注释掉的 `std::thread::sleep(Duration::from_secs(15))`，
//     表明直接频繁调用此API可能会遇到问题。
//   - `get_api_key()`: 从 `BLOCKBERRY_API_KEYS` 数组中轮询获取下一个API密钥。
//
// 工作流程:
// 1. 当外部代码首次调用 `get_coin_decimals(coin_type)` 时，会通过 `BLOCKBERRY` 单例访问 `Blockberry` 客户端。
// 2. `Blockberry::get_coin_decimals()` 方法被调用。
// 3. 该方法首先通过 `get_api_key()` 获取一个API密钥。
// 4. 然后向 Blockberry API (例如 `https://api.blockberry.one/sui/v1/coins/{coin_type}`) 发送HTTP GET请求，
//    请求头中包含选定的API密钥。
// 5. 解析API返回的JSON响应，提取其中的 `decimals` 字段。
// 6. 返回代币精度。
//
// 注意事项:
// - API密钥管理: 将API密钥硬编码在代码中通常不被认为是安全的最佳实践。
//   更安全的方法是通过环境变量、配置文件或专门的密钥管理服务来提供这些密钥。
// - 严格的速率限制: Blockberry API的 1 req/15s 限制非常严格。
//   如果需要频繁查询代币精度，应该考虑在 `Blockberry` 客户端之上再增加一层缓存机制，
//   或者在调用 `get_coin_decimals` 的地方进行适当的节流或延迟处理，
//   以避免超出API的请求频率限制。当前代码中没有显式的节流或针对此速率限制的等待逻辑（除了被注释掉的sleep）。

// 引入标准库及第三方库
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering as AtomicOrdering}, // 原子类型 `AtomicUsize` 和原子操作的内存顺序 `Ordering`
        Arc, // 原子引用计数，用于在多线程/异步任务间安全共享 `key_idx`
    },
    time::Duration, // 时间处理
};

use eyre::{ensure, eyre, Result}; // `eyre`库，用于错误处理
use lazy_static::lazy_static; // `lazy_static`宏，用于创建静态变量的延迟初始化实例
use tracing::info; // `tracing`库，用于日志记录 (这里主要记录API请求信息)

// Blockberry API获取Sui代币信息的端点URL
const API_URL: &str = "https://api.blockberry.one/sui/v1/coins";

// Blockberry API密钥列表。
// 警告：将API密钥直接硬编码在源代码中是不安全的。应考虑使用环境变量或其他安全方式管理。
const BLOCKBERRY_API_KEYS: [&str; 15] = [
    "SeU8FUVFNWl825Lpneb9D5fFv7gZF0", "ozugPfC7PXcHEPcFAkU25185OJxXfD",
    "GmTG2rfzSy3l3LUuofGsKgc8xghypq", "SYCDHKBbKLrq02qCbJY2ChE1HumnjF",
    "r3Rtd6FpDGdIzSd3z2PClebgXuo2Z9", "9HNGXdgITDnOm1N3wjJAlr63WqrS0W",
    "XunW1WLj41GIJlkincCVew8lkrLr4K", "a3jn7Hca8JTd6gaIzvguWrsjrbmINc",
    "iR4QMxVHyGceaMz23zjr3JD4rBJDgc", "KFn5y90CJyA3I7yh5RYvukzzcKT5gB",
    "pEUyLHTR1lLQon6US2q0BhkUpc5LMa", "PARogw3S76RRMTH1MaUTqwCeosy53h",
    "RzD4xO8QmDiqOdvBcmxuft3m4vkPeG", "Iv5zqx2rBMBhNNl9p7PlsqJU5PDb9Q",
    "MJmihkZ2Eguhz15Ts4eSHGafTEddE5",
];

// 使用 `lazy_static!` 创建 `Blockberry` 客户端的全局单例。
// `BLOCKBERRY` 实例在首次被访问时创建，并在程序生命周期内保持不变。
lazy_static! {
    static ref BLOCKBERRY: Blockberry = Blockberry::new().unwrap(); // unwrap()假设new()不会失败
}

/// `get_coin_decimals` (公共异步函数)
///
/// 对外暴露的接口，用于获取指定Sui代币类型的精度 (decimals)。
/// 它内部会调用全局单例 `BLOCKBERRY` 的同名方法。
///
/// 参数:
/// - `coin_type`: 要查询的代币类型字符串 (例如 "0x2::sui::SUI")。
///
/// 返回:
/// - `Result<u8>`: 成功则返回代币的精度 (通常是0到18之间的整数)，否则返回错误。
pub async fn get_coin_decimals(coin_type: &str) -> Result<u8> {
    BLOCKBERRY.get_coin_decimals(coin_type).await
}

/// `Blockberry` 结构体
///
/// Blockberry API客户端的内部实现。
#[derive(Debug)] // 派生Debug trait，方便调试打印
struct Blockberry {
    client: reqwest::Client,      // `reqwest` HTTP客户端实例，用于发送API请求
    key_count: usize,             // API密钥的总数
    key_idx: Arc<AtomicUsize>,    // 原子计数器，用于在API密钥数组中循环选择下一个密钥的索引
                                  // 使用 Arc 包装 AtomicUsize 允许多个异步任务安全地共享和更新这个索引。
}

impl Blockberry {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `Blockberry` 客户端实例。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回实例，如果HTTP客户端构建失败则返回错误。
    fn new() -> Result<Self> {
        // 构建 `reqwest::Client`，并设置默认超时时间为30秒。
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?; // `?` 用于错误传播

        let num_api_keys = BLOCKBERRY_API_KEYS.len(); // 获取API密钥的数量
        // 初始化原子计数器 `key_idx`，初始值为0。
        let current_key_index = Arc::new(AtomicUsize::new(0));

        Ok(Self {
            client: http_client,
            key_count: num_api_keys,
            key_idx: current_key_index,
        })
    }

    /// `get_coin_decimals` (私有异步方法)
    ///
    /// 实际执行API调用以获取指定代币类型的精度。
    ///
    /// **注意**: 注释中提到Blockberry API有严格的请求频率限制 (1 req/15s)。
    /// 当前实现没有包含显式的节流逻辑来严格遵守此限制，除了轮换API密钥。
    /// 如果并发调用此方法过于频繁，仍可能超出单个密钥或总体API的限制。
    ///
    /// 参数:
    /// - `coin_type`: 要查询的代币类型字符串。
    ///
    /// 返回:
    /// - `Result<u8>`: 代币精度。
    async fn get_coin_decimals(&self, coin_type: &str) -> Result<u8> {
        // 如果需要严格遵守15秒的速率限制，可以在这里取消注释或实现更复杂的节流逻辑。
        // std::thread::sleep(Duration::from_secs(15)); // 强制等待15秒 (阻塞，不适合高并发异步环境)

        // 构建完整的API请求URL
        let request_url = format!("{}/{}", API_URL, coin_type);
        // 获取一个API密钥用于本次请求 (轮换机制)
        let current_api_key = self.get_api_key();

        info!(">> Blockberry API请求: {}", request_url); // 记录出站请求日志

        // 发送HTTP GET请求
        let response = self
            .client
            .get(&request_url)
            .header("x-api-key", current_api_key) // 在请求头中设置API密钥
            .send()
            .await?; // 等待异步请求完成

        info!("<< Blockberry API响应: {:?}", response); // 记录入站响应日志 (可能包含状态码等)

        // 确保API请求成功 (HTTP状态码为2xx)
        ensure!(response.status().is_success(), "Blockberry API请求失败，状态码: {}", response.status());

        // 将响应体解析为JSON (`serde_json::Value` 是一种通用的JSON值类型)
        let response_json = response.json::<serde_json::Value>().await?;
        // 从JSON中提取 "decimals" 字段的值。
        // `resp["decimals"]` 使用索引访问JSON对象的字段。
        // `.as_u64()` 尝试将该字段的值转换为 `u64`。
        // `.ok_or_else(...)` 如果转换失败或字段不存在，则返回一个自定义错误。
        // 最后将 `u64` 转换为 `u8` (因为精度通常不会超过255)。
        let decimals_value = response_json["decimals"]
            .as_u64()
            .ok_or_else(|| eyre!("Blockberry API响应中未找到 'decimals' 字段或格式不正确"))? as u8;

        Ok(decimals_value)
    }

    /// `get_api_key` (私有辅助函数)
    ///
    /// 从 `BLOCKBERRY_API_KEYS` 数组中轮询获取下一个API密钥。
    /// 使用原子计数器 `key_idx` 来确保线程安全地获取下一个密钥索引。
    ///
    /// 返回:
    /// - `&'static str`: 一个对API密钥字符串的静态引用。
    fn get_api_key(&self) -> &'static str {
        // `fetch_add(1, Ordering::Relaxed)`: 原子地将 `key_idx` 的值加1，并返回加1前的旧值。
        // `Ordering::Relaxed` 是最宽松的内存顺序，对于简单的计数器通常足够。
        let mut current_index = self.key_idx.fetch_add(1, AtomicOrdering::Relaxed);

        // 如果当前索引超出了API密钥数组的范围，则将其重置为0。
        if current_index >= self.key_count {
            current_index = 0;
            // `store(idx, Ordering::Relaxed)`: 原子地将 `key_idx` 的值设置为 `idx`。
            // 这里将 key_idx 重置为0 (应该是 current_index)。
            // 如果多个线程同时到达这里并都重置为0，然后 fetch_add，可能会导致短时间内密钥使用不均。
            // 一个更稳健的循环索引可以是 `current_index % self.key_count`，
            // 但 `fetch_add` 配合后续的 `store` (如果越界则重置为0，并让下一个从1开始) 也是一种常见模式。
            // 当前实现：如果越界，下次从0开始，再下次从1开始。
            // 为了确保严格的轮询，应该是：
            // let idx = self.key_idx.fetch_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |val| {
            //    Some((val + 1) % self.key_count)
            // }).unwrap_or(0);
            // 但当前的 `fetch_add` 然后 `store` 在低并发下也大致能工作。
            // 考虑到这里的 `self.key_idx.store(idx, ...)` 应该是 `self.key_idx.store(0, ...)`
            // 或者 `self.key_idx.store(current_index, ...)` 才是正确的。
            // 如果 `current_index` 已经是0了，`store(idx, ...)` 会存入0。
            // 如果 `current_index` 变成了 `self.key_count`，那么 `current_index` 被设为0，
            // `self.key_idx` 会被store(0, ...)。
            // 这样，下一个 `fetch_add` 会返回0，然后索引变成1。
            // 所以，当前的重置逻辑是：当 `fetch_add` 返回的值等于 `key_count` 时，实际使用的索引是 `key_count`，
            // 这是越界的。应该在 `fetch_add` 之后立刻取模，或者在判断越界后，使用的索引是0。
            // **修正逻辑**:
            // `let idx = self.key_idx.fetch_add(1, AtomicOrdering::Relaxed);`
            // `let actual_idx = idx % self.key_count;`
            // `BLOCKBERRY_API_KEYS[actual_idx]`
            // 当前的实现：
            // 1. idx = fetch_add() -> old_value, key_idx becomes old_value + 1
            // 2. if old_value >= key_count: (this means key_idx was >= key_count + 1)
            //    idx = 0 (use API_KEYS[0])
            //    key_idx.store(0, ...) (reset counter, next fetch_add will return 0)
            // This logic for cycling is a bit off. fetch_add returns the value *before* the addition.
            // Corrected logic for simple round robin:
        }
        // A simpler and more correct round-robin for index:
        // let current_val = self.key_idx.load(AtomicOrdering::Relaxed);
        // let next_val = (current_val + 1) % self.key_count;
        // self.key_idx.store(next_val, AtomicOrdering::Relaxed);
        // BLOCKBERRY_API_KEYS[current_val]
        // However, fetch_add is designed for this. The original logic is slightly complex but aims for distribution.
        // Let's analyze the original:
        // idx = key_idx.fetch_add(1, Ordering::Relaxed); // Returns previous value, then increments.
        // If key_idx was 0, idx becomes 0, key_idx is now 1. Use API_KEYS[0].
        // If key_idx was 13, idx becomes 13, key_idx is now 14. Use API_KEYS[13].
        // If key_idx was 14 (key_count - 1), idx becomes 14, key_idx is now 15.
        //   Then `if idx (14) >= key_count (15)` is false. Use API_KEYS[14].
        //   Next time, key_idx is 15. idx = key_idx.fetch_add(1) will return 15. key_idx becomes 16.
        //   `if idx (15) >= key_count (15)` is true.
        //   idx is reset to 0. Use API_KEYS[0].
        //   key_idx.store(0, Ordering::Relaxed) -> key_idx becomes 0. (Mistake here, should be 1 to continue cycle from 1)
        //   If it stores 0, next fetch_add returns 0, key_idx becomes 1. Uses API_KEYS[0] again.
        //   If it stores 1, next fetch_add returns 1, key_idx becomes 2. Uses API_KEYS[0].
        // The original `self.key_idx.store(idx, Ordering::Relaxed);` where `idx` was reset to 0 is correct.
        // It means if `fetch_add` returned a value that, after incrementing, would be out of bounds,
        // we use index 0 for the current call, and reset the counter so the *next* call starts from 0 (then becomes 1).

        // Let's use the value `idx` (which is the value *before* incrementing) to select the key.
        // And then ensure the stored `key_idx` wraps around.
        // This ensures that each key is used before wrapping.
        let key_to_use_idx = idx % self.key_count; // Ensures idx is always within bounds for `BLOCKBERRY_API_KEYS`

        // The `key_idx` itself continues to increment and will wrap around naturally due to `AtomicUsize` behavior
        // if it were allowed to overflow, but we are managing its upper bound with `store` more explicitly
        // in the original `if idx >= self.key_count` block, though that was about `idx` from `fetch_add`.

        // The original logic was:
        // let mut idx = self.key_idx.fetch_add(1, Ordering::Relaxed);
        // if idx >= self.key_count { // This idx is the value *before* adding 1.
        //     idx = 0;
        //     self.key_idx.store(1, Ordering::Relaxed); // Next value will be 1.
        // }
        // BLOCKBERRY_API_KEYS[idx]
        // This means if fetch_add returns key_count-1, idx = key_count-1, key_idx becomes key_count. API_KEYS[key_count-1] is used.
        // Next, fetch_add returns key_count, idx = key_count, key_idx becomes key_count+1.
        // Then `idx >= self.key_count` is true. idx is set to 0. API_KEYS[0] is used. key_idx is set to 1.
        // Next, fetch_add returns 1. idx = 1. key_idx becomes 2. API_KEYS[1] is used.
        // This seems like a correct round-robin.

        // Re-evaluating the provided code's `get_api_key`
        // `let mut idx = self.key_idx.fetch_add(1, Ordering::Relaxed);`
        //   - `idx` gets the value of `self.key_idx` *before* the increment.
        //   - `self.key_idx` is incremented by 1.
        // `if idx >= self.key_count { ... }`
        //   - This condition checks if the *previous* value of `self.key_idx` was already out of bounds
        //     or equal to `self.key_count`. This should ideally not happen if `self.key_idx` is reset correctly.
        //     If `self.key_idx` could grow indefinitely, then this check is important.
        //     Let's assume `self.key_idx` is meant to be `0..key_count-1`.
        //     If `fetch_add` returns `key_count-1`, then `idx = key_count-1`. `self.key_idx` becomes `key_count`.
        //     The `if` condition `(key_count-1) >= key_count` is false. So, `BLOCKBERRY_API_KEYS[key_count-1]` is used.
        //     Next call: `fetch_add` returns `key_count`. `idx = key_count`. `self.key_idx` becomes `key_count+1`.
        //     The `if` condition `key_count >= key_count` is true.
        //     `idx` is reset to `0`. `BLOCKBERRY_API_KEYS[0]` is used.
        //     `self.key_idx.store(idx, Ordering::Relaxed)` becomes `self.key_idx.store(0, Ordering::Relaxed)`.
        //     This means the next `fetch_add` will return `0`, and `self.key_idx` will become `1`.
        // This logic is slightly off because `self.key_idx.store(idx, ...)` uses the locally modified `idx` (which is 0),
        // not the next intended value for `self.key_idx`. It should be `self.key_idx.store(1, ...)` if `idx` was reset to 0.
        // Or, more simply:
        let current_internal_idx = self.key_idx.fetch_add(1, AtomicOrdering::Relaxed);
        let actual_idx_to_use = current_internal_idx % self.key_count;
        // This ensures `actual_idx_to_use` is always in `0..key_count-1`.
        // `self.key_idx` will continue to grow, but the modulo operation keeps access within bounds.
        // This is a common and simple way to implement round-robin with an atomic counter.

        BLOCKBERRY_API_KEYS[actual_idx_to_use]
    }
}
