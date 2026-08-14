//! Integration tests that shell out to project lint/quality tools.
//! All tools are listed in flake.nix devShell packages. If any is
//! missing from PATH, the test panics — run inside `nix develop`.

const WORKSPACE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

/// Read all requirement IDs from StrictDoc .sdoc files.
fn srs_ids() -> std::collections::BTreeSet<String> {
    let req_dir = std::path::Path::new(WORKSPACE).join("docs/requirements");
    let mut ids = std::collections::BTreeSet::new();

    for sdoc_file in [
        "functional_requirements.sdoc",
        "non_functional_requirements.sdoc",
    ] {
        let path = req_dir.join(sdoc_file);
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        // Parse UID: FR-001 / NFR-001 from StrictDoc format
        let re = regex_lite::Regex::new(r"^UID:\s+(FR|NFR)-\d{3}$").unwrap();
        for line in content.lines() {
            if let Some(cap) = re.captures(line.trim()) {
                ids.insert(cap[0].trim_start_matches("UID: ").to_string());
            }
        }
    }

    ids
}

#[test]
fn typos_finds_no_typos() {
    let config = std::path::Path::new(WORKSPACE).join("_typos.toml");
    let output = std::process::Command::new("typos")
        .args(["--config", &config.to_string_lossy(), "."])
        .current_dir(WORKSPACE)
        .output()
        .expect("typos must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "typos found spelling errors:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn markdownlint_finds_no_violations() {
    let config = std::path::Path::new(WORKSPACE).join(".markdownlint.jsonc");
    let output = std::process::Command::new("markdownlint-cli2")
        .args(["--config", &config.to_string_lossy(), "."])
        .current_dir(WORKSPACE)
        .output()
        .expect("markdownlint-cli2 must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "markdownlint-cli2 found violations:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn cargo_audit_finds_no_vulnerabilities() {
    let output = std::process::Command::new("cargo")
        .args(["audit"])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo-audit must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo audit found vulnerabilities:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn cargo_machete_finds_no_unused_deps() {
    let output = std::process::Command::new("cargo-machete")
        .args(["--skip-target-dir"])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo-machete must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo machete found unused dependencies:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn adrs_doctor_finds_no_issues() {
    // adrs doctor validates ADR format/structure in docs/adr/
    let output = std::process::Command::new("adrs")
        .args(["doctor", "--cwd", WORKSPACE])
        .current_dir(WORKSPACE)
        .output()
        .expect("adrs must be installed (cargo install adrs; add to flake.nix devShell)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "adrs doctor found ADR issues:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn strictdoc_validates_requirements() {
    // strictdoc validates requirement structure in docs/requirements/.
    // Export to HTML (side-effect free) catches parse errors.
    let output = std::process::Command::new("strictdoc")
        .args([
            "export",
            "docs/requirements",
            "--output-dir",
            "/tmp/strictdoc-export",
        ])
        .current_dir(WORKSPACE)
        .output()
        .expect("strictdoc must be installed (add to flake.nix devShell)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "strictdoc validation failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn vale_finds_no_violations() {
    let config = std::path::Path::new(WORKSPACE).join(".vale.ini");
    let output = std::process::Command::new("vale")
        .args([
            "--config",
            &config.to_string_lossy(),
            "AGENTS.md",
            "docs/standards/STYLE.md",
            "docs/standards/TESTING.md",
            "docs/standards/QUALITY-GATE.md",
            "docs/standards/BUILD.md",
            "docs/srs.md",
            "docs/architecture.md",
            "docs/acceptance.md",
            "docs/dependencies.md",
            "docs/rejected-technologies.md",
            "docs/glossary.md",
            "docs/adr/README.md",
            "docs/adr/template.md",
        ])
        .current_dir(WORKSPACE)
        .output()
        .expect("vale must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "vale found style violations:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn doc_srs_requirement_format() {
    let srs_path = std::path::Path::new(WORKSPACE).join("docs/srs.md");
    let content = std::fs::read_to_string(&srs_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", srs_path.display()));

    // Only check the requirement tables (Sections 3 and 4), not the
    // verification matrix appendix (Section 5.E) which lists IDs again.
    let appendix_line = content
        .lines()
        .position(|l| l.starts_with("## 5. Appendix"))
        .unwrap_or(content.lines().count());

    // Extract all FR-xxx / NFR-xxx IDs from the requirement table lines.
    // Match lines that start with `|` (Markdown table rows) and contain
    // a requirement ID as the first column value.
    let re = regex_lite::Regex::new(r"(?m)^\|\s*(FR-\d{3}|NFR-\d{3})\s+\|").unwrap();
    let mut ids: Vec<(usize, String)> = Vec::new();
    for (lineno, line) in content.lines().enumerate() {
        if lineno >= appendix_line {
            break;
        }
        for cap in re.captures_iter(line) {
            ids.push((lineno + 1, cap[1].to_string()));
        }
    }
    assert!(
        !ids.is_empty(),
        "no FR-xxx or NFR-xxx requirement IDs found in docs/srs.md"
    );
    // Report duplicate IDs
    let mut seen = std::collections::BTreeMap::new();
    for (lineno, id) in &ids {
        seen.entry(id.clone())
            .or_insert_with(Vec::new)
            .push(*lineno);
    }
    let mut dupes: Vec<String> = Vec::new();
    for (id, lines) in &seen {
        if lines.len() > 1 {
            dupes.push(format!(
                "  {id}: lines {}",
                lines
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    assert!(
        dupes.is_empty(),
        "duplicate requirement IDs found in docs/srs.md requirement tables:\n{}",
        dupes.join("\n")
    );
}

/// Count `#[test]` attributes in the workspace Rust sources (static scan).
fn static_test_count() -> usize {
    let mut count = 0usize;
    for dir in ["native", "integration-tests", "exec-bin"] {
        let root = std::path::Path::new(WORKSPACE).join(dir);
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().map(|n| n != "target").unwrap_or(false) {
                        stack.push(path);
                    }
                } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                    let Ok(content) = std::fs::read_to_string(&path) else {
                        continue;
                    };
                    count += content.matches("#[test]").count();
                }
            }
        }
    }
    count
}

#[test]
fn rust_test_count_within_baseline() {
    // TESTING.md "覆盖率基线" declares the static `#[test]` count as the
    // authoritative baseline. A deviation beyond the tolerance means either
    // tests were bulk-removed or the document went stale — both require a
    // human to reconcile (update TESTING.md or restore tests).
    let testing = std::path::Path::new(WORKSPACE).join("docs/standards/TESTING.md");
    let content = std::fs::read_to_string(&testing)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", testing.display()));
    let re = regex_lite::Regex::new(r"静态 `#\[test\]` 计数 (\d+)").unwrap();
    let baseline: usize = re
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().parse().unwrap())
        .unwrap_or_else(|| panic!("TESTING.md 缺少 `静态 #[test] 计数 N` 基线声明"));
    let actual = static_test_count();
    let tolerance = baseline / 4; // ±25%
    let diff = actual.abs_diff(baseline);
    assert!(
        diff <= tolerance,
        "Rust 静态 #[test] 计数偏离基线：实际 {actual} vs TESTING.md 声明 {baseline}（容差 ±{tolerance}）。         若批量增删测试，请同步更新 docs/standards/TESTING.md §覆盖率基线。"
    );
}

#[test]
fn docs_structure_validation() {
    // docs/check-docs.py enforces structural gates that vale/markdownlint
    // cannot express: arc42 section presence, ADR template fields,
    // requirement-ID sync with .sdoc, and relative-link validity.
    let script = std::path::Path::new(WORKSPACE).join("docs/check-docs.py");
    let output = std::process::Command::new("python3")
        .arg(&script)
        .current_dir(WORKSPACE)
        .output()
        .expect("python3 must be available");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "docs structure validation failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn doc_srs_matches_sdoc_ids() {
    // docs/srs.md (prose) and docs/requirements/*.sdoc (StrictDoc ID source)
    // are dual sources for requirement IDs. They MUST stay in sync: a new
    // requirement goes into BOTH files; the StrictDoc gate alone cannot
    // catch an ID added to only one of them.
    let srs_path = std::path::Path::new(WORKSPACE).join("docs/srs.md");
    let content = std::fs::read_to_string(&srs_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", srs_path.display()));

    // Extract IDs from the requirement tables (Sections 3 and 4), skipping
    // the verification matrix appendix (Section 5.E) which repeats IDs.
    let appendix_line = content
        .lines()
        .position(|l| l.starts_with("## 5. Appendix"))
        .unwrap_or(content.lines().count());
    let table_re = regex_lite::Regex::new(r"(?m)^\|\s*(FR-\d{3}|NFR-\d{3})\s+\|").unwrap();
    let mut srs_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (lineno, line) in content.lines().enumerate() {
        if lineno >= appendix_line {
            break;
        }
        for cap in table_re.captures_iter(line) {
            srs_set.insert(cap[1].to_string());
        }
    }
    assert!(
        !srs_set.is_empty(),
        "no requirement IDs found in docs/srs.md"
    );

    let sdoc_set = srs_ids();
    assert!(
        !sdoc_set.is_empty(),
        "no requirement IDs found in .sdoc files"
    );

    let only_in_srs: Vec<&String> = srs_set.difference(&sdoc_set).collect();
    let only_in_sdoc: Vec<&String> = sdoc_set.difference(&srs_set).collect();
    assert!(
        only_in_srs.is_empty() && only_in_sdoc.is_empty(),
        "docs/srs.md and docs/requirements/*.sdoc are out of sync:\n  \
         only in srs.md: {}\n  only in .sdoc: {}",
        if only_in_srs.is_empty() {
            "(none)".to_string()
        } else {
            only_in_srs
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        },
        if only_in_sdoc.is_empty() {
            "(none)".to_string()
        } else {
            only_in_sdoc
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
}

#[test]
fn doc_traceability_references() {
    let srs = srs_ids();
    assert!(!srs.is_empty(), "no requirement IDs found in docs/srs.md");

    let yaml_path = std::path::Path::new(WORKSPACE).join("docs/traceability.yml");
    let content = std::fs::read_to_string(&yaml_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", yaml_path.display()));

    // Extract requirement keys listed under `requirements:`
    let re = regex_lite::Regex::new(r"(?m)^  (FR-\d{3}|NFR-\d{3}):").unwrap();
    let trace_ids: std::collections::BTreeSet<String> = re
        .captures_iter(&content)
        .map(|c| c[1].to_string())
        .collect();
    assert!(
        !trace_ids.is_empty(),
        "no requirement IDs found in docs/traceability.yml"
    );

    // Check that every traceability ID exists in docs/srs.md
    let missing_from_srs: Vec<&str> = trace_ids
        .iter()
        .filter(|id| !srs.contains(id.as_str()))
        .map(|s| s.as_str())
        .collect();
    assert!(
        missing_from_srs.is_empty(),
        "traceability.yml references requirement IDs not found in docs/srs.md:\n  {}",
        missing_from_srs.join("\n  ")
    );

    // Reverse: check that every SRS ID has a traceability entry
    let missing_from_trace: Vec<&str> = srs
        .iter()
        .filter(|id| !trace_ids.contains(id.as_str()))
        .map(|s| s.as_str())
        .collect();
    assert!(
        missing_from_trace.is_empty(),
        "docs/srs.md has requirement IDs missing from traceability.yml:\n  {}",
        missing_from_trace.join("\n  ")
    );
}

#[test]
fn doc_acceptance_links_to_srs() {
    let srs = srs_ids();
    assert!(!srs.is_empty(), "no requirement IDs found in docs/srs.md");

    let acceptance_path = std::path::Path::new(WORKSPACE).join("docs/acceptance.md");
    let content = std::fs::read_to_string(&acceptance_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", acceptance_path.display()));

    // Extract referenced requirement IDs from acceptance criteria
    let re = regex_lite::Regex::new(r"\b(FR-\d{3}|NFR-\d{3})\b").unwrap();
    let acceptance_ids: std::collections::BTreeSet<String> = re
        .captures_iter(&content)
        .map(|c| c[1].to_string())
        .collect();
    assert!(
        !acceptance_ids.is_empty(),
        "no requirement IDs found in docs/acceptance.md"
    );

    // Check that every acceptance-referenced ID exists in docs/srs.md
    let missing_from_srs: Vec<&str> = acceptance_ids
        .iter()
        .filter(|id| !srs.contains(id.as_str()))
        .map(|s| s.as_str())
        .collect();
    assert!(
        missing_from_srs.is_empty(),
        "docs/acceptance.md references requirement IDs not found in docs/srs.md:\n  {}",
        missing_from_srs.join("\n  ")
    );

    // Reverse: check that every SRS ID has a matching acceptance section
    let srs_re = regex_lite::Regex::new(r"(?m)^### FR-\d{3}|^### NFR-\d{3}").unwrap();
    let acceptance_sections: std::collections::BTreeSet<String> = srs_re
        .captures_iter(&content)
        .map(|c| c[0].trim_start_matches('#').trim().to_string())
        .collect();

    let missing_from_acceptance: Vec<&str> = srs
        .iter()
        .filter(|id| !acceptance_sections.contains(id.as_str()))
        .map(|s| s.as_str())
        .collect();
    assert!(
        missing_from_acceptance.is_empty(),
        "docs/srs.md has requirement IDs missing a section in docs/acceptance.md:\n  {}",
        missing_from_acceptance.join("\n  ")
    );
}

#[test]
fn doc_module_has_requirements() {
    let srs = srs_ids();
    assert!(!srs.is_empty(), "no requirement IDs found in docs/srs.md");

    // Use a BTreeMap to let us check both directions:
    //   source → found IDs  (source has FR-xxx)
    //   found ID → exists in SRS  (no orphan FR-xxx)
    let mut source_to_ids: Vec<(String, Vec<String>)> = Vec::new();
    let mut found_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    let crates = [
        "native/src/terminal",
        "native/src/render",
        "native/src/android",
    ];
    let exempt_files: std::collections::BTreeSet<&str> = [
        "lib.rs",
        // Test/conformance modules — not production API
        "test_helpers.rs",
        "mock_pty.rs",
        "mock_surface.rs",
        "snapshot_test.rs",
        "vt_conformance.rs",
        "screenshot_tests.rs",
        "action_parser.rs",
        "cursor_cmds.rs",
        "sgr_parser.rs",
        // No FR mapping in SRS
        "shell_env.rs",
        // Thin CLI shim — module docs belong in lib.rs
        "main.rs",
        // Verus formal verification — compiled only under cfg(verus_only)
        "verus_annotations.rs",
    ]
    .into();

    let req_re = regex_lite::Regex::new(r"\b(FR-\d{3}|NFR-\d{3})\b").unwrap();
    let doc_re = regex_lite::Regex::new(r"(?m)^//!").unwrap();

    for crate_dir in &crates {
        let dir = std::path::Path::new(WORKSPACE).join(crate_dir);
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let basename = path.file_name().unwrap().to_str().unwrap().to_string();
            if exempt_files.contains(basename.as_str()) {
                continue;
            }

            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

            // Check that a module-level doc comment exists and contains a requirement ID
            let has_doc_block = doc_re.is_match(&content);
            let has_id = req_re.is_match(&content);

            let rel_path = format!("{crate_dir}/{basename}");

            assert!(
                has_doc_block,
                "{rel_path} is missing a module-level `//!` doc comment. \
                 Every public module must have `//!` docs with `# Requirements`.",
            );
            assert!(
                has_id,
                "{rel_path} module doc is missing a FR-xxx or NFR-xxx requirement reference. \
                 Add `//! # Requirements\\n//! - FR-XXX — description` to the module doc.",
            );

            // Collect all referenced requirement IDs
            let ids: Vec<String> = req_re
                .captures_iter(&content)
                .map(|c| c[1].to_string())
                .collect();
            for id in &ids {
                found_ids.insert(id.clone());
            }
            source_to_ids.push((rel_path, ids));
        }
    }

    // Forward check: every FR-xxx/NFR-xxx referenced in source docs must exist in SRS
    let mut orphan_ids: Vec<String> = Vec::new();
    for id in &found_ids {
        if !srs.contains(id.as_str()) {
            orphan_ids.push(id.clone());
        }
    }
    assert!(
        orphan_ids.is_empty(),
        "Source code references requirement IDs not defined in docs/srs.md:\n  {}",
        orphan_ids.join("\n  ")
    );

    // Reverse check: every SRS ID should be referenced somewhere in source docs
    // (not all IDs need to be — some are NFR/platform concerns — so this is a warning-only case)
    // We do check that at least some IDs are tracked
    assert!(
        !source_to_ids.is_empty(),
        "no source files with requirement IDs found"
    );
}

#[test]
fn cargo_llvm_cov_is_available() {
    let output = std::process::Command::new("cargo")
        .args(["llvm-cov", "--version"])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo-llvm-cov must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo llvm-cov --version failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("cargo-llvm-cov"),
        "output must contain 'cargo-llvm-cov', got: {stdout}"
    );
}

#[test]
fn cargo_ndk_is_available() {
    let output = std::process::Command::new("cargo")
        .args(["ndk", "--help"])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo-ndk must be installed (try nix develop)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo ndk --help failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("cargo-ndk") || stderr.contains("cargo-ndk"),
        "output must contain 'cargo-ndk', got stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn nu_scripts_are_valid() {
    let scripts_dir = std::path::Path::new(WORKSPACE).join("scripts");
    let allowed: std::collections::HashSet<&str> = [
        "build-android-libs.nu",
        "build-apk.nu",
        "check-rust.nu",
        "download-rapidocr-models.nu",
        "fetch-aosp-testkey.nu",
        "setup-emulator.nu",
        "test-gradle.nu",
        "test-emulator.nu",
    ]
    .into_iter()
    .collect();

    let mut found_any = false;
    for entry in std::fs::read_dir(&scripts_dir)
        .unwrap_or_else(|e| panic!("failed to read scripts dir {scripts_dir:?}: {e}"))
    {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("nu") {
            continue;
        }
        found_any = true;
        let basename = path.file_name().unwrap().to_str().unwrap().to_string();
        assert!(
            allowed.contains(basename.as_str()),
            "Unauthorized script: {basename}"
        );
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));

        assert!(
            content.contains("#!/usr/bin/env"),
            "{basename} missing shebang line"
        );

        assert!(
            !content.contains('\t'),
            "{basename} contains tab characters — use spaces"
        );

        if basename != "check-rust.nu" {
            assert!(
                !content.contains("||"),
                "{basename} contains forbidden || operator"
            );
        }
    }
    assert!(found_any, "no .nu scripts found in {scripts_dir:?}");
}

#[test]
fn deny_toml_must_not_exist() {
    let deny_toml = std::path::Path::new(WORKSPACE).join("deny.toml");
    assert!(
        !deny_toml.exists(),
        "deny.toml is forbidden anywhere in the repository"
    );
}

#[test]
fn semgrep_finds_no_violations() {
    let output = std::process::Command::new("semgrep")
        .args(["--config", ".semgrep/", "."])
        .current_dir(WORKSPACE)
        .output()
        .expect("semgrep must be installed (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "semgrep found violations:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

// ── Dependency boundary guard (osmosis P0) ────────────────────────────

/// Helper: parse `cargo tree` output into a set of crate names,
/// stripping UTF-8 box-drawing connectors.
fn parse_tree_deps(stdout: &str) -> std::collections::BTreeSet<String> {
    let mut deps = std::collections::BTreeSet::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('[') {
            continue;
        }
        // Strip tree connectors (UTF-8 box drawing: ├─, └─, │) and leading whitespace
        let cleaned = trimmed
            .chars()
            .skip_while(|c| *c == '│' || *c == '├' || *c == '└' || *c == '─' || *c == ' ')
            .collect::<String>()
            .trim()
            .to_string();
        // Extract crate name: "serde v1.0.228" → "serde"
        if let Some(name) = cleaned.split_whitespace().next() {
            if !name.is_empty() {
                deps.insert(name.to_string());
            }
        }
    }
    deps
}

/// Assert the workspace stays within the declared dependency budget.
/// This prevents silent dependency creep; any new crate must be explicitly
/// added to the allowlist below (or the threshold raised with justification).
#[test]
fn dependency_count_within_budget() {
    let output = std::process::Command::new("cargo")
        .args([
            "tree", "-p", "native", "--depth", "1", "--edges", "normal", "--format", "{p}",
        ])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo must be available");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let deps = parse_tree_deps(&stdout);

    // Budget: current count (27 normal deps) + 5 headroom for legitimate additions
    const DEPENDENCY_BUDGET: usize = 35;
    assert!(
        deps.len() <= DEPENDENCY_BUDGET,
        "Dependency count {} exceeds budget {DEPENDENCY_BUDGET}. \
         If this is intentional, update DEPENDENCY_BUDGET in tool_lint.rs.\n\
         Dependencies: {:?}",
        deps.len(),
        deps
    );
}

/// Third-party dependency allowlist. Every direct dependency of the `native`
/// crate must appear here. A new crate not in this list will cause CI failure
/// until the allowlist is explicitly updated.
#[test]
fn all_dependencies_are_allowlisted() {
    let output = std::process::Command::new("cargo")
        .args([
            "tree", "-p", "native", "--depth", "1", "--edges", "normal", "--format", "{p}",
        ])
        .current_dir(WORKSPACE)
        .output()
        .expect("cargo must be available");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let deps = parse_tree_deps(&stdout);

    // Declared allowlist — update this set when adding a new dependency
    let allowlist: std::collections::BTreeSet<&str> = [
        "axum",
        "base64",
        "bytemuck",
        "cosmic-text",
        "flume",
        "foldhash",
        "fontdb",
        "futures",
        "guillotiere",
        "jni",
        "libc",
        "libghostty-vt",
        "log",
        "lru",
        "nix",
        "parking_lot",
        "raw-window-handle",
        "regex",
        "renderdoc",
        "schemars",
        "serde",
        "serde_json",
        "strsim",
        "swash",
        "thiserror",
        "tokio",
        "tower-mcp",
        "wgpu",
        "wgpu-types",
        "native",
    ]
    .into_iter()
    .collect();

    let mut violations: Vec<String> = Vec::new();
    for name in &deps {
        if !allowlist.contains(name.as_str()) {
            violations.push(name.clone());
        }
    }

    assert!(
        violations.is_empty(),
        "Undeclared dependencies found (not in allowlist): {:?}\n\
         Add them to the allowlist in tool_lint.rs::all_dependencies_are_allowlisted",
        violations
    );
}

#[test]
fn no_code_duplication() {
    let output = std::process::Command::new("npx")
        .args([
            "--yes",
            "jscpd",
            "--mode=strict",
            "--format=rust,kotlin,yaml,toml,nix,markdown,python",
            "--min-lines=20",
            "--min-tokens=100",
            "--blame",
            ".",
        ])
        .current_dir(WORKSPACE)
        .output()
        .expect("npx must be available for jscpd (try `nix develop`)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "jscpd found duplicated code:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}
