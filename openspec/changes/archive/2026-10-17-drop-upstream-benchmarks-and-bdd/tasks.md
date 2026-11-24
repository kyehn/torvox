## 任务

- [x] 删除 `native/tests/external_benchmarks/`（5 文件 9 例）：期望由上游决定，判红只说明上游 rev 变动。
- [x] 删除 `native/tests/bdd/`（5 文件）与 `native/tests/features/`（7 特性 12 场景）：12/12 场景与 `native/src` 既有单测重复。
- [x] `native/Cargo.toml` 删 `[[test]] name = "bdd"` 与 dev-dependencies `cucumber`／`tokio`／`ureq`；`criterion`／`proptest`／`shuttle` 保留。
- [x] `native/tests/concurrency.rs` 保留（`EventQueue`，非上游、非重复）。
- [x] `cargo machete` 通过（无孤儿 dev-dependency）；`Cargo.lock` 随之收缩 934 行。

## 验证证据

nix develop 内，改动后实测：

- `cargo clippy --all-targets -- --deny warnings` → `clippy_exit=0`
- `cargo machete` → `machete_exit=0`
- `cargo test --no-fail-fast` → lib `623 passed; 0 failed`（76.37s）＋ `tests/concurrency.rs` `2 passed; 0 failed` ＋ doc-tests 0；`test_exit=0`
- `cargo bench --no-run` → `bench_exit=0`（三个 bench 目标在新 lock 下链接通过）；`cargo doc --no-deps` → `doc_exit=0`
- `cargo fmt --check` → exit 0

删除前后目标数对照：`cargo test` 目标从 lib ＋ `concurrency` ＋ `bdd` ＋ `external_benchmarks` 收缩为 lib ＋ `concurrency`；断言总数只减不增（无独占覆盖被删）。
