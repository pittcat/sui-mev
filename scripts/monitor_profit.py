# -*- coding: utf-8 -*-
# 该脚本用于监控Sui区块链上特定地址的利润。
# 它会定期检查地址的余额，并在余额增加超过特定阈值时发送Telegram通知。
# 这对于追踪套利机器ンの盈利情况非常有用。

import requests  # 用于发送HTTP请求，与Sui节点和Telegram API通信
import time      # 用于在每次检查之间添加延迟
from decimal import Decimal  # 用于精确的十进制数运算，避免浮点数精度问题

# 全局变量定义

profit_address = ""  # 需要监控利润的Sui地址
URL = ''  # Sui RPC节点的URL，用于查询区块链数据
SUI_ARB_BOT_TOKEN = ""  # Telegram机器人的Token，用于发送通知
GROUP_SUI_ARB = ""  # Telegram群组的ID，通知将发送到此群组
THREAD_ONCHAIN_LARGE_PROFIT = ""  # Telegram群组中特定话题的ID，用于发送大额利润通知
BALANCE_DIFF_THRESHOLD = 500000000  # 余额差异阈值（以最小单位SUI，即MIST为单位，1 SUI = 10^9 MIST）
                                   # 当余额增加超过此阈值时，会触发通知

profit_address_balance = 0  # 用于存储上一次查询到的利润地址余额，初始化为0


def get_current_balance(profit_address):
    """
    查询指定Sui地址的当前余额。

    参数:
    profit_address (str): 需要查询余额的Sui地址。

    返回:
    str: 地址的当前总余额（以MIST为单位）。如果查询失败，则返回"0"。
         注意：返回的是字符串形式的数字，需要在使用时转换为整数或Decimal。
    """
    # 构建Sui RPC请求的payload
    # "jsonrpc": "2.0" 指定JSON-RPC协议版本
    # "id": 1 是请求的唯一标识符
    # "method": "suix_getBalance" 是要调用的Sui API方法，用于获取余额
    # "params": [profit_address] 是传递给方法的参数，即要查询的地址
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "suix_getBalance",
        "params": [profit_address]
    }

    try:
        # 向Sui RPC节点发送POST请求
        response = requests.post(URL, json=payload)
        response.raise_for_status()  # 如果HTTP请求返回错误状态码（4xx或5xx），则抛出异常
    except requests.exceptions.HTTPError as http_err:
        print(f'HTTP请求错误: {http_err}')  # 打印HTTP错误信息
        return "0" # 发生错误时返回 "0"
    except Exception as err:
        print(f'发生其他错误: {err}')  # 打印其他类型的错误信息
        return "0" # 发生错误时返回 "0"

    # 检查响应状态码是否为200 (OK)
    if response.status_code == 200:
        result = response.json()  # 将响应内容解析为JSON格式
        if 'error' in result:
            # 如果响应中包含错误信息，则打印错误
            print(f'RPC错误: {result["error"]}')
            return "0"
        elif 'result' in result and "totalBalance" in result["result"]:
            # 如果响应中包含结果，则返回总余额
            # result["result"]["totalBalance"] 包含了地址的余额信息
            return result["result"]["totalBalance"]
        else:
            # 如果响应格式不符合预期，则打印提示信息
            print('非预期的响应格式或结果中缺少totalBalance。')
            return "0"
    else:
        # 如果状态码不是200，则打印状态码
        print(f'响应状态码: {response.status_code}')
        return "0"


def monitor_profit(profit_address):
    """
    监控利润地址的余额变化，并在余额增加超过阈值时发送Telegram通知。

    参数:
    profit_address (str): 需要监控利润的Sui地址。
    """
    global profit_address_balance  # 声明profit_address_balance为全局变量，以便修改它

    # 获取当前利润地址的余额
    current_profit_address_balance_str = get_current_balance(profit_address)
    
    # 检查获取余额是否成功，如果不成功或者地址本身就没有余额且之前记录余额也为0，则打印信息并跳过此次处理
    if current_profit_address_balance_str == "0":
        # 如果之前记录的余额也为0，说明可能是初始化阶段或RPC持续有问题
        if profit_address_balance == 0:
            print("无法获取当前余额或当前余额为0，且先前记录余额为0，暂时跳过利润计算。")
        else:
            # 如果之前记录的余额不为0，但现在获取到0，可能是一个暂时的RPC问题，或者余额真的变为0
            print(f"无法获取当前余额或当前余额为0 (先前余额: {profit_address_balance} MIST)，暂时跳过利润计算。")
        return

    current_profit_address_balance = int(current_profit_address_balance_str)


    # 检查当前余额与上次记录的余额之差是否大于等于阈值
    # BALANCE_DIFF_THRESHOLD 是预设的利润阈值
    # 确保 profit_address_balance 也被正确初始化（在 main 中处理）
    if profit_address_balance != 0 and current_profit_address_balance - profit_address_balance >= BALANCE_DIFF_THRESHOLD:
        
        # 如果利润超过阈值，获取与该利润相关的最新交易哈希
        profit_tx_hash = get_tx(profit_address=profit_address)
        
        # 将余额从MIST转换为SUI，便于阅读 (1 SUI = 10^9 MIST)
        profit_address_balance_decimal = Decimal(profit_address_balance) / Decimal(10**9)
        current_profit_address_balance_decimal = Decimal(current_profit_address_balance) / Decimal(10**9)
        profit_decimal = current_profit_address_balance_decimal - profit_address_balance_decimal

        if profit_tx_hash:
            # 如果成功获取到交易哈希
            # Telegram的Markdown格式要求对某些特殊字符（如'_'）进行转义
            profit_tx_hash_md = profit_tx_hash.replace('_', '\\_')  
            
            # 构建Telegram通知消息
            # 消息中包含：交易哈希的链接、之前的余额、当前余额和利润额
            msg = (
                f'*监控到大额利润交易*: [{profit_tx_hash_md}]({profit_tx_hash_md})\n' # 链接到Sui浏览器
                f'*先前余额*: `{profit_address_balance_decimal:.9f}` SUI\n' # .9f 表示保留9位小数
                f'*当前余额*: `{current_profit_address_balance_decimal:.9f}` SUI\n'
                f'*利润*: `{profit_decimal:.9f}` SUI'
            )
            # 发送Telegram消息
            send_telegram_message(SUI_ARB_BOT_TOKEN, GROUP_SUI_ARB, THREAD_ONCHAIN_LARGE_PROFIT, msg)
            
    # 更新全局变量profit_address_balance为当前查询到的余额，用于下一次比较
    profit_address_balance = current_profit_address_balance


def get_tx(profit_address):
    """
    查询指定Sui地址最近的一笔收款交易。

    参数:
    profit_address (str): 需要查询交易的Sui地址。

    返回:
    str or None: 最新一笔收款交易的哈希，并构造成SuiVision浏览器的链接。如果查询失败或没有交易，则返回None。
    """
    # 构建Sui RPC请求的payload
    # "method": "suix_queryTransactionBlocks" 是用于查询交易块的方法
    # "params": 包含了查询条件：
    #   - {"filter": {"ToAddress": profit_address}}: 按接收地址过滤，即查找发送到profit_address的交易
    #   - "options": 指定返回的交易详情选项，这里只要求返回最少必要的信息以提高效率
    #   - 1: 限制返回结果数量为1，即我们只需要最新的那笔交易
    #   - True: 表示按降序排列（最新的在前）
    params = [
        {"filter": {"ToAddress": profit_address}, 
         "options": {"showEffects": False, "showInput": False, "showEvents": False, "showObjectChanges": False, "showBalanceChanges": False}}, # 精简options，仅获取digest
        None,  # cursor, for pagination, not used here
        1,     # limit
        True   # descending order
    ]

    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "suix_queryTransactionBlocks",
        "params": params
    }

    try:
        # 向Sui RPC节点发送POST请求
        response = requests.post(URL, json=payload)
        response.raise_for_status()  # 检查HTTP错误
    except requests.exceptions.HTTPError as http_err:
        print(f'HTTP请求错误 (get_tx): {http_err}')
        return None
    except Exception as err:
        print(f'发生其他错误 (get_tx): {err}')
        return None

    tx_hash = None  # 初始化交易哈希为None
    if response.status_code == 200:
        result = response.json()
        if 'error' in result:
            print(f'RPC错误 (get_tx): {result["error"]}')
        elif 'result' in result and result["result"]["data"] and len(result["result"]["data"]) > 0: # 确保data字段存在且不为空列表
            # 获取交易摘要 (digest)，即交易哈希
            tx_hash = result["result"]["data"][0]["digest"]
            # 构建SuiVision浏览器的链接，方便直接查看交易详情
            tx_hash = f'https://suivision.xyz/txblock/{tx_hash}'
        else:
            print('非预期的响应格式或没有找到交易 (get_tx)。')
    else:
        print(f'响应状态码 (get_tx): {response.status_code}')
    return tx_hash


def send_telegram_message(bot_token, chat_id, thread_id, message):
    """
    通过Telegram机器人发送消息。

    参数:
    bot_token (str): Telegram机器人的API Token。
    chat_id (str): 目标聊天（群组或用户）的ID。
    thread_id (str): 目标群组中话题（thread）的ID。如果是一般聊天，此参数可能被忽略或导致错误，取决于群组设置。
    message (str): 要发送的消息内容。

    返回:
    dict: Telegram API的响应结果 (JSON格式)。
    """
    # Telegram Bot API的URL，用于发送消息
    url = f"https://api.telegram.org/bot{bot_token}/sendMessage"
    # 构建请求的payload
    payload = {
        "chat_id": chat_id,                   # 接收消息的群组ID
        "message_thread_id": thread_id,       # 群组中的话题ID
        "text": message,                      # 消息内容
        "parse_mode": "Markdown"              # 指定消息格式为Markdown，允许使用加粗、链接等
    }
    try:
        # 发送POST请求到Telegram API
        response = requests.post(url, json=payload)
        response.raise_for_status() # 检查HTTP错误
        return response.json()  # 返回API的响应
    except requests.exceptions.HTTPError as http_err:
        print(f'Telegram发送HTTP错误: {http_err}, Response: {response.text}')
        return {"ok": False, "error_code": response.status_code, "description": response.text}
    except Exception as e:
        print(f'Telegram发送时发生其他错误: {e}')
        return {"ok": False, "description": str(e)}


# Python脚本的主入口点
if __name__ == '__main__':
    # 首先，初始化profit_address_balance为程序启动时的当前余额
    # 这样可以避免程序刚启动时，因为与初始值0比较而产生误报
    # (除非地址确实是新地址，余额为0)
    print(f"开始监控地址: {profit_address}")
    print(f"Sui RPC URL: {URL}")
    print(f"余额差异阈值: {BALANCE_DIFF_THRESHOLD} MIST")

    initial_balance_str = get_current_balance(profit_address)
    if initial_balance_str != "0": # "0" 表示获取失败或真实余额为0
        profit_address_balance = int(initial_balance_str)
        print(f"程序启动，初始监控地址余额: {profit_address_balance} MIST ({Decimal(profit_address_balance)/Decimal(10**9):.9f} SUI)")
    else:
        # 如果get_current_balance返回"0"，可能是RPC错误或真实余额为0
        # 在这种情况下，profit_address_balance保持其默认值0或根据具体错误处理逻辑设定
        # 这里的逻辑是，如果获取失败，则 profit_address_balance 维持为 0，
        # monitor_profit 函数会在 profit_address_balance 为 0 时跳过首次利润计算（除非当前余额也为0）
        # 或在能够获取到余额后，再进行正常的利润计算。
        print("程序启动，无法获取初始余额或初始余额为0。将基于后续成功获取的余额进行利润计算。")
        profit_address_balance = 0 # 明确设置为0，如果之前不是的话


    # 无限循环，持续监控利润
    while True:
        # 调用monitor_profit函数来检查并报告利润
        monitor_profit(profit_address)
        # 暂停1秒，然后继续下一次检查
        # time.sleep(1) 可以根据实际需求调整轮询频率
        # 过于频繁的请求可能会给RPC节点带来压力，或者被限速
        time.sleep(1)
