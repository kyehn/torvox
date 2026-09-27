//! 纯文本 URL 点击检测，基于 `linkify` crate。
//!
//! linkify 实现 RFC-3986 风格的 URL 扫描（Unicode/IRI、括号配对、尾部标点清理），
//! 手写正则无法企及；`url_must_have_scheme(true)` 把检测限定在显式 `scheme:` URL，
//! 裸 `www.` 或 `user@host` 不视为链接。唯一保留的手写模式是 linkify 故意跳过的
//! 无 `//` 主机段的纯 scheme 兜底（`mailto:`/`tel:`/`sms:`/`callto:` 等）。
use linkify::{LinkFinder, LinkKind};
use std::sync::OnceLock;

/// linkify 扫描器会跳过、但终端点击仍须打开的无 `//` 主机段纯 scheme 协议
/// （邮件客户端、拨号器等）；`ipfs`/`ipns` 对应冒号形式 `ipfs:<cid>`。
const SCHEME_ONLY_PROTOCOLS: &[&str] = &["mailto", "tel", "sms", "callto", "ipfs", "ipns"];

fn finder() -> LinkFinder {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);
    finder.url_must_have_scheme(true);
    finder
}

fn scheme_only_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let schemes = SCHEME_ONLY_PROTOCOLS.join("|");
        regex::Regex::new(&format!(r#"(?i)(?P<url>(?:{schemes}):[^\s<>,;!'"\\]*)"#))
            .expect("scheme_only_regex: invalid pattern")
    })
}

fn trim_trailing_punctuation(raw: &str) -> String {
    raw.trim_end_matches(['.', ',', ';', ':', '!', '?', '"', '\'', ')'])
        .to_string()
}

/// 在 `line`（每列一字符的终端行，宽字符展开为两个副本）中查找列区间包含 `col`
/// 的 URL 并返回清理后的结果。供 `hyperlinkAt` 的纯文本 URL 点击回退使用。
pub fn url_at_column(line: &str, col: usize) -> Option<String> {
    for link in finder().links(line) {
        let start = line[..link.start()].chars().count();
        let end = line[..link.end()].chars().count();
        if start <= col && col < end {
            return Some(link.as_str().to_string());
        }
    }
    for caps in scheme_only_regex().captures_iter(line) {
        let m = caps.name("url")?;
        let start = line[..m.start()].chars().count();
        let end = line[..m.end()].chars().count();
        if start <= col && col < end {
            return Some(trim_trailing_punctuation(m.as_str()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_http_and_https() {
        assert_eq!(
            url_at_column("see https://example.com", 5),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            url_at_column("see http://example.com/path", 5),
            Some("http://example.com/path".to_string())
        );
    }

    #[test]
    fn detects_multiple_protocols() {
        // linkify 扫描任意 RFC 合法 scheme，无需手工维护 20+ 协议前缀表。
        for url in [
            "ftp://files.example.com",
            "ssh://git@github.com/repo",
            "git://github.com/user/repo.git",
            "gemini://example.com",
            "file:///path/to/file",
            "ipfs://QmTzQ1a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t",
            "ipns://docs.ipfs.tech",
        ] {
            let line = format!("prefix {url} suffix");
            assert_eq!(url_at_column(&line, 7), Some(url.to_string()), "url={url}");
        }
    }

    #[test]
    fn detects_mailto_tel_sms() {
        // 有 scheme 时无主机段的纯 scheme 链接仍能匹配。
        assert_eq!(
            url_at_column("mail me at mailto:user@example.com now", 12),
            Some("mailto:user@example.com".to_string())
        );
        assert_eq!(
            url_at_column("call tel:+1234567890 now", 6),
            Some("tel:+1234567890".to_string())
        );
        // ipfs/ipns 冒号形式（无 `//`）须继续匹配，点击仍应打开 CID。
        assert_eq!(
            url_at_column(
                "get ipfs:QmTzQ1a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t now",
                5
            ),
            Some("ipfs:QmTzQ1a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t".to_string())
        );
        assert_eq!(
            url_at_column("text sms:+1234567890 now", 6),
            Some("sms:+1234567890".to_string())
        );
    }

    #[test]
    fn cleans_trailing_punctuation() {
        assert_eq!(
            url_at_column("link https://example.com. done", 6),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            url_at_column("link https://example.com/path), done", 6),
            Some("https://example.com/path".to_string())
        );
        // scheme-only 回退同样剥离包裹右括号。
        assert_eq!(
            url_at_column("(call tel:+1234567890)", 7),
            Some("tel:+1234567890".to_string())
        );
    }

    #[test]
    fn bracket_balancing() {
        assert_eq!(
            url_at_column("see https://example.com/wiki/Foo_(bar) now", 5),
            Some("https://example.com/wiki/Foo_(bar)".to_string())
        );
        assert_eq!(
            url_at_column("see https://example.com/wiki/Foo_(bar)) now", 5),
            Some("https://example.com/wiki/Foo_(bar)".to_string())
        );
    }

    #[test]
    fn does_not_match_bare_text() {
        assert_eq!(url_at_column("hello world", 5), None);
        assert_eq!(url_at_column("visit www.example.com now", 7), None);
        assert_eq!(url_at_column("email user@example.com", 7), None);
    }

    #[test]
    fn keeps_query_string_and_port() {
        let line = "see https://example.com/a/b?q=1&x=2 end";
        assert_eq!(
            url_at_column(line, 5),
            Some("https://example.com/a/b?q=1&x=2".to_string())
        );
        let port = "see https://example.com:8080/path end";
        assert_eq!(
            url_at_column(port, 5),
            Some("https://example.com:8080/path".to_string())
        );
    }

    #[test]
    fn url_at_column_hits_span() {
        let line = "see https://example.com/a?q=1 end";
        // 4..=29 列覆盖 URL（"see " 占 4 列，URL 25 字符）。
        assert_eq!(
            url_at_column(line, 4),
            Some("https://example.com/a?q=1".to_string())
        );
        assert_eq!(
            url_at_column(line, 28),
            Some("https://example.com/a?q=1".to_string())
        );
        // 落在任何 URL 区间外的列返回 None。
        assert_eq!(url_at_column(line, 0), None);
        assert_eq!(url_at_column(line, 33), None);
    }

    #[test]
    fn url_at_column_counts_wide_char_columns() {
        // 输入行“每列一字符”：`cell_line_text` 把宽度 2 的单元展开为两个副本，
        // 故 URL 前的宽字符占两字符，URL 的列区间相应后移。
        let line = "中中中中https://example.com"; // 2 wide chars = 4 cols
        // URL 从第 4 列开始（前有 4 个宽字符列）。
        assert_eq!(
            url_at_column(line, 4),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            url_at_column(line, 4 + "https://example.com".len() - 1),
            Some("https://example.com".to_string())
        );
        // 宽字符内的列（1..3）不得匹配。
        assert_eq!(url_at_column(line, 1), None);
        assert_eq!(url_at_column(line, 3), None);
    }
}
