//! Kitty Unicode 占位符（virtual placement）解码。
//!
//! 协议：占位符 U+10EEEE 以 fg 颜色编码图像 id，underline 颜色编码放置 id，
//! 紧随其后的组合记号依次编码图像行、列与图像 id 高位。行/列是图像内的绝对
//! 索引，因此图片跨多行、或顶部滚出视口时仍能算出整图的锚点。
//!
//! 表与上游 ghostty `src/terminal/kitty/graphics_unicode.zig` 的 `diacritics`
//! 一致，原始来源为 kitty 的 rowcolumn-diacritics.txt；二分查找要求表有序。

/// Unicode 占位符码位。
pub(crate) const PLACEHOLDER: u32 = 0x10EEEE;

/// 网格中一个可见占位符单元解码后的位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaceholderCell {
    pub grid_row: u32,
    pub grid_col: u32,
    pub image_id: u32,
    pub placement_id: u32,
    pub image_row: u32,
    pub image_col: u32,
}

/// 一个虚拟放置的锚点：整幅图左上角在视口中的网格位置（可为负）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VirtualAnchor {
    pub image_id: u32,
    pub placement_id: u32,
    pub grid_row: i32,
    pub grid_col: i32,
}

/// 把同一放置的可见占位符归并成一个锚点。
///
/// 扫描顺序是视口从上到下、从左到右，因此每组保留首个单元即可；同一放置的
/// 任意单元都满足 `锚点 = 单元网格位置 - 单元图像行列`。不同放置（underline
/// 颜色不同）即使引用同一图像也各自成组。
pub(crate) fn anchors(cells: &[PlaceholderCell]) -> Vec<VirtualAnchor> {
    let mut out: Vec<VirtualAnchor> = Vec::new();
    for cell in cells {
        if out
            .iter()
            .any(|a| a.image_id == cell.image_id && a.placement_id == cell.placement_id)
        {
            continue;
        }
        out.push(VirtualAnchor {
            image_id: cell.image_id,
            placement_id: cell.placement_id,
            grid_row: cell.grid_row as i32 - cell.image_row as i32,
            grid_col: cell.grid_col as i32 - cell.image_col as i32,
        });
    }
    out
}

/// 组合记号码位到图像行列索引（0 起）。
pub(crate) fn diacritic_index(codepoint: u32) -> Option<u32> {
    DIACRITICS
        .binary_search(&codepoint)
        .ok()
        .map(|index| index as u32)
}

const DIACRITICS: [u32; 297] = [
    0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F, 0x0346, 0x034A, 0x034B, 0x034C,
    0x0350, 0x0351, 0x0352, 0x0357, 0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
    0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484, 0x0485, 0x0486, 0x0487, 0x0592,
    0x0593, 0x0594, 0x0595, 0x0597, 0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
    0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611, 0x0612, 0x0613, 0x0614, 0x0615,
    0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
    0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2, 0x06E4, 0x06E7, 0x06E8, 0x06EB,
    0x06EC, 0x0730, 0x0732, 0x0733, 0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
    0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE, 0x07EF, 0x07F0, 0x07F1, 0x07F3,
    0x0816, 0x0817, 0x0818, 0x0819, 0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C, 0x082D, 0x0951, 0x0953, 0x0954,
    0x0F82, 0x0F83, 0x0F86, 0x0F87, 0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
    0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D, 0x1B6E, 0x1B6F, 0x1B70, 0x1B71,
    0x1B72, 0x1B73, 0x1CD0, 0x1CD1, 0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
    0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1, 0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5,
    0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9, 0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
    0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1, 0x20D4, 0x20D5, 0x20D6, 0x20D7,
    0x20DB, 0x20DC, 0x20E1, 0x20E7, 0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
    0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA, 0x2DEB, 0x2DEC, 0x2DED, 0x2DEE,
    0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2, 0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
    0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D, 0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1,
    0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5, 0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
    0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7, 0xAAB8, 0xAABE, 0xAABF, 0xAAC1,
    0xFE20, 0xFE21, 0xFE22, 0xFE23, 0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186,
    0x1D187, 0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243, 0x1D244,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_matches_known_ends() {
        assert!(DIACRITICS.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(DIACRITICS.len(), 297);
        assert_eq!(DIACRITICS[0], 0x0305);
        assert_eq!(DIACRITICS[296], 0x1D244);
    }

    #[test]
    fn diacritic_index_is_zero_based() {
        assert_eq!(diacritic_index(0x0305), Some(0));
        assert_eq!(diacritic_index(0x030D), Some(1));
        assert_eq!(diacritic_index(0x1D244), Some(296));
        assert_eq!(diacritic_index(0x10EEEE), None);
    }

    #[test]
    fn anchor_is_unit_grid_position_minus_encoded_image_position() {
        let cells = [PlaceholderCell {
            grid_row: 10,
            grid_col: 20,
            image_id: 7,
            placement_id: 3,
            image_row: 2,
            image_col: 5,
        }];
        assert_eq!(
            anchors(&cells),
            vec![VirtualAnchor {
                image_id: 7,
                placement_id: 3,
                grid_row: 8,
                grid_col: 15,
            }]
        );
    }

    #[test]
    fn same_image_with_different_placements_stays_separate() {
        let cells = [
            PlaceholderCell {
                grid_row: 0,
                grid_col: 0,
                image_id: 7,
                placement_id: 1,
                image_row: 0,
                image_col: 0,
            },
            PlaceholderCell {
                grid_row: 0,
                grid_col: 1,
                image_id: 7,
                placement_id: 1,
                image_row: 0,
                image_col: 1,
            },
            PlaceholderCell {
                grid_row: 2,
                grid_col: 0,
                image_id: 7,
                placement_id: 2,
                image_row: 0,
                image_col: 0,
            },
        ];
        assert_eq!(anchors(&cells).len(), 2);
    }
}
