// 该文件 `lib.rs` 是 `object-pool` crate (库) 的根文件。
// `object-pool` crate 实现了一个通用的对象池 (`ObjectPool<T>`)。
// 对象池是一种设计模式，用于管理和复用那些创建成本较高的对象。
// 它预先创建一定数量的对象实例，并将它们存储在池中。当需要对象时，可以从池中获取；
// 使用完毕后，对象并不会被销毁，而是“归还”到池中（通过减少其Arc引用计数，使其可以被后续的`get`选中），
// 以便后续再次使用。这可以显著减少对象创建和销毁的开销，提高性能。
//
// **文件概览 (File Overview)**:
// 这个文件定义了一个叫做 `ObjectPool<T>` 的“对象池子”。
// 想象一下，有些东西（对象）造起来很麻烦、很花时间（比如连接数据库、初始化复杂的组件等）。
// 如果每次要用都重新造一个，用完就扔掉，那效率就很低。
// 对象池就是为了解决这个问题：它预先造好一批这样的东西放在池子里。
// -   要用的时候，就从池子里拿一个出来。
// -   用完之后，不是把它销毁，而是让它可以被别人再次使用（通过Arc的引用计数机制，当没人用它时，它的强引用计数会减少）。
//
// **核心组件 (Core Components)**:
// 1.  **`ObjectPool<T>` 结构体**:
//     -   `objects: Vec<Arc<T>>`: 这是对象池的核心存储。它是一个向量（动态数组），
//         里面存放着用 `Arc<T>` 包裹的对象实例。
//         -   `T`: 是一个泛型参数，代表池中对象的具体类型。这个类型 `T` 必须满足 `Send + Sync + 'static` 的约束，
//             这意味着它可以安全地在线程间传递和共享。
//         -   `Arc<T>` (Atomic Reference Counting): 是一种智能指针，允许多个所有者安全地共享同一个对象 `T`。
//             对象池通过 `Arc` 来管理池中对象的生命周期和共享。当一个对象从池中被“获取”时，
//             实际上是克隆了它的 `Arc` 指针，增加了其强引用计数。当使用者用完对象（`Arc` 指针被drop）后，
//             引用计数会减少。这使得对象池可以追踪哪些对象当前“空闲”（引用计数较低）。
//
// 2.  **`ObjectPool<T>::new<F>(num_objects: usize, init_fn: F) -> Self` (构造函数)**:
//     -   **功能**: 创建一个新的 `ObjectPool` 实例，并并行初始化池中的所有对象。
//     -   **参数**:
//         -   `num_objects: usize`: 要在池中预先创建的对象数量。
//         -   `init_fn: F`: 一个初始化函数（闭包），类型为 `F: Fn() -> T + Send + Sync + 'static`。
//             -   `Fn() -> T`: 这个函数不需要参数，并且返回一个类型为 `T` 的新对象实例。
//             -   `Send + Sync + 'static`: 确保这个初始化函数本身也可以安全地在线程间传递和共享。
//     -   **实现**:
//         1.  将传入的 `init_fn` 用 `Arc` 包裹起来，以便在多个初始化线程中共享它。
//         2.  创建一个线程句柄的向量 `handles`，容量为 `num_objects`。
//         3.  循环 `num_objects` 次，每次都：
//             a.  克隆 `Arc<init_fn>`。
//             b.  `std::thread::spawn` 创建一个新的操作系统线程。
//             c.  在这个新线程中，调用 `init_fn()` 来创建一个新的对象 `T`。
//             d.  将新创建的对象用 `Arc::new()` 包裹起来，使其成为 `Arc<T>`。
//             e.  将该线程的句柄存入 `handles` 向量。
//         4.  遍历 `handles` 向量，对每个句柄调用 `handle.join().unwrap()`。
//             这会等待每个初始化线程执行完毕，并获取其返回的 `Arc<T>` 结果。
//             `.unwrap()` 用于处理线程panic的情况（假设初始化不会panic）。
//         5.  将所有初始化好的 `Arc<T>` 对象收集到一个 `Vec<Arc<T>>` 中，并用它创建 `ObjectPool` 实例。
//     -   **并行初始化**: 通过为每个对象的创建都分配一个新线程，`new()` 方法实现了对象的并行初始化，
//         这对于那些创建成本非常高昂的对象来说，可以显著缩短对象池的整体启动时间。
//
// 3.  **`ObjectPool<T>::get(&self) -> Arc<T>` (获取对象方法)**:
//     -   **功能**: 从对象池中获取一个对象实例。
//     -   **实现**:
//         1.  遍历池中的 `self.objects` 向量 (这是一个 `Vec<Arc<T>>`)。
//         2.  对每个 `Arc<T>` 对象，使用 `Arc::strong_count(obj)` 来获取其当前的强引用计数。
//             强引用计数表示当前有多少个 `Arc` 指针正指向这个对象。
//             -   如果一个对象刚刚被初始化并放入池中，且尚未被任何外部代码获取，其强引用计数通常是1（来自池中 `Vec` 的那个 `Arc`）。
//             -   当外部代码通过 `get()` 获取并持有一个 `Arc` 时，计数会增加。当外部代码drop其 `Arc` 时，计数会减少。
//         3.  `min_by_key(|obj| Arc::strong_count(obj))` 会找到那个具有**最小强引用计数**的 `Arc<T>` 对象。
//             这是一种简单的负载均衡策略：优先选择当前“最不忙”（被引用次数最少）的对象。
//         4.  `.unwrap()` 假设池中至少有一个对象（`new` 方法会确保这一点，除非 `num_objects` 为0）。
//         5.  `.clone()` 克隆选中的 `Arc<T>` 指针。这会增加该对象的强引用计数，表示它现在被一个新的使用者获取了。
//             返回的是这个新的 `Arc` 指针。
//     -   **注意**: 这个 `get()` 方法返回的是对象的共享引用 (`Arc<T>`)。使用者不拥有对象本身，
//         当 `Arc` 被drop时，只是减少引用计数。对象仍然保留在池的 `objects` 向量中。
//         对象池本身并没有实现“归还” (return) 的显式方法，对象的“复用”是通过 `Arc` 的引用计数机制和
//         `get()` 方法选择引用计数最小的对象来实现的。
//
// 4.  **`impl<T> Debug for ObjectPool<T>` (Debug trait 实现)**:
//     -   使得 `ObjectPool` 实例可以使用 `{:?}` 或 `{:#?}` 格式化宏进行打印，方便调试。
//     -   它会打印出池中对象的数量、池中所有对象的最大和最小引用计数。
//     -   如果池中对象数量小于32个，它还会打印出每个对象的具体引用计数值。
//
// **使用场景 (Use Cases)**:
// -   管理数据库连接池。
// -   管理昂贵的网络客户端实例（如SuiClient, reqwest::Client）。
// -   管理线程池中的工作者或任何其他初始化耗时且希望复用的资源。
//   在这个项目中，它被用来管理 `Simulator` 实例 (`Arc<ObjectPool<Box<dyn Simulator>>>`)，
//   因为创建模拟器（尤其是 `DBSimulator`）可能非常耗时。

// 引入标准库的 fmt::Debug (用于实现Debug trait) 和 sync::Arc (原子引用计数)。
use std::{fmt::Debug, sync::Arc};

/// `ObjectPool<T>` 结构体
///
/// 一个通用的对象池，用于存储和复用类型为 `T` 的对象。
/// 对象在池中以 `Arc<T>` 的形式存储，允许共享所有权。
pub struct ObjectPool<T> {
    /// `objects` 字段是一个向量，存储了池中所有对象的 `Arc` 智能指针。
    pub objects: Vec<Arc<T>>,
}

impl<T> ObjectPool<T> {
    /// `new` 构造函数
    ///
    /// 创建一个新的 `ObjectPool<T>` 实例，并并行初始化指定数量的对象。
    ///
    /// 参数:
    /// - `num_objects`: 要在池中创建的对象数量。
    /// - `init_fn`: 一个函数闭包，用于创建类型为 `T` 的单个对象实例。
    ///   此闭包必须是 `Fn() -> T` 类型，并且满足 `Send + Sync + 'static` 约束，
    ///   以允许它在多个线程中被安全地调用。
    ///
    /// 类型参数约束:
    /// - `T: Send + Sync + 'static`: 池中对象的类型 `T` 必须可以安全地在线程间发送和共享，
    ///   并且不包含任何非静态生命周期的引用。
    ///
    /// 返回:
    /// - `Self`: 新创建的 `ObjectPool<T>` 实例。
    pub fn new<F>(num_objects: usize, init_fn: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static, // 初始化函数的约束
        T: Send + Sync + 'static,             // 池中对象类型的约束
    {
        // 将初始化函数包装在 Arc 中，以便在多个初始化线程中共享。
        let init_fn_arc = Arc::new(init_fn);
        // 创建一个向量来存储线程句柄，预分配容量。
        let mut thread_handles = Vec::with_capacity(num_objects);

        // --- 并行初始化对象 ---
        // 启动 `num_objects` 个线程，每个线程负责创建一个对象实例。
        for _ in 0..num_objects {
            let current_init_fn_clone = Arc::clone(&init_fn_arc); // 克隆 Arc<init_fn> 给新线程
            // `std::thread::spawn` 创建并启动一个新线程。
            // `move` 关键字将 `current_init_fn_clone` 的所有权移入闭包。
            thread_handles.push(std::thread::spawn(move || {
                // 在新线程中执行初始化函数 `current_init_fn_clone()` 来创建对象 T，
                // 然后将创建的对象用 `Arc::new()` 包裹起来。
                Arc::new((current_init_fn_clone)())
            }));
        }

        // --- 收集所有线程的初始化结果 ---
        // `handles.into_iter()` 将线程句柄向量转换为迭代器。
        // `map(|handle| handle.join().unwrap())` 对每个句柄：
        //   - `handle.join()`: 等待对应线程执行完成。返回 `Result` (如果线程panic则为Err)。
        //   - `.unwrap()`: 简化处理，假设初始化线程不会panic。在生产代码中应更稳健地处理错误。
        // `collect()` 将所有线程返回的 `Arc<T>` 收集到一个新的向量 `initialized_objects` 中。
        let initialized_objects = thread_handles.into_iter().map(|handle| handle.join().unwrap()).collect();

        // 使用初始化好的对象向量创建并返回 ObjectPool 实例。
        Self { objects: initialized_objects }
    }

    /// `get` 方法
    ///
    /// 从对象池中获取一个对象的共享引用 (`Arc<T>`)。
    /// 此方法通过查找池中当前具有最小强引用计数 (`Arc::strong_count`) 的对象来实现一种简单的负载均衡。
    ///
    /// 返回:
    /// - `Arc<T>`: 池中一个对象的克隆的 `Arc` 指针。调用者获得该对象的共享所有权。
    ///   如果池为空 (理论上不应发生，除非 `new` 时 `num_objects` 为0)，则 `.unwrap()` 会panic。
    pub fn get(&self) -> Arc<T> {
        self.objects // 访问池中的对象向量 `Vec<Arc<T>>`
            .iter() // 获取迭代器
            .min_by_key(|arc_obj| Arc::strong_count(arc_obj)) // 根据每个 Arc 的强引用计数来查找最小值
                                                             // `Arc::strong_count` 返回当前有多少个 Arc 指针指向同一个对象。
                                                             // 选择引用计数最小的，意味着选择当前“最不常用”或“最空闲”的对象。
            .unwrap() // `min_by_key` 返回 `Option<Arc<T>>`。假设池非空，所以直接 unwrap。
            .clone()  // 克隆选中的 `Arc<T>`。这会增加对象的强引用计数，表示有一个新的使用者。
    }
}

/// 为 `ObjectPool<T>` 实现 `Debug` trait，以便进行调试打印。
impl<T> Debug for ObjectPool<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pool_len = self.objects.len(); // 获取池中对象的数量
        // 获取池中每个对象的当前强引用计数值
        let ref_counts_vec: Vec<_> = self.objects.iter().map(|arc_obj| Arc::strong_count(arc_obj)).collect();
        // 找到所有引用计数中的最大值和最小值
        let max_ref_count = ref_counts_vec.iter().max().unwrap_or(&0); // 如果池为空，默认为0
        let min_ref_count = ref_counts_vec.iter().min().unwrap_or(&0); // 如果池为空，默认为0

        // 开始格式化输出
        write!(f, "ObjectPool(对象数量={}, 最大引用计数={}, 最小引用计数={}", pool_len, max_ref_count, min_ref_count)?;

        // 如果池中对象数量不多 (小于32个)，则同时打印出所有对象的引用计数值，方便详细查看。
        if pool_len < 32 {
            write!(f, ", 各对象引用计数={:?}", ref_counts_vec)?;
        }

        write!(f, ")") // 结束格式化输出
    }
}

[end of crates/object-pool/src/lib.rs]
