# 22 frostre1997/kosh

上游：<https://github.com/frostre1997/kosh>
本地：/tmp/competitor-study/frostre1997__kosh
版本：主线 2026-08-20 取样，375 文件。与 ReTerminal 同系的 Termux View 加 Compose 壳加 proot。`diff -rq` 双向比对已做。

## 血缘与差异

相对 ReTerminal：组件库与主壳大面积改动，assets 预装 Alpine rootfs 与 busybox 与 python 与包脚本（ReTerminal 无此预装），初始化脚本改动，包名 `com.kosh` 独立。终端仍复用 Termux 会话。末提交为可写 HOME 与起于共享存储。

## 与本项目对照

结论与 15 基本相同并更差：预装 rootfs 与工具链撞禁止内嵌 bootstrap 与预装发行版；起于共享存储与本项目固定启动目录冲突；可写 HOME 方向虽一致但本项目不得修改 home。无新增可参考点，仅作同系换皮记录。

## 可参考点

- 无。沿用 15 的文档提供器与透传名单参考。

## 禁止项勿抄

预装 rootfs 与工具链、起于共享存储、proot 全套、复用 Termux 会话。

证据：双向 `diff -rq` 结果、`assets: alpine-rootfs.tar.gz/busybox/python/pkg.sh`、`init-host.sh` 差异。
