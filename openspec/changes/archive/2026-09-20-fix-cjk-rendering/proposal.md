# fix-cjk-rendering

## 概述

修复中文（CJK）渲染链路问题：此前模拟器 en-US locale 时 CJK fallback 被跳过导致回退到错误字体（宋体/发虚），且存在"输入中文需点击才出现"的显示不稳定问题。已确认 zh-CN locale 下 `CJK_FALLBACK: found 3` 且 fonts.xml 精确匹配 `NotoSansCJK-Regular.ttc` 首位。

## 背景

- docs/specification/DESIGN.md 要求"遵循 Android 系统 fonts.xml 解析字体信息"、"CJK 字体正常渲染且与设置字体对应（简体中文用户用 Noto Sans CJK SC 而非 Serif/JP）"、"CJK 渲染速度与西文基本一致"
- 真机取证：模拟器 `/system/fonts/` 有 `NotoSansCJK-Regular.ttc`（Sans，黑体）与 `NotoSerifCJK-Regular.ttc`（Serif，宋体）；fonts.xml zh-Hans 链精确指向 Sans ttc index=2
- locale 已验证：`persist.sys.locale=zh-CN` + `adb reboot` 后 `setSystemLocale(zh-CN)` 生效，`CJK_FALLBACK: found 3`，`FONTS_XML_FALLBACK: file='NotoSansCJK-Regular.ttc' index=2 id=ID(InnerId(56v1))` 排第一
- 中文渲染已确认存在（OCR 识别出中文文本），IME 为拼音键盘

## 任务

- [ ] 1 取证并修复 CJK 字形渲染质量（发虚根因：CJK fallback 字形尺寸/缩放与单元格匹配）
- [ ] 2 IME 中文输入链路验证（输入触发、首帧可见性、点击行为）
- [ ] 3 CJK 渲染性能（渲染速度与西文一致，输入法动画流畅）