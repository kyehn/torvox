# Proposal

## Why

真机 logcat 里，同一字族的下发被逐次记录：

```text
D/native::render::font::pipeline::families: 字体族 'Maple Mono Normal NF CN' 已解析为字面 InnerId(4v1)
D/native::android::ffi::font: setFontFamily: Maple Mono Normal NF CN found=true changed=false
```

同一字族在一次会话里被下发二十余次（设置应用、字体应用与会话状态发射三条路径都会
下发），每次都记一行「解析成了哪个字面」与「没有变化」。这些行不带新信息，却把真正
的告警挤出可读范围——日志分级是区分应用缺陷与环境噪声的第一道闸门，逐次重复的
无变化记录等于把这道闸门废掉。

根因是记录点选错了位置：解析结果与「本次是否真的换了字面」是两件事，日志只该记
后者。前者每次下发都会重算一遍，自然每次都有一行。

## What Changes

- 字体管线的解析日志只在字面真的换了时记录：同一字族重复下发不再逐次刷屏。
- JNI 层的字族下发日志同样只在字面切换时记录；未变化时不再记「未变化」一行。
- 字族找不到的告警照旧出声（它由管线自己记录），去重不吞异常信号。
- 不改公开 API，不改字体的解析与选取逻辑。

## Capabilities

### Modified Capabilities

- `logcat-hygiene`：新增「无状态变化的重复下发不得逐次记日志」要求。
