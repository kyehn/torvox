## 1. 规范定稿

- [x] 1.1 对照 termux-app 源码确认安装流程（staging/SYMLINKS/0700/termux.env）与项目实现一致
- [x] 1.2 对照确认环境变量映射，记录 LANG 等两处有意差异及理由
- [x] 1.3 长期规范与变更 delta 一致，`openspec validate --changes` 通过

## 2. 回归门控（后续实现轮次引用）

- [x] 2.1 安装失败保留旧 prefix 的用例存在且通过（BootstrapInstallerTest staging/原子重命名覆盖，testDebugUnitTest 通过）
- [x] 2.2 新会话 env 断言（PREFIX/HOME/PATH/TMPDIR/LANG/TERM）用例存在且通过（pty.rs build_env 系列 35/35 通过）
