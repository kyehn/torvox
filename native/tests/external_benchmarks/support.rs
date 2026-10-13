//! 测试期外部数据取用。
//!
//! 基准数据（Unicode UCD、ghostty 模糊语料）都能由外部地址自动取得，按 BUILD
//! 「可从外部下载可以自动处理的文件不得 vendor」不入库，一律运行期现取。取不到即
//! 判红并附上 URL：基准缺失时静默跳过等于把断言摘掉。

use std::sync::OnceLock;
use std::time::Duration;

/// 取数客户端。复用连接以免每个文件重走 TLS 握手；单请求 30s 上限——UCD 两份文件
/// 合计数 MB，冷启动或服务端慢时仍能读完。
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into()
    })
}

/// 取回外部文本。失败（含 404）即带 URL 与原因判红。
pub fn fetch_text(url: &str) -> String {
    agent()
        .get(url)
        .call()
        .unwrap_or_else(|error| panic!("取外部基准失败 {url}：{error}"))
        .body_mut()
        .read_to_string()
        .unwrap_or_else(|error| panic!("读外部基准正文失败 {url}：{error}"))
}

/// 取回外部原始字节。模糊语料是未转义的控制序列，按文本读会损坏字节。
pub fn fetch_bytes(url: &str) -> Vec<u8> {
    let mut response = agent()
        .get(url)
        .call()
        .unwrap_or_else(|error| panic!("取外部基准失败 {url}：{error}"));
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut response.body_mut().as_reader(), &mut bytes)
        .unwrap_or_else(|error| panic!("读外部基准字节失败 {url}：{error}"));
    bytes
}
