# 24 Rprop/AnsolePlus

上游：<https://github.com/Rprop/AnsolePlus>
本地：/tmp/competitor-study/Rprop__AnsolePlus
版本：主线 2025-10-01 取样，179 文件。旧版 Termux 打包，图像协议剥离。`diff -rq` 全模块比对已做。

## 血缘与回退

相对上游：模拟器删图像类、会话客户端删三档日志接口、`termux.c` 有一处释放错位（上游 `ReleaseStringUTFChars(cwd, cmd_cwd)` 被改成 `(cmd, cmd_cwd)`，属 JNI 引用释放错位缺陷），应用层删外部命令、文件选择、设置片段等。整体为旧基线删减打包。

## 与本项目对照

无新增能力。JNI 释放错位属反面教材：字符串须配对释放，错位即泄漏或崩溃。沿用 02 上游行为参考即可。

## 可参考点

- 无。反面价值：基线删减与 JNI 失配的教训。

## 禁止项勿抄

删减后的模拟器、错位的 JNI 释放、删减的应用层。

证据：三方 `diff -rq` 结果、`TerminalSessionClient.java` 接口删减、`termux.c:164-165`。
