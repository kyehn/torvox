## MODIFIED Requirements

### Requirement: 应用自身的正常路径不得产生警告与错误

应用自有日志 MUST 按「是否代表异常」分级。正常退化路径——子进程已退出、信号目标
退化为直杀子进程、Surface 尚未就绪而延迟处理、平台收走原生窗口、共享内存等介质
在平台不支持时——MUST 记为 debug 或 info，MUST NOT 记为 warn 或 error；只有真正
表示异常的路径（信号投递失败、命令通道失效、载荷解码失败、宿主迟迟换不来新窗口）
才可记为 warn 或 error。

正常路径产生 warn 的代价是真实的：日志分级是排障时区分「应用缺陷」与「环境噪声」
的第一道闸门，把它淹没在正常噪声里，等于取消这道闸门。

#### Scenario: 会话正常收尾

- **WHEN** 子进程已退出，或信号目标退化为直杀子进程
- **THEN** 收尾日志记为 debug，MUST NOT 出现进程组相关 warn

#### Scenario: execve 失败的判定依据

- **WHEN** 子进程以退出码结束，且该退出码落在「100 + errno」编码区间内
- **THEN** MUST NOT 据此判定 execve 失败；判定 MUST 只依据子进程回报的确切 errno。
      正常收尾（组杀 SIGHUP 后 shell 按 128+信号码惯例退出）MUST NOT 产生
      error 级日志

#### Scenario: Surface 尚未就绪

- **WHEN** 布局阶段先于 `surfaceCreated` 到达，尺寸变化被延迟处理
- **THEN** 该延迟日志记为 debug，MUST NOT 记为 warn

#### Scenario: 平台收走原生窗口后的瞬时渲染错误

- **WHEN** 平台收走原生窗口导致渲染帧取不到纹理，宿主换新窗口重挂载后即恢复
- **THEN** 该次渲染失败记为 debug，宿主侧的瞬时渲染错误汇总首次记为 info；
      仅当连续出现（新窗口迟迟不来）才记为 warn

## ADDED Requirements

### Requirement: 未捕获 GPU 错误必须按来源分级

未捕获 GPU 错误 MUST 按来源分级：只有「平台收走原生窗口后 wgpu 重新配置 surface
必然失败」这一类可记为 debug，其余一律 MUST 记为 error。判别 MUST 依据错误种类
与上游固定文案，MUST NOT 依据「看起来像」或整段描述的模糊匹配。

#### Scenario: 窗口被收回后的配置失败

- **WHEN** 未捕获错误是校验错误，且描述同时含上游的 surface 配置调用点标识与
      「不支持该适配器的队列族」
- **THEN** 该错误记为 debug，MUST NOT 记为 error

#### Scenario: 自家配置错误仍是应用缺陷

- **WHEN** 未捕获错误出自 surface 配置，但失败原因是格式不支持、尺寸非法等自家问题
- **THEN** 该错误 MUST 记为 error

#### Scenario: 非校验错误不按平台事件降级

- **WHEN** 未捕获错误不是校验错误
- **THEN** 该错误 MUST 记为 error
