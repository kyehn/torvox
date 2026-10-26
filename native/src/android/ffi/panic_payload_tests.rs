use super::take_panic_text_and_leak;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// 载荷的 `Drop` 会置位。用它来判定「载荷有没有被销毁过」。
struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// 回归钉：解卷路径上**绝不销毁** panic 载荷。
///
/// 载荷若在此按值销毁、而它的 `Drop` 又 panic，就是「解卷中解卷」——Rust 直接
/// `abort`，`jni_export_guard!` 接住的 panic 就此变成整进程崩溃，调用方连
/// Java 异常都拿不到。代价只是漏几字节（载荷通常是 `&str`/`String`），
/// 换来的却是保住已备好的诊断信息。
///
/// 判别力：去掉 `mem::forget` → 载荷被销毁 → `DropFlag` 置位 → 用例判红。
#[test]
fn panic_payload_is_never_dropped_on_the_unwinding_path() {
    let dropped = Arc::new(AtomicBool::new(false));
    let payload: Box<dyn std::any::Any + Send> = Box::new(DropFlag(Arc::clone(&dropped)));
    let _text = take_panic_text_and_leak(payload);
    assert!(
        !dropped.load(Ordering::SeqCst),
        "解卷路径上销毁 panic 载荷会招致双重 panic（进程 abort）"
    );
}

/// 文本仍要取得到：泄漏的是载荷本身，不是诊断信息。
#[test]
fn panic_payload_text_is_still_reported() {
    // 显式标注 `&'static str`：否则推断可能选成与本函数泛型无关的短生命周期
    // `&str`，载荷的 `TypeId` 便对不上，downcast 落空。
    let static_text: &'static str = "boom";
    assert_eq!(
        take_panic_text_and_leak(Box::new(static_text)),
        "boom",
        "&'static str 载荷应原样透出"
    );
    assert_eq!(
        take_panic_text_and_leak(Box::new(String::from("owned boom"))),
        "owned boom",
        "String 载荷应取出内容"
    );
    assert_eq!(
        take_panic_text_and_leak(Box::new(42u32)),
        "non-string panic payload",
        "未知载荷退化为占位文本，不得 panic"
    );
}
