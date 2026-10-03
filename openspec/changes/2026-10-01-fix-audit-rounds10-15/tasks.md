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
