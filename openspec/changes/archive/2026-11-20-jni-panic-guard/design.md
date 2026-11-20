# Design

## 问题

Rust 函数以 `extern "system"` 导出给 JVM 后，panic **不得**越过边界：越过即 abort
（Rust 2021 起 `extern "C"`/`extern "system"` 函数体 unwinding 会终止进程）。一次 panic
于是不是「一次调用失败」，而是「整个应用连同全部会话一起消失」。

本仓的既有处置是 `jni_export_guard!`：

```rust
jni_export_guard!(&mut unowned_env, JNI_FALSE, |env| {
    destroy_session_inner(env, session_id)
})
```

守卫做两件事：

- `with_env` 的错误策略把 Rust 错误与 panic 转为 Java `RuntimeException`；
- 失败值由调用点声明（`JNI_FALSE`／`-1`／`()`），而不是类型默认值。

失败值必须由调用点声明的原因：渲染导出的失败值是 `-1`，而 `0` 在 Kotlin 侧意为
「空闲帧」。守卫若照搬「返回类型默认值」，panic 就被伪装成一帧无输出，渲染线程据此
继续空转——失败被降级成了静默。

## 现状复核

本次逐个核对了 `native/src` 下的全部 `#[unsafe(no_mangle)]` 导出：

| 形态 | 数量 | 经守卫 |
| --- | --- | --- |
| `pub(super) extern "system" fn Java_*` | 71 | 71 |
| `pub extern "system" fn Java_*` | 5 | 5 |
| `pub(super) unsafe extern "system" fn Java_*` | 1 | 1 |
| `pub unsafe extern "system" fn Java_*` | 1 | 1 |
| `pub extern "system" fn JNI_OnLoad` | 1 | 不适用 |

`JNI_OnLoad` 由 VM 在加载本库时调用，早于任何 JNI 方法，此刻既没有 `Env` 也没有
`EnvUnowned`，守卫无从套用；它的函数体只有 logger 安装，不含可失败的业务逻辑。

结论：现状合规，缺的是**防止回退**的手段。

## 修复

### 静态规则

新增 semgrep 规则 `jni-export-must-use-panic-guard`，用「导出声明 + 函数体首个 `}` 之前
必须出现 `jni_export_guard!`」的形状判定：

- 正则先匹配 `#[unsafe(no_mangle)]` + `extern "system" fn Java_*` 的声明与函数体开头；
- 负向断言要求函数体在第一个 `}` 之前出现 `jni_export_guard!`——守卫必须是函数体的
  第一条语句，放在后面的守卫覆盖不到它之前的代码；
- `JNI_OnLoad` 因函数名不匹配 `Java_*` 而天然豁免。

规则用 `pattern-regex`（正则）实现，而非 AST 匹配：本仓导出的形态高度规整
（参数无嵌套圆括号、签名内无花括号、返回类型只有 `jboolean`/`jint`/`()`），正则足以
覆盖；而 AST 匹配对 `pub(super)`、`unsafe extern` 与属性位置的解析差异会带来
不必要的误报面。

规则在真实代码树上跑出 0 处命中。合成样本用 semgrep 本身跑过（不是只用脚本模拟正则）：
缺守卫的导出命中、带守卫的导出不命中、`JNI_OnLoad` 不命中、普通 `pub` 函数不命中；
带 `paths` 过滤时同样命中，排除了「规则被路径过滤整体吞掉」这一假阴性。

### 交接协议的并发回归

取纹理结果的交接协议（`render-stability` 的「迟到结果的归还责任唯一」）此前只有两条
单线程用例，分别覆盖「先放弃」与「先回送」两种固定次序。协议的全部价值在「互斥」，
而互斥是并发属性——两条固定次序的用例证明不了交错下仍是恰好一方取走。

新增用例以真实线程反复制造交错（工作线程 `deliver` 与渲染线程 `abandon` 并发），
断言两侧的「取到」布尔值恰好一个为真：两边都真会让同一张纹理被归还两次，两边都假
则纹理随通道析构而无人归还（Vulkan 后端丢弃未呈现纹理是空实现，交换链图像永不归还，
累积后终端永久停帧）。

## 不做

- 不改守卫宏的实现与失败值口径：现状已满足「panic 转异常 + 失败值由调用点声明」。
- 不为 `JNI_OnLoad` 补守卫：加载期没有 `Env`。
- 不把「宿主是否捕获 Java 异常」纳入本能力：那是宿主的处置策略，边界只负责把
  panic 变成可归因的异常。
- 不把规则扩到非 JNI 导出：库内的 `pub` 函数没有「越过 FFI 边界」的问题。
