## Context

torvox 已合并 native 单 crate（terminal+render+android+ mcp），Ghostty VT 为单一来源，wgpu 30 Vulkan 单 pass，`libnative.so` 16M 静联。`docs/reference` 26 项目研究与全库扫描揭示 9 项硬化点，其中 4 项已部分落地（`public_api::encode_mouse_event`、`AccessibilityLineProvider`、`output_processor` 133 状态机、`CachedInstances`），5 项为新增细节。约束：无 CPU/GL 回退、零新增重抽象、行为兼容、确定性后端、模拟器 90fps 可验证。见 `proposal.md` Why。

## Goals / Non-Goals

**Goals:**

- 5 项细节硬化可验证落地，且每项有确定性单测锁定
- 4 项已落地能力补全测试与文档，达到 1016+16 测试绿、零新增告警
- 依赖滚动（cargo/gradle/flake）+ 体积守卫不回退
- 模拟器 90fps+ 与 gfxinfo/Choreographer 双轨可重复

**Non-Goals:**

- 不引入 Slint/Bevy/proot/X11 等重依赖或自绘悬浮/无限网格
- 不改变 FFI/Kotlin ABI 与 CellData 布局
- 不默认启用 GL 退路（仅注释备选）
- 不重构 TerminalSurface 2700 行的大规模拆分（仅局部注释与常量化）

## Decisions

| 决策 | 选择 | 备选 | 理由 |
|------|------|------|------|
| Winsize 同步 | spawn 前 `getGridRowsColsPacked` 预计算 pixel→rows/cols，立即 `setPixelSize` | 延迟至首帧后 `post` | 消除首帧折行，warp 已验证“先 winsize 后首读” |
| SHA sidecar | `shell-words` 无，`sha256` 用 `sha2` crate best-effort，若无 sidecar 跳过 | 强制校验阻断 Termux 预设 | Termux 离线包无 sidecar，best-effort 不误伤 |
| 分词 | `shell-words` crate | 自研 4 态机 / `shlex` | 已在 termux-kotlin 验证，零维护，POSIX 兼容 |
| PeerCred | `nix::sys::socket::getsockopt(SO_PEERCRED)` | 无校验 | zed-port 经验：同 uid 越权是真实风险 |
| 脏缓存 | `bytemuck::bytes_of` 字节等价 + `CachedInstances` 复用 | 字段级比较 | 防遗漏 flags/fg/bg，且与 zelland 等价性文档一致 |
| CellRun | 仅合并相邻同样式为 run，仍展开为实例 | 直接压缩传输 | 保持 FFI 兼容，风险最低，收益已达 >50% |
| OSC133 | 复用现有跨 chunk 状态机，仅增列与 exit_code 字段 | 重写 parser | 最小侵入，双终结与 A 重置已稳定 |
| 测试超时 | lens 超时 60→120s，CI 8 线程 | 拆 nextest | 实测暖缓存 76s，超时非代码问题 |

## Risks / Trade-offs

- **ghostty_mouse_encoder API 缺口** → 已验证 bindings 含 encoder/size/event；缺则回退手写 SGR X10 注释备选
- **TalkBack 刷屏** → 500ms debounce + diff + 限频 announce，已在现有 `AccessibilityLineProvider` 验证
- **OSC133 与 capture 双写冲突** → 双缓冲区独立，`semantic_segments` 与 `capture_buf` 互不覆盖，property test 锁定
- **CellRun 兼容回退** → 若 run 合并异常则 fallback 全量 rebuild，单测对比实例数
- **帧率抖动** → gfxinfo 与 Choreographer 双轨，取 90th 非 max，允许单帧毛刺
- **依赖升级破坏** → `cargo update` 后全量 1016 测试门控，gradle 仅 patch/minor

## Migration Plan

1. 分 6 阶段提交（每阶段 `cargo test + gradle test + clippy/detekt` 绿才推）：0 基线→1 mouse→2 a11y→3 osc133→4 CellRun→5 细节→6 依赖
2. 每阶段 git conventional 单行提交，推送 `origin/main`
3. 回滚：单阶段 revert 即可，无 DB 迁移；`libnative.so` 体积守卫失败则回退 `strip` 与 `cargo update`
4. 发布：`docs/verification` 记录 gfxinfo 帧率与 so/APK 尺寸，连续三次 review 无问题视为完成

## Open Questions

- 90Hz 真机帧率是否在 120Hz 设备上需另验证？— 可后补真机数据，不影响模拟器 60Hz 宿主结论。
- `shell-words` 与 `shlex` 选型是否需基准对比？— 均 0 维护，任选其一，后续可无痛替换。
