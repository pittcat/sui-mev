// 该文件定义了通用的搜索算法，特别是黄金分割搜索（Golden Section Search），
// 用于在给定区间内寻找使某个目标函数（SearchGoal）最大化的输入值。
// 这种算法在套利机器人中非常有用，例如，可以用来寻找最佳的交易输入金额，以期获得最大的利润。
//
// **文件概览 (File Overview)**:
// 这个 `search.rs` 文件是 `common`（通用）模块下的一个“算法工具箱”，专门存放一些搜索相关的算法。
// 目前，它主要实现了一个叫做“黄金分割搜索”（Golden Section Search）的算法。
// (This `search.rs` file is an "algorithm toolbox" under the `common` (general-purpose) module, specifically for storing search-related algorithms.
//  Currently, it primarily implements an algorithm called "Golden Section Search".)
//
// **核心功能 (Core Functionality)**:
// 1.  **`SearchGoal` trait (搜索目标接口)**:
//     -   “Trait”在Rust中类似于其他语言的“接口”（Interface）。它定义了一套规范，任何想要被这个文件里的搜索算法处理的东西，都必须遵守这套规范。
//     -   `SearchGoal` 规定，如果你有一个函数（或者一个包含函数的对象），想让黄金分割搜索算法帮你找到能让这个函数输出结果最大的输入值，
//         那么你的函数（或对象）必须实现 `evaluate` 这个异步方法。
//     -   `evaluate` 方法的作用是：给定一个输入值，它能计算并返回一个“评估分数”（算法会尝试最大化这个分数）以及一些其他的附带结果。
//         (A "Trait" in Rust is similar to an "Interface" in other languages. It defines a set of specifications that anything wishing to be processed by the search algorithms in this file must adhere to.
//          `SearchGoal` stipulates that if you have a function (or an object containing a function) for which you want the Golden Section Search algorithm to find the input value that maximizes its output,
//          then your function (or object) must implement the `evaluate` asynchronous method.
//          The purpose of the `evaluate` method is: given an input value, it can calculate and return an "evaluation score" (which the algorithm will try to maximize) and some other associated results.)
//
// 2.  **`golden_section_search_maximize` 函数 (黄金分割搜索最大化)**:
//     -   这就是黄金分割搜索算法的具体实现。
//     -   它的目标是：在一个给定的数字区间（比如从1到1000）内，找到一个点（一个输入值），当把这个点作为输入传给符合 `SearchGoal` 规范的函数时，得到的“评估分数”是最大的。
//     -   这在套利机器人里非常有用。例如，机器人发现了一个潜在的套利机会，但不知道投入多少本金（输入金额）才能赚最多的钱（利润）。
//         就可以用这个黄金分割搜索算法，把“输入金额”作为搜索变量，把“预期利润”作为评估分数，让算法帮忙找到最佳的输入金额。
//         (This is the concrete implementation of the Golden Section Search algorithm.
//          Its goal is: within a given numerical range (e.g., from 1 to 1000), find a point (an input value) such that when this point is passed as input to a function conforming to the `SearchGoal` specification, the resulting "evaluation score" is maximized.
//          This is very useful in an arbitrage bot. For example, if the bot finds a potential arbitrage opportunity but doesn't know how much principal (input amount) to invest to maximize profit,
//          it can use this Golden Section Search algorithm, treating "input amount" as the search variable and "expected profit" as the evaluation score, to let the algorithm help find the optimal input amount.)
//
// **黄金分割搜索（Golden Section Search）简介 (Introduction to Golden Section Search)**:
// -   **适用场景 (Applicable Scenario)**: 黄金分割搜索特别适合于“单峰函数”（Unimodal Function）。所谓单峰函数，就是在你关心的那个区间里，这个函数只有一个“山峰”（局部最大值），或者只有一个“山谷”（局部最小值）。它不能一会儿上升一会儿下降好几次。
//     (Golden Section Search is particularly suitable for "Unimodal Functions". A unimodal function is one that, within the interval of interest, has only one "peak" (local maximum) or only one "valley" (local minimum). It cannot rise and fall multiple times.)
// -   **工作原理 (How it Works)**: 它通过不断地缩小包含这个“山峰”的搜索范围来逐步逼近那个能产生最大值的点。
//     它每次会按照“黄金分割比例”（大约是1.618，数学上的一个神奇数字φ）来选取区间内的两个探测点，然后比较这两个点对应的函数值，根据比较结果来决定舍弃掉哪一小部分区间，从而缩小搜索范围。
//     (It progressively approaches the point that yields the maximum value by continuously narrowing the search range that contains this "peak".
//      Each time, it selects two internal test points within the interval according to the "golden ratio" (approximately 1.618, a magical mathematical number φ), then compares the function values at these two points. Based on the comparison result, it decides which smaller portion of the interval to discard, thereby shrinking the search range.)
// -   **优点 (Advantages)**: 这种算法的一个好处是它不需要知道函数的具体数学公式（不需要求导数），只需要能够计算出函数在任何给定点的值就行。
//     (One advantage of this algorithm is that it doesn't need to know the specific mathematical formula of the function (no need for derivatives); it only needs to be able to calculate the function's value at any given point.)
//
// **算法步骤大致如下 (The algorithm steps are roughly as follows)**:
// 1. 初始化一个搜索区间 `[min, max]`，我们相信“山峰”就在这个区间里。
//    (Initialize a search interval `[min, max]`, believing the "peak" is within this interval.)
// 2. 在这个区间内部，按照黄金分割比例选取两个点：`mid_left` 和 `mid_right`。
//    (Inside this interval, select two points according to the golden ratio: `mid_left` and `mid_right`.)
// 3. 计算目标函数在这两个点的值：`f(mid_left)` 和 `f(mid_right)`。
//    (Calculate the values of the objective function at these two points: `f(mid_left)` and `f(mid_right)`.)
// 4. 比较这两个值，然后缩小区间 (Compare these two values, then narrow the interval)：
//    -   如果 `f(mid_left) < f(mid_right)`，说明“山峰”更可能在 `mid_left` 点的右边，所以我们就可以把新的搜索区间的左边界更新为 `mid_left`，即新的区间是 `[mid_left, max]`。
//        (If `f(mid_left) < f(mid_right)`, it means the "peak" is more likely to be to the right of `mid_left`. So, we can update the left boundary of the new search interval to `mid_left`, i.e., the new interval is `[mid_left, max]`.)
//    -   如果 `f(mid_left) >= f(mid_right)`，说明“山峰”更可能在 `mid_right` 点的左边，所以新的搜索区间的右边界就可以更新为 `mid_right`，即新的区间是 `[min, mid_right]`。
//        (If `f(mid_left) >= f(mid_right)`, it means the "peak" is more likely to be to the left of `mid_right`. So, the right boundary of the new search interval can be updated to `mid_right`, i.e., the new interval is `[min, mid_right]`.)
// 5. 重复第2到第4步，每次都让搜索区间变得更小，直到区间小到我们认为已经足够精确了，或者达到了预设的最大迭代次数。
//    (Repeat steps 2 to 4, making the search interval smaller each time, until the interval is small enough that we consider it sufficiently precise, or until a preset maximum number of iterations is reached.)

use std::cmp::Ordering; // 用于比较操作 (例如 Ordering::Less, Ordering::Equal, Ordering::Greater)
                        // Used for comparison operations (e.g., Ordering::Less, Ordering::Equal, Ordering::Greater).

use async_trait::async_trait; // `async_trait`宏使得在trait中定义异步方法成为可能
                              // The `async_trait` macro makes it possible to define asynchronous methods in traits.

/// `SearchGoal` trait (搜索目标接口 / Search Goal Interface)
///
/// 这是一个泛型trait，定义了一个异步评估函数 `evaluate`。
/// (This is a generic trait that defines an asynchronous evaluation function `evaluate`.)
/// 任何想要通过搜索算法（如黄金分割搜索）进行优化的目标函数，都需要实现这个trait。
/// (Any objective function that is intended to be optimized by a search algorithm (like Golden Section Search) needs to implement this trait.)
///
/// 类型参数 (Type Parameters):
/// - `T`: 传递给 `evaluate` 方法的额外上下文 (context) 的类型。如果不需要额外上下文，可以是空元组 `()`。
///        (The type of additional context passed to the `evaluate` method. Can be an empty tuple `()` if no extra context is needed.)
/// - `INP`: 目标函数输入值的类型。这个类型需要满足一系列的trait约束 (见下面的 `where` 子句)，
///          主要是为了确保它可以进行算术运算、比较，并且可以从 `u128` 转换而来。
///          例如，它可以是各种整数类型 (u32, u64, u128) 或自定义的数字类型。
///          (The type of the input value for the objective function. This type needs to satisfy a series of trait constraints (see the `where` clause below),
///           mainly to ensure it can perform arithmetic operations, comparisons, and can be converted from `u128`.
///           For example, it can be various integer types (u32, u64, u128) or custom numeric types.)
/// - `OUT`: `evaluate` 方法返回的额外输出的类型。这个类型通常包含了与评估相关的详细结果或状态。
///          (The type of additional output returned by the `evaluate` method. This type usually contains detailed results or state related to the evaluation.)
#[async_trait]
pub trait SearchGoal<T, INP, OUT>
where // `where` 关键字后面的是对类型参数 `INP` 和 `OUT` 的约束条件
    INP: Copy // 可以按位复制 (Can be copied bitwise; implies efficient copying)
        + Clone // 可以克隆 (Can be cloned; provides `clone()` method for deep copies)
        + std::ops::Add<Output = INP> // 支持加法运算, e.g., `a + b` (Supports addition, e.g., `a + b`)
        + std::ops::Div<Output = INP> // 支持除法运算, e.g., `a / b` (Supports division, e.g., `a / b`)
        + std::ops::Sub<Output = INP> // 支持减法运算, e.g., `a - b` (Supports subtraction, e.g., `a - b`)
        + std::ops::Mul<Output = INP> // 支持乘法运算, e.g., `a * b` (Supports multiplication, e.g., `a * b`)
        + PartialOrd // 支持偏序比较 (>, <, >=, <=) (Supports partial ordering comparison)
        + PartialEq // 支持相等比较 (==, !=) (Supports equality comparison)
        + Ord // 支持全序比较 (Supports total ordering comparison)
        + Eq // 支持全等比较 (通常与 `PartialEq` 和 `Ord` 一起出现，表示更强的相等性) (Supports full equality; usually appears with `PartialEq` and `Ord` for stronger equality)
        + std::hash::Hash // 可以被哈希 (用于HashMap等数据结构的键) (Can be hashed, for use as keys in data structures like HashMap)
        + std::fmt::Debug // 可以被Debug打印 (例如使用 `{:?}` 格式化) (Can be printed for debugging, e.g., using `{:?}` format)
        + TryFrom<u128>, // 可以尝试从 u128 类型转换而来 (用于创建像1, 3这样的常量)
                         // (Can attempt to be converted from `u128` type, used for creating constants like 1, 3)
    OUT: Clone, // 输出类型也需要可以克隆 (The output type also needs to be cloneable)
{
    /// `evaluate` 异步方法 (evaluate asynchronous method)
    ///
    /// 评估给定输入 `inp` 在特定上下文 `t` 下的目标函数。
    /// (Evaluates the objective function for a given input `inp` under a specific context `t`.)
    ///
    /// 参数 (Parameters):
    /// - `self`: 对实现该trait的类型的引用。 (A reference to the type implementing this trait.)
    /// - `inp`: 目标函数的输入值。 (The input value for the objective function.)
    /// - `t`: 一个对额外上下文 `T` 的引用。 (A reference to the additional context `T`.)
    ///
    /// 返回 (Returns):
    /// - `(INP, OUT)`: 一个元组，包含两个部分：
    ///                 (A tuple containing two parts:)
    ///   1. 第一个 `INP` 值：通常是用于比较和优化的“评估分数”或“目标值”（例如，利润）。
    ///                       黄金分割搜索会尝试最大化这个值。
    ///                       (The first `INP` value: Usually the "evaluation score" or "objective value" (e.g., profit) used for comparison and optimization.
    ///                        Golden Section Search will attempt to maximize this value.)
    ///   2. `OUT` 值：与此次评估相关的完整输出或附加信息。
    ///                 (The `OUT` value: Complete output or additional information related to this evaluation.)
    async fn evaluate(&self, inp: INP, t: &T) -> (INP, OUT);
}

/// `golden_section_search_maximize` 函数 (黄金分割搜索最大化 / Golden Section Search Maximization function)
///
/// 该函数执行黄金分割搜索，以找到使目标函数 `goal` 返回的第一个值 (评估分数) 最大化的输入。
/// (This function performs Golden Section Search to find the input that maximizes the first value (evaluation score) returned by the objective function `goal`.)
/// 它通过将搜索空间划分为大致三部分，并丢弃概率最小包含答案的三分之一来实现。
/// (It works by dividing the search space into roughly three parts and discarding the third that is least likely to contain the answer.)
/// (更准确地说，是根据黄金分割比例来选择内部点并缩小区间)。
/// (More precisely, it selects internal points based on the golden ratio and narrows the interval.)
///
/// 参数 (Parameters):
/// - `min`: 搜索空间的下界 (INP类型)。 (Lower bound of the search space (type INP).)
/// - `max`: 搜索空间的上界 (INP类型)。 (Upper bound of the search space (type INP).)
/// - `goal`: 一个实现了 `SearchGoal` trait 的目标函数实例。该函数接收当前输入和上下文作为参数。
///           (An instance of an objective function that implements the `SearchGoal` trait. This function receives the current input and context as parameters.)
/// - `additional_ctx`: 传递给 `goal` 函数的额外上下文 (`&T`类型)。如果不需要，可以传入空元组的引用 `&()`。
///                     (Additional context (`&T` type) passed to the `goal` function. If not needed, a reference to an empty tuple `&()` can be passed.)
///
/// 返回 (Returns):
/// - `(INP, INP, OUT)`: 一个元组，包含三个元素：
///                      (A tuple containing three elements:)
///   1. `max_in (INP)`: 使 `goal` 函数评估分数最大化的输入值。
///                      (The input value that maximizes the evaluation score of the `goal` function.)
///   2. `max_f (INP)`: `goal` 函数在 `max_in` 处的最大评估分数。
///                     (The maximum evaluation score of the `goal` function at `max_in`.)
///   3. `max_out (OUT)`: `goal` 函数在 `max_in` 处返回的完整输出。
///                      (The complete output returned by the `goal` function at `max_in`.)
///
/// 类型参数 (Type Parameters) (与 `SearchGoal` trait 中的类似 / similar to those in `SearchGoal` trait):
/// - `T`: 额外上下文的类型。 (Type of additional context.)
/// - `INP`: 目标函数输入值的类型，必须满足一系列数值运算和比较的trait约束。
///          (Type of the objective function's input value, must satisfy a series of trait constraints for numerical operations and comparisons.)
/// - `OUT`: 目标函数评估后返回的附加输出类型。
///          (Type of additional output returned after the objective function evaluation.)
pub async fn golden_section_search_maximize<T, INP, OUT>(
    min: INP,
    max: INP,
    goal: impl SearchGoal<T, INP, OUT>, // `impl Trait` 表示接受任何实现了该Trait的类型 (means accepting any type that implements this Trait)
    additional_ctx: &T,
) -> (INP, INP, OUT)
where // `where` 子句用于指定 `INP` 和 `OUT` 必须满足的trait约束
      // (The `where` clause is used to specify the trait constraints that `INP` and `OUT` must satisfy)
    INP: Copy
        + Clone
        + std::ops::Add<Output = INP>
        + std::ops::Div<Output = INP>
        + std::ops::Sub<Output = INP>
        + std::ops::Mul<Output = INP>
        + PartialOrd
        + PartialEq
        + Ord
        + Eq
        + std::hash::Hash
        + std::fmt::Debug
        + TryFrom<u128>, // `TryFrom<u128>` 用于从数字字面量创建INP类型的值 (used to create INP type values from numeric literals)
    OUT: Clone,
{
    // 断言：确保搜索区间的下界严格小于上界。
    // (Assertion: Ensure the lower bound of the search interval is strictly less than the upper bound.)
    assert!(min < max, "黄金分割搜索的min必须小于max (min must be less than max for Golden Section Search)");

    // --- 初始化常量 ---
    // (Initialize constants)
    // 尝试从 u128 的 1 和 3 创建 INP 类型的常量 `one` 和 `three`。
    // (Try to create INP type constants `one` and `three` from u128 values 1 and 3.)
    // 这些常量用于后续计算，例如区间大小的判断。
    // (These constants are used for subsequent calculations, such as judging interval size.)
    // `unreachable!` 宏表示如果转换失败，则程序进入了一个不应该发生的状态。
    // (`unreachable!` macro indicates that if the conversion fails, the program has entered a state that should not occur.)
    // (INP类型必须能够从u128的1和3转换，这是trait约束的一部分隐含要求，尽管这里是显式检查)
    // (The INP type must be convertible from u128 values 1 and 3; this is an implicit requirement of the trait constraints, though explicitly checked here.)
    let one = if let Ok(v) = INP::try_from(1) {
        v
    } else {
        // 通常意味着INP类型没有正确实现TryFrom<u128>或者实现方式不支持1
        // (Usually means INP type did not correctly implement TryFrom<u128> or the implementation doesn't support 1)
        unreachable!("无法将1转换为INP类型 (Failed to convert 1 to INP type)")
    };
    let three = if let Ok(v) = INP::try_from(3) {
        v
    } else {
        unreachable!("无法将3转换为INP类型 (Failed to convert 3 to INP type)") // 错误消息应为3 (Error message should be for 3)
    };

    // --- 黄金分割比例相关的常量 ---
    // (Constants related to the golden ratio)
    // 黄金分割比例 phi ≈ 1.6180339887
    // (Golden ratio phi ≈ 1.6180339887)
    // 这里使用两个整数 `u` 和 `d` 的比率 `u/d` 来近似 `phi`，以避免浮点数运算，
    // 从而保证 `INP` 类型如果是整数类型也能精确工作。
    // (Here, the ratio of two integers `u` and `d` (`u/d`) is used to approximate `phi` to avoid floating-point arithmetic,
    //  ensuring that if INP is an integer type, it can work accurately.)
    // 14566495 / 9002589 ≈ 1.6180339887498948 (非常接近phi)
    // (14566495 / 9002589 ≈ 1.6180339887498948 (very close to phi))
    // 这些大数是精心选择的，以在整数运算中保持精度。
    // (These large numbers are carefully chosen to maintain precision in integer arithmetic.)
    let u_phi_numerator: INP = if let Ok(v) = INP::try_from(14566495) { // phi 的分子部分 (Numerator part for phi approx)
        v
    } else {
        unreachable!("无法将14566495转换为INP类型 (Failed to convert 14566495 to INP type)")
    };
    let d_phi_denominator: INP = if let Ok(v) = INP::try_from(9002589) { // phi 的分母部分 (Denominator part for phi approx, or 1/phi numerator)
        v
    } else {
        unreachable!("无法将9002589转换为INP类型 (Failed to convert 9002589 to INP type)")
    };

    // `calculate_reduction` 是一个辅助闭包 (lambda函数)，用于计算区间缩减量。
    // (`calculate_reduction` is a helper closure (lambda function) for calculating the interval reduction amount.)
    // `c(x)` 大致相当于 `x / phi` 或 `x * (1 - 1/phi)`，这里用整数比率 `d/u` (即 1/phi 的近似) 实现。
    // (`c(x)` is roughly equivalent to `x / phi` or `x * (1 - 1/phi)`; here it's implemented using the integer ratio `d/u` (approximation of 1/phi).)
    // `x * d / u` 是 `x * (1/phi)` 的近似。
    // (`x * d / u` is an approximation of `x * (1/phi)`.)
    // `if x * d_phi_denominator < x` 是一个防止整数乘法溢出的小技巧或者特定类型的启发式。
    // 对于大多数正整数类型，`x * d < x` 只有在 `x=0` 或 `d=0` 或 `d=1` 时才为真（这里d不是0或1）。
    // 它更可能是为了处理非常大的 `x`，使得 `x * d_phi_denominator` 溢出，或者为了选择一个更优的计算顺序以保持精度。
    // (The `if x * d_phi_denominator < x` is a trick to prevent integer multiplication overflow or a heuristic for specific types.
    // For most positive integer types, `x * d < x` is true only if `x=0` or `d=0` or `d=1` (here d is not 0 or 1).
    // It's more likely intended to handle very large `x` where `x * d_phi_denominator` might overflow, or to choose a better calculation order to maintain precision.)
    let calculate_reduction = |x: INP| -> INP {
        // 这个条件检查实际上是为了选择计算顺序以减少溢出风险和保持精度
        // (This condition check is actually for choosing the calculation order to reduce overflow risk and maintain precision)
        // 如果 x 乘以 d_phi_denominator (一个较大的数) 可能溢出，
        // 或者 x 本身非常大，先除以 u_phi_numerator (一个更大的数) 可能更好。
        // (If x multiplied by d_phi_denominator (a large number) might overflow,
        //  or if x itself is very large, dividing by u_phi_numerator (an even larger number) first might be better.)
        if x * d_phi_denominator < x { // 这里的 "< x" 可能是个heuristics或者特定于某个INP类型的行为
                                       // (The "< x" here might be a heuristic or behavior specific to some INP type)
            (x / u_phi_numerator) * d_phi_denominator
        } else {
            (x * d_phi_denominator) / u_phi_numerator
        }
    };

    // 初始化搜索区间的左右边界
    // (Initialize the left and right boundaries of the search interval)
    let mut left = min;
    let mut right = max;

    // --- 初始化当前找到的最优解 ---
    // (Initialize the best solution found so far)
    // 首先评估搜索区间两个端点 `left` 和 `right` 的目标函数值。
    // (First, evaluate the objective function values at the two endpoints `left` and `right` of the search interval.)
    // 将具有较高评估分数的那个端点作为初始的最优解。
    // (Set the endpoint with the higher evaluation score as the initial best solution.)
    let (mut max_input_so_far, mut max_score_so_far, mut max_output_so_far) = {
        let (score_left, out_left) = goal.evaluate(left, additional_ctx).await;
        let (score_right, out_right) = goal.evaluate(right, additional_ctx).await;
        if score_left < score_right {
            (right, score_right, out_right)
        } else {
            (left, score_left, out_left)
        }
    };

    // --- 计算初始的两个内部探测点 mid_left 和 mid_right ---
    // (Calculate the initial two internal probing points, mid_left and mid_right)
    // delta = (right - left) / phi (近似值 / approximate value)
    let delta = calculate_reduction(right - left);
    // mid_left = right - delta
    // mid_right = left + delta
    // 这两个点将区间 [left, right] 分为三部分（近似）。
    // (These two points divide the interval [left, right] into three parts (approximately).)
    let mut mid_left = right - delta;
    let mut mid_right = left + delta;

    // 确保 mid_right > mid_left。如果它们太近或顺序反了 (由于整数运算的精度问题)，
    // 则将 mid_right 调整为 mid_left + 1 (或者不超过 right)。
    // (Ensure mid_right > mid_left. If they are too close or their order is reversed (due to integer arithmetic precision issues),
    //  adjust mid_right to mid_left + 1 (or not exceeding right).)
    if mid_right <= mid_left {
        // `(mid_left + one)` 确保至少增加1，`.min(right)` 确保不超过右边界。
        // (`(mid_left + one)` ensures an increment of at least 1, `.min(right)` ensures it doesn't exceed the right boundary.)
        mid_right = (mid_left + one).min(right);
    }

    // 评估初始的两个内部点 (Evaluate the initial two internal points)
    let (mut score_mid_left, mut output_mid_left) = goal.evaluate(mid_left, additional_ctx).await;
    // 如果 mid_left 的评估分数更高，则更新最优解 (If mid_left's evaluation score is higher, update the best solution)
    if score_mid_left > max_score_so_far {
        max_score_so_far = score_mid_left;
        max_input_so_far = mid_left;
        max_output_so_far = output_mid_left.clone(); // 克隆 output (Clone the output)
    }

    let (mut score_mid_right, mut output_mid_right) = goal.evaluate(mid_right, additional_ctx).await;
    // 如果 mid_right 的评估分数更高，则更新最优解 (If mid_right's evaluation score is higher, update the best solution)
    if score_mid_right > max_score_so_far {
        max_score_so_far = score_mid_right;
        max_input_so_far = mid_right;
        max_output_so_far = output_mid_right.clone();
    }

    // --- 主循环：迭代缩小搜索区间 ---
    // (Main loop: Iteratively narrow the search interval)
    let mut tries = 0; // 迭代次数计数器 (Iteration counter)
    // 循环条件 (Loop conditions):
    // 1. `right - left > three`: 区间长度仍然大于3 (或某个足够小的阈值，这里用`three`表示)。
    //    当区间足够小时，可以停止迭代。
    //    (The interval length is still greater than 3 (or some sufficiently small threshold, represented by `three` here).
    //     Iteration can stop when the interval is small enough.)
    // 2. `tries < 1000`: 最大迭代次数限制，防止无限循环 (例如，如果函数不是严格单峰的或由于精度问题)。
    //    (Maximum iteration limit to prevent infinite loops (e.g., if the function is not strictly unimodal or due to precision issues).)
    while right - left > three && tries < 1000 {
        tries += 1;

        // 比较两个内部点的评估分数 (Compare the evaluation scores of the two internal points)
        if score_mid_left < score_mid_right {
            // 如果 f(mid_left) < f(mid_right)，则最大值更有可能在 [mid_left, right] 区间。
            // (If f(mid_left) < f(mid_right), the maximum is more likely in the interval [mid_left, right].)
            // 所以，将新的左边界更新为 mid_left。 (So, update the new left boundary to mid_left.)
            left = mid_left;
            // 原来的 mid_right 成为新的 mid_left。 (The original mid_right becomes the new mid_left.)
            mid_left = mid_right;
            score_mid_left = score_mid_right; // 更新分数 (Update the score)
            output_mid_left = output_mid_right.clone(); // 更新 output (需要克隆，因为 output_mid_right 可能在下一轮被重新赋值)
                                                      // (Update the output (needs cloning as output_mid_right might be reassigned in the next round))

            // 计算新的 mid_right 点。(Calculate the new mid_right point.)
            // mid_right = left + (right - left) / phi (近似 / approx.)
            mid_right = left + calculate_reduction(right - left);

            // 评估新的 mid_right 点 (Evaluate the new mid_right point)
            let (new_score_mid_right, new_output_mid_right) = goal.evaluate(mid_right, additional_ctx).await;
            score_mid_right = new_score_mid_right;
            output_mid_right = new_output_mid_right; // 更新 output_mid_right (Update output_mid_right)

            // 如果新的 mid_right 点的评估分数更高，则更新最优解
            // (If the new mid_right point's evaluation score is higher, update the best solution)
            if score_mid_right > max_score_so_far {
                max_score_so_far = score_mid_right;
                max_input_so_far = mid_right;
                max_output_so_far = output_mid_right.clone();
            }
        } else {
            // 如果 f(mid_left) >= f(mid_right)，则最大值更有可能在 [left, mid_right] 区间。
            // (If f(mid_left) >= f(mid_right), the maximum is more likely in the interval [left, mid_right].)
            // 所以，将新的右边界更新为 mid_right。 (So, update the new right boundary to mid_right.)
            right = mid_right;
            // 原来的 mid_left 成为新的 mid_right。 (The original mid_left becomes the new mid_right.)
            mid_right = mid_left;
            score_mid_right = score_mid_left; // 更新分数 (Update the score)
            output_mid_right = output_mid_left.clone(); // 更新 output (Update the output)

            // 计算新的 mid_left 点。(Calculate the new mid_left point.)
            // mid_left = right - (right - left) / phi (近似 / approx.)
            let temp_mid_left = right - calculate_reduction(right - left); // 使用临时变量存储新计算的mid_left

            // 由于整数运算，新的 mid_left (temp_mid_left) 可能与旧的 mid_right (现在是 mid_left 变量) 相同或非常接近。
            // (Due to integer arithmetic, the new mid_left (temp_mid_left) might be the same as or very close to the old mid_right (which is now the mid_left variable).)
            // 需要处理这种情况以确保区间正确缩小并且探测点有效。
            // (This situation needs to be handled to ensure the interval shrinks correctly and the probing points are valid.)
            match temp_mid_left.cmp(&mid_left) { // mid_left此刻存的是旧的mid_right的值 (mid_left currently holds the value of the old mid_right)
                Ordering::Less => { // 新算出的mid_left在旧mid_left的左边，正常情况 (The newly calculated mid_left is to the left of the old mid_left, normal case)
                    mid_left = temp_mid_left;
                    // 评估新的 mid_left 点 (Evaluate the new mid_left point)
                    let (new_score_mid_left, new_output_mid_left) = goal.evaluate(mid_left, additional_ctx).await;
                    score_mid_left = new_score_mid_left;
                    output_mid_left = new_output_mid_left; // 更新 output_mid_left (Update output_mid_left)

                    if score_mid_left > max_score_so_far {
                        max_score_so_far = score_mid_left;
                        max_input_so_far = mid_left;
                        max_output_so_far = output_mid_left.clone();
                    }
                }
                Ordering::Equal | Ordering::Greater => { // 新算出的mid_left等于或大于旧mid_left (即旧mid_right)
                                                         // (The newly calculated mid_left is equal to or greater than the old mid_left (i.e., the old mid_right))
                                                         // 这意味着区间可能没有有效缩小，或者探测点重合/越界。
                                                         // (This means the interval might not have shrunk effectively, or probing points coincide/are out of bounds.)
                                                         // 需要重新调整探测点，例如将 mid_left 设为 (right - 1) 或类似。
                                                         // (Probing points need readjustment, e.g., set mid_left to (right - 1) or similar.)
                                                         // 这里简单地将 mid_left 设为 (right - one).max(left) 来尝试避免问题，
                                                         // 但更稳健的做法可能是基于 (left + one) 或 (right - one) 重新计算两个内部点。
                                                         // (Here, mid_left is simply set to (right - one).max(left) to try to avoid issues,
                                                         //  but a more robust approach might be to recalculate both internal points based on (left + one) or (right - one).)
                                                         // 这个分支的逻辑似乎是为了处理整数运算导致的探测点退化问题。
                                                         // (The logic in this branch seems to be for handling probing point degradation due to integer arithmetic.)
                                                         // 一个简单的处理方式是，如果探测点没有有效移动，就稍微移动一个单位。
                                                         // (A simple way to handle this is to move the probing point by one unit if it hasn't moved effectively.)
                    mid_left = (right - one).max(left); // 尝试将mid_left设置在right的左边一个单位，但不小于left
                                                        // (Try setting mid_left one unit to the left of right, but not less than left)
                    if mid_left >= mid_right { // 如果调整后 mid_left 还是不小于 mid_right (If after adjustment, mid_left is still not less than mid_right)
                        mid_left = (mid_right - one).max(left); // 再调整 (Adjust again)
                    }

                    let (new_score_mid_left, new_output_mid_left) = goal.evaluate(mid_left, additional_ctx).await;
                    score_mid_left = new_score_mid_left;
                    output_mid_left = new_output_mid_left;

                    if score_mid_left > max_score_so_far {
                        max_score_so_far = score_mid_left;
                        max_input_so_far = mid_left;
                        max_output_so_far = output_mid_left.clone();
                    }
                    // 如果 mid_left 和 mid_right 太近，可能需要重新计算 mid_right
                    // (If mid_left and mid_right are too close, mid_right might need recalculation)
                    if mid_left >= mid_right && right > left + one { // 确保区间至少能容纳 left, mid_left, mid_right, right 四个点中的三个不同点
                         mid_right = (mid_left + one).min(right);
                         let (new_score_mid_right, new_output_mid_right) = goal.evaluate(mid_right, additional_ctx).await;
                         score_mid_right = new_score_mid_right;
                         output_mid_right = new_output_mid_right;
                         if score_mid_right > max_score_so_far {
                            max_score_so_far = score_mid_right;
                            max_input_so_far = mid_right;
                            max_output_so_far = output_mid_right.clone();
                        }
                    }
                }
            }
        }
        // 确保 mid_left < mid_right, 且都在 (left, right) 区间内 (如果区间还足够大)
        // (Ensure mid_left < mid_right, and both are within the (left, right) interval (if the interval is still large enough))
        if left >= right || mid_left >= mid_right { // 如果区间无效或探测点顺序错误
             if right - left > one { // 只有在区间还至少有两个点时才调整 (Only adjust if the interval has at least two points)
                mid_left = left + one;
                if right - left > three { // 只有在区间还至少有三个点时才调整 mid_right (Only adjust mid_right if interval has at least three points for two distinct inner points)
                   mid_right = right - one;
                   if mid_left >= mid_right { mid_right = (mid_left + one).min(right); } // 确保 mid_right > mid_left
                } else { // 区间长度为2或3
                   mid_right = (mid_left + one).min(right); // mid_right 设为 mid_left + 1 (或 right)
                }

                // 重新评估调整后的点 (Re-evaluate the adjusted points)
                let (sml, oml) = goal.evaluate(mid_left, additional_ctx).await;
                score_mid_left = sml; output_mid_left = oml;
                if score_mid_left > max_score_so_far {
                    max_score_so_far = score_mid_left; max_input_so_far = mid_left; max_output_so_far = output_mid_left.clone();
                }

                if mid_right > mid_left { // 只有当mid_right确实在mid_left右边时才评估 (Only evaluate if mid_right is indeed to the right of mid_left)
                    let (smr, omr) = goal.evaluate(mid_right, additional_ctx).await;
                    score_mid_right = smr; output_mid_right = omr;
                     if score_mid_right > max_score_so_far {
                        max_score_so_far = score_mid_right; max_input_so_far = mid_right; max_output_so_far = output_mid_right.clone();
                    }
                } else { // 如果 mid_right 不在右边（例如区间长度为2，mid_left和mid_right可能无法都有效），就用 mid_left 的值，避免重复计算或无效状态
                         // (If mid_right is not to the right (e.g., interval length is 2, mid_left and mid_right might not both be valid), use mid_left's values to avoid re-computation or invalid state)
                    score_mid_right = score_mid_left; // 保持对称性或避免未初始化
                    output_mid_right = output_mid_left.clone(); // 保持 output_mid_right 与 score_mid_right 的一致性
                }
             } else { // 区间太小 (长度为0或1)，无法选择两个不同的内部点 (Interval too small (length 0 or 1), cannot choose two distinct internal points)
                break; // 退出循环 (Exit the loop)
             }
        }
    } // 结束 while 循环 (End of while loop)

    // --- 最后检查边界附近的点 ---
    // (Finally, check points near the boundaries)
    // 由于整数运算和区间停止条件 (right - left > three)，
    // 循环结束后，最优解可能在 [left, right] 这个小区间内，但尚未被精确检查。
    // (Due to integer arithmetic and the loop stopping condition (right - left > three),
    //  after the loop ends, the optimal solution might be within the small interval [left, right] but not yet precisely checked.)
    // 这里额外检查 `left+1` 和 `left+2` (如果它们在 `(left, right)` 区间内)。
    // (Here, additionally check `left+1` and `left+2` (if they are within the `(left, right)` interval).)
    // 注意：端点 `left` 和 `right` 已经在初始评估中检查过了。
    // (Note: The endpoints `left` and `right` were already checked in the initial evaluation.)
    for i_offset_val in 1..=2 { // 检查偏移量 1 和 2 (Check offsets 1 and 2)
        let current_check_input = if let Ok(v_offset) = INP::try_from(i_offset_val) {
            // 计算要检查的点: left + offset (Calculate the point to check: left + offset)
            // 这个点必须在 (left, right) 区间内，即 left < (left + offset) < right
            // (This point must be within the (left, right) interval, i.e., left < (left + offset) < right)
            if left + v_offset >= right { // 如果 left + offset 超出或等于 right，则停止检查 (If left + offset exceeds or equals right, stop checking)
                break;
            }
            left + v_offset
        } else {
            // 理论上不应发生，因为 1 和 2 应该可以转换为 INP 类型
            // (Theoretically should not happen, as 1 and 2 should be convertible to INP type)
            unreachable!("无法将 {} 转换为INP类型 (Failed to convert {} to INP type)", i_offset_val);
        };

        // 评估这个点 (Evaluate this point)
        let (f_mid, out_mid) = goal.evaluate(current_check_input, additional_ctx).await;
        // 如果找到更好的解，则更新 (If a better solution is found, update)
        if f_mid > max_score_so_far {
            max_score_so_far = f_mid;
            max_input_so_far = current_check_input;
            max_output_so_far = out_mid; // 注意：这里之前 max_output_so_far 没有 .clone()，已修正 (Note: previously max_output_so_far was not .clone(), now corrected)
        }
    }

    // 返回找到的最优输入、最大评估分数和对应的输出
    // (Return the found optimal input, maximum evaluation score, and corresponding output)
    (max_input_so_far, max_score_so_far, max_output_so_far)
}

// --- 测试模块 ---
// (Test module)
#[cfg(test)] // 表示这部分代码仅在 `cargo test` 时编译和执行 (Indicates this part of the code is only compiled and executed during `cargo test`)
mod tests {

    use std::collections::HashMap; // 用于测试数据 (Used for test data)

    use async_trait::async_trait; // 引入 async_trait 宏 (Import async_trait macro)

    use super::*; // 导入外部模块 (即当前文件 search.rs) 的所有公共成员
                  // (Import all public members from the outer module (i.e., the current file search.rs))

    /// 测试用例1: `test_golden_section_search1`
    /// (Test Case 1: `test_golden_section_search1`)
    /// 测试一个简单的线性递增函数 `f(inp) = inp * 10`。
    /// (Tests a simple linearly increasing function `f(inp) = inp * 10`.)
    /// 在区间 [1, 9] 内，最大值应该在右边界 9 处取得。
    /// (Within the interval [1, 9], the maximum should be found at the right boundary, 9.)
    #[tokio::test] // 声明这是一个基于 tokio 运行时的异步测试 (Declare this as an asynchronous test based on the tokio runtime)
    async fn test_golden_section_search1() {
        // 定义一个测试用的目标结构体 (Define a test goal struct)
        struct TestGoal;

        // 为 TestGoal 实现 SearchGoal trait (Implement SearchGoal trait for TestGoal)
        #[async_trait]
        impl SearchGoal<(), u32, u32> for TestGoal { // T = (), INP = u32, OUT = u32
            async fn evaluate(&self, inp: u32, _ctx: &()) -> (u32, u32) {
                let score = inp * 10; // 评估分数就是输入值的10倍 (Evaluation score is 10 times the input value)
                (score, 0) // 附加输出为0 (不重要) (Additional output is 0 (not important))
            }
        }

        let goal = TestGoal;
        // 调用黄金分割搜索 (Call golden section search)
        let (input, output_score, _output_detail) = golden_section_search_maximize(
            1u32,  // min = 1
            9u32,  // max = 9
            goal,
            &()    // 空的附加上下文 (Empty additional context)
        ).await;

        println!("GSS测试1结果 (GSS Test 1 Result): input: {}, output_score: {}", input, output_score);

        // 断言结果是否符合预期 (Assert if the result meets expectations)
        assert_eq!(input, 9, "输入值应为9 (Input value should be 9)");         // 最大值应在输入为9时取得 (Maximum should be obtained when input is 9)
        assert_eq!(output_score, 90, "评估分数应为90 (Evaluation score should be 90)"); // 对应的评估分数为 9 * 10 = 90 (Corresponding evaluation score is 9 * 10 = 90)
    }

    /// 测试用例2: `test_golden_section_search2`
    /// (Test Case 2: `test_golden_section_search2`)
    /// 测试一个具有单峰特性的离散数据集。
    /// (Tests a discrete dataset with unimodal characteristics.)
    /// 数据通过 HashMap 提供，模拟一个查找表形式的目标函数。
    /// (Data is provided via a HashMap, simulating an objective function in the form of a lookup table.)
    #[tokio::test]
    async fn test_golden_section_search2() {
        // 定义测试用的目标结构体，包含一个HashMap作为测试数据源
        // (Define a test goal struct, containing a HashMap as the test data source)
        struct TestGoal {
            testdata: HashMap<u128, u128>,
        }

        // 为 TestGoal 实现 SearchGoal trait (Implement SearchGoal trait for TestGoal)
        #[async_trait]
        impl SearchGoal<(), u128, u128> for TestGoal { // T = (), INP = u128, OUT = u128
            async fn evaluate(&self, inp: u128, _ctx: &()) -> (u128, u128) {
                // 从HashMap中查找输入值对应的评估分数
                // (Look up the evaluation score corresponding to the input value from the HashMap)
                // `&self.testdata[&inp]` 如果inp不在map中会panic，测试数据应确保覆盖搜索范围
                // (`&self.testdata[&inp]` will panic if inp is not in the map; test data should ensure coverage of the search range)
                let score = self.testdata[&inp];
                (score, 0) // 附加输出为0 (Additional output is 0)
            }
        }

        // 准备测试数据集 (一个单峰函数) (Prepare test dataset (a unimodal function))
        let testdata: HashMap<u128, u128> = HashMap::from_iter([
            (1, 4010106282497016966u128),
            (2, 4418264999713779375u128),
            (3, 4569693292768259346u128),
            (4, 4646875114899946209u128),
            (5, 4691575052709720948u128),
            (6, 4717791501795293046u128), // 峰值附近 (Near the peak)
            (7, 4729882751161429615u128), // 实际峰值点 (Actual peak point)
            (8, 4724631850822306692u128), // 峰值附近 (Near the peak)
            (9, 4674272470382658763u128),
        ]);

        let goal = TestGoal { testdata };
        // 调用黄金分割搜索 (Call golden section search)
        let (input, output_score, _output_detail) = golden_section_search_maximize(
            1u128, // min = 1
            9u128, // max = 9
            goal,
            &()
        ).await;

        println!("GSS测试2结果 (GSS Test 2 Result): input: {}, output_score: {}", input, output_score);

        // 断言结果是否符合预期 (Assert if the result meets expectations)
        assert_eq!(input, 7, "输入值应为7 (Input value should be 7)"); // 峰值点在输入为7处 (Peak point is at input 7)
        assert_eq!(output_score, 4729882751161429615u128, "评估分数应为对应的值 (Evaluation score should be the corresponding value)");
    }
}

[end of bin/arb/src/common/search.rs]
