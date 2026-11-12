//! 设备轮询：把 wgpu 的结局归一成「继续 / 失败」，不让设备错误升级成进程崩溃。
//!
//! wgpu 的 `Device::poll` 有三种失败形态，**没有一种能被调用方的 `Result` 接住**：
//! - 返回 `Err(PollError::Timeout)`：这一步等满了还没排空。这是分步轮询的**常态**
//!   （下一步还会再等），不是错误。
//! - 返回 `Err(PollError::WrongSubmissionIndex)`：提交序号对不上，设备真出错。
//! - 直接 `panic!`：设备报出非轮询类错误时，wgpu 内部走 `handle_error_fatal`
//!   （`wgpu-30.0.1/src/backend/wgpu_core.rs:1924`）——panic 发生在 wgpu 内部，
//!   会一路穿出 JNI 边界变成未捕获的 Java 异常，直接杀掉整个进程。
//!   真机实测：切后台时遮罩帧回读撞上设备错误，整进程退出（2026-11-12 日志）。
//!
//! 故在此把三种形态归一：常态继续，真错误带原因退出。panic 也照原样带出原因
//! （不吞、不掩盖），只是不再让它掀桌子。取纹理的 `get_current_texture` 早先已按
//! 同一思路兜住（见 `pass::acquire`），此处是同一规范的另一半。

use std::time::Duration;

/// 一次设备轮询的原始结局：wgpu 返回的结果，或 wgpu 内部 panic 的载荷。
type PollAttempt = Result<Result<wgpu::PollStatus, wgpu::PollError>, Box<dyn std::any::Any + Send>>;

/// 设备轮询的归一结局。
pub(crate) enum PollOutcome {
    /// 轮询本身没出错：GPU 已排空，或这一步等满仍未排空（分步轮询的常态）。
    /// 调用方可以继续等自己的完成条件。
    Proceed,
    /// 设备出错：本次轮询之后不可能再排空，调用方应放弃并如实上报原因。
    Failed(String),
}

/// 轮询设备一次（最多等 `timeout`），把 wgpu 的结局归一成「继续 / 失败」。
pub(crate) fn poll_device_wait(device: &wgpu::Device, timeout: Duration) -> PollOutcome {
    poll_outcome(|| {
        device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(timeout),
        })
    })
}

/// 把一次「可能 panic 的轮询」归一成结局。
///
/// 接 panic 这一层单独抽出来，才能直接喂一个会 panic 的闭包做回归测试：
/// 真机上正是这道 panic 掀掉了整个进程，而它只在设备真出错时才出现，
/// 没法在测试里稳定复现。
fn poll_outcome(poll: impl FnOnce() -> Result<wgpu::PollStatus, wgpu::PollError>) -> PollOutcome {
    let polled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(poll));
    classify(polled)
}

/// 把一次轮询的原始结局归一成「继续 / 失败」。抽成纯函数以便单测覆盖，
/// 不必构造 GPU 设备。
fn classify(polled: PollAttempt) -> PollOutcome {
    match polled {
        Ok(Ok(_)) => PollOutcome::Proceed,
        // 等满一步还没排空是分步轮询的常态，不是错误：下一步还会再等。
        Ok(Err(wgpu::PollError::Timeout)) => PollOutcome::Proceed,
        Ok(Err(error)) => PollOutcome::Failed(error.to_string()),
        // wgpu 内部 panic：载荷是 `&'static str` 或 `String`（见 `crate::panic_text`）。
        Err(payload) => PollOutcome::Failed(one_line(&crate::panic_text(payload.as_ref()))),
    }
}

/// 把原因压成单行：wgpu 的错误文本自带缩进与换行树，压平后仍能一眼读出
/// 「哪一步、什么错」，也不会在按行采集的日志里断成半截。
fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::{PollOutcome, classify, one_line, poll_outcome};

    /// 轮询成功的两种形态都算「继续」。
    #[test]
    fn success_is_proceed() {
        assert!(matches!(
            classify(Ok(Ok(wgpu::PollStatus::Poll))),
            PollOutcome::Proceed
        ));
    }

    /// 等满一步还没排空是分步轮询的常态，不该被判成失败——否则每次分步轮询
    /// 都会报一次「设备出错」。
    #[test]
    fn timeout_is_proceed() {
        assert!(matches!(
            classify(Ok(Err(wgpu::PollError::Timeout))),
            PollOutcome::Proceed
        ));
    }

    /// 提交序号不匹配是设备真的报错，须判失败并带出原因。
    #[test]
    fn wrong_submission_index_is_failure_with_reason() {
        match classify(Ok(Err(wgpu::PollError::WrongSubmissionIndex(7, 3)))) {
            PollOutcome::Failed(reason) => assert!(reason.contains('7'), "{reason}"),
            PollOutcome::Proceed => panic!("提交序号不匹配不得判成继续"),
        }
    }

    /// wgpu 内部 panic 必须被归成「失败」而不是向上传播——它是本模块存在的
    /// 理由：真机上它会一路穿出 JNI 边界杀掉整个进程。
    #[test]
    fn panic_is_failure_carrying_the_message() {
        match classify(Err(Box::new("Error in Device::poll: Validation Error"))) {
            PollOutcome::Failed(reason) => {
                assert!(reason.contains("Device::poll"), "{reason}");
            }
            PollOutcome::Proceed => panic!("panic 不得判成继续"),
        }
    }

    /// 回归：会 panic 的轮询**不得**把 panic 抛给调用方。
    ///
    /// 这正是真机崩溃的那一步（`Device::poll` 在设备真出错时内部 panic）。
    /// 这里用会 panic 的闭包替身复现同一机制：panic 必须被接住并转成「失败」，
    /// 否则它会穿出 JNI 边界杀掉整个进程。
    #[test]
    fn panicking_poll_does_not_propagate() {
        let outcome = poll_outcome(|| panic!("Error in Device::poll: Validation Error"));
        match outcome {
            PollOutcome::Failed(reason) => {
                assert!(reason.contains("Device::poll"), "{reason}");
            }
            PollOutcome::Proceed => panic!("panic 不得判成继续"),
        }
    }

    /// 多行原因压成单行后内容不丢：wgpu 的错误是「首行 + 因果树」，
    /// 逐行去掉缩进与空行后用分隔符接起来。
    #[test]
    fn multiline_reason_is_flattened_without_loss() {
        let flattened = one_line("Validation Error\n\nCaused by:\n  Parent device is lost\n");
        assert_eq!(
            flattened,
            "Validation Error | Caused by: | Parent device is lost"
        );
    }
}
