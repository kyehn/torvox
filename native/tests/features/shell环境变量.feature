# language: zh-CN
功能: 子进程环境变量白名单
  作为终端模拟器
  为了遵循规范只设置声明过的环境变量
  不得设置 LD_LIBRARY_PATH、PWD、LD_PRELOAD 等未声明变量

  背景:
    假如 家目录为 "/data/data/com.termux/files/home"
    而且 前缀为 "/data/data/com.termux/files/usr"

  场景: 规范要求的变量齐全
    当 构建子进程环境变量
    那么 环境变量 "TERM" 的值为 "xterm-256color"
    而且 环境变量 "COLORTERM" 的值为 "truecolor"
    而且 环境变量 "LANG" 的值为 "en_US.UTF-8"
    而且 环境变量 "HOME" 的值为 "/data/data/com.termux/files/home"
    而且 环境变量 "TERMUX_HOME_DIR_PATH" 的值为 "/data/data/com.termux/files/home"
    而且 环境变量 "PREFIX" 的值为 "/data/data/com.termux/files/usr"
    而且 环境变量 "TERMUX_PREFIX_DIR_PATH" 的值为 "/data/data/com.termux/files/usr"
    而且 环境变量 "TMPDIR" 的值为 "/data/data/com.termux/files/usr/tmp"
    而且 环境变量 "TERMUX_TMP_PREFIX_DIR_PATH" 的值为 "/data/data/com.termux/files/usr/tmp"
    而且 环境变量 "TERMUX_VERSION" 的值为 "0.119.0-beta.3"

  场景: 禁止未声明变量
    当 构建子进程环境变量
    那么 环境变量中不存在 "LD_LIBRARY_PATH"
    而且 环境变量中不存在 "LD_PRELOAD"
    而且 环境变量中不存在 "PWD"
