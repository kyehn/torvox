## MODIFIED Requirements

### Requirement: 放置生成戳独立于图像存储生成戳

渲染缓存 MUST 以 VT 的**放置生成戳**（`image_placements_generation`）判定放置是否变化，
MUST NOT 仅凭图像存储生成戳（`Graphics::generation`）：撤下放置（ED2）与 TUI 原地重排
都只推进放置生成戳，图像存储生成戳在这些场景**不变**（实测不变式），
仅凭它会漏掉撤除，导致图像残留到第二次清屏。
VT MUST 在放置变化时推进 placements generation，并把该值随每帧单元数据送达渲染线程；
渲染线程 MUST 在放置生成戳变化时重新查询放置并整帧重绘以抹掉残留像素。

实现：单元数据同时携带 `kitty_generation`（存储）与 `kitty_placement_generation`（放置）；
`kitty_cache_invalidation` 以放置生成戳的变化作为「放置被增删或移动」的唯一信号。

#### Scenario: ED2 撤下虚拟放置时 placements generation 变化

- **WHEN** 虚拟放置显示一幅图像后收到 ED2
- **THEN** placements generation 变化，图像存储生成戳不变；渲染线程据此重新查询
    并在首次清屏即抹掉图像像素

#### Scenario: 撤下放置不推进存储生成戳

- **WHEN** 直接放置通道或虚拟放置通道送入图像后收到一次 ED2
- **THEN** 图像存储生成戳 MUST 保持不变，放置生成戳 MUST 变化；
    只盯存储生成戳的缓存会保留已被撤下的放置
