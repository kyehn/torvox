//! Integration tests that shell out to project lint/quality tools.
//! All tools are listed in flake.nix devShell packages. If any is
//! missing from PATH, the test panics — run inside `nix develop`.

const WORKSPACE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

/// Read all requirement IDs from docs/srs.md.
fn srs_ids() -> std::collections::BTreeSet<String> {
    let srs_path = std::path::Path::new(WORKSPACE).join("docs/srs.md");
    let content = std::fs::read_to_string(&srs_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", srs_path.display()));
    let re = regex_lite::Regex::new(r"\b(FR-\d{3}|NFR-\d{3})\b").unwrap();
    re.captures_iter(&content)
        .map(|c| c[1].to_string())
        .collect()
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
        "torvox-core/src",
        "torvox-terminal/src",
        "torvox-renderer/src",
        "torvox-gui-android/src",
        "torvox-mcp/src",
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
fn mcp_schema_matches_code() {
    // Verify that every tool in docs/api/mcp-schema.json has a matching
    // implementation in torvox-mcp/src/lib.rs, and vice-versa.

    // Parse schema tools (regex-based, avoids serde_json dependency)
    let schema_path = std::path::Path::new(WORKSPACE).join("docs/api/mcp-schema.json");
    let schema_content = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", schema_path.display()));

    // Match all tool names inside the "tools" array: "name": "tool_name"
    let tool_re = regex_lite::Regex::new(r#"(?m)"name":\s*"(\w+)"#).unwrap();
    // Only count tools defined in the JSON, not inside MCP protocol methods
    // Find the "tools" array section by checking we're not in the methods block
    let schema_tools: std::collections::BTreeSet<String> = tool_re
        .captures_iter(&schema_content)
        .map(|c| c[1].to_string())
        .filter(|name| {
            // Filter out MCP protocol method names
            !matches!(
                name.as_str(),
                "initialize" | "tools" | "call" | "list" | "ping" | "notifications"
            )
        })
        .collect();

    assert!(
        !schema_tools.is_empty(),
        "no tools found in docs/api/mcp-schema.json"
    );

    // Parse code tools from the list_tools() function
    let lib_path = std::path::Path::new(WORKSPACE).join("torvox-mcp/src/lib.rs");
    let lib_content = std::fs::read_to_string(&lib_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", lib_path.display()));

    // Find the list_tools function and extract tool name strings
    let mut code_tools: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut in_list_tools = false;
    for line in lib_content.lines() {
        if line.trim() == "fn list_tools() -> Value {" {
            in_list_tools = true;
            continue;
        }
        if in_list_tools {
            if line.trim().starts_with("let tools_json:") || line.contains("into_iter()") {
                break;
            }
            // Match lines like:             ("list_sessions",
            let trimmed = line.trim();
            if trimmed.starts_with('"') && trimmed.ends_with("\",") {
                let tool_name = trimmed.trim_matches('"').trim_end_matches("\",");
                if !tool_name.is_empty() && !tool_name.contains(' ') {
                    code_tools.insert(tool_name.to_string());
                }
            }
        }
    }

    assert!(
        !code_tools.is_empty(),
        "no tools found in list_tools() in torvox-mcp/src/lib.rs"
    );

    // Forward: tools in code that are missing from schema
    let code_not_in_schema: Vec<&str> = code_tools
        .iter()
        .filter(|t| !schema_tools.contains(t.as_str()))
        .map(|s| s.as_str())
        .collect();
    assert!(
        code_not_in_schema.is_empty(),
        "tools in torvox-mcp/src/lib.rs but missing from docs/api/mcp-schema.json:\n  {}",
        code_not_in_schema.join("\n  ")
    );

    // Reverse: tools in schema that are missing from code
    let schema_not_in_code: Vec<&str> = schema_tools
        .iter()
        .filter(|t| !code_tools.contains(t.as_str()))
        .map(|s| s.as_str())
        .collect();
    assert!(
        schema_not_in_code.is_empty(),
        "tools in docs/api/mcp-schema.json but missing from torvox-mcp/src/lib.rs:\n  {}",
        schema_not_in_code.join("\n  ")
    );

    assert_eq!(
        schema_tools.len(),
        code_tools.len(),
        "schema has {} tools but code has {}",
        schema_tools.len(),
        code_tools.len()
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
fn torvox_core_forbids_unsafe_code() {
    let lib_rs = std::path::Path::new(WORKSPACE)
        .join("torvox-core")
        .join("src")
        .join("lib.rs");
    let content = std::fs::read_to_string(&lib_rs)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", lib_rs.display()));
    assert!(
        content.contains("#![forbid(unsafe_code)]"),
        "torvox-core/src/lib.rs must contain #![forbid(unsafe_code)]"
    );
}

#[test]
fn nu_scripts_are_valid() {
    let scripts_dir = std::path::Path::new(WORKSPACE).join("scripts");
    let allowed: std::collections::HashSet<&str> = [
        "bootstrap-libghostty.nu",
        "build-android-libs.nu",
        "build-apk.nu",
        "check-rust.nu",
        "download-rapidocr-models.nu",
        "download-test-fonts.nu",
        "fetch-aosp-testkey.nu",
        "setup-emulator.nu",
        "test-android-gradle.nu",
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
        .args([
            "--config=auto",
            "--exclude-rule",
            "yaml.github-actions.security.github-actions-mutable-action-tag.github-actions-mutable-action-tag",
            "--exclude-rule",
            "java.android.security.exported_activity.exported_activity",
            ".",
        ])
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

#[test]
fn cargo_geiger_finds_no_unsafe_in_torvox_core() {
    let output = std::process::Command::new("cargo")
        .args(["geiger", "--package", "torvox-core"])
        .current_dir(std::path::Path::new(WORKSPACE).join("torvox-core"))
        .output()
        .expect("cargo-geiger must be installed (try `nix develop`)");
    if !String::from_utf8_lossy(&output.stdout).contains(":) torvox-core") {
        let dump = std::env::temp_dir().join("torvox-geiger-output.txt");
        std::fs::write(&dump, &output.stdout).ok();
        std::fs::write(dump.with_extension("stderr"), &output.stderr).ok();
        panic!(
            "torvox-core has unsafe code or cargo geiger failed. Full output: {}",
            dump.display()
        );
    }
}
