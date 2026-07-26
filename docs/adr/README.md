# Architecture Decision Records

## What Are ADRs and Why We Use Them

An Architecture Decision Record (ADR) is a short document that captures an
architecturally significant decision and the reasoning behind it. ADRs answer
the question "why is the system built this way?" for future contributors.

We use ADRs to:

- **Preserve context**: Decisions made today are understood months or years
  later, even when the original authors are unavailable.
- **Enforce transparency**: Every architectural trade-off is documented and
  reviewable.
- **Avoid repeated debate**: Written rationale prevents revisiting closed
  discussions.
- **Integrate with requirements**: Each ADR references the functional or
  non-functional requirement it satisfies (see `docs/srs.md`).
- **Trace across artifacts**: ADRs are cross-linked in
  `docs/traceability.yml` alongside requirements, tests, and checks.

## Status Options

Every ADR has one of four statuses:

| Status       | Meaning |
|--------------|---------|
| **Proposed** | Under review; not yet adopted |
| **Accepted** | Approved and in effect |
| **Deprecated** | Still observed but no longer recommended; a replacement ADR exists |
| **Superseded** | Replaced by a later ADR; kept only for historical reference |

## Template

New ADRs are created by copying `docs/adr/template.md` and following its
structure. The format is the standard **Michael Nygard** layout:

1. **Title** — short noun-phrase
2. **Status** — one of the four statuses above
3. **Context** — the problem, constraints, and forces
4. **Decision** — what was decided and why
5. **Consequences** — trade-offs, both positive and negative

In addition to the Nygard sections, Torvox ADRs include:

- **Date** — YYYY-MM-DD adoption date
- **Requirement IDs** — links to `docs/srs.md`
- **Compliance** — how compliance is verified

## Existing ADRs

| ID | Title | Status | Date |
|----|-------|--------|------|
| *(none yet)* | | | |

New ADRs are numbered sequentially: `0001-my-decision.md`,
`0002-next-decision.md`, etc.

## Cross-Linking

All ADRs are registered in `docs/traceability.yml` alongside the requirements,
tests, and checks they relate to. Run `cargo test -p integration-tests --test tool_lint -- doc_` to
verify that every ADR, requirement, and test is properly linked.

## Writing Guidelines

- One decision per ADR. If a decision depends on another, link to it.
- Keep the total document under one page when possible.
- Use plain language. Avoid jargon that won't be understood in two years.
- Use present tense for the decision itself ("the terminal connects via boltffi"
  not "will connect via boltffi").
- Record rejected alternatives briefly when they are commonly proposed again.
