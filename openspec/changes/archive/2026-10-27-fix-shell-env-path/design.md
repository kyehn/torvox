# Design

## 合成规则

```text
PATH = 宿主 PATH 原值
     + （{prefix}/bin 目录存在时）":{prefix}/bin"
```

两处顺序取舍：

- **Termux 的 bin 目录在前**：Termux 引导程序提供 GNU 版 `grep` `sed` `ls`，
  系统的 toybox 同名命令能力更弱（`grep -P` 等 GNU 专有开关缺失）。bin 目录在前
  保持既有解析结果不变，只把系统命令补进 PATH，不产生行为回退。
- **宿主 PATH 原值在后**：系统命令（`am` `pm` `getprop` `dumpsys`）由此可按名解析，
  与 DESIGN Shell 节「原 `PATH` 附加」的要求一致。

宿主没有 `PATH` 时只给 `{prefix}/bin`，不拼出空前缀（`":/x"` 会被 shell 当作
「当前目录」项）。

## 注入点

`base_env_with_host` 已有宿主变量注入（`host_var`），本次再增一个「目录是否存在」
的注入（`dir_exists`）。两者都依赖运行环境，测试经注入取值，断言不随 CI runner
漂移——沿用既有 `base_env_with_host` 的封闭断言模式。

## 不做的事

- 不改 `docs/specification/DESIGN.md`：约束已存在，本次只修正实现与主规范的偏差。
- 不引入 `LD_LIBRARY_PATH` / `PWD` / `LD_PRELOAD`：DESIGN 明令禁止。
- 不为「引导程序已安装」增加额外状态标记：目录是否存在即判据。
