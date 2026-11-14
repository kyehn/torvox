# cjk-rendering Specification

## REMOVED Requirements

### Requirement: 字体管线重建统一失效实例缓存并请求新帧

该要求以「重建字体管线」为唯一载体，重建入口已随 JNI 模块拆分删除；现行字体面、字号、
光栅缩放与 CJK 回退顺序均就地作用于活动字体管线。原要求要防的「清缓存 + 置脏各自复制」
由下一条要求承接。

## ADDED Requirements

### Requirement: 字面与度量变更统一失效实例缓存并请求新帧

任何改变字形来源或度量的入口（主字面切换、字号、光栅缩放、CJK 回退顺序）以及改变
主题色的入口 MUST 在改动生效后失效字形实例缓存并请求新帧：实例缓存的 UV 指向旧图集、
行高或旧颜色，混用会导致字形错乱且不会主动重绘。

这些入口 MUST 共用 `RenderState::invalidate_glyph_instances`
（`native/src/android/ffi.rs`）这一实现，MUST NOT 各自复制「清缓存 + 置脏」两步——
复制会使某入口遗漏其中一步，且遗漏处不会产生任何日志。改动是否真的发生由各入口自己的
判据决定（字体面未变、字号未变则不失效），判据 MUST 留在调用点。

#### Scenario: 更换主字体后字形立即正确

- **WHEN** 用户选定字体族或加载字体文件使主字面切换
- **THEN** 实例缓存失效且请求新帧，终端以新字体渲染
