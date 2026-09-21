## 1. 规范文档

- [x] 1.1 `docs/specification/DESIGN.md` Bootstrap 节白名单新增 `ENV` 条目
- [x] 1.2 本 change 文档（proposal / specs / design / tasks）编写完成
- [x] 1.3 实现验证后，按实际补充 openspec specs 实现细节
      （specs/shell-env/spec.md 已含 ENV 注入 + 白名单拒绝两条 requirement 与场景）

## 2. 代码实现（另一 agent 负责）

- [x] 2.1 `build_env` 新增 `ENV` 键，值为 `$HOME/.mkshrc`
      （native/src/terminal/pty.rs `build_env`：`ENV` → `{home}/.mkshrc`；已合入 main d72ab04）
- [x] 2.2 白名单单测/BDD 覆盖 `ENV`
      （`build_env_includes_mksh_startup_pointer` 单测断言 `ENV=/tmp/test_home/.mkshrc`）

## 3. 设备验证

- [x] 3.1 `ENV=$HOME/.mkshrc /system/bin/sh -i` 加载短提示符，无横滚与左缘裁剪，`clear` 正常
      （emulator-5554 实测：有 ENV 时 PS1_LEN=2（`$ `），无 ENV 时 77 列长提示符；`clear` exit 0；80 列长命令单行输出无截断。`.mkshrc` 按 `ensureMkshPromptRc` 相同内容经 run-as 播种后验证 ENV 加载路径）
- [x] 3.2 bash 启动行为与注入前一致
      （`ENV=<不存在路径> bash --noprofile --norc -c` 两次均 exit 0；`build_env_rejects_unlisted_variables` 白名单约束不变）
