// 该文件 `file_db.rs` (位于 `dex-indexer` crate中) 定义了一个名为 `FileDB` 的结构体。
// `FileDB` 实现了一个简单的基于本地文件系统的“数据库”功能，用于存储和检索DEX池的信息
// 以及已处理Sui事件的游标 (cursor)。
// 它实现了 `DB` trait (可能在 `lib.rs` 或其他地方定义)，该trait定义了数据库操作的通用接口。
//
// 文件概览:
// - `FileDB` 结构体:
//   - `inner`: 使用 `Arc<Mutex<Inner>>` 包装内部状态 `Inner`，以实现线程安全。
//     - `Arc` (Atomic Reference Counting) 允许多个所有者共享 `Inner`。
//     - `Mutex` (Mutual Exclusion) 确保在任何时候只有一个线程可以修改 `Inner`，防止数据竞争。
// - `Inner` 结构体 (私有):
//   - `pools_paths`: 一个 `HashMap`，键是 `Protocol` (DEX协议枚举)，值是该协议的池数据存储文件的路径 (`PathBuf`)。
//     例如，Cetus协议的池可能存储在 "base_path/Cetus_pools.txt"。
//   - `cursors_path`: 存储已处理事件游标的JSON文件的路径。
//   - `processed_cursors`: 一个 `HashMap`，键是 `Protocol`，值是 `Option<EventID>` (Sui事件ID)。
//     它记录了每个协议已经处理到的最后一个事件的ID，用于在重启或下次运行时从断点继续处理。
// - `FileDB::new()`: 构造函数，初始化文件路径并从磁盘加载已保存的游标信息。
// - `DB` trait 实现 for `FileDB`:
//   - `flush()`: 将新的池数据追加到对应协议的文件中，并将更新后的游标信息写回JSON文件。
//     注意：池数据是追加写入的，而游标文件是覆盖写入的。
//   - `load_token_pools()`: 从所有协议的池文件中读取数据，反序列化每一行（假设每行是一个`Pool`对象的字符串表示），
//     并将这些 `Pool` 对象组织到一个 `PoolCache` 结构中。`PoolCache` 可能包含多种索引方式，
//     如按代币类型索引、按交易对索引、按池ID索引，以方便快速查询。
//     `DashMap` 用于 `pool_map`，表明在构建缓存时可能有并发访问的需求。
//   - `get_processed_cursors()`: 获取当前所有协议已处理事件的游标。
//   - `pool_count()`: 计算指定协议的池文件中有多少行（即多少个池）。
//   - `get_all_pools()`: 读取并返回指定协议的所有池对象。
//
// 工作原理和用途:
// `FileDB` 为 `dex-indexer` 提供了一种简单的数据持久化方案。
// - 当索引器处理新的Sui事件并发现新的DEX池或池状态更新时，可以将这些信息通过 `flush()` 方法写入文件。
// - 同时，`flush()` 也会更新该协议的事件处理游标，记录下当前处理到的进度。
// - 当索引器启动或需要访问池数据时，可以通过 `load_token_pools()` 将文件中的数据加载到内存中的 `PoolCache`，
//   或者通过 `get_all_pools()` 获取特定协议的所有池。
// - `get_processed_cursors()` 允许索引器在启动时知道从哪个事件开始继续处理，避免重复处理旧事件。
//
// 这种基于文件的方法对于中小型数据集或简单应用是可行的，但对于大规模、高并发的场景，
// 可能不如专用的数据库系统（如SQL数据库或KV存储）高效和健壮。

// 引入标准库及第三方库
use std::{
    collections::HashMap, // 哈希图，用于存储协议到文件路径的映射等
    fs::{File, OpenOptions}, // 文件系统操作：File代表文件，OpenOptions用于控制文件打开方式
    io::{BufRead, BufReader, Write}, // IO操作：BufRead (带缓冲读取), BufReader, Write (写入trait)
    path::PathBuf,        // 路径操作类型
    sync::{Arc, Mutex},    // 线程安全相关：Arc (原子引用计数), Mutex (互斥锁)
};

use dashmap::DashMap; // `dashmap` crate，提供并发安全的哈希图 (DashMap)
use eyre::{eyre, Result}; // `eyre`库，用于错误处理
use sui_sdk::types::event::EventID; // Sui SDK中的EventID类型，用于表示事件的唯一标识
use tracing::{debug, error}; // `tracing`库，用于日志记录

// 从当前 `dex-indexer` crate 的其他模块引入
use crate::{
    token01_key, // 一个辅助函数，可能用于生成交易对的唯一键 (例如，按字典序组合token0和token1的类型)
    types::{PoolCache, Token01Pools, TokenPools}, // 自定义的缓存和池集合类型
    Pool, Protocol, DB, // Pool结构体, Protocol枚举, DB trait (数据库接口)
};

/// `FileDB` 结构体
///
/// 一个基于文件的数据库实现，用于存储DEX池信息和处理进度。
#[derive(Debug, Clone)] // Clone是必要的，因为DB trait的实现可能需要克隆它 (例如传递给不同组件)
pub struct FileDB {
    // 使用 `Arc<Mutex<Inner>>` 来允许多线程安全地访问内部状态 `Inner`。
    // `Arc` 允许多个所有者共享 `Inner` 的实例。
    // `Mutex` 确保在任何时刻只有一个线程可以修改 `Inner` 的内容。
    inner: Arc<Mutex<Inner>>,
}

/// `Inner` 结构体 (私有)
///
/// 包含 `FileDB` 的实际数据和状态。
#[derive(Debug, Clone)] // Clone是必要的，因为get_processed_cursors返回它的克隆
struct Inner {
    pools_paths: HashMap<Protocol, PathBuf>, // 存储每个DEX协议的池数据文件的路径
    cursors_path: PathBuf,                   // 存储已处理事件游标的JSON文件的路径
    processed_cursors: HashMap<Protocol, Option<EventID>>, // 内存中缓存的各协议的游标信息
}

impl FileDB {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `FileDB` 实例。
    ///
    /// 参数:
    /// - `base_path`: 一个可以转换为 `PathBuf` 的路径，表示存储所有数据文件的基目录。
    /// - `protocols`:一个 `Protocol` 枚举的切片，列出了此 `FileDB` 实例将要支持和管理的DEX协议。
    ///
    /// 返回:
    /// - `Result<Self>`: 成功则返回 `FileDB` 实例。如果无法打开或读取游标文件，则返回错误。
    pub fn new(base_path: impl Into<PathBuf>, protocols: &[Protocol]) -> Result<Self> {
        let base_path_buf = base_path.into(); // 将输入转换为 PathBuf

        // 为每个支持的协议生成池数据文件的完整路径，并存储在 HashMap中。
        // 文件名格式为 "{ProtocolName}_pools.txt"。
        let pools_files_paths = protocols
            .iter()
            .map(|protocol| {
                let file_name = format!("{}_pools.txt", protocol); // 例如 "Cetus_pools.txt"
                let path = base_path_buf.join(file_name);
                (protocol.clone(), path) // 键是Protocol枚举，值是PathBuf
            })
            .collect();

        // 生成游标文件的完整路径。
        let cursors_file_path = base_path_buf.join("processed_cursors.json");
        let mut current_processed_cursors = HashMap::new(); // 初始化为空的游标HashMap

        // 如果游标文件已存在，则尝试读取并解析其中的内容。
        if cursors_file_path.exists() {
            let cursors_file_handle = File::open(&cursors_file_path)?; // 打开文件
            let reader = BufReader::new(cursors_file_handle); // 使用带缓冲的读取器
            // 从JSON文件内容反序列化游标数据到 HashMap。
            // 如果文件为空或格式不正确，`serde_json::from_reader` 可能会返回错误。
            current_processed_cursors = serde_json::from_reader(reader)?;
        }

        // 创建并返回 FileDB 实例，其内部状态 `Inner` 被 Arc 和 Mutex 包装。
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                pools_paths: pools_files_paths,
                cursors_path: cursors_file_path,
                processed_cursors: current_processed_cursors,
            })),
        })
    }
}

/// 为 `FileDB` 实现 `DB` trait (数据库操作接口)。
impl DB for FileDB {
    /// `flush` 方法
    ///
    /// 将一批新的池数据 (`pools`) 和该批数据对应的事件游标 (`cursor`) 持久化到文件。
    /// - 池数据会以追加模式写入到特定协议的池文件中。
    /// - 整个游标映射 (`processed_cursors`) 会被序列化为JSON并覆盖写入到游标文件中。
    ///
    /// 参数:
    /// - `protocol`: 要刷新数据的DEX协议 (`&Protocol`)。
    /// - `pools`: 要写入的 `Pool` 对象列表 (`&[Pool]`)。
    /// - `cursor`: (可选) 与这批 `pools` 数据相关的最后一个已处理事件的 `EventID`。
    ///             如果是 `None`，表示没有新的游标或不更新游标。
    ///
    /// 返回:
    /// - `Result<()>`: 如果刷新成功则返回Ok，否则返回IO错误或序列化错误。
    fn flush(&self, protocol: &Protocol, pools: &[Pool], cursor: Option<EventID>) -> Result<()> {
        // 获取对内部状态 `Inner` 的互斥锁。
        // `.unwrap()` 会在互斥锁被"毒化"(poisoned)时panic（即持有锁的线程panic了）。
        // 在生产代码中，可能需要更优雅地处理毒化错误，例如使用 `.lock().map_err(...)`。
        let mut inner_guard = self.inner.lock().unwrap();

        // 获取指定协议的池数据文件路径。
        let pool_file_path = inner_guard
            .pools_paths
            .get(protocol)
            .ok_or_else(|| eyre!("协议 {:?} 不受此FileDB支持", protocol))?;

        // 以追加模式打开（或创建）池数据文件。
        // `create(true)`: 如果文件不存在则创建。
        // `append(true)`: 以追加模式写入。
        let mut pool_file_handle = OpenOptions::new().create(true).append(true).open(pool_file_path)?;
        // 遍历 `pools` 列表，将每个 `Pool` 对象序列化为字符串（通过其 `Display` trait实现）并写入文件，每行一个。
        for pool_item in pools {
            writeln!(pool_file_handle, "{}", pool_item)?; // `writeln!` 会自动添加换行符
        }

        // 更新内存中对应协议的游标信息。
        inner_guard.processed_cursors.insert(protocol.clone(), cursor);

        // 将整个 `processed_cursors` HashMap 序列化为JSON并覆盖写入到游标文件中。
        // `create(true)`: 如果文件不存在则创建。
        // `write(true)`: 以写入模式打开。
        // `truncate(true)`: 在写入前清空文件内容 (实现覆盖效果)。
        let cursors_file_handle = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&inner_guard.cursors_path)?;
        serde_json::to_writer(cursors_file_handle, &inner_guard.processed_cursors)?; // 序列化并写入

        Ok(())
    }

    /// `load_token_pools` 方法
    ///
    /// 从所有受支持协议的池文件中加载数据，并将它们组织到一个 `PoolCache` 对象中。
    /// `PoolCache` 提供了多种索引方式（按单个代币、按交易对、按池ID）以方便快速查询。
    ///
    /// 参数:
    /// - `protocols`: 要为其加载池数据的协议列表 (`&[Protocol]`)。
    ///
    /// 返回:
    /// - `Result<PoolCache>`: 包含加载和索引后池数据的 `PoolCache`。
    fn load_token_pools(&self, protocols: &[Protocol]) -> Result<PoolCache> {
        let inner_guard = self.inner.lock().map_err(|_| eyre!("获取FileDB内部锁失败 (Mutex poisoned)"))?;

        // 初始化用于构建 PoolCache 的数据结构。
        let token_pools_map = TokenPools::new(); // HashMap<TokenType, HashSet<Pool>>
        let token01_pools_map = Token01Pools::new(); // HashMap<Token01Key, HashSet<Pool>>
        let pool_id_map = DashMap::new(); // DashMap<ObjectID, Pool> (并发安全的HashMap)

        // 遍历指定的每个协议
        for protocol_item in protocols {
            debug!(protocol = ?protocol_item, "正在加载协议的池数据...");
            // 获取该协议的池数据文件路径
            let pool_file_path = inner_guard
                .pools_paths
                .get(protocol_item)
                .ok_or_else(|| eyre!("协议 {:?} 不受此FileDB支持", protocol_item))?;

            // 尝试打开池文件。如果文件不存在（例如，某个协议还没有任何池数据被索引），则记录调试信息并跳过。
            let pool_file_handle = match File::open(pool_file_path) {
                Ok(file) => file,
                Err(e) => {
                    debug!(protocol = ?protocol_item, error = ?e, "打开池文件失败或文件不存在，跳过加载");
                    continue;
                }
            };
            let reader = BufReader::new(pool_file_handle); // 使用带缓冲的读取器

            let mut line_count = 0;
            // 逐行读取池文件
            for line_result in reader.lines() {
                line_count += 1;
                if line_result.is_err() { // 处理行读取错误
                    error!("读取池文件行错误: {:?}", line_result);
                    continue;
                }
                let line_str = line_result.unwrap(); // 获取行字符串

                // 尝试将行字符串反序列化为 `Pool` 对象。
                // `Pool` 类型需要实现 `TryFrom<&str>` (或 `FromStr`) trait。
                let pool_obj = match Pool::try_from(line_str.as_str()) {
                    Ok(p) => p,
                    Err(e) => {
                        error!("反序列化Pool对象失败，行内容: '{}', 错误: {:?}", line_str, e);
                        continue;
                    }
                };

                // --- 将 Pool 对象添加到不同的索引中 ---
                // 1. 按单个代币类型索引 (token_pools_map)
                for token_info in &pool_obj.tokens { // `pool_obj.tokens` 是 Vec<TokenInfo>
                    let token_type_key = token_info.token_type.clone();
                    // `entry(key).or_default()` 获取键对应的 `HashSet<Pool>`，如果不存在则插入一个空的HashSet。
                    // 然后将当前 `pool_obj` 的克隆插入到这个HashSet中。
                    token_pools_map.entry(token_type_key).or_default().insert(pool_obj.clone());
                }

                // 2. 按交易对 (token0/token1) 索引 (token01_pools_map)
                // `pool_obj.token01_pairs()` 返回池中所有可能的交易对 (例如，对于多币池)。
                // 对于标准双币池，它通常只返回一个交易对。
                for (token0_type_str, token1_type_str) in pool_obj.token01_pairs() {
                    // `token01_key` 函数生成一个规范化的交易对键 (例如，按字典序排序Token类型)。
                    let pair_key = token01_key(&token0_type_str, &token1_type_str);
                    token01_pools_map.entry(pair_key).or_default().insert(pool_obj.clone());
                }

                // 3. 按池ID索引 (pool_id_map)
                pool_id_map.insert(pool_obj.pool, pool_obj); // DashMap可以直接插入
            }
            debug!(protocol = ?protocol_item, pools_count = %line_count, "协议的池数据加载完毕");
        }

        // 使用构建好的索引创建并返回 PoolCache 对象。
        Ok(PoolCache::new(token_pools_map, token01_pools_map, pool_id_map))
    }

    /// `get_processed_cursors` 方法
    ///
    /// 返回当前所有协议已处理事件的游标的克隆。
    fn get_processed_cursors(&self) -> Result<HashMap<Protocol, Option<EventID>>> {
        let inner_guard = self.inner.lock().map_err(|_| eyre!("获取FileDB内部锁失败 (Mutex poisoned)"))?;
        Ok(inner_guard.processed_cursors.clone()) // 返回游标HashMap的克隆
    }

    /// `pool_count` 方法
    ///
    /// 计算指定协议的池文件中有多少行（即大致等于池的数量）。
    /// 这是一个相对粗略的计数，因为它只是读取行数。
    fn pool_count(&self, protocol: &Protocol) -> Result<usize> {
        let inner_guard = self.inner.lock().map_err(|_| eyre!("获取FileDB内部锁失败 (Mutex poisoned)"))?;
        let pool_file_path = inner_guard
            .pools_paths
            .get(protocol)
            .ok_or_else(|| eyre!("协议 {:?} 不受此FileDB支持", protocol))?;
        let pool_file_handle = File::open(pool_file_path)?; // 打开文件
        let reader = BufReader::new(pool_file_handle);
        Ok(reader.lines().count()) // 计算行数
    }

    /// `get_all_pools` 方法
    ///
    /// 读取并返回指定协议的所有池对象列表。
    fn get_all_pools(&self, protocol: &Protocol) -> Result<Vec<Pool>> {
        let inner_guard = self.inner.lock().map_err(|_| eyre!("获取FileDB内部锁失败 (Mutex poisoned)"))?;
        let pool_file_path = inner_guard
            .pools_paths
            .get(protocol)
            .ok_or_else(|| eyre!("协议 {:?} 不受此FileDB支持", protocol))?;
        let pool_file_handle = File::open(pool_file_path)?;
        let reader = BufReader::new(pool_file_handle);

        let mut pools_vec = vec![];
        for line_result in reader.lines() {
            let line_str = line_result?;
            // 将每行字符串反序列化为 Pool 对象并添加到向量中。
            pools_vec.push(Pool::try_from(line_str.as_str())?);
        }
        Ok(pools_vec)
    }
}
