## 1. 规范

- [x] 新增 `openspec/specs/bootstrap-integrity/spec.md`：归档 MUST 校验后才解压执行；https 与体积区间 MUST NOT 被当作完整性依据；校验和缺失/不匹配 MUST 中止而非放行
- [x] 修正 `BootstrapDownloader` 注释：不再把 https 门控称为「完整性门控」，注明真实完整性门控为校验和

## 2. 实现

- [x] `TermuxBootstrap.sha256(architecture)`：内置 aarch64 / x86_64 已知 SHA-256（与 `RELEASE`/`VARIANT` 同源）
- [x] `BootstrapDownloader` 流式 `MessageDigest`，收尾比对预设 URL 的归档
- [x] 不匹配即删除暂存文件并返回失败，记 error（不静默）
- [x] 自定义 URL 记 warn 日志后放行（明示未校验），不做「无校验和就放行」之外的降级
- [x] 单测：篡改字节 → 失败；自定义 URL → 记 warn 且放行

## 3. 验证

- [x] 哈希与上游 termux-app `downloadBootstraps` 权威值逐位核对（aarch64 `ea2aeba8819e517db711f8c32369e89e7c52cee73e07930ff91185e1ab93f4f3` / x86_64 `b7fd0f2e3a4de534be3144f9f91acc768630fc463eaf134ab2e64c545e834f7a`）
- [x] 与完整下载归档实测 SHA-256 一致（分别 30,542,758 / 30,393,585 字节）
