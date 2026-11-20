# 任务

## 一、规范留档

- [x] 核对 2026-11-20 12-45-04 日志的设备丢失时序（首帧 → 丢失 → 换窗 → 重建 → 恢复）
- [x] 确认应用侧四个环节（判定/换窗/重建/恢复）均命中既有规范，无可修环节
- [x] `render-stability` 补「冷启动首帧后的设备丢失按有界恢复自愈」场景：
      完整时序、丢失的报告路径、261ms 恢复耗时、根因未定位的如实记录

## 二、验证

- [x] `openspec validate --all --strict`：通过
- [x] `markdownlint-cli2 "**/*.md"`：0 issues
- [x] `semgrep scan --error --config .semgrep/rules.yml`：0 findings
