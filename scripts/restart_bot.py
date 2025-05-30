#!/usr/bin/env python3
# -*- coding: utf-8 -*-

# 该脚本用于定期重启在tmux会话中运行的MEV（Miner Extractable Value，矿工可提取价值）套利机器人。
# MEV套利机器人通常在区块链上寻找并执行有利可图的交易机会。
# 由于各种原因（例如内存泄漏、网络问题、机器人逻辑卡死等），长时间运行的机器人可能会变得不稳定或停止工作。
# 此脚本通过定期（例如每3小时）杀掉旧的tmux会话并启动一个新的会话来确保机器人的持续运行和稳定性。
# 它使用了`tmux`，一个终端多路复用器，允许在后台运行和管理多个终端会话。

import subprocess  # 用于执行外部命令，如tmux指令
import time        # 用于控制时间，例如设置重启间隔
import logging     # 用于记录脚本运行过程中的信息、警告和错误
from datetime import datetime  # 用于处理日期和时间，例如记录下次重启时间

# 配置日志记录器 (logging)
# 日志对于跟踪脚本的运行情况和排查问题非常重要。
logging.basicConfig(
    level=logging.INFO,  # 设置日志级别为INFO，表示记录INFO、WARNING、ERROR、CRITICAL级别的日志
    format="%(asctime)s - %(levelname)s - %(message)s",  # 定义日志消息的格式，包括时间、级别和消息内容
    handlers=[  # 指定日志处理器
        logging.FileHandler("bot_restarter.log"),  # 将日志写入到名为 "bot_restarter.log" 的文件中
        logging.StreamHandler()  # 同时也将日志输出到控制台（标准输出）
    ]
)

def restart_bot():
    """
    执行重启MEV套利机器人的主要逻辑。
    这个函数会：
    1. 尝试杀掉名为 "mev-arb-bot" 的现有tmux会话（如果存在）。
    2. 创建一个新的名为 "mev-arb-bot" 的tmux会话。
    3. 在新的tmux会话中发送启动机器人的命令。
    """
    try:
        # 步骤1: 杀掉可能存在的旧tmux会话
        # `subprocess.run()` 用于执行外部命令。
        # ["tmux", "kill-session", "-t", "mev-arb-bot"] 是要执行的命令和参数。
        #   - tmux: tmux程序。
        #   - kill-session: tmux命令，用于终止一个会话。
        #   - -t mev-arb-bot: 指定目标会话的名称为 "mev-arb-bot"。
        # `stderr=subprocess.PIPE` 用于捕获标准错误输出。如果会话不存在，`kill-session`会报错，
        # 但我们不希望脚本因此停止，所以捕获错误并记录日志。
        kill_result = subprocess.run(
            ["tmux", "kill-session", "-t", "mev-arb-bot"], stderr=subprocess.PIPE
        )
        if kill_result.returncode == 0:
            logging.info("成功终止已存在的tmux会话 `mev-arb-bot`。")
        else:
            # 如果返回码非0，通常意味着会话不存在，这也是可接受的。
            logging.info("tmux会话 `mev-arb-bot` 未找到或无法终止 (可能是首次运行)。错误信息: %s", kill_result.stderr.decode('utf-8').strip())


        # 步骤2: 创建新的tmux会话
        # `check=True` 表示如果命令执行失败（返回非零退出码），则会抛出 `CalledProcessError` 异常。
        #   - new-session: tmux命令，用于创建新会话。
        #   - -d: 以分离模式（detached）启动会话，即不在当前终端附加到该会话。
        #   - -s mev-arb-bot: 指定新会话的名称。
        subprocess.run(["tmux", "new-session", "-d", "-s", "mev-arb-bot"], check=True)
        logging.info("成功创建新的tmux会话 `mev-arb-bot`。")

        # 步骤3: 在新的tmux会话中发送启动机器人的命令
        # 这是实际启动套利机器人的命令。它是一个比较长的字符串。
        # `cargo run -r --bin arb start-bot` 表示使用Rust的包管理器cargo来编译并运行名为`arb`的项目中的`start-bot`程序。
        #   - `-r` 是 `--release` 的缩写, 表示以优化模式编译和运行，速度更快。
        #   - `--bin arb` 指定要运行的二进制文件。
        #   - `start-bot` 是传递给`arb`程序的参数，告诉它启动机器人。
        # 后面的参数是机器人的配置：
        #   - `--private-key {}`: 指定机器人操作Sui账户所需的私钥。注意：这里的 `{}` 是一个占位符，
        #     实际使用时需要替换为真实的私钥。在这个脚本的当前版本中，私钥没有被动态插入，
        #     这意味着机器人启动命令本身可能需要从配置文件或其他安全途径获取私钥。
        #     **重要安全提示**: 直接在脚本中硬编码私钥是非常不安全的做法。更好的做法是使用环境变量或配置文件。
        #   - `--use-db-simulator`: 可能表示使用数据库模拟器。
        #   - `--max-recent-arbs 5`: 可能限制最近套利记录的数量。
        #   - `--workers 10`: 指定工作线程数量为10。
        #   - `--num-simulators 18`: 指定模拟器数量为18。
        #   - `--preload-path /home/ubuntu/sui/pool_related_ids.txt`: 预加载与交易池相关的ID列表，可能用于加速机器人启动或交易发现。
        #   - `ENABLE_RECORD_POOL_RELATED_ID=1`: 这是一个环境变量，设置为1可能表示启用记录与交易池相关ID的功能。
        cmd = (
            "ENABLE_RECORD_POOL_RELATED_ID=1 cargo run -r --bin arb start-bot "
            # "--private-key {} "  # 注意：实际私钥应通过安全方式提供，而不是硬编码或留空
            "--use-db-simulator --max-recent-arbs 5 --workers 10 --num-simulators 18 "
            "--preload-path /home/ubuntu/sui/pool_related_ids.txt "
        )
        # 此处假设私钥是通过其他方式（例如机器人内部读取配置文件或环境变量）来管理的。
        # 如果机器人启动命令 `arb start-bot` 强制要求 `--private-key` 参数，
        # 那么这个脚本需要修改以安全地提供该参数。

        # `tmux send-keys` 命令用于向指定的tmux会话（或窗口、窗格）发送按键序列。
        #   - -t mev-arb-bot: 指定目标会话。
        #   - cmd: 要发送的命令字符串。
        #   - "Enter": 发送回车键，以执行命令。
        subprocess.run(
            ["tmux", "send-keys", "-t", "mev-arb-bot", cmd, "Enter"], check=True
        )
        logging.info("已向tmux会话发送机器人启动命令。机器人应已开始启动。")

    except subprocess.CalledProcessError as e:
        # 如果 `subprocess.run()` 中设置了 `check=True` 并且命令执行失败，则会捕获此异常。
        logging.error(f"执行tmux命令失败: {e}")
        logging.error(f"命令: {e.cmd}")
        logging.error(f"返回码: {e.returncode}")
        if e.stdout:
            logging.error(f"标准输出: {e.stdout.decode('utf-8')}")
        if e.stderr:
            logging.error(f"标准错误: {e.stderr.decode('utf-8')}")
    except Exception as e:
        # 捕获其他任何预料之外的错误。
        logging.error(f"发生意外错误: {e}")


def main():
    """
    脚本的主函数。
    它会启动一个无限循环，在每个预设的时间间隔后调用 `restart_bot()` 函数。
    """
    logging.info("机器人重启脚本已启动。")

    # 设置重启间隔为3小时（以秒为单位）
    # 3 小时 * 60 分钟/小时 * 60 秒/分钟
    interval = 3 * 60 * 60

    while True:  # 无限循环，以持续监控和重启机器人
        try:
            logging.info("开始执行机器人重启流程...")
            restart_bot()  # 调用重启函数

            # 计算并记录下一次重启的计划时间
            next_restart_timestamp = time.time() + interval
            next_restart_datetime = datetime.fromtimestamp(next_restart_timestamp)
            logging.info(
                f"机器人重启流程执行完毕。下次重启计划于: {next_restart_datetime.strftime('%Y-%m-%d %H:%M:%S')}"
            )

            # 等待指定的时间间隔
            time.sleep(interval)

        except KeyboardInterrupt:
            # 如果用户按下 Ctrl+C，则会捕获 KeyboardInterrupt 异常，脚本将优雅地退出。
            logging.info("脚本被用户终止。")
            break  # 跳出while循环，结束脚本
        except Exception as e:
            # 如果在主循环中发生其他错误（例如 restart_bot() 内部未捕获的异常），则记录错误。
            logging.error(f"主循环中发生错误: {e}")
            # 为了避免在发生持续错误时过于频繁地重试，这里可以设置一个较短的等待时间，
            # 例如等待60秒后再尝试下一次重启。
            logging.info("等待60秒后重试...")
            time.sleep(60)


# 当脚本作为主程序直接运行时（而不是作为模块导入时），执行 main() 函数。
if __name__ == "__main__":
    # 在实际部署时，需要确保机器人启动命令中的私钥（如果需要作为命令行参数传递）
    # 是通过安全的方式管理的，例如从受保护的配置文件读取或通过环境变量传入。
    # 此脚本目前假设机器人能自行处理私钥的获取。
    # 例如，机器人可能从环境变量 `BOT_PRIVATE_KEY` 读取私钥。
    # 在这种情况下，启动tmux会话后，可能还需要发送类似 `export BOT_PRIVATE_KEY='your_actual_key_here'` 的命令，
    # 或者在 `~/.bashrc` 或类似shell启动脚本中设置好环境变量，以确保tmux会话中的进程能访问到它。
    #
    # 示例（如果需要通过tmux发送环境变量设置）：
    # subprocess.run(["tmux", "send-keys", "-t", "mev-arb-bot", "export BOT_PRIVATE_KEY='your_key'", "Enter"], check=True)
    # 然后再发送机器人启动命令。但这仍然不理想，因为密钥会出现在命令历史中。
    # 最好的方式是机器人程序自身设计为从文件或专门的密钥管理服务读取。
    main()
