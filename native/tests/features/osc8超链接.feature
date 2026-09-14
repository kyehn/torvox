# language: zh-CN
功能: OSC8 超链接
  作为终端模拟器
  为了让终端内的超链接可点击
  需要上游按单元格记录超链接并提供查询

  场景: 打开超链接
    假如 创建 24 行 80 列终端
    当 程序输出写入转义字节 "\x1b]8;id=link1;https://example.com\x07click"
    那么 第 0 行第 0 列超链接为 "https://example.com"

  场景: 关闭超链接
    假如 创建 24 行 80 列终端
    当 程序输出写入转义字节 "\x1b]8;id=link1;https://example.com\x07click"
    而且 程序输出写入转义字节 "\x1b]8;;\x07plain"
    那么 第 0 行第 0 列超链接为 "https://example.com"
    而且 第 0 行第 5 列无超链接
