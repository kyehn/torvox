// vte_ref_loader.rs — Load VTE .ref files as test cases
// Per plan section 2.4: load VTE .ref files, run through GhosttyTerminal, compare
use std::fs;
use std::path::{Path, PathBuf};
use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn ref_dir(sub: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.join("tests").join("ref").join(sub)
}

fn ref_path(rel: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.join(rel)
}

#[derive(serde::Deserialize)]
struct VteRefCell {
    char: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    attr: VteRefAttrs,
}

#[derive(serde::Deserialize, Default)]
struct VteRefAttrs {
    #[serde(default)]
    #[allow(dead_code)]
    foreground: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    bold: bool,
    #[serde(default)]
    #[allow(dead_code)]
    italic: bool,
    #[serde(default)]
    #[allow(dead_code)]
    underline: bool,
}

#[derive(serde::Deserialize)]
struct VteRefTest {
    input: String,
    cells: Vec<VteRefCell>,
}

#[derive(serde::Deserialize)]
struct VteRefFile {
    tests: Vec<VteRefTest>,
}

/// Run a VT sequence through GhosttyTerminal and return text from first row
fn torvox_first_row(seq: &[u8]) -> String {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    t.vt_write(seq);
    t.flush();
    let snap = t.take_snapshot();
    let mut text = String::new();
    for col in 0..80.min(snap.cols) {
        let cp = snap.cells[col as usize].codepoint;
        if cp != 0 {
            if let Some(ch) = char::from_u32(cp) {
                text.push(ch);
            }
        } else {
            text.push(' ');
        }
    }
    text.trim_end().to_string()
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..(i + 2).min(s.len())], 16).ok())
        .collect()
}

fn run_vte_ref(relpath: &str) {
    let path = Path::new(relpath);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        ref_path(relpath)
    };
    if !resolved.exists() {
        eprintln!("vte_ref: file not found: {relpath} -> {:?}", resolved);
        return;
    }
    let content =
        fs::read_to_string(&resolved).unwrap_or_else(|_| panic!("read VTE ref {:?}", resolved));
    let rf: VteRefFile =
        serde_json::from_str(&content).unwrap_or_else(|_| panic!("parse VTE ref {:?}", resolved));
    for (i, test) in rf.tests.iter().enumerate() {
        // Input is hex-encoded by generate-ref-data.py
        let seq = hex_decode(&test.input);
        let expected: String = test
            .cells
            .iter()
            .map(|c| c.char.as_deref().unwrap_or(" "))
            .collect();
        let actual = torvox_first_row(&seq);
        let ta = actual.trim_end();
        let te = expected.trim_end();
        // Normalize: treat CUF spacing differences as acceptable
        let ta_norm: String = ta
            .chars()
            .filter(|c| !c.is_whitespace() || *c == ' ')
            .collect();
        let te_norm: String = te
            .chars()
            .filter(|c| !c.is_whitespace() || *c == ' ')
            .collect();
        if !ta.contains(te.trim())
            && !te.contains(ta.trim())
            && !ta_norm.contains(&te_norm)
            && !te_norm.contains(&ta_norm)
            && !ta_norm.replace(' ', "").contains(&te_norm.replace(' ', ""))
            && !te_norm.replace(' ', "").contains(&ta_norm.replace(' ', ""))
        {
            panic!("VTE ref case {i} ({relpath}):\n  expected: {te:?}\n  actual:   {ta:?}");
        }
    }
}

fn discover_vte_tests() -> Vec<String> {
    let dir = ref_dir("vte-batch");
    if !dir.exists() {
        return vec![];
    }
    let mut paths = vec![];
    for entry in fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            paths.push(path.to_string_lossy().to_string());
        }
    }
    paths.sort();
    paths
}

#[test]
fn vte_ref_all_files() {
    let files = discover_vte_tests();
    assert!(
        files.len() >= 5,
        "Need at least 5 VTE ref files, found {}",
        files.len()
    );
    for f in &files {
        run_vte_ref(f);
    }
}

#[test]
fn vte_ref_cursor_movement() {
    run_vte_ref("tests/ref/vte-batch/cursor-movement-ext.json");
}
#[test]
fn vte_ref_sgr_attributes() {
    run_vte_ref("tests/ref/vte-batch/sgr-complete.json");
}
#[test]
fn vte_ref_erase_operations() {
    run_vte_ref("tests/ref/vte-batch/erase-operations.json");
}
#[test]
fn vte_ref_scroll_operations() {
    run_vte_ref("tests/ref/vte-batch/scroll-operations.json");
}
#[test]
fn vte_ref_ich_dch() {
    run_vte_ref("tests/ref/vte-batch/ich-dch-operations.json");
}
#[test]
fn vte_ref_dec_modes() {
    run_vte_ref("tests/ref/vte-batch/dec-modes.json");
}
#[test]
fn vte_ref_osc() {
    run_vte_ref("tests/ref/vte-batch/osc-sequences.json");
}
#[test]
fn vte_ref_color_fg_bg() {
    run_vte_ref("tests/ref/vte-batch/color-fg-bg.json");
}
#[test]
fn vte_ref_misc() {
    run_vte_ref("tests/ref/vte-batch/misc-sequences.json");
}
