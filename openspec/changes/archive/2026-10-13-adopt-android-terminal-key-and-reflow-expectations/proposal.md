# 采纳同类安卓终端的按键期望与重排回归

## 背景

对照四个同类安卓终端仓库（`bingnnvjn/phanes`、`Lord1Egypt/AndroidThothTerm`、
`imlunixdev/puppy`、`mrndstvndv/termux-ghostty`）逐项检查后，两处本仓的断言缺口
被它们各自的既有断言直接暴露。两处都已修复并加了判别力验证。

## 变更

### 1. Ctrl 数字行/标点行的折叠期望（采纳 AndroidThothTerm）

`TerminalInputEncoder.controlByteForCodePoint` 为数字 2-8 与空格各写了专用分支，
标点行走 `c & 0x1F`。测试此前只钉 Ctrl+2 一条——把 3/4/5/6/7/8 任意一个分支改错、
或删掉标点那个公式，都不会有用例变红。

AndroidThothTerm 沿用 jackpal/androidterm 谱系的 `TermKeyListenerTest`，对全部
数字行与标点行给出了字面期望。按 `upstream-alignment` 的采纳门槛，这些期望来自
外部真相（VT100 传统 + 既有实现），不是本仓实现输出的回抄，故采纳。

### 2. 缩放往返的重排回归（采纳 phanes）

phanes 的工单 15 记录过一个真实缺陷：连续放大（列数减少）到临界点再缩小回来，
一行内容变成两份。为此他们写了两个回归回路。本仓三条 resize 用例都不检查内容，
该缺陷在本仓同样无人看守。

新增三条断言，判据是屏幕文本本身。

## 不采纳的对照项

- **AndroidThothTerm 的 `controlSequences/*.txt` 与 `wideChars/*.txt`**：
  仓库内无任何代码引用（grep 全仓为空），是失效语料。本仓已有 vttest 血统的
  内联语料（59 例）覆盖同类语义，采纳它们只会增加第二份真相。
- **termux-ghostty 的 `ScreenSnapshotE2ETest`（71 例）**：其 Tier 2/3 大量用例
  只断「反序列化不抛异常」与「字段等于自己刚写入的值」，属自验证；且它针对的是
  裸指针 + 手工布局的快照协议，本仓走 typed JNI 返回值，不存在该边界。
- **puppy 的仓库结构**：448/491 个入库文件是 `app/build/` 与 `app/.cxx/` 构建
  产物。不得反向采纳。
