# N. Title

Date: YYYY-MM-DD

## Status

Proposed | Accepted | Deprecated | Superseded

## Requirement IDs

FR-xxx, NFR-xxx
(see [docs/requirements/](../requirements/) for requirement definitions)

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

- CI enforces `adrs doctor` (see `docs/standards/QUALITY-GATE.md`) — new
  ADRs must pass the adrs Nygard-format lint.
- CI enforces `cargo clippy --all -- --deny warnings` (see
  [docs/standards/QUALITY-GATE.md](../standards/QUALITY-GATE.md)).
- The JNI bridge type sync check in `docs/standards/QUALITY-GATE.md` is run
  before every commit that changes JNI signatures in `ffi.rs`.

---

*This decision is registered in [docs/traceability.yml](../traceability.yml)
for cross-artifact tracing.*
