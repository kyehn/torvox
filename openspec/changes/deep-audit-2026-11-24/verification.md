## 连续五轮静态与动态验证

在同一提交上连续执行五轮，命令：

```bash
nix develop --command bash -c 'cd native && cargo clippy --all-targets -- -D warnings && cargo test'
```

| 轮次 | clippy | cargo test |
| --- | --- | --- |
| 1 | 通过 | 953 passed, 0 failed |
| 2 | 通过 | 953 passed, 0 failed |
| 3 | 通过 | 953 passed, 0 failed |
| 4 | 通过 | 953 passed, 0 failed |
| 5 | 通过 | 953 passed, 0 failed |

另在同一基线上通过：Kotlin `testDebugUnitTest`、`detekt`、`spotlessKotlinCheck`，以及连通测试 `VtCorrectnessInstrumentedTest` 15/15、`terminal.emulator.ui` 144/144、`terminal.emulator.installer` 全部通过。
