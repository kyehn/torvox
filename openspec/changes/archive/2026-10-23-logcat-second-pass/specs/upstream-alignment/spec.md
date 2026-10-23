## MODIFIED Requirements

### Requirement: VT 能力缺口按绑定层与终端分别记录

判断某个 VT 特性是否可用时，MUST 区分两层并分别记录：终端上游（如 ghostty 的
`main`）与绑定层（如 `libghostty-vt` 所钉的终端提交）。只记「上游没有」会把
「绑定层没跟进」误判为不可逾越。

#### Scenario: 特性在上游终端已实现但绑定层未跟进

- **WHEN** 某 VT 特性在终端上游 master 存在、在绑定层所钉提交中不存在
- **THEN** 记录 MUST 写明是绑定层滞后，并给出可复核的两处上游事实——提交日期与
      提交数差；MUST NOT 写 hash（见「rev 本身不进正文」，rev 的唯一载体是
      `Cargo.toml` 的依赖钉）

#### Scenario: 终端上游与绑定层同时缺失

- **WHEN** 某 VT 特性在终端上游 master 与绑定层所钉提交中都不存在
- **THEN** 记录 MUST 写明是上游缺口，并给出两处可复核事实（上游 master 的源码
      事实与绑定层所钉提交的源码事实），MUST NOT 记为绑定层滞后
- **AND** 应用 MUST NOT 以绕过上游的方式补齐该能力（如自研 VT 解析、自建共享
      内存通道）；协议定义了应答时 MUST 按协议回明确失败，未定义应答时 MUST NOT
      伪造应答
