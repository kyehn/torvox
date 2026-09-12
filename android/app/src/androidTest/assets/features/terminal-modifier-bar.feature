# language: zh-CN
@REQ_ANDR_011
功能: 修饰键栏

  @REQ_ANDR_011
  场景: 显示全部修饰键
    假如 应用已启动
    那么 修饰键栏显示 ESC TAB CTRL ALT HOME END PGUP PGDN 按键

  @REQ_ANDR_011
  场景: 修饰键可以切换状态
    假如 应用已启动
    当 轻触 CTRL 键
    那么 CTRL 键呈选中态
    当 双击 CTRL 键
    那么 CTRL 键恢复默认态
