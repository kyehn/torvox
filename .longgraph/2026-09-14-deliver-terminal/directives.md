# torvox 终端全量修复交付 — Directives (supervisor → executor, one-way)

## Supervisor state (updated in place; executor ignores)

Last completed tick: none | audited through round: 0 | repo tips: main=87bacdf clean

## STANDING — authority only (always in force; treat like red lines)

S-001 · PRE-AUTH — 每小步 git commit 并 push 到当前分支 main 已由 owner 授权；仅提交本轮已验证 slice 的 exact write set，提交信息一行；禁止 reset/checkout 还原、禁止 force push、禁止跨分支推送。
S-002 · .github/scripts/flake.nix/rust-toolchain.toml/README/AGENTS/docs/specification 只读，禁止改实际内容；拼写排版修正除外。GHA 变绿只能靠修产品代码。
S-003 · 不降低验收口径：整包交付，模拟器两次连续全绿，日志加截图/OCR 验证；mock-only 证据不算数；不得依靠旧 run 结论，一切重新取证。
S-004 · 高成本命令输出一律 tee 到 /tmp 具名日志，不在 ledger 粘贴全文。

## Corrections (numbered; live queue = not-yet-folded only)

(none yet)
