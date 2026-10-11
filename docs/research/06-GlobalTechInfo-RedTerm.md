# 06 GlobalTechInfo/RedTerm

上游：<https://github.com/GlobalTechInfo/RedTerm>
本地：/tmp/competitor-study/GlobalTechInfo__RedTerm
版本：v2.1.0（2026-10-09），272 文件。Kotlin 应用层复用 Termux 终端库（v0.118.3）加 View 体系，proot 多发行版，无 X11，无文档提供器。全部源码已通读。

## 架构

`distro/` 管发行版注册、安装、启动脚本与诊断共 16 文件；`proot/` 管 proot 二进制安装；`service/` 管前台终端服务与唤醒锁；`ui/` 约 45 Activity 管终端、设置、备份、录制、SSH、文件浏览、编辑器、小部件；`util/` 管 SSH 与 SFTP 自研客户端；`assets/` 捆绑 bashrc、发行版图标、字体、四架构 ssh 与 xz、初始化脚本。

## 发行版与启动

11 发行版注册，10 个走 proot-distro 固定哈希，仅 kali 走官网无哈希。启动经单一 `launch.sh` 生成器：固定 HOME 与 PATH 与 TERM 与 LANG，经 proot 挂载 dev、proc、sys、system、apex、存储与链接器配置，备份排除同源。shell 引用做 POSIX 转义。安装支持断点续传与哈希校验、xz 双解码、权限修复、Android DNS 写 resolv.conf、rootfs 修复。启动脚本按发行版生成并可单测。

## 会话与服务

前台服务 specialUse 类型，通知常驻不可划，带获取释放与退出三动作，标题取会话描述。会话描述符经 SharedPreferences 持久化，进程被杀后恢复，此条本项目禁止。另有一键命令执行器与交互终端同环境。

## 界面与主题

终端 Activity 为抽屉加终端视图加双排键栏加搜索面板。键栏 token 为常用功能键，可编辑与自动隐藏。搜索扫 transcript 高亮并计数跳转。9 套主题同时染应用、终端与键栏，256 色由前后景推导，此条本项目禁止。无背景图透明模糊实现。

## 备份录制与传输

备份走共享存储需全部文件权限，新包完整后才替换旧包并清理孤儿。录制因库无原始钩子而用 script 命令经 pty 保色，回放用哑 pty。SSH 自带四架构二进制经启动器调起，SFTP 自研帧协议，密钥库支持主流算法，另有隧道管理。无文档提供器，仅暴露备份目录。

## 与本项目对照

本项目禁内嵌 proot、预装发行版、X11、自定义主题、会话持久化；要求单 bootstrap 经 URL 加原子替换、文档提供器暴露 home、通知常驻带退出。竞品发行版、主题、持久化三条主线均不可抄。通知退出形态、键栏搜索选择行为、备份完成替换语义、DNS 写入等机制可参考。

## 可参考点

- P0：单源启动脚本加挂载清单同源加引用转义；通知退出常驻形态；键栏 token 与双排与自动隐藏；搜索面板加 transcript 扫描加高亮；备份完成后替换加孤儿清理；DNS 写 resolv.conf。
- P1：启动脚本纯函数可单测；崩溃与 ANR 与诊断与磁盘占用的排障矩阵；快捷设置磁贴与快捷方式形态。
- P2：字体导入管理；bashrc 模板；传输错误可解释化；密钥与已知主机交互。

## 禁止项勿抄

内嵌 proot 及解压链、多发行版注册与安装全套、自带 ssh 与 xz 与兼容库、9 自定义主题与调色推导、会话持久化与恢复、全部文件权限换文档提供器。

证据：`Distro.kt:3-32`、`DistroRegistry.kt:4-167`、`ProotLaunch.kt:24-159`、`DistroInstaller.kt:22-1097`、`DistroSetup.kt:10-475`、`ProotInstaller.kt:39-112`、`TerminalService.kt:16-305`、`SessionStore.kt:20-49`、`themes.xml:3-140`、`SearchHighlightOverlay.kt:12-98`、`BackupManagerActivity.kt`、`SessionRecorder.kt:66-72`、`SshClient.kt:273-414`。
