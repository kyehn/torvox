# 对齐超大配置文件的边界口径

## Why

`openspec/specs/termux-configuration` 的「读取路径与容错」里，「配置过大」场景写的是
「配置取默认值」，而同一条 requirement 的总则写的是「文件读取或解析失败 MUST 保留
上一次成功的解析结果」。实现走的是后者（`TermuxPropertiesRepository.reloadNow` 读失败
时用 `lastGood.properties`）：首次加载时两者一致，重读时才会分叉。

`docs/specification/config.md` 只写「视为无效并跳过」，读者会以为整份配置回到默认值。

## What Changes

- 「配置过大」场景改为「沿用上一次成功的配置；首次加载即默认值」。
- `config.md` 补同一口径：超大文件不读进内存，沿用上一次成功的配置。

## Impact

- 受影响能力：`termux-configuration`。
- 用户可见：无行为变化，只把规范写成实现本来的样子。
