# Spec: OSC 133 Semantic Prompt Segments

> Status: Implemented | Since: v7 (reference-adoption-v6)

## Purpose

Parse OSC 133 A/B/C/D semantic prompt markers from terminal output to identify
prompt boundaries, command input regions, and command output regions. This
enables features like command output extraction and prompt-start detection.

## Design

### OSC 133 Protocol (FinalTerm / termlib)

```
OSC 133 ; A ST   — Prompt start
OSC 133 ; B ST   — Prompt end / command input start
OSC 133 ; C ST   — Command output start
OSC 133 ; D ST   — Command output end (with optional exit code)
```

### State machine

```
IDLE → on A → CAPTURING_PROMPT
CAPTURING_PROMPT → on B → CAPTURING_OUTPUT
CAPTURING_OUTPUT → on D → output captured → IDLE
Any state → on A → reset, CAPTURING_PROMPT
```

### Key invariants

1. **BEL or ST terminators**: Both `\x07` (BEL) and `ESC \` (ST) terminate
   the escape sequence. A new `A` marker resets any in-progress capture.
2. **Cross-chunk boundary**: Segments may span multiple `process_output` calls;
   the state machine persists across calls.
3. **Exit code extraction**: The `D` marker may carry an exit code parameter
   (`OSC 133 ; D ; <exit_code> ST`).
4. **Byte offset tracking**: Each byte's offset within the current chunk is
   tracked for `SemanticSegment` construction.

### Files

- `native/src/terminal/output_processor.rs` — `scan_osc133()`, `SemanticSegment`
- Ghostty origin: `libghostty-vt-sys` callbacks drive the parser

### Test contract

- 8 test cases covering: normal flow, cross-chunk, BEL vs ST terminators,
  exit code extraction, reset-on-A, non-OSC-133 passthrough
