# hugomyb/Lume 研究报告

## 仓库概况
- 源克隆：`/tmp/opencode/lume`
- 技术栈：Tauri 2 + SolidJS + xterm.js + Rust (portable_pty, git, cloudflared tunnel, SSH)
- 定位：Linux/macOS/Windows 桌面终端 + 可从手机/浏览器远程控制；command blocks, inline AI, workspaces, web dev tunnel.
- 许可证 MIT.
- Rust backend includes pty.rs, osc.rs, native_grid.rs, shell.rs, git, config, workspaces, ai, ssh.

## Code reading highlights
- `pty.rs`: sessions map, writer lock separate, output broadcast, ring buffer replay + reconnect/resume, process group kill, remote-control session, total byte offset.
- `osc.rs`: command blocks via shell integration OSC133 and OSC events.
- `native_grid.rs`: a security+native grid model that consumes xterm dump and applies pending byte chunks under version offset.
- `remote.rs` protocol, allowlist devices, QR pair, tunnel, E2E bridge.
- `shell.rs`: shell integration installer.
- `complete.rs` completion.
- `workspaces.rs`: persisted tabs/panes/sizes.
- `config.rs`: thorough settings.
- Frontend SolidJS plus xterm.js; rendering delegated to web side, not Rust WebGPU/Vulkan.

## Directly usable
- Lume itself runs no Android or wgpu; not direct backend. xterm.js web cannot be embedded into Android native app without WebView; not recommended.
- Rust dependency patterns:
  - `portable_pty` can be used on Linux-based environments; Android kernel viable but security sandbox.
  - broadcast fanout plus replay ring is directly useful for Android terminal streaming to remote controller or frame playback.
- OSC parser/source concepts, shell integration installer useful.
- Native grid synchronization logic provides algorithm for resync after emulator restart.
- Config and workspace schemas useful as template.

## Indirect use
- command block UX design / semantic blocks using OSC 133; can integrate into torvox later.
- Remote-control pairing, tunnel/LAN, encryption model as inspiration (future feature).
- AI assistant integration contexts (optional).
- Thorough process termination method (`terminus? process group kill`) is good practice on Android background process if native PTY ever used.
- Flat render as xterm serialized buffer + xterm dump can inform testing snapshot paradigms.

## Slightly modifiable
- Rust session code is desktop-oriented; no reuse attempt is made in Torvox.
- None of Lume's web source directly fits.
- CLI AI provider integrations / command palette can be future work.

## Tests / test data
- Unit tests are embedded in Rust crates with `#[cfg(test)]`; full coverage breadth was not transcribed in this report.
- TypeScript front-end tests and config UI coverage were not searched in this report.
- Ring and replay calculation is desktop Rust logic; Android integration would require a JNI boundary, therefore it is not rendered as an integration test in this report.

## Design patterns
- Separate session writer lock from global session registry, explicitly avoids blocking UI.
- Output broadcast with ring + offsets/resync for client recovery.
- `Backend` enum supports local and remote sessions.
- OSC parsed from stream.
- Front-end has strong native Rust services.

## Compare torvox
- Lume violates project stack (Web/Tauri UI; Xterm.js renderer); not designed for Android a priori.
- Java/Kotlin and Rust in torvox already; Lume examples just show desktop Rust robustness.
- Event JSON IPC style has latency and memory overhead vs native JNI/wgpu.
- Project's focus should instead adopt selected Lume UX/concepts, not code.

## Conclusion
- Useful engineering patterns, hostile architecture for Android. Document as UX/reference.

## 四次审阅记录
- 准确性: pty/osc/native_grid source.
- 完整性: much backend covered.
- 对比正确性: desktop vs Android.
- 文档 consistency: slug.
