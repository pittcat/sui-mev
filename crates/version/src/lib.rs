// 该文件 `lib.rs` 是 `version` crate (库) 的根文件。
// 这个crate的主要功能是定义一个过程宏 (procedural macro) `build_version`，
// 该宏在编译时执行，用于获取当前的Git提交信息（包括分支、提交哈希、日期以及工作区是否“脏”），
// 并将这些信息组合成一个版本字符串。
// 这个版本字符串随后可以被嵌入到主应用程序中 (例如 `arb/src/main.rs` 中的 `BUILD_VERSION` 常量)，
// 从而在运行时能够确切知道程序是由哪个代码版本构建的。
//
// **文件概览 (File Overview)**:
// 这个文件是 `version` 库的“身份证生成器”。
// 它的工作是在程序被编译（构建）的时候，自动去查询Git版本控制系统，
// 拿到当前代码的版本信息（比如最新的提交是什么，代码有没有被修改过但还没提交等），
// 然后把这些信息整合成一个唯一的“版本号”字符串。
//
// **核心功能和组件 (Core Functionalities and Components)**:
//
// 1.  **`extern crate proc_macro;`**:
//     -   这行代码声明了此crate将定义过程宏。过程宏是一种特殊的Rust宏，它在编译时操作TokenStream，
//         允许进行更复杂的代码生成和转换。
//
// 2.  **`get_git_commit() -> eyre::Result<String>` 函数**:
//     -   **功能**: 这个私有辅助函数负责实际执行Git命令来获取版本信息。
//     -   **实现**:
//         1.  **获取最新提交哈希和日期**:
//             -   执行 `git log -1 --pretty=format:%h,%ad --date=format:%Y-%m-%d` 命令。
//                 -   `log -1`: 只取最新的一条日志。
//                 -   `--pretty=format:%h,%ad`: 自定义输出格式。`%h` 是缩写的提交哈希，`%ad` 是作者提交日期。
//                 -   `--date=format:%Y-%m-%d`: 将日期格式化为 "年-月-日"。
//             -   解析命令输出，期望格式为 "{commit_hash},{date_string}"。
//         2.  **检查工作区是否“脏” (Dirty Status)**:
//             -   执行 `git status -s` 命令。`-s` 表示简短格式输出。
//             -   如果输出不为空，则表示工作区有未提交的修改 (即“脏”状态)，版本字符串会附加 "-dirty"。
//         3.  **获取当前分支名**:
//             -   执行 `git rev-parse --abbrev-ref HEAD` 命令来获取当前活动分支的简写名称。
//         4.  **格式化版本字符串**:
//             -   将获取到的分支名、提交哈希、是否脏状态、以及日期组合成一个格式为
//               `"{branch}-{commit_hash}{dirty_suffix}@{date}"` 的字符串。
//               例如: `"main-a1b2c3d-dirty@2023-10-27"` 或 `"feature/xyz-f0e1d2c@2023-10-26"`。
//             -   注意，最终返回的字符串会被双引号 `"` 包裹，这可能是因为过程宏在生成代码时，
//                 这个字符串会直接作为字符串字面量插入到代码中。
//     -   **错误处理**: 使用 `eyre::Result` 和 `ensure!` 来处理命令执行失败或输出格式不符合预期的情况。
//
// 3.  **`build_version(_item: TokenStream) -> TokenStream` 过程宏**:
//     -   **`#[proc_macro]`**: 这个属性将 `build_version` 函数标记为一个过程宏。
//         当其他代码中使用 `build_version!()` 时，编译器会调用这个函数。
//     -   **参数**: `_item: TokenStream`。对于函数式过程宏 (function-like procedural macro)，
//         `_item` 是宏调用时括号内的TokenStream。在这个例子中，`build_version!()` 没有参数，所以 `_item` 未被使用。
//     -   **实现**:
//         1.  调用 `get_git_commit().unwrap()` 来获取版本字符串。
//             `.unwrap()` 会在 `get_git_commit` 返回 `Err` 时panic。这在过程宏中是常见的，
//             因为宏展开失败通常意味着编译失败。
//         2.  使用 `TokenStream::from_str(version.as_str()).unwrap()` 将获取到的版本字符串
//             转换为一个 `TokenStream`。这个 `TokenStream` 随后会替换掉源码中 `build_version!()` 宏调用的位置。
//             例如，如果 `version` 是 `"main-a1b2c3d@2023-10-27"`，
//             那么 `let v = build_version!();` 就会在编译后变成 `let v = "main-a1b2c3d@2023-10-27";`。
//
// **用途 (Purpose)**:
// -   **版本信息嵌入**: 允许开发者在编译时自动将详细的Git版本信息嵌入到最终的可执行程序或库中。
// -   **调试与追踪**: 当程序出现问题或需要追踪其来源时，这个嵌入的版本字符串可以提供精确的代码版本快照，
//     极大地方便了问题的定位和复现。
// -   **发布管理**: 可以确保发布的每个构建都带有可识别的版本标识。
//
// **前提条件 (Prerequisites)**:
// -   这个crate在编译时需要能够访问 `.git` 目录并执行 `git` 命令。
//     这意味着编译环境必须安装了Git，并且编译操作是在一个Git仓库的上下文中进行的。
// -   如果编译环境不满足这些条件（例如，在某些CI/CD流水线中，或者从一个没有 `.git` 目录的源码包构建），
//     `git` 命令会失败，导致 `get_git_commit()` 返回错误，进而可能使 `build_version` 宏panic，最终导致编译失败。
//     在这种情况下，可能需要备用逻辑（例如从环境变量读取版本号，或在构建脚本中预先生成版本文件）。

// 声明此crate是一个过程宏crate。
extern crate proc_macro;
// 引入标准库的 Not trait (用于布尔取反) 和 Command (用于执行外部命令)，以及 FromStr trait (用于从字符串转换)。
use std::{ops::Not, process::Command, str::FromStr};

// 引入 eyre 库的 Result 和 ensure! 宏，用于错误处理。
use eyre::ensure;
// 引入 proc_macro 库的 TokenStream 类型，这是过程宏操作的核心数据类型。
use proc_macro::TokenStream;

/// `get_git_commit` 函数
///
/// 执行一系列git命令来获取当前仓库的版本信息，包括分支名、最新提交哈希、
/// 提交日期以及工作区是否“脏”（有未提交的修改）。
///
/// 返回:
/// - `eyre::Result<String>`: 成功则返回一个格式化的版本字符串 (例如 `"main-a1b2c3d-dirty@2023-10-27"`)，
///   该字符串会被双引号包裹。如果执行git命令失败或输出格式不符合预期，则返回错误。
fn get_git_commit() -> eyre::Result<String> {
    // 步骤 1: 获取最新提交的缩写哈希和作者提交日期。
    // `git log -1 --pretty=format:%h,%ad --date=format:%Y-%m-%d`
    // - `-1`: 只显示最新的一条提交。
    // - `--pretty=format:%h,%ad`: 自定义输出格式。`%h` 是缩写提交哈希，`%ad` 是作者提交日期。
    // - `--date=format:%Y-%m-%d`: 将日期格式化为 "YYYY-MM-DD"。
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=format:%h,%ad", "--date=format:%Y-%m-%d"])
        .output()?; // 执行命令并获取输出，`?` 用于错误传播。
    // 将命令的标准输出 (stdout) 从UTF-8字节流转换为字符串，并去除首尾空白。
    let output_str = std::str::from_utf8(&output.stdout)?.trim().to_string();
    // 按逗号分割字符串，期望得到两部分：[提交哈希, 日期]。
    let parts: Vec<&str> = output_str.split(',').collect();
    // 确保分割后确实是两部分。
    ensure!(parts.len() == 2, "获取git提交信息时，输出格式不符合预期 (应为 hash,date)");

    // 步骤 2: 检查工作区是否有未提交的修改 ("dirty"状态)。
    // `git status -s` (简短格式) 如果有输出，则表示工作区是脏的。
    let dirty_output = Command::new("git").args(["status", "-s"]).output()?;
    let dirty_suffix = std::str::from_utf8(&dirty_output.stdout)?
        .is_empty() // 检查输出是否为空
        .not() // 取反 (如果非空，则为true，表示是脏的)
        .then(|| "-dirty".to_string()) // 如果是脏的，则附加 "-dirty" 后缀
        .unwrap_or_default(); // 如果不是脏的 (输出为空)，则使用空字符串作为后缀

    // 步骤 3: 获取当前分支的名称。
    // `git rev-parse --abbrev-ref HEAD` 获取当前HEAD指向的分支的简写名称。
    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    let branch_name = std::str::from_utf8(&branch_output.stdout)?.trim().to_string(); // 转换为字符串并去除空白

    // 步骤 4: 组合所有信息，格式化为最终的版本字符串。
    // 注意：整个字符串被额外的双引号 `"` 包裹，因为这个结果将作为字符串字面量插入到代码中。
    Ok(format!(
        "\"{branch}-{commit}{dirty}@{date}\"", // 格式: "分支名-提交哈希[-dirty]@日期"
        branch = branch_name,
        commit = parts[0], // parts[0] 是提交哈希
        dirty = dirty_suffix,
        date = parts[1]    // parts[1] 是日期
    ))
}

/// `build_version` 过程宏
///
/// 这是一个函数式的过程宏 (`#[proc_macro]`)。当在代码中以 `build_version!()` 的形式调用它时，
/// 编译器会执行此函数。
/// 它调用 `get_git_commit()` 函数来获取包含Git版本信息的字符串，
/// 然后将这个字符串转换为 `TokenStream`，替换掉源码中宏调用的位置。
///
/// 参数:
/// - `_item: TokenStream`: 对于函数式过程宏，这是宏调用时括号内的TokenStream。
///   由于 `build_version!()` 调用时不带参数，所以 `_item` 未被使用 (用 `_` 忽略)。
///
/// 返回:
/// - `TokenStream`: 一个包含了版本信息字符串字面量的TokenStream。
#[proc_macro] // 将此函数标记为一个过程宏
pub fn build_version(_item: TokenStream) -> TokenStream {
    // 调用 get_git_commit() 获取版本字符串。
    // `.unwrap()` 会在 `get_git_commit` 返回 `Err` 时导致编译期panic。
    // 这是过程宏中处理错误的常见方式，因为宏展开失败通常意味着编译无法继续。
    let version_str = get_git_commit().unwrap();
    // 将获取到的版本字符串 (它已经被双引号包裹) 转换为 TokenStream。
    // `TokenStream::from_str` 会将字符串解析为一个 Rust 表达式（在这里是一个字符串字面量）。
    // `.unwrap()` 假设 `version_str` 总是一个有效的表达式（因为它总是被双引号包裹的字符串）。
    TokenStream::from_str(version_str.as_str()).unwrap()
}

[end of crates/version/src/lib.rs]
