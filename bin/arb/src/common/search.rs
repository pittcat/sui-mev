// 该文件定义了通用的搜索算法，特别是黄金分割搜索（Golden Section Search），
// 用于在给定区间内寻找使某个目标函数（SearchGoal）最大化的输入值。
// 这种算法在套利机器人中非常有用，例如，可以用来寻找最佳的交易输入金额，以期获得最大的利润。
//
// 文件概览:
// 1. `SearchGoal` trait: 定义了一个异步的评估函数接口。任何想要被黄金分割搜索优化的目标函数都需要实现这个trait。
// 2. `golden_section_search_maximize` 函数: 实现了黄金分割搜索算法，用于最大化目标函数。
//
// 黄金分割搜索（Golden Section Search）简介:
// 黄金分割搜索是一种用于单峰函数（unimodal function，即在区间内只有一个局部最大值或最小值）的优化算法。
// 它通过不断缩小搜索区间来逼近最优点。其名字来源于它在划分区间时使用了黄金分割比例（phi ≈ 1.618）。
// 优点是不需要计算函数的导数，只需要比较函数在某些点的值。
//
// 算法步骤大致如下:
// 1. 初始化搜索区间 [min, max]。
// 2. 在区间内选择两个内部点 mid_left 和 mid_right，它们的位置与黄金分割比例相关。
// 3. 评估目标函数在 mid_left 和 mid_right 处的值 (f(mid_left) 和 f(mid_right))。
// 4. 根据比较结果缩小区间：
//    - 如果 f(mid_left) < f(mid_right)，则最优点不可能在 [min, mid_left] 区间，所以新的搜索区间变为 [mid_left, max]。
//    - 如果 f(mid_left) >= f(mid_right)，则最优点不可能在 [mid_right, max] 区间，所以新的搜索区间变为 [min, mid_right]。
// 5. 重复步骤2-4，直到区间足够小或达到最大迭代次数。

use std::cmp::Ordering; // 用于比较操作 (例如 Ordering::Less, Ordering::Equal, Ordering::Greater)

use async_trait::async_trait; // `async_trait`宏使得在trait中定义异步方法成为可能

/// `SearchGoal` trait (搜索目标接口)
///
/// 这是一个泛型trait，定义了一个异步评估函数 `evaluate`。
/// 任何想要通过搜索算法（如黄金分割搜索）进行优化的目标函数，都需要实现这个trait。
///
/// 类型参数:
/// - `T`: 传递给 `evaluate` 方法的额外上下文 (context) 的类型。如果不需要额外上下文，可以是空元组 `()`。
/// - `INP`: 目标函数输入值的类型。这个类型需要满足一系列的trait约束 (见下面的 `where` 子句)，
///          主要是为了确保它可以进行算术运算、比较，并且可以从 `u128` 转换而来。
///          例如，它可以是各种整数类型 (u32, u64, u128) 或自定义的数字类型。
/// - `OUT`: `evaluate` 方法返回的额外输出的类型。这个类型通常包含了与评估相关的详细结果或状态。
#[async_trait]
pub trait SearchGoal<T, INP, OUT>
where
    INP: Copy // 可以按位复制
        + Clone // 可以克隆
        + std::ops::Add<Output = INP> // 支持加法运算
        + std::ops::Div<Output = INP> // 支持除法运算
        + std::ops::Sub<Output = INP> // 支持减法运算
        + std::ops::Mul<Output = INP> // 支持乘法运算
        + PartialOrd // 支持偏序比较 (>, <, >=, <=)
        + PartialEq // 支持相等比较 (==, !=)
        + Ord // 支持全序比较
        + Eq // 支持全等比较
        + std::hash::Hash // 可以被哈希 (用于HashMap等)
        + std::fmt::Debug // 可以被Debug打印
        + TryFrom<u128>, // 可以尝试从 u128 类型转换而来 (用于创建像1, 3这样的常量)
    OUT: Clone, // 输出类型也需要可以克隆
{
    /// `evaluate` 异步方法
    ///
    /// 评估给定输入 `inp` 在特定上下文 `t` 下的目标函数。
    ///
    /// 参数:
    /// - `self`: 对实现该trait的类型的引用。
    /// - `inp`: 目标函数的输入值。
    /// - `t`: 一个对额外上下文 `T` 的引用。
    ///
    /// 返回:
    /// - `(INP, OUT)`: 一个元组，包含两个部分：
    ///   1. 第一个 `INP` 值：通常是用于比较和优化的“评估分数”或“目标值”（例如，利润）。
    ///      黄金分割搜索会尝试最大化这个值。
    ///   2. `OUT` 值：与此次评估相关的完整输出或附加信息。
    async fn evaluate(&self, inp: INP, t: &T) -> (INP, OUT);
}

/// `golden_section_search_maximize` 函数 (黄金分割搜索最大化)
///
/// 该函数执行黄金分割搜索，以找到使目标函数 `goal` 返回的第一个值 (评估分数) 最大化的输入。
/// 它通过将搜索空间划分为大致三部分，并丢弃概率最小包含答案的三分之一来实现。
/// (更准确地说，是根据黄金分割比例来选择内部点并缩小区间)。
///
/// 参数:
/// - `min`: 搜索空间的下界 (INP类型)。
/// - `max`: 搜索空间的上界 (INP类型)。
/// - `goal`: 一个实现了 `SearchGoal` trait 的目标函数实例。该函数接收当前输入和上下文作为参数。
/// - `additional_ctx`: 传递给 `goal` 函数的额外上下文 (`&T`类型)。如果不需要，可以传入空元组的引用 `&()`。
///
/// 返回:
/// - `(INP, INP, OUT)`: 一个元组，包含三个元素：
///   1. `max_in (INP)`: 使 `goal` 函数评估分数最大化的输入值。
///   2. `max_f (INP)`: `goal` 函数在 `max_in` 处的最大评估分数。
///   3. `max_out (OUT)`: `goal` 函数在 `max_in` 处返回的完整输出。
///
/// 类型参数 (与 `SearchGoal` trait 中的类似):
/// - `T`: 额外上下文的类型。
/// - `INP`: 目标函数输入值的类型，必须满足一系列数值运算和比较的trait约束。
/// - `OUT`: 目标函数评估后返回的附加输出类型。
pub async fn golden_section_search_maximize<T, INP, OUT>(
    min: INP,
    max: INP,
    goal: impl SearchGoal<T, INP, OUT>, // `impl Trait` 表示接受任何实现了该Trait的类型
    additional_ctx: &T,
) -> (INP, INP, OUT)
where // `where` 子句用于指定 `INP` 和 `OUT` 必须满足的trait约束
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
        + TryFrom<u128>, // `TryFrom<u128>` 用于从数字字面量创建INP类型的值
    OUT: Clone,
{
    // 断言：确保搜索区间的下界严格小于上界。
    assert!(min < max, "黄金分割搜索的min必须小于max");

    // --- 初始化常量 ---
    // 尝试从 u128 的 1 和 3 创建 INP 类型的常量 `one` 和 `three`。
    // 这些常量用于后续计算，例如区间大小的判断。
    // `unreachable!` 宏表示如果转换失败，则程序进入了一个不应该发生的状态。
    // (INP类型必须能够从u128的1和3转换，这是trait约束的一部分隐含要求，尽管这里是显式检查)
    let one = if let Ok(v) = INP::try_from(1) {
        v
    } else {
        // 通常意味着INP类型没有正确实现TryFrom<u128>或者实现方式不支持1
        unreachable!("无法将1转换为INP类型")
    };
    let three = if let Ok(v) = INP::try_from(3) {
        v
    } else {
        unreachable!("无法将3转换为INP类型") // 错误消息应为3
    };

    // --- 黄金分割比例相关的常量 ---
    // 黄金分割比例 phi ≈ 1.6180339887
    // 这里使用两个整数 `u` 和 `d` 的比率 `u/d` 来近似 `phi`，以避免浮点数运算，
    // 从而保证 `INP` 类型如果是整数类型也能精确工作。
    // 14566495 / 9002589 ≈ 1.6180339887498948 (非常接近phi)
    // 这些大数是精心选择的，以在整数运算中保持精度。
    let u_phi_numerator: INP = if let Ok(v) = INP::try_from(14566495) {
        v
    } else {
        unreachable!("无法将14566495转换为INP类型")
    };
    let d_phi_denominator: INP = if let Ok(v) = INP::try_from(9002589) {
        v
    } else {
        unreachable!("无法将9002589转换为INP类型")
    };

    // `c` 是一个辅助闭包 (lambda函数)，用于计算区间缩减量。
    // `c(x)` 大致相当于 `x / phi` 或 `x * (1 - 1/phi)`，这里用整数比率 `d/u` (即 1/phi 的近似) 实现。
    // `x * d / u` 是 `x * (1/phi)` 的近似。
    // `if x * d < x` 是一个防止整数乘法溢出的小技巧：如果 `x*d` 会溢出，那么它可能会小于 `x` (如果类型是环绕的)。
    // 但对于常规的非负整数且 `d > 0`，`x*d < x` 主要在 `x=0` 或 `d=0` (这里d非0)或`d=1`时有特殊行为。
    // 更可能是为了处理非常大的 `x`，使得 `x * d_phi_denominator` 溢出。
    // 假设 `INP` 是非负的，`x * d / u` 是主要的计算路径。
    // 如果 `x * d` 可能溢出，`x / u * d` 可能是另一种计算顺序，试图先除再乘来减小中间值。
    // (注意：整数除法会截断，这可能会影响精度，但这些数字是为此优化的)
    let calculate_reduction = |x: INP| -> INP {
        // 这个条件检查实际上是为了选择计算顺序以减少溢出风险和保持精度
        // 如果 x 乘以 d_phi_denominator (一个较大的数) 可能溢出，
        // 或者 x 本身非常大，先除以 u_phi_numerator (一个更大的数) 可能更好。
        if x * d_phi_denominator < x { // 这里的 "< x" 可能是个heuristics或者特定于某个INP类型的行为
            (x / u_phi_numerator) * d_phi_denominator
        } else {
            (x * d_phi_denominator) / u_phi_numerator
        }
    };

    // 初始化搜索区间的左右边界
    let mut left = min;
    let mut right = max;

    // --- 初始化当前找到的最优解 ---
    // 首先评估搜索区间两个端点 `left` 和 `right` 的目标函数值。
    // 将具有较高评估分数的那个端点作为初始的最优解。
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
    // delta = (right - left) / phi
    let delta = calculate_reduction(right - left);
    // mid_left = right - delta
    // mid_right = left + delta
    // 这两个点将区间 [left, right] 分为三部分（近似）。
    let mut mid_left = right - delta;
    let mut mid_right = left + delta;

    // 确保 mid_right > mid_left。如果它们太近或顺序反了 (由于整数运算的精度问题)，
    // 则将 mid_right 调整为 mid_left + 1 (或者不超过 right)。
    if mid_right <= mid_left {
        // `(mid_left + one)` 确保至少增加1，`.min(right)` 确保不超过右边界。
        mid_right = (mid_left + one).min(right);
    }

    // 评估初始的两个内部点
    let (mut score_mid_left, mut output_mid_left) = goal.evaluate(mid_left, additional_ctx).await;
    // 如果 mid_left 的评估分数更高，则更新最优解
    if score_mid_left > max_score_so_far {
        max_score_so_far = score_mid_left;
        max_input_so_far = mid_left;
        max_output_so_far = output_mid_left.clone(); // 克隆 output
    }

    let (mut score_mid_right, mut output_mid_right) = goal.evaluate(mid_right, additional_ctx).await;
    // 如果 mid_right 的评估分数更高，则更新最优解
    if score_mid_right > max_score_so_far {
        max_score_so_far = score_mid_right;
        max_input_so_far = mid_right;
        max_output_so_far = output_mid_right.clone();
    }

    // --- 主循环：迭代缩小搜索区间 ---
    let mut tries = 0; // 迭代次数计数器
    // 循环条件:
    // 1. `right - left > three`: 区间长度仍然大于3 (或某个足够小的阈值，这里用`three`表示)。
    //    当区间足够小时，可以停止迭代。
    // 2. `tries < 1000`: 最大迭代次数限制，防止无限循环 (例如，如果函数不是严格单峰的或由于精度问题)。
    while right - left > three && tries < 1000 {
        tries += 1;

        // 比较两个内部点的评估分数
        if score_mid_left < score_mid_right {
            // 如果 f(mid_left) < f(mid_right)，则最大值更有可能在 [mid_left, right] 区间。
            // 所以，将新的左边界更新为 mid_left。
            left = mid_left;
            // 原来的 mid_right 成为新的 mid_left。
            mid_left = mid_right;
            score_mid_left = score_mid_right; // 更新分数
            // output_mid_left = output_mid_right.clone(); // 更新输出 (如果需要保留每个点的输出)

            // 计算新的 mid_right 点。
            // mid_right = left + (right - left) / phi
            mid_right = left + calculate_reduction(right - left);
            
            // 评估新的 mid_right 点
            let (new_score_mid_right, new_output_mid_right) = goal.evaluate(mid_right, additional_ctx).await;
            score_mid_right = new_score_mid_right;
            output_mid_right = new_output_mid_right; // 更新 output_mid_right

            // 如果新的 mid_right 点的评估分数更高，则更新最优解
            if score_mid_right > max_score_so_far {
                max_score_so_far = score_mid_right;
                max_input_so_far = mid_right;
                max_output_so_far = output_mid_right.clone();
            }
        } else {
            // 如果 f(mid_left) >= f(mid_right)，则最大值更有可能在 [left, mid_right] 区间。
            // 所以，将新的右边界更新为 mid_right。
            right = mid_right;
            // 原来的 mid_left 成为新的 mid_right。 (这里逻辑有些绕，实际是更新mid_right为旧mid_left, 再算新mid_left)
            mid_right = mid_left; // mid_right 现在是原来的 mid_left
            score_mid_right = score_mid_left; // 更新分数
            // output_mid_right = output_mid_left.clone(); // 更新输出

            // 计算新的 mid_left 点。
            // mid_left = right - (right - left) / phi
            let temp_mid_left = right - calculate_reduction(right - left);

            // 由于整数运算，新的 mid_left (temp_mid_left) 可能与旧的 mid_right (现在是 mid_left 变量) 相同或非常接近。
            // 需要处理这种情况以确保区间正确缩小并且探测点有效。
            match temp_mid_left.cmp(&mid_left) { // mid_left此刻存的是旧的mid_right的值
                Ordering::Less => { // 新算出的mid_left在旧mid_left的左边，正常情况
                    mid_left = temp_mid_left;
                    // 评估新的 mid_left 点
                    let (new_score_mid_left, new_output_mid_left) = goal.evaluate(mid_left, additional_ctx).await;
                    score_mid_left = new_score_mid_left;
                    output_mid_left = new_output_mid_left; // 更新 output_mid_left

                    if score_mid_left > max_score_so_far {
                        max_score_so_far = score_mid_left;
                        max_input_so_far = mid_left;
                        max_output_so_far = output_mid_left.clone();
                    }
                }
                Ordering::Equal | Ordering::Greater => { // 新算出的mid_left等于或大于旧mid_left (即旧mid_right)
                                                         // 这意味着区间可能没有有效缩小，或者探测点重合/越界。
                                                         // 需要重新调整探测点，例如将 mid_left 设为 (right - 1) 或类似。
                                                         // 这里简单地将 mid_left 设为 (right - one).min(left) 来尝试避免问题，
                                                         // 但更稳健的做法可能是基于 (left + one) 或 (right - one) 重新计算两个内部点。
                                                         // 这个分支的逻辑似乎是为了处理整数运算导致的探测点退化问题。
                                                         // 一个简单的处理方式是，如果探测点没有有效移动，就稍微移动一个单位。
                    mid_left = (right - one).max(left); // 尝试将mid_left设置在right的左边一个单位，但不小于left
                    if mid_left >= mid_right { // 如果调整后 mid_left 还是不小于 mid_right
                        mid_left = (mid_right - one).max(left); // 再调整
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
                    if mid_left >= mid_right && right > left + one {
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
        if left >= right || mid_left >= mid_right {
             if right - left > one { // 只有在区间还至少有两个点时才调整
                mid_left = left + one;
                if right - left > three { // 只有在区间还至少有三个点时才调整 mid_right
                   mid_right = right - one;
                   if mid_left >= mid_right { mid_right = (mid_left + one).min(right); }
                } else {
                   mid_right = (mid_left + one).min(right);
                }

                // 重新评估调整后的点
                let (sml, oml) = goal.evaluate(mid_left, additional_ctx).await;
                score_mid_left = sml; output_mid_left = oml;
                if score_mid_left > max_score_so_far {
                    max_score_so_far = score_mid_left; max_input_so_far = mid_left; max_output_so_far = output_mid_left.clone();
                }
                if mid_right > mid_left { // 只有当mid_right确实在mid_left右边时才评估
                    let (smr, omr) = goal.evaluate(mid_right, additional_ctx).await;
                    score_mid_right = smr; output_mid_right = omr;
                     if score_mid_right > max_score_so_far {
                        max_score_so_far = score_mid_right; max_input_so_far = mid_right; max_output_so_far = output_mid_right.clone();
                    }
                } else { // 如果 mid_right 不在右边，就用 mid_left 的值，避免重复计算或无效状态
                    score_mid_right = score_mid_left; 
                    // output_mid_right = output_mid_left.clone(); // 这行可能不需要，因为比较时只用score
                }
             } else { // 区间太小，无法选择两个不同的内部点
                break; // 退出循环
             }
        }


    } // 结束 while 循环

    // --- 最后检查边界附近的点 ---
    // 由于整数运算和区间停止条件 (right - left > three)，
    // 循环结束后，最优解可能在 [left, right] 这个小区间内，但 chưa được kiểm tra kỹ.
    // 这里额外检查 `left+1` 和 `left+2` (即 `right-1` 如果 `right-left=3`) 这两个内部点。
    // 注意：端点 `left` 和 `right` 已经在初始评估中检查过了。
    for i_offset_val in 1..=2 { // 检查偏移量 1 和 2
        let current_check_input = if let Ok(v_offset) = INP::try_from(i_offset_val) {
            // 计算要检查的点: left + offset
            // 这个点必须在 (left, right) 区间内，即 left < (left + offset) < right
            if left + v_offset >= right { // 如果 left + offset 超出或等于 right，则停止检查
                break;
            }
            left + v_offset
        } else {
            // 理论上不应发生，因为 1 和 2 应该可以转换为 INP 类型
            unreachable!("无法将 {} 转换为INP类型", i_offset_val);
        };

        // 评估这个点
        let (f_mid, out_mid) = goal.evaluate(current_check_input, additional_ctx).await;
        // 如果找到更好的解，则更新
        if f_mid > max_score_so_far {
            max_score_so_far = f_mid;
            max_input_so_far = current_check_input;
            max_output_so_far = out_mid; // 注意：这里之前 max_out_so_far 没有 .clone()，已修正
        }
    }

    // 返回找到的最优输入、最大评估分数和对应的输出
    (max_input_so_far, max_score_so_far, max_output_so_far)
}

// --- 测试模块 ---
#[cfg(test)] // 表示这部分代码仅在 `cargo test` 时编译和执行
mod tests {

    use std::collections::HashMap; // 用于测试数据

    use async_trait::async_trait; // 引入 async_trait 宏

    use super::*; // 导入外部模块 (即当前文件 search.rs) 的所有公共成员

    /// 测试用例1: `test_golden_section_search1`
    /// 测试一个简单的线性递增函数 `f(inp) = inp * 10`。
    /// 在区间 [1, 9] 内，最大值应该在右边界 9 处取得。
    #[tokio::test] // 声明这是一个基于 tokio 运行时的异步测试
    async fn test_golden_section_search1() {
        // 定义一个测试用的目标结构体
        struct TestGoal;

        // 为 TestGoal 实现 SearchGoal trait
        #[async_trait]
        impl SearchGoal<(), u32, u32> for TestGoal { // T = (), INP = u32, OUT = u32
            async fn evaluate(&self, inp: u32, _ctx: &()) -> (u32, u32) {
                let score = inp * 10; // 评估分数就是输入值的10倍
                (score, 0) // 附加输出为0 (不重要)
            }
        }

        let goal = TestGoal;
        // 调用黄金分割搜索
        let (input, output_score, _output_detail) = golden_section_search_maximize(
            1u32,  // min = 1
            9u32,  // max = 9
            goal,
            &()    // 空的附加上下文
        ).await;
        
        println!("GSS测试1结果: input: {}, output_score: {}", input, output_score);

        // 断言结果是否符合预期
        assert_eq!(input, 9, "输入值应为9");         // 最大值应在输入为9时取得
        assert_eq!(output_score, 90, "评估分数应为90"); // 对应的评估分数为 9 * 10 = 90
    }

    /// 测试用例2: `test_golden_section_search2`
    /// 测试一个具有单峰特性的离散数据集。
    /// 数据通过 HashMap 提供，模拟一个查找表形式的目标函数。
    #[tokio::test]
    async fn test_golden_section_search2() {
        // 定义测试用的目标结构体，包含一个HashMap作为测试数据源
        struct TestGoal {
            testdata: HashMap<u128, u128>,
        }

        // 为 TestGoal 实现 SearchGoal trait
        #[async_trait]
        impl SearchGoal<(), u128, u128> for TestGoal { // T = (), INP = u128, OUT = u128
            async fn evaluate(&self, inp: u128, _ctx: &()) -> (u128, u128) {
                // 从HashMap中查找输入值对应的评估分数
                // `&self.testdata[&inp]` 如果inp不在map中会panic，测试数据应确保覆盖搜索范围
                let score = self.testdata[&inp]; 
                (score, 0) // 附加输出为0
            }
        }

        // 准备测试数据集 (一个单峰函数)
        let testdata: HashMap<u128, u128> = HashMap::from_iter([
            (1, 4010106282497016966u128),
            (2, 4418264999713779375u128),
            (3, 4569693292768259346u128),
            (4, 4646875114899946209u128),
            (5, 4691575052709720948u128),
            (6, 4717791501795293046u128), // 峰值附近
            (7, 4729882751161429615u128), // 实际峰值点
            (8, 4724631850822306692u128), // 峰值附近
            (9, 4674272470382658763u128),
        ]);

        let goal = TestGoal { testdata };
        // 调用黄金分割搜索
        let (input, output_score, _output_detail) = golden_section_search_maximize(
            1u128, // min = 1
            9u128, // max = 9
            goal,
            &()
        ).await;

        println!("GSS测试2结果: input: {}, output_score: {}", input, output_score);

        // 断言结果是否符合预期
        assert_eq!(input, 7, "输入值应为7"); // 峰值点在输入为7处
        assert_eq!(output_score, 4729882751161429615u128, "评估分数应为对应的值");
    }
}
