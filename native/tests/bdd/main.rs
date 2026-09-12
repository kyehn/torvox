//! Rust 终端引擎的中文 BDD 套件（cucumber-rs）。
//!
//! 每个 `tests/features/*.feature` 文件描述一条规范行为，步骤实现见
//! [`steps`]。只覆盖确定性公共 API：环境变量白名单、链接识别、
//! OSC 7/8/52、回显与光标、中文列宽、滚动回滚、窗口标题。

mod steps;

use cucumber::World as _;
use steps::TerminalWorld;

#[tokio::main]
async fn main() {
    TerminalWorld::run("tests/features").await;
}
