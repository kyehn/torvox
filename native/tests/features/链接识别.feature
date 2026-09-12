# language: zh-CN
功能: 终端文本链接识别
  作为终端模拟器
  为了让用户点击纯文本链接时能打开浏览器
  需要按显示列号准确识别行内链接

  场景: 识别常见链接
    假如 待测文本行为 "see https://example.com now"
    当 查询第 5 列的链接
    那么 识别结果为 "https://example.com"

  场景: 无协议文本不是链接
    假如 待测文本行为 "visit www.example.com now"
    当 查询第 7 列的链接
    那么 没有识别出链接

  场景: 链接范围外的列返回空
    假如 待测文本行为 "see https://example.com/a?q=1 end"
    当 查询第 0 列的链接
    那么 没有识别出链接

  场景: 识别 mailto 与 tel
    假如 待测文本行为 "mail me at mailto:user@example.com now"
    当 查询第 12 列的链接
    那么 识别结果为 "mailto:user@example.com"

  场景: 去掉链接末尾标点
    假如 待测文本行为 "link https://example.com. done"
    当 查询第 6 列的链接
    那么 识别结果为 "https://example.com"

  场景: 中文字符后链接列号按显示列计算
    假如 待测文本行为 "中中https://example.com"
    当 查询第 4 列的链接
    那么 识别结果为 "https://example.com"
