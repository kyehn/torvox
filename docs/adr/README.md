# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for the torvox project.

All ADRs follow the Nygard format (`# N. Title`, `Date:`, `## Status`,
`## Context`, `## Decision`, `## Alternatives Considered`, `## Consequences`,
`## Compliance`) with requirement traceability via `docs/requirements/` (StrictDoc `.sdoc`)
and `docs/traceability.yml`. The `adrs doctor` gate (CI: `tool_lint.rs`
`adrs_doctor_finds_no_issues`) enforces this format.

## Active ADRs

| # | Title | Status | Requirement IDs |
|---|-------|--------|----------------|
| 1 | Crate Consolidation | ✅ Accepted | NFR-02, NFR-05 |
| 2 | Ghostty as the Single Source of Terminal State | ✅ Accepted | FR-01, NFR-01, NFR-03 |
| 3 | Bridge and Rendering Strategy | ✅ Accepted | FR-02, NFR-01, NFR-04 |
| 4 | Thread Model | ✅ Accepted | NFR-01, NFR-03, FR-02 |
| 5 | MCP Architecture | ✅ Accepted | FR-06, NFR-05 |
| 6 | Build and Dependency Simplification | ✅ Accepted | NFR-02, NFR-05 |
| 7 | Session Lifecycle and Android Integration | ✅ Accepted | FR-03, FR-04, NFR-03 |
| 8 | Rendering Pipeline | ✅ Accepted | FR-02, NFR-01 |
| 9 | Security Model | ✅ Accepted | NFR-009, NFR-014, NFR-019, FR-013 |
| 10 | Logging Architecture | ✅ Accepted | NFR-025, NFR-028 |
| 11 | Testing Strategy | ✅ Accepted | NFR-020, NFR-026, NFR-027, NFR-028, NFR-029, FR-017 |
| 12 | Dependency Management | ✅ Accepted | NFR-011, NFR-012, NFR-013 |

## Template

New ADRs should use `template.md`. See `docs/standards/QUALITY-GATE.md` for when an ADR is required.
