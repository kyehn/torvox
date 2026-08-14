//! URL detection regex with 20+ protocols, bracket balancing, and trailing punctuation cleanup.
//!
//! # Requirements
//! - FR-023 — Word boundary and URL detection: auto-expand word selections to URLs.
//! - zed-android-port `URL_REGEX` (research-zed-port.md:675): the 20-protocol
//!   prefix list must include `ipfs:`/`ipns:` (added to SCHEMES_COLON below).
use std::sync::OnceLock;

/// Schemes that use `://` after the scheme name.
const SCHEMES_SLASH: &[&str] = &[
    "http", "https", "ftp", "ftps", "file", "data", "ssh", "git", "svn", "hg", "sftp", "scp",
    "irc", "ircs", "magnet", "gemini", "gopher", "news", "nntp", "ed2k", "steam", "skype", "xmpp",
];

/// Schemes that use just `:` (no `//`).
const SCHEMES_COLON: &[&str] = &["mailto", "tel", "sms", "callto", "ipfs", "ipns"];

pub fn url_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let slash = SCHEMES_SLASH.join("|");
        let colon = SCHEMES_COLON.join("|");
        // Body excludes only characters that cannot appear inside a URL:
        // whitespace, angle brackets, comma, semicolon, exclamation, single
        // and double quotes, and backslash. Dot, colon, question mark,
        // equals, and parentheses are KEPT so `?q=1&x=2` queries, `:port`
        // suffixes, and `wiki/Foo_(bar)` paths survive; trailing punctuation
        // is stripped by clean_url(). Matches UrlDetector.kt exactly.
        let body = r#"[^\s<>,;!'"\\]*"#;
        // NOTE: pattern is built from const arrays above — compile-time valid regex.
        let pattern = format!(r"(?P<url>(?i:{slash})://{body}|(?i:{colon}):{body})");
        regex::Regex::new(&pattern).expect("url_regex: invalid pattern")
    })
}

/// Scans `line` (a terminal row rendered as one char per column — wide chars
/// expanded to two copies) for a URL whose column span contains `col`.
/// Returns the cleaned URL, if any. Backs the plain-text URL tap fallback in
/// `hyperlinkAt` (zed-port pattern: taps on bare URLs must open the browser).
pub fn url_at_column(line: &str, col: usize) -> Option<String> {
    let re = url_regex();
    for caps in re.captures_iter(line) {
        let Some(m) = caps.name("url") else { continue };
        // Regex offsets are byte offsets; the line is one char per column,
        // so the char count is the column count.
        let start = line[..m.start()].chars().count();
        let end = line[..m.end()].chars().count();
        if start <= col && col < end {
            return Some(clean_url(m.as_str()));
        }
    }
    None
}

pub fn clean_url(raw: &str) -> String {
    let mut s = raw.to_string();
    // First pass: strip trailing punctuation that's not part of balanced parens
    let trailing = ['.', ',', ';', ':', '!', '?', '"', '\''];
    while s.ends_with(trailing) {
        s.pop();
    }
    // Second pass: bracket balancing for parentheses
    let open_count = s.chars().filter(|&c| c == '(').count();
    let close_count = s.chars().filter(|&c| c == ')').count();
    if close_count > open_count {
        let excess = close_count - open_count;
        let mut removed = 0;
        while removed < excess && s.ends_with(')') {
            s.pop();
            removed += 1;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_http_and_https() {
        let re = url_regex();
        assert!(re.is_match("https://example.com"));
        assert!(re.is_match("http://example.com/path"));
    }

    #[test]
    fn detects_multiple_protocols() {
        let re = url_regex();
        assert!(re.is_match("ftp://files.example.com"));
        assert!(re.is_match("ssh://git@github.com/repo"));
        assert!(re.is_match("mailto:user@example.com"));
        assert!(re.is_match("git://github.com/user/repo.git"));
        assert!(re.is_match("gemini://example.com"));
        assert!(re.is_match("file:///path/to/file"));
        assert!(re.is_match("tel:+1234567890"));
        assert!(re.is_match("sms:+1234567890"));
    }

    #[test]
    fn cleans_trailing_punctuation() {
        assert_eq!(clean_url("https://example.com."), "https://example.com");
        assert_eq!(
            clean_url("https://example.com/path)."),
            "https://example.com/path"
        );
    }

    #[test]
    fn bracket_balancing() {
        // Balanced parens should be kept
        assert_eq!(
            clean_url("https://example.com/wiki/Foo_(bar)"),
            "https://example.com/wiki/Foo_(bar)"
        );
        // Unbalanced: extra close paren stripped
        assert_eq!(
            clean_url("https://example.com/wiki/Foo_(bar))"),
            "https://example.com/wiki/Foo_(bar)"
        );
        // Text around URL preserved
        assert_eq!(
            clean_url("(see https://example.com)"),
            "(see https://example.com)"
        );
    }

    #[test]
    fn does_not_match_bare_text() {
        let re = url_regex();
        assert!(!re.is_match("hello world"));
    }

    #[test]
    fn keeps_query_string_and_port() {
        // Regression: the body character class must keep `?`, `=`, `&`, and
        // `:` so `?q=1&x=2` queries and `:8080` ports survive detection
        // (mirrors UrlDetector.kt). The old class excluded them, truncating
        // matches at the query/port boundary.
        let re = url_regex();
        assert!(re.is_match("https://example.com/a/b?q=1&x=2"));
        assert!(re.is_match("https://example.com:8080/path"));
        assert_eq!(
            clean_url("https://example.com/a/b?q=1&x=2"),
            "https://example.com/a/b?q=1&x=2"
        );
    }

    #[test]
    fn detects_ipfs_and_ipns() {
        // zed-android-port URL_REGEX 20-protocol list includes ipfs:/ipns:.
        let re = url_regex();
        assert!(re.is_match("ipfs://QmTzQ1a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t"));
        assert!(re.is_match("ipns://docs.ipfs.tech"));
    }

    #[test]
    fn url_at_column_hits_span() {
        let line = "see https://example.com/a?q=1 end";
        // Columns 4..=28 cover the URL ("see " = 4 cols, URL is 25 chars).
        assert_eq!(
            url_at_column(line, 4),
            Some("https://example.com/a?q=1".to_string())
        );
        assert_eq!(
            url_at_column(line, 28),
            Some("https://example.com/a?q=1".to_string())
        );
        // A trailing '.' is inside the raw match span but cleaned away.
        let dotted = "link https://example.com.";
        // A trailing '.' (col 24) is inside the raw match span but cleaned away.
        assert_eq!(
            url_at_column(dotted, 24),
            Some("https://example.com".to_string())
        );
        // Column outside any URL span returns None.
        assert_eq!(url_at_column(line, 0), None);
        assert_eq!(url_at_column(line, 33), None);
        assert_eq!(url_at_column("no url here", 5), None);
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

    #[test]
    fn clean_url_strips_trailing_question_and_bang() {
        assert_eq!(clean_url("https://example.com?"), "https://example.com");
        assert_eq!(clean_url("https://example.com!"), "https://example.com");
        assert_eq!(
            clean_url("https://example.com/path?!,"),
            "https://example.com/path"
        );
        // Query strings keep interior '?'.
        assert_eq!(
            clean_url("https://example.com/a?q=1"),
            "https://example.com/a?q=1"
        );
    }
}
