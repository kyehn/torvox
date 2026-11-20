## ADDED Requirements

### Requirement: 取到的交换链纹理必须呈现归还

渲染 MUST NOT 丢弃已取到但本帧不用的交换链纹理。wgpu-hal 的 Vulkan 后端对「未呈现即被
丢弃」的纹理，`discard_texture` 是空实现：那张交换链图像既不呈现、也不归还，累积几次后
交换链再也拿不到可用图像，终端永久停帧且无自愈。此类纹理 MUST 先清成背景色再呈现
（未清内容可能是未定义像素），呈现 MUST 发生在交换链销毁之前。

三条路径 MUST 遵守：

- 纹理尺寸与当前配置不符时：MUST 在 `surface.configure` **之前**归还，configure 会销毁
  旧交换链，之后呈现即是悬垂图像；
- 渲染管线缺失、本帧无法绘制时：MUST 归还后再返回错误，MUST NOT 让纹理随帧上下文丢弃；
- 取纹理工作线程在渲染线程已超时放弃后取得纹理时：MUST 由工作线程亲自归还；在途记账
  MUST 覆盖「纹理尚未归还」的窗口，使释放 surface 前的排干能保证交换链销毁前纹理已呈现。

归还 MUST NOT 被实现为「先记账、下一帧再处理」：纹理在下一帧之前一直占用交换链图像。

#### Scenario: 纹理尺寸与配置不符

- **WHEN** 取得的交换链纹理尺寸与当前 surface 配置不一致
- **THEN** 该纹理 MUST 清成背景色并呈现，随后 MUST 按纹理实际尺寸 reconfigure；
      画面 MAY 显示一帧背景色，MUST NOT 永久停帧，MUST NOT 丢弃该纹理

#### Scenario: 管线缺失

- **WHEN** 本帧已取得纹理，而渲染管线不可用
- **THEN** 纹理 MUST 先归还再返回错误，错误仍按既有口径上报，MUST NOT 静默丢弃纹理

#### Scenario: 取纹理超时后的迟到结果

- **WHEN** 渲染线程按 100ms 期限放弃某帧，工作线程此后才取到纹理
- **THEN** 工作线程 MUST 把该纹理清成背景色并呈现，MUST NOT 随发送失败一并丢弃；
      此后的取纹理 MUST 能重新拿到可用图像

#### Scenario: 释放 surface 前的排干覆盖迟到结果

- **WHEN** 释放或重建 surface 前执行在途取帧排干
- **THEN** 排干返回时 MUST 保证该 surface 的纹理已被呈现或已被渲染线程接收，
      MUST NOT 遗留未归还的纹理导致交换链销毁后再呈现

#### Scenario: 归还的纹理被清成背景色

- **WHEN** 归还一张已取得但未绘制的纹理
- **THEN** 该纹理的像素 MUST 等于当前背景色（回归测试
      `returned_texture_is_cleared_to_the_background` 以 GPU 回读断言）
