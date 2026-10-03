# 任务

## 1. 门禁恢复（非保护文件）

- [x] 1.1 新增`.semgrepignore`恢复`src/test`扫描并实测验证
- [x] 1.2 每步跑相关门禁并小步提交推送

## 1a. 本轮增量修复（轮次 10–15 持续审查项，均已合入验证）

- [x] N1-30 Terminate 投递改短时 send_timeout
- [x] N1-32 destroySession 立即投递子进程结束信号（Session::request_exit）
- [x] N1-33 REQUEST_REGISTRY guard 不跨 JNI 调用
- [x] N1-34 createSession 回滚桥接关闭遵守渲染线程存活守卫
- [x] N2-65 ACTIVE_SESSION_ID CAS 成功序改 Release
- [x] N2-69 空闲 tick 不再无脑重建 CellData
- [x] N2-70 清空路径 _state 重置改 CAS
- [x] N2-72 PARTIAL_WAKE_LOCK 改无超时持有
- [x] N2-73 引导 URL 防抖冲刷挂到 onCleared
- [x] N2-77 移除无调用方的 GLOBAL_SURFACE/pending_gpu_drain
- [x] N2-41 PTY 主从 fd 置 CLOEXEC
- [x] N2-5 工作线程死亡冻结升级为 error
- [x] N2-6 kgp 管线按表面格式比对重建
- [x] N2-72b 唤醒锁改「超时 + 半程续期」（lint WakelockTimeout 与空闲冻结同时满足）
- [x] N2-73b ViewModel `onCleared` 冲刷引导 URL（lint EmptySuperCall 一并消除）
- [x] 门禁两处 `runCatching` 改显式 try/catch / assertThrows（check 工作流恢复绿）
- [x] 渲染线程存活守卫收敛为 `closeBridgeUnlessRenderThreadAlive` 单一入口

## 2. Kotlin小步修复

- [x] 2.1 主题应用切后台调度（N1-21）
- [x] 2.2 点选计数复位与拖尾守卫前移（N1-22）
- [x] 2.3 表面销毁清尺寸字段（N1-24）
- [x] 2.4 清除缓存递归与安装包残留删除（N2-27/N2-28）

## 3. Rust小步修复

- [x] 3.1 删除`cached_scrollback`并更正超时注释（N1-36）
- [x] 3.2 批量更正成本数字注释（N2-79~N2-84，已修Rust三处）

## 4. 高危项（需调试定位后动）

- [x] 4.1 行缓存方向裁决后动（N0-13，`row_cache` 已核实不存在）
- [x] 4.2 滚动去重键纳入视口偏移（N0-14）
- [x] 4.3 选区行钳位改绝对空间（N0-15）
- [x] 4.4 渲染锁序与输出泵（2.1/2.2/3.1延续，均已合入并验证）

## 5. 本轮低风险切片

- [x] 5.1 默认字体名查询锁外分配（P1-3/N2-66）
- [x] 5.2 Surface内绝对行换算收敛到唯一来源（N2-97）
- [x] 5.3 缩放落定用未钳位值判定（N2-96）

## 6. CI门禁与查询日志（本轮增量，已验证）

- [x] 6.1 子进程诊断串改通用前缀（`pty.rs`三处），check门禁semgrep 32规则0发现
- [x] 6.2 Bridge九个查询经onQuery记警告回缺省，kotlin semgrep 16规则0发现
- [x] 6.3 搜索用例prompt断言附终端尾部，定向下一轮build诊断
- [x] 6.4 写回通知线程名合规（`terminal-documents-writeback`）

## 7. 审计文档退役前的移交（`docs/REVIEW*.md` 全部删除后，此处是唯一记录）

已修复并验证的条目见上；以下条目**不属代码缺陷**，需要用户裁决或对保护文件的
授权才能动，故不在本 change 内执行，也不随审计文档一并丢失：

- D3 / P0-5：五个监控类（`AnrWatchDog`/`BootGuard`/`MemoryMonitor`/`ThermalMonitor`/
  `TerminalForegroundService`）与 `PROHIBITED.md:10` 字面冲突。`AnrWatchDog` 触发即
  `Process.killProcess` 销毁全部 shell。裁决：删除整块，还是补规范声明后保留？
- N1-29：`ktlint` 与 `ktfmt` 插件已声明并 apply，但没有任何门禁请求它们
  （`BUILD.md:23` 要求不得保留未使用依赖）。二选一：从两个 `build.gradle.kts`
  删除（保护文件，需授权），或在 `scripts/check-gradle.nu` 补 `ktlintCheck ktfmtCheck`
  （脚本同样属保护文件，需授权）。
- D8：`docs/specification/BUILD.md:15-17` 要求的 `.so` 三项校验（`NEEDED` / 体积 /
  APK 含 `.so`）在 `scripts/build-android-libs.nu` 中缺失（保护文件，需授权）。
- D4：`BUILD.md:7`「`ANDROID_NDK_HOME` 已预设」与 `flake.nix` 未声明 NDK 不符；
  `BUILD.md:20` 要求 r30 而实测 r27d（保护文件，需授权）。
- D1/D2/D5/D6/D7：规范之间或规范与实现的字面冲突，逐条需用户定口径（见
  `docs/REVIEW-SUMMARY.md` 第四节——本次删除前请以本清单为准）。
- N2-64：`CellData.grapheme_extra` 仅 7 槽，超过 8 码点的组合字形被静默截断；
  扩大槽位会改变 `CellData` 布局与全部 FFI stride，需与渲染侧一并设计。
- `2026-09-28-render-idle-cursor`：唯一未完成任务为环境受阻（新建 AVD 未启用
  中文输入法），按 `TESTING.md:11` 如实悬挂，不得跳过或删除。
