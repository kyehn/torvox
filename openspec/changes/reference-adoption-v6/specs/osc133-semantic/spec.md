# Spec: osc133-semantic

## Purpose

Shell integration via OSC 133 escape sequences to mark prompt/command/output boundaries.

## Requirements

- REQ-O1: Parse OSC 133 A (prompt start), B (command input), C (command output), D (command finished)
- REQ-O2: Store semantic segments with start_col, end_col, type
- REQ-O3: Record exit code on D (command finished)
- REQ-O4: Provide `getLastCommandOutput()` JNI method
- REQ-O5: Handle multiline commands correctly

## Test Cases

| ID | Input | Expected |
|----|-------|----------|
| TC-O1 | `\x1b]133;A\x07` at col 0 | PromptStart segment at col 0 |
| TC-O2 | `\x1b]133;B\x07` at col 5 | CommandInput segment at col 5 |
| TC-O3 | `\x1b]133;C\x07` at col 0 | CommandOutput segment at col 0 |
| TC-O4 | `\x1b]133;D;0\x07` | CommandFinished with exit_code=0 |
| TC-O5 | Multiple commands | getLastCommandOutput returns latest |

## Traceability

- Source: termlib `SemanticType.kt` + `OscParser.kt:275-340`
- Gap: torvox has `extract_osc133` single-value marker, no segments
