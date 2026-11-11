# logcat-hygiene Specification

## MODIFIED Requirements

### Requirement: 平台与驱动侧条目必须给出可复核的上游事实

日志审计 MUST 把平台与驱动侧条目归到具体的一层，并给出可复核的事实：库源码中的
判据位置，或 Vulkan 扩展名这类可查的能力判据。归因 MUST 说明该条目由哪一层决定，
以及应用侧有无开关；MUST NOT 以「当前路径不使用该能力」或笼统的「平台/驱动」代替
根因。

需要原始日志才能重算的总数 MUST NOT 在原始文件未入库时凭记忆补写；留档 MUST 只写
可复现的计数与结构性倍数关系。

由表面重建触发的条目（驱动格式探测、平台帧时间线同步组查询）MUST 记明条数与重建
次数的倍数关系，并说明应用侧减少重建的手段（平台在后台拆掉窗口 surface，重建不可避免，
只能靠开孔遮罩消除闪烁）——这类条目
的条数不是独立的缺陷，而是重建次数的函数。

厂商框架与厂商系统镜像带来的条目（厂商改写的默认字体、厂商特性开关探测、厂商
dex 元数据缺失、厂商私有属性读取被拒）MUST 指出条目由厂商层决定，并把「应用无对应
调用」写明；内核 AVC 审计行 MUST 指出应用是检查的主体而非成因。

#### Scenario: 渲染能力提示缺失

- **WHEN** 日志出现渲染后端的能力缺失提示（如表面视图格式能力位缺失）
- **THEN** 归因 MUST 给出决定该能力位的驱动扩展判据，MUST 说明应用侧无可用开关

#### Scenario: 设备分配器的格式探测告警

- **WHEN** 日志出现设备图形分配器对格式的探测告警（真机为 `mali_gralloc` 报告 R8
      格式不被支持）
- **THEN** 归因 MUST 说明探测由表面重建触发、由驱动在创建交换链时枚举格式引起，
      MUST 指出被拒的是驱动自己的格式试分配而非应用的呈现缓冲（试分配失败后交换链
      仍以 8 位 RGBA 族建立、帧照常呈现），MUST 给出应用实际请求的交换链格式与上游
      判据位置，MUST 说明应用侧无开关，MUST NOT 记为应用选错格式；MUST 同时记明
      该组条数与表面重建次数的倍数关系，以及应用侧减少重建的手段

#### Scenario: 系统窗口事务的空 Binder 返回

- **WHEN** 日志出现系统窗口加入显示列表或重排布局时的 Binder 空返回告警
- **THEN** 归因 MUST 指出该事务由平台发起、应用代码无对应调用，MUST NOT 记为
      应用侧缺陷

#### Scenario: 平台帧时间线的同步组查询告警

- **WHEN** 日志出现平台帧时间线机制在同步组查询失败时打出的告警
- **THEN** 归因 MUST 说明该告警由平台的帧时间线机制产生、应用无对应调用，MUST 写明
      它按表面重建成组出现（序号随已建立的同步组递增），MUST NOT 记为应用侧缺陷

#### Scenario: 厂商改写的默认字体条目

- **WHEN** 日志出现 `Typeface` 更新默认字体的 error（真机为 `updateDefaultTypeface`）
- **THEN** 归因 MUST 指出 AOSP 的 `android.graphics.Typeface` 没有该日志与该更新路径
      （判据：上游源码内无此字样），条目由厂商改写的框架在 ROM 用自己的默认字体覆盖
      系统默认字体时打出；MUST 说明应用无对应调用、终端字体由原生渲染器另行装载，
      应用侧无开关

#### Scenario: 调试器未启动条目

- **WHEN** 进程启动时出现「无法加载 JDWP agent 故不启动调试器」的 error
- **THEN** 归因 MUST 说明进程可调试（安装的是 debug 构建）与 ROM 未提供 JDWP agent
      两个事实的组合，MUST 写明该信息经进程 stderr 转入 logcat（tag 为进程名）、
      应用代码无对应调用，MUST 写明改用 release 构建即不再出现

#### Scenario: 厂商特性开关探测条目

- **WHEN** 日志出现厂商框架的特性开关探测告警（真机为 `FeatureOpt` 的
      `ensureGetRight`）
- **THEN** 归因 MUST 指出被探测的键名属于厂商框架的可折叠屏特性开关、应用无对应
      调用，MUST 说明应用侧无开关

#### Scenario: 系统 dex 元数据缺失条目

- **WHEN** 日志出现系统镜像内 dex 装载器读不到厂商框架 dex 元数据的告警（真机为
      `ziparchive` 的 `Unable to open`）
- **THEN** 归因 MUST 说明缺失的文件属于系统镜像（厂商框架 jar 未配套 `.dm`）、
      dex 装载器读不到即告警并继续，应用无对应调用，应用侧无开关

#### Scenario: 属性读取被拒条目

- **WHEN** 日志出现 `libc` 打出的属性读取被拒告警（`Access denied finding property`）
- **THEN** 归因 MUST 给出 bionic `SystemProperties::Find` 的判据位置（属性区对应用
      的 SELinux 域不可见时打这条 W），MUST 说明被读的是厂商库在应用进程里读的私有
      属性，MUST 说明应用侧无开关

#### Scenario: 内核 AVC 审计行

- **WHEN** 日志出现 `type=1400 audit(...)` 形式的内核审计行
- **THEN** 归因 MUST 指出该行由内核 `audit` 子系统产生、经 logcat 送达，应用是检查
      的**主体**而非成因，MUST 说明其中 `granted` 记录是放行决定，MUST NOT 记为
      应用侧缺陷

#### Scenario: 平台读到的空 Binder 条目

- **WHEN** 日志出现 `Parcel` 打出的「读到 null binder」告警
- **THEN** 归因 MUST 指出 framework/native 的 `Parcel::readStrongBinder` 在读到 null
      binder 时打这条 W 并返回 `UNEXPECTED_NULL`，调用方在平台一侧，应用无对应
      调用，MUST NOT 记为应用侧缺陷
