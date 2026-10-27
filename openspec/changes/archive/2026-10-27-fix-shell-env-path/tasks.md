# Tasks

- [x] `base_env_with_host` 按宿主 PATH + prefix bin 合成 `PATH`，`build_env` 不再
      单独写入 `PATH`。
- [x] 增补 `PATH` 合成规则单测（有 prefix / 目录缺失 / failsafe / 无宿主 PATH）。
- [x] 增补端到端用例：真实 fork/execve 后子进程回显 `PATH`。
- [x] 更新 `shell-env` 主规范。
