# render-stability Specification

## RENAMED Requirements

- FROM: `### Requirement: 开孔无缓冲区时由窗口层盖底色`
- TO: `### Requirement: 开孔无缓冲区时由 SurfaceView 自己盖底色`

## MODIFIED Requirements

### Requirement: 切后台返回时开孔必须被覆盖且重新出帧

`SurfaceView` 的 surface 在 `onStop` 之后被系统拆除时，宿主 MUST 立即回到「无缓冲区」
状态并遮住开孔，直到新 surface 提交首帧。该状态 MUST NOT 依赖随生命周期停止收集的
订阅方式：写入发生在 `onStop` 之后，`ON_START` 重启收集与返回后的首帧绘制会抢跑，
首帧按「有缓冲区」绘制时开孔区就透出桌面或上一个应用。

遮罩 MUST 由 `SurfaceView` **自己**在 `onDraw` 里画，MUST NOT 画在组合层（窗口层）：
平台在 `SurfaceView` 绘制时先给应用窗口打洞，洞里窗口层画过的像素一律被抹掉。

遮罩 MUST 用**离开时的画面**（而非仅一块平底色）：平底色与桌面都是「闪一下」，只是
闪成另一种颜色。

返回前台后 MUST 重新出帧，MUST NOT 停留在渲染暂停状态形成永久黑屏。

#### Scenario: 切后台拆除 surface

- **WHEN** 应用切到后台且系统拆除了 `SurfaceView` 的 surface
- **THEN** 开孔覆盖状态 MUST 落回「无缓冲区」，由 `SurfaceView` 在 `onDraw` 里遮住

#### Scenario: 返回前台首帧前不透出上一个应用

- **WHEN** 应用回到前台，新 surface 尚未提交首帧
- **THEN** `SurfaceView` MUST 在 `onDraw` 里保持遮罩，MUST NOT 透出桌面或上一个应用

#### Scenario: 返回前台后重新出帧

- **WHEN** 应用回到前台且新 surface 已挂载
- **THEN** 终端 MUST 在 30s 内重新提交并呈现帧，覆盖状态随首帧撤下

#### Scenario: 遮罩画在 SurfaceView 自己的绘制里

- **WHEN** 宿主把开孔覆盖画在组合层（窗口层）
- **THEN** 覆盖 MUST NOT 出现在开孔里——平台给窗口打洞时会把该处像素抹掉（实测：
      组合层涂的底色只在 `SurfaceView` 之外的边距上可见）
- **AND** 覆盖 MUST 画在 `SurfaceView` 的 `onDraw` 里（打洞在其之前，`onDraw` 在其之后）

#### Scenario: 窗口 surface 被拆导致交换链重建

- **WHEN** 平台在切后台后拆掉本应用**窗口**的 surface
- **THEN** 返回前台时交换链 MUST 重建，开孔 MUST 由遮罩覆盖重建期间的空档

#### Scenario: 切后台时把离开时的画面留作开孔遮罩

- **WHEN** 应用切到后台，渲染线程停下之前
- **THEN** 终端 MUST 把保留的上一帧回读成 CPU 像素并发布为开孔遮罩
- **AND** 回读 MUST 在渲染线程上完成（它是一次 GPU 同步等待），MUST NOT 放在主线程
- **AND** 遮罩的尺寸 MUST 与 surface 尺寸一致，不一致时 MUST 放弃遮罩（MUST NOT 画错位的内容）

#### Scenario: 遮罩在画面恢复后撤下

- **WHEN** 新 surface 提交了首帧
- **THEN** 遮罩 MUST 被撤下并释放像素

#### Scenario: 没有可回读的内容时不遮

- **WHEN** 回读失败、没有保留的帧或帧格式不是 RGBA8
- **THEN** 宿主 MUST 沿用平底色遮罩，MUST NOT 以空遮罩代替（空遮罩等于一块黑）

### Requirement: 开孔无缓冲区时由 SurfaceView 自己盖底色

`SurfaceView` 的原生 surface 位于应用窗口**之下**，窗口在该区域是透明开孔：缓冲区
一旦缺失（冷启动首帧未出、切后台回来重建窗口、原生判死后换视图），开孔就透出后方
的桌面或上一个应用，表现为「切换应用返回一闪」。

窗口层 MUST NOT 用来遮这个洞：平台在 `SurfaceView` 绘制时给窗口打洞，洞里窗口层
画的像素一律被抹掉（实测：组合层涂的底色只在 `SurfaceView` 之外的边距上可见）。

宿主 MUST 在「当前 Surface 尚无缓冲区」这段窗口内，由 `SurfaceView` 在 `onDraw`
里于开孔区域绘制与当前终端主题背景一致的颜色，并在该 Surface 首次成功提交缓冲后
立即撤销。撤销 MUST 以「已提交」为条件，MUST NOT 以时间或重试次数为条件：提前撤销
即退回原缺陷，延后撤销则遮挡画面。

宿主 MUST 区分「换了 Surface 实例」与「同一 Surface 改尺寸」：只有前者使开孔失去
缓冲区（后者交换链里仍有上一帧），故旋转、输入法重排 MUST NOT 触发这次覆盖。

#### Scenario: 冷启动首帧前不黑洞

- **WHEN** 应用冷启动，原生尚未提交第一帧
- **THEN** 开孔区域显示终端主题背景色，MUST NOT 显示黑色或桌面

#### Scenario: 切后台回来不闪桌面

- **WHEN** 应用从后台返回，系统交付了新的 `Surface` 实例且尚无新帧
- **THEN** 开孔区域显示终端主题背景色，桌面 MUST NOT 可见

#### Scenario: 首帧提交即撤销

- **WHEN** 该 Surface 第一次成功提交缓冲（渲染计数 > 0）
- **THEN** 覆盖在同一组合提交内撤销，终端内容完整可见

#### Scenario: 同 Surface 改尺寸不覆盖

- **WHEN** 旋转或输入法重排导致同一 `Surface` 实例以新尺寸重新挂载
- **THEN** MUST NOT 置「无缓冲区」，画面不闪底色

## ADDED Requirements

### Requirement: 开孔覆盖只在原生无缓冲时画

`onDraw` 的内容盖在原生 surface 之上，故 MUST 只在「原生尚无缓冲」这段窗口内画，
否则遮罩会把终端内容一起盖住（实测去掉这道闸后开孔里只剩一块平底色）。

判据 MUST 取自运行期的事实（谁呈现过帧），MUST NOT 由视图自行猜测。组合层 MAY 把
该事实推给视图以便及时重绘，但视图绘制时 MUST 以运行期的当前事实为准——遮罩的撤下
晚一帧就是用户看到的那一下闪。

#### Scenario: 有缓冲时不画覆盖

- **WHEN** 原生 surface 已提交过缓冲
- **THEN** `onDraw` MUST NOT 画覆盖，终端内容完整可见

#### Scenario: 判据取自运行期的事实

- **WHEN** 视图判断是否画覆盖
- **THEN** 判据 MUST 取自运行期记录的「已呈现帧」事实，MUST NOT 由视图猜测
