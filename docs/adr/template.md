# TITLE

- **Status**: Proposed | Accepted | Deprecated | Superseded
- **Date**: YYYY-MM-DD
- **Requirement IDs**: FR-xxx, NFR-xxx
  (see [docs/srs.md](../srs.md) for requirement definitions)

## Context

Describe the problem that motivated this decision. Include:

- The architectural forces at play (performance, maintainability, safety,
  platform constraints, team expertise, timeline).
- Any alternatives that were seriously considered and why they were rejected.
- Relevant background: prior decisions, upstream changes, platform
  deprecations, or external factors.

## Decision

State the decision clearly and in present tense. Explain:

- What was chosen
- Why it was chosen over the alternatives
- How it satisfies the linked requirements

**Example**: "The renderer uses wgpu with Vulkan as the backend on all
platforms, including Android, because …"

## Consequences

List the trade-offs introduced by this decision, both positive and negative.

### Positive

- Benefit 1
- Benefit 2

### Negative

- Drawback 1 (and mitigation, if any)
- Drawback 2 (and mitigation, if any)

## Compliance

Describe how to verify that the decision is followed. Be specific about
automated checks where possible.

**Examples**:

- CI enforces `cargo clippy --all -- --deny warnings` (see
  [docs/standards/QUALITY-GATE.md](../standards/QUALITY-GATE.md)).
- `cargo geiger --package torvox-core` exits zero — no new `unsafe` in the
  core data-model crate.
- The bridge type sync check in `docs/standards/QUALITY-GATE.md` is run
  before every commit that changes `torvox-core` types.

---

*This decision is registered in [docs/traceability.yml](../traceability.yml)
for cross-artifact tracing.*
