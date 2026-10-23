## ADDED Requirements

### Requirement: 共享内存与文件介质已启用

终端 MUST 在平台支持时启用 Kitty 图像协议的 file 与 shared memory 介质；不支持时
MUST 返回明确错误应答，MUST NOT 无应答。

#### Scenario: 客户端使用文件或共享内存介质

- **WHEN** 客户端用 `t=f` 或 `t=s` 传输图像
- **THEN** 终端在平台支持时按该介质读取并显示图像；不支持时返回
  `ESC_Gi=<id>;ERROR:…`，MUST NOT 无应答
