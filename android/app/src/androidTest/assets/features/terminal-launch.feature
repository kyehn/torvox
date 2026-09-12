# language: zh-CN
@REQ_TERM_001 @REQ_ANDR_004 @REQ_SYS_002
功能: 终端启动

  @REQ_TERM_001
  场景: 启动后渲染终端界面
    假如 应用已启动
    那么 终端界面已显示
    而且 修饰键栏可见
    而且 终端内容区宽高为正

  @REQ_ANDR_004
  场景: SurfaceView 渲染在 Compose 布局上层
    假如 应用已启动
    那么 SurfaceView 可见
    而且 它渲染在 Compose 布局上层
