# Tasks

## 一、实现

- [x] 1.1 `font_db::load_files` 按路径去重：同一字体文件只装入一次
- [x] 1.2 `font_db::load_font_database` 把主字体、符号族、区域族的文件合成一份
      清单后统一装入，去重覆盖跨族重复

## 二、验证

- [x] 2.1 宿主单测：同一路径重复交给 `load_files` 时只装一次，库内无重复面
- [x] 2.2 `cargo test`、`cargo clippy --all-targets -- -D warnings`、
      `cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings` 通过
- [x] 2.3 `openspec validate --all --strict` 通过
- [x] 2.4 `markdownlint-cli2` 对本变更目录 0 issues
- [x] 2.5 semgrep 对改动文件无命中
