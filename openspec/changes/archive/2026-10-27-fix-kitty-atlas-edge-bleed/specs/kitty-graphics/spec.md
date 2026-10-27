# kitty-graphics Specification

## ADDED Requirements

### Requirement: 图像边缘采样不得跨条目串色

同一张 Kitty 图集里并排放置多幅图像时，渲染 MUST 保证每幅图像的采样不越出该图像
自身的像素范围：越界取样 MUST NOT 采到相邻图像的像素（串色），MUST NOT 采到图集里
未被任何图像覆盖的像素（半透明虚边）。显示尺寸与源矩形不等时（缩放、`source_*`
子矩形裁剪）同样适用：边缘 MUST 取该图像自身的边缘像素。

#### Scenario: 相邻图像不互相串色

- **WHEN** 图集里并排放着红色与绿色两幅图像，绿色被放大显示
- **THEN** 绿色图像的像素全部为纯绿，MUST NOT 出现红色的竖条

#### Scenario: 矮条目底缘不发虚

- **WHEN** 图集里某幅图像比图集矮，且被放大显示
- **THEN** 该图像底缘仍为不透明像素，MUST NOT 出现半透明的虚边
