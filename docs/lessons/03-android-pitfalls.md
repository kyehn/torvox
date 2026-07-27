# Android 特定陷阱

## 1. JNA 不支持 Array\<ByteArray\> 参数类型

### 问题
JNA 接口方法中声明 `Array<ByteArray>` 参数类型会在运行时抛出：
```
Unsupported array argument type: class [B
```

这导致 `setExtraFontPaths` 在 `the terminal runtime start()` 初始化阶段崩溃，终端完全黑屏无法启动。

### 根因
JNA 的反射机制无法处理多维数组类型。它只知道如何映射 `byte[]` 到原生内存，但 `Array<ByteArray>` 这种"数组的数组"超出了 JNA 的类型系统表达能力。

### 修复
将 JNA 接口方法改为接受 `com.sun.jna.Pointer`，然后手动使用 JNA `Memory` 类分配原生内存：
1. 为指针数组分配一块连续内存
2. 为每个字符串的字节数组单独分配内存
3. 将指针写入指针数组

### 教训
- JNA 不支持多维数组或嵌套数组类型
- 复杂参数类型（字符串数组、结构体数组等）需要降级到 `Pointer` + 手动内存管理
- JNA 接口设计中，优先使用 `String`、`byte[]`、`int` 等基本类型
- JNA 调用失败时，终端完全黑屏无其他错误信息 → 设置日志记录启动阶段细节

### 相关提交
- `8428c9ec5d`: fix(bridge): convert Array ByteArray to JNA Pointer for setExtraFontPaths

## 2. Keyboard Jelly Effect — 软键盘弹出时终端反复 resize

### 问题
软键盘弹出时，终端窗口反复 resize。这是因为 Activity 的 `adjustResize` 模式和 Compose 的 `imePadding()` 配合不当，导致键盘弹出→窗口resize→键盘位置变化→窗口又resize 的反复循环（jelly effect）。

### 修复
1. 使用 `adjustResize` + `setDecorFitsSystemWindows(false)` 组合
2. 添加 200ms 时间戳基础的输入抑制（在焦点过渡期间忽略输入事件）
3. ModifierBar 使用 `imePadding()` 随键盘升起

### 教训
- Compose 中软键盘和 `imePadding()` 的配置需要与 Activity 的 windowSoftInputMode 匹配
- `adjustResize` 是终端模拟器的推荐模式
- 焦点过渡期间的输入事件需要抑制（输入法切换时会产生伪输入）

## 3. Coroutine Scope 泄漏

### 问题
`writeToPty` 和 `pasteFromClipboard` 使用了错误的 CoroutineScope（全局 scope），导致 ViewModel 被销毁后协程仍在运行。当 ViewModel 重建时，旧协程可能仍在向已关闭的 PTY 写入数据。

### 修复
使用 `viewModelScope` 替代全局 scope，确保 ViewModel 销毁时协程自动取消。

### 教训
- Android ViewModel 中的协程必须使用 `viewModelScope`
- 生命周期感知的协程作用域是 Android 开发中的基本模式
- 泄漏的协程在重建场景中表现为不可预测的行为

## 4. 设置默认值不同步

### 问题
`useNerdFontGlyphs` 在 ViewModel 中默认为 `true`，但在 `SettingsRepository` 中默认为 `false`。导致用户设置不生效，因为 ViewModel 总是用自己默认值覆盖 Repository 的值。

### 教训
- ViewModel 的默认值必须与 Repository/DataStore 的默认值一致
- 推荐: Repository 作为唯一真实源，ViewModel 不设独立的默认值
- 添加测试来验证 ViewModel 初始化时从 Repository 读取的值

### 相关提交
- `4f9da7a1e9`: fix(android): remove file manager, fix keyboard jelly, search highlight, logging
- `52c891de10`: fix(android): cursor style renderer push, background image UI, coroutine leak, font defaults
