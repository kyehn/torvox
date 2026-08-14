//! URL detection for plain-text URL taps, backed by the `linkify` crate.
//!
//! # Requirements
//! - FR-023 — Word boundary and URL detection: auto-expand word selections to URLs.
//! - zed-android-port `URL_REGEX` (research-zed-port.md:675): the 20-protocol
//!   prefix list must include `ipfs:`/`ipns:` (linkify scans any valid scheme).
//!
//! linkify implements RFC-3986-style URL scanning (Unicode/IRI, bracket
//! balancing, trailing-punctuation cleanup) that a hand-written regex cannot
//! match; `url_must_have_scheme(true)` keeps detection scoped to explicit
//! `scheme:` URLs exactly like the previous regex, so bare `www.` or
//! `user@host` text is not treated as a link. The only hand-written pattern
//! left is a tiny scheme-only fallback for `mailto:`/`tel:`/`sms:`/`callto:`
//! (no `//` host part) that linkify deliberately skips.
use linkify::{LinkFinder, LinkKind};
use std::sync::OnceLock;

/// Scheme-only protocols with no `//` host part that linkify's URL scanner
/// skips but terminal taps must still open (mail client, dialer, ...).
const SCHEME_ONLY_PROTOCOLS: &[&str] = &["mailto", "tel", "sms", "callto"];

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

/// Strips trailing punctuation from a scheme-only link (linkify does this
/// itself for its own matches).
fn trim_trailing_punctuation(raw: &str) -> String {
    raw.trim_end_matches(['.', ',', ';', ':', '!', '?', '"', '\''])
        .to_string()
}

/// Scans `line` (a terminal row rendered as one char per column — wide chars
/// expanded to two copies) for a URL whose column span contains `col`.
/// Returns the cleaned URL, if any. Backs the plain-text URL tap fallback in
/// `hyperlinkAt` (zed-port pattern: taps on bare URLs must open the browser).
pub fn url_at_column(line: &str, col: usize) -> Option<String> {
    for link in finder().links(line) {
        // Link offsets are byte offsets; the line is one char per column,
        // so the char count is the column count.
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
        // linkify scans any RFC-valid scheme, so the 20+ protocol list
        // (http, ftp, ssh, git, gemini, file, mailto, tel, sms, ipfs, ...)
        // is covered without a hand-maintained prefix table.
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
        // Scheme-only links (no host) still match when a scheme is present.
        assert_eq!(
            url_at_column("mail me at mailto:user@example.com now", 12),
            Some("mailto:user@example.com".to_string())
        );
        assert_eq!(
            url_at_column("call tel:+1234567890 now", 6),
            Some("tel:+1234567890".to_string())
        );
        assert_eq!(
            url_at_column("text sms:+1234567890 now", 6),
            Some("sms:+1234567890".to_string())
        );
    }

    #[test]
    fn cleans_trailing_punctuation() {
        // Trailing dot / comma / paren are excluded by linkify's scanner.
        assert_eq!(
            url_at_column("link https://example.com. done", 6),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            url_at_column("link https://example.com/path), done", 6),
            Some("https://example.com/path".to_string())
        );
    }

    #[test]
    fn bracket_balancing() {
        // Balanced parens are kept...
        assert_eq!(
            url_at_column("see https://example.com/wiki/Foo_(bar) now", 5),
            Some("https://example.com/wiki/Foo_(bar)".to_string())
        );
        // ...while an extra close paren is stripped.
        assert_eq!(
            url_at_column("see https://example.com/wiki/Foo_(bar)) now", 5),
            Some("https://example.com/wiki/Foo_(bar)".to_string())
        );
    }

    #[test]
    fn does_not_match_bare_text() {
        assert_eq!(url_at_column("hello world", 5), None);
        // No scheme → not a URL (url_must_have_scheme(true)).
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
        // Columns 4..=29 cover the URL ("see " = 4 cols, URL is 25 chars).
        assert_eq!(
            url_at_column(line, 4),
            Some("https://example.com/a?q=1".to_string())
        );
        assert_eq!(
            url_at_column(line, 28),
            Some("https://example.com/a?q=1".to_string())
        );
        // Column outside any URL span returns None.
        assert_eq!(url_at_column(line, 0), None);
        assert_eq!(url_at_column(line, 33), None);
    }

    #[test]
    fn url_at_column_counts_wide_char_columns() {
        // Input lines are "one char per column": cell_line_text expands a
        // width-2 cell into two copies, so a wide char before the URL
        // occupies two chars and the URL's column span shifts accordingly.
        let line = "中中中中https://example.com"; // 2 wide chars = 4 cols
        // URL starts at column 4 (after 4 wide-char columns).
        assert_eq!(
            url_at_column(line, 4),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            url_at_column(line, 4 + "https://example.com".len() - 1),
            Some("https://example.com".to_string())
        );
        // A column inside the wide chars (col 1..3) must not match.
        assert_eq!(url_at_column(line, 1), None);
        assert_eq!(url_at_column(line, 3), None);
    }
}
