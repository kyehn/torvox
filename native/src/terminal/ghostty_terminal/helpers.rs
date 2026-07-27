use super::types::*;
use libghostty_vt::Terminal;
use libghostty_vt::terminal::{Point, PointCoordinate};

impl super::GhosttyTerminal {
    pub(crate) fn read_line_text_impl(terminal: &Terminal, row: u32) -> Option<String> {
        let cols = terminal.cols().unwrap_or(80) as u32;
        let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;
        let mut text = String::new();
        for col in 0..cols {
            let coord = PointCoordinate {
                x: col as u16,
                y: row,
            };
            let point = if row < scrollback_rows {
                terminal.grid_ref(Point::History(coord))
            } else {
                let viewport_row = row - scrollback_rows;
                let vp_coord = PointCoordinate {
                    x: col as u16,
                    y: viewport_row,
                };
                terminal.grid_ref(Point::Viewport(vp_coord))
            };
            if let Ok(point) = point
                && let Ok(cell) = point.cell()
            {
                let cp = cell.codepoint().unwrap_or(0);
                if cp != 0 {
                    if let Some(ch) = char::from_u32(cp) {
                        text.push(ch);
                    }
                } else {
                    text.push(' ');
                }
            }
        }
        let trimmed = text.trim_end().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    pub(crate) fn search_in_scrollback_impl(
        terminal: &Terminal,
        query: &str,
    ) -> Option<(u32, u32)> {
        if query.is_empty() {
            return None;
        }
        let total = terminal.total_rows().unwrap_or(0) as u32;
        for row in 0..total {
            if let Some(line) = Self::read_line_text_impl(terminal, row)
                && let Some(col) = line.find(query)
            {
                return Some((row, col as u32));
            }
        }
        None
    }

    pub(crate) fn search_in_scrollback_all_impl(
        terminal: &Terminal,
        query: &str,
        case_sensitive: bool,
        fuzzy: bool,
    ) -> Vec<SearchMatch> {
        if query.is_empty() {
            return vec![];
        }
        let total = terminal.total_rows().unwrap_or(0) as u32;
        let mut results = Vec::new();
        let search_query = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };
        for row in 0..total {
            if let Some(line) = Self::read_line_text_impl(terminal, row) {
                let search_line = if case_sensitive {
                    line.clone()
                } else {
                    line.to_lowercase()
                };
                if fuzzy {
                    let max_distance = std::cmp::max(1, search_query.len() / 3);
                    if search_query.len() <= search_line.len() {
                        let end = search_line.len() - search_query.len();
                        // Sliding window: find all windows whose edit distance is within threshold.
                        // Return each match position so all results are highlighted, not just
                        // the nearest one (which would miss overlapping near-matches).
                        for start in 0..=end {
                            let window = &search_line[start..start + search_query.len()];
                            let dist = Self::levenshtein_distance(&search_query, window);
                            if dist <= max_distance {
                                results.push(SearchMatch {
                                    row,
                                    start_col: start as u32,
                                    end_col: (start + search_query.len()) as u32,
                                });
                            }
                        }
                    }
                } else {
                    let mut start = 0;
                    while let Some(col) = search_line[start..].find(&search_query) {
                        let abs_col = start + col;
                        results.push(SearchMatch {
                            row,
                            start_col: abs_col as u32,
                            end_col: (abs_col + search_query.len()) as u32,
                        });
                        start = abs_col + 1;
                    }
                }
            }
        }
        results
    }

    /// Compute the Levenshtein distance (edit distance) between two strings.
    /// Uses the classic dynamic programming approach with O(min(m,n)) memory.
    pub(crate) fn levenshtein_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let m = a_chars.len();
        let n = b_chars.len();
        // Use the shorter string as the column vector for memory efficiency
        if m < n {
            return Self::levenshtein_distance(b, a);
        }
        let mut prev: Vec<usize> = (0..=n).collect();
        for i in 1..=m {
            let mut current = i;
            for j in 1..=n {
                let cost = (a_chars[i - 1] != b_chars[j - 1]) as usize;
                let next =
                    std::cmp::min(std::cmp::min(current + 1, prev[j] + 1), prev[j - 1] + cost);
                prev[j - 1] = current;
                current = next;
            }
            prev[n] = current;
        }
        prev[n]
    }

    /// Serialize terminal state to VT sequences using Ghostty Formatter.
    /// Sends the result back via `tx`, or an empty vec on failure.
    pub(crate) fn handle_save_session(
        terminal: &libghostty_vt::Terminal,
        tx: flume::Sender<Vec<u8>>,
    ) {
        use libghostty_vt::fmt::{Format, Formatter, FormatterOptions};

        let mut formatter = match Formatter::new(
            terminal,
            FormatterOptions::new()
                .with_format(Format::Vt)
                .with_unwrap(false)
                .with_trim(false)
                .with_palette(false)
                .with_modes(false)
                .with_cursor(false),
        ) {
            Ok(f) => f,
            Err(e) => {
                log::error!("handle_save_session: Formatter::new failed: {e}");
                let _ = tx.send(Vec::new());
                return;
            }
        };

        match formatter.format_alloc(None) {
            Ok(bytes) => {
                let _ = tx.send(bytes.to_vec());
            }
            Err(e) => {
                log::error!("handle_save_session: format_alloc failed: {e}");
                let _ = tx.send(Vec::new());
            }
        }
    }
}
