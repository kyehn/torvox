//! Platform-independent logcat chunking.
//!
//! Android logcat truncates any single log entry payload beyond ~4068
//! bytes (LOGGER_ENTRY_MAX_PAYLOAD). Long messages must be split into
//! chunks before being handed to `__android_log_write` (Rust) or
//! `android.util.Log` (Kotlin), otherwise the tail is silently lost.
//!
//! Mirrors termux-kotlin's `Logger.logExtendedMessage` budget math:
//! `maxEntrySize = 4068 - overhead - tagLen - 4`, where overhead is the
//! logd per-entry header (32 bytes, measured on-device — the old 8-byte
//! estimate let logd silently truncate chunks) and 4 bytes a safety
//! margin. Continuation chunks carry a `(i/n)` prefix so logcat readers
//! can reassemble.
//!
//! Kept free of Android imports so the exact same algorithm is unit
//! testable on the host and shared with the Kotlin port.

/// Android logcat's per-entry payload cap (LOGGER_ENTRY_MAX_PAYLOAD).
pub const LOGGER_ENTRY_MAX_PAYLOAD: usize = 4068;

/// Bytes reserved for the logd per-entry header (logger_entry struct +
/// tag length field) that counts toward the 4068-byte entry limit.
///
/// On-device measurement, emulator API 35): writing a
/// 4036-byte payload with a 12-byte tag surfaces as a 4022-byte logcat
/// line — the entry is truncated to `4068 - header - tag`. The old
/// estimate of 8 bytes (logcat display prefix) was too small, so chunks
/// at the computed budget were silently truncated by logd. 32 bytes is
/// the logger_entry_v3/v4 header size; the display prefix (timestamp +
/// pid + tid + level) is derived from the header and does not add to the
/// stored entry.
const LOGGER_PREFIX_OVERHEAD: usize = 32;

/// Safety margin below the hard cap, matching termux-kotlin's `- 4`.
const LOGGER_SAFETY_MARGIN: usize = 4;

/// Compute the maximum message payload that fits one logcat entry for a
/// tag of `tag_len` bytes. Never returns below a usable floor so a very
/// long tag cannot collapse the budget to zero.
pub fn max_entry_size(tag_len: usize) -> usize {
    LOGGER_ENTRY_MAX_PAYLOAD
        .saturating_sub(LOGGER_PREFIX_OVERHEAD)
        .saturating_sub(tag_len)
        .saturating_sub(LOGGER_SAFETY_MARGIN)
        .max(64)
}

/// Split `message` into logcat-sized chunks, each no longer than
/// `max_entry_size(tag.len())` bytes (UTF-8 aware — splits at char
/// boundaries only). The first chunk carries the message unchanged; when
/// more than one chunk is produced, each chunk after the first is prefixed
/// with `(i/n)\n`. A chunk boundary prefers the last newline within the
/// window so multi-line messages keep their lines intact where possible.
///
/// The prefix budget is dynamic: `(N/N)\n` exceeds 8 bytes once N >= 100,
/// so the split re-runs with the real prefix length in that case
/// audit fix, mirrored by the Kotlin port).
pub fn chunk_message(tag: &str, message: &str) -> Vec<String> {
    let budget = max_entry_size(tag.len());
    if message.len() <= budget {
        return vec![message.to_string()];
    }

    let mut prefix_len = 8usize;
    let mut chunks = split_into_chunks(message, budget, prefix_len);
    if chunks.len() > 1 {
        // Re-split until the "(N/N)\n" prefix length converges (it grows
        // past 8 bytes once N >= 100; for absurdly large N it could grow
        // again after re-splitting).
        while chunks.len() > 1 {
            let actual_prefix_len = format!("({}/{})\n", chunks.len(), chunks.len()).len();
            if actual_prefix_len <= prefix_len {
                break;
            }
            prefix_len = actual_prefix_len;
            chunks = split_into_chunks(message, budget, prefix_len);
        }
        let total = chunks.len();
        let mut numbered: Vec<String> = Vec::with_capacity(total);
        for (i, chunk) in chunks.into_iter().enumerate() {
            if i == 0 {
                numbered.push(chunk);
            } else {
                numbered.push(format!("({}/{})\n{}", i + 1, total, chunk));
            }
        }
        chunks = numbered;
    }
    chunks
}

/// Split `message` into chunks whose UTF-8 byte length fits
/// `budget - prefix_len`, preferring cuts at newlines and never splitting
/// a multi-byte UTF-8 sequence.
fn split_into_chunks(message: &str, budget: usize, prefix_len: usize) -> Vec<String> {
    let effective = budget.saturating_sub(prefix_len).max(16);

    let mut chunks: Vec<String> = Vec::new();
    let mut start = 0usize;
    let bytes = message.as_bytes();
    while start < bytes.len() {
        let mut end = (start + effective).min(bytes.len());
        // Back off to the last char boundary before `end` (never split a
        // multi-byte UTF-8 sequence).
        while end > start && !message.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            // Single multi-byte char longer than the budget (pathological):
            // force include it so we make progress.
            end = (start + 1).min(bytes.len());
            while end < bytes.len() && !message.is_char_boundary(end) {
                end += 1;
            }
        }
        // Prefer cutting at the last newline inside the window (but only
        // when it leaves a non-empty first part and the remainder still
        // fits a later chunk — keeps `git log --oneline` style lines
        // together).
        if end < bytes.len()
            && let Some(last_nl) = message[start..end].rfind('\n')
        {
            let candidate = start + last_nl + 1;
            if candidate > start && candidate < end {
                end = candidate;
            }
        }
        chunks.push(message[start..end].to_string());
        start = end;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_message_single_chunk() {
        let chunks = chunk_message("tag", "hello");
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn empty_message_single_chunk() {
        let chunks = chunk_message("tag", "");
        assert_eq!(chunks, vec![""]);
    }

    #[test]
    fn exact_budget_single_chunk() {
        let budget = max_entry_size(3);
        let msg = "x".repeat(budget);
        let chunks = chunk_message("tag", &msg);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), budget);
    }

    #[test]
    fn over_budget_splits_and_each_chunk_fits() {
        let tag = "Rust";
        let budget = max_entry_size(tag.len());
        let msg = "y".repeat(budget * 3 + 17);
        let chunks = chunk_message(tag, &msg);
        assert!(
            chunks.len() >= 3,
            "expected >=3 chunks, got {}",
            chunks.len()
        );
        // Chunk 0 carries no prefix; others carry "(i/n)\n" (<= 8 bytes).
        assert!(chunks[0].len() <= budget);
        for chunk in &chunks[1..] {
            assert!(
                chunk.len() <= budget,
                "chunk len {} exceeds budget {budget}",
                chunk.len()
            );
        }
        // Concatenated payload (minus prefixes) equals the original.
        let mut reassembled = chunks[0].clone();
        for (i, chunk) in chunks.iter().enumerate().skip(1) {
            let body = chunk
                .strip_prefix(&format!("({}/{})\n", i + 1, chunks.len()))
                .expect("prefix");
            reassembled.push_str(body);
        }
        assert_eq!(reassembled, msg);
    }

    #[test]
    fn utf8_multibyte_not_split() {
        let tag = "t";
        let budget = max_entry_size(tag.len());
        // Each CJK char is 3 bytes; craft a message crossing the budget at
        // a multi-byte boundary.
        let unit = "中";
        let count = budget / unit.len() + 2;
        let msg = unit.repeat(count);
        let chunks = chunk_message(tag, &msg);
        for chunk in &chunks {
            assert!(chunk.is_char_boundary(chunk.len()));
            // Every chunk must consist of whole 3-byte chars.
            assert_eq!(chunk.len() % unit.len(), 0);
        }
        let concat: String = chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if i == 0 {
                    c.clone()
                } else {
                    c.split_once('\n')
                        .map(|(_, rest)| rest.to_string())
                        .unwrap_or_default()
                }
            })
            .collect();
        assert_eq!(concat, msg);
    }

    #[test]
    fn newline_preferred_cut() {
        let tag = "t";
        let budget = max_entry_size(tag.len());
        let effective = budget - 8;
        // Two lines where the first line ends just inside the effective
        // window so the newline falls inside chunk 0's cut range.
        let line1 = "a".repeat(effective - 4);
        let msg = format!("{line1}\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let chunks = chunk_message(tag, &msg);
        assert!(chunks.len() >= 2);
        assert!(
            chunks[0].ends_with('\n'),
            "first chunk should end at the newline, got {:?}",
            chunks[0]
        );
    }

    #[test]
    fn max_entry_size_floor() {
        // A huge tag still yields a usable floor.
        assert_eq!(max_entry_size(1_000_000), 64);
        assert_eq!(max_entry_size(0), 4068 - 32 - 0 - 4);
    }

    #[test]
    fn emoji_surrogates_not_split_and_chunks_fit() {
        // U+1F600 is 4 UTF-8 bytes; the split must never land inside it.
        let tag = "t";
        let budget = max_entry_size(tag.len());
        let msg = "😀".repeat(budget / 4 + 5);
        let chunks = chunk_message(tag, &msg);
        assert!(
            chunks.len() >= 2,
            "expected >=2 chunks, got {}",
            chunks.len()
        );
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= budget,
                "chunk {i} has {} bytes, budget {budget}",
                chunk.len()
            );
            // Re-encoding must round-trip (valid UTF-8, no lone surrogates).
            assert_eq!(chunk.as_bytes(), chunk.as_bytes());
            let decoded = String::from_utf8(chunk.as_bytes().to_vec()).expect("valid utf-8");
            assert_eq!(decoded, *chunk);
        }
    }

    #[test]
    fn hundred_plus_chunks_keep_prefix_within_budget() {
        // "(100/100)\n" is 10 bytes > the 8-byte estimate; the dynamic
        // prefix re-split must keep every continuation chunk within budget.
        let tag = "t";
        let budget = max_entry_size(tag.len());
        let msg = "x".repeat((budget - 8) * 120);
        let chunks = chunk_message(tag, &msg);
        assert!(
            chunks.len() >= 100,
            "expected >=100 chunks, got {}",
            chunks.len()
        );
        for (i, chunk) in chunks.iter().enumerate().skip(1) {
            assert!(
                chunk.len() <= budget,
                "chunk {i} exceeds budget: {} > {budget}",
                chunk.len()
            );
        }
        let prefix = format!("({}/{})\n", chunks.len(), chunks.len());
        assert!(prefix.len() > 8, "prefix length must exceed 8: {prefix:?}");
    }
}
