//! 与平台无关的 logcat 分块。
//! logcat 单条载荷超约 4068 字节即被**静默**截断，长消息必须先分块，否则尾部丢失；
//! 预算为 `4068 - 头部 - tagLen - 4`，续接块带 `(i/n)` 前缀供重组。
//! 不引入 Android 依赖，使同一算法可在 host 上单元测试并与 Kotlin 端共享。

/// logcat 单条载荷上限（`LOGGER_ENTRY_MAX_PAYLOAD`）。
pub const LOGGER_ENTRY_MAX_PAYLOAD: usize = 4068;

/// 计入 4068 字节上限的 logd 单条头部预留（`logger_entry` 结构 + tag 长度字段）。
///
/// 32 字节是 `logger_entry_v3/v4` 头部大小。设备实测（API 35 模拟器）：12 字节 tag 配
/// 4036 字节载荷时，logcat 只显示 4022 字节——条目被截为 `4068 - 头部 - tag`；
/// 原先按 8 字节（logcat 显示前缀）估算偏小，logd 会静默截断按预算切出的块。
const LOGGER_PREFIX_OVERHEAD: usize = 32;

const LOGGER_SAFETY_MARGIN: usize = 4;

pub fn max_entry_size(tag_len: usize) -> usize {
    LOGGER_ENTRY_MAX_PAYLOAD
        .saturating_sub(LOGGER_PREFIX_OVERHEAD)
        .saturating_sub(tag_len)
        .saturating_sub(LOGGER_SAFETY_MARGIN)
        .max(64)
}

/// 把 `message` 切成不超过 `max_entry_size(tag.len())` 字节的块（UTF-8 感知，
/// 仅在字符边界切分）。首块原样输出；多于一块时其余块带 `(i/n)\n` 前缀。
/// 切点优先取窗口内最后一个换行，使多行消息尽量保持整行。
///
/// 前缀预算是动态的：`N >= 100` 时 `(N/N)\n` 超过 8 字节，故用真实前缀长度重切。
pub fn chunk_message(tag: &str, message: &str) -> Vec<String> {
    let budget = max_entry_size(tag.len());
    if message.len() <= budget {
        return vec![message.to_string()];
    }

    let mut prefix_len = 8usize;
    let mut chunks = split_into_chunks(message, budget, prefix_len);
    if chunks.len() > 1 {
        // 重切直到 `(N/N)\n` 前缀长度收敛（N >= 100 时超过 8 字节；极端大的 N
        // 在重切后仍可能再变长）。
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

fn split_into_chunks(message: &str, budget: usize, prefix_len: usize) -> Vec<String> {
    let effective = budget.saturating_sub(prefix_len).max(16);

    let mut chunks: Vec<String> = Vec::new();
    let mut start = 0usize;
    let bytes = message.as_bytes();
    while start < bytes.len() {
        let mut end = (start + effective).min(bytes.len());
        // 退到 `end` 之前最近的字符边界，绝不切开多字节序列。
        while end > start && !message.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            // 单个多字节字符超预算（异常情形）：强行纳入以保证推进。
            end = (start + 1).min(bytes.len());
            while end < bytes.len() && !message.is_char_boundary(end) {
                end += 1;
            }
        }
        // 优先在窗口内最后一个换行处切分（仅当首段非空且余下部分仍能装入后续块，
        // 以保持 `git log --oneline` 风格整行不被拆散）。
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
        assert!(chunks[0].len() <= budget);
        for chunk in &chunks[1..] {
            assert!(
                chunk.len() <= budget,
                "chunk len {} exceeds budget {budget}",
                chunk.len()
            );
        }
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
        let unit = "中";
        let count = budget / unit.len() + 2;
        let msg = unit.repeat(count);
        let chunks = chunk_message(tag, &msg);
        for chunk in &chunks {
            assert!(chunk.is_char_boundary(chunk.len()));
            // 每块都只由完整的 3 字节字符组成。
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
        // 超长 tag 仍得到可用的下限。
        assert_eq!(max_entry_size(1_000_000), 64);
        // 无 tag 下限：4068 载荷 − 32 头部开销 − 4 安全余量。
        assert_eq!(max_entry_size(0), 4032);
    }

    #[test]
    fn emoji_surrogates_not_split_and_chunks_fit() {
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
            let decoded = String::from_utf8(chunk.as_bytes().to_vec()).expect("valid utf-8");
            assert_eq!(decoded, *chunk);
        }
    }

    #[test]
    fn hundred_plus_chunks_keep_prefix_within_budget() {
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
