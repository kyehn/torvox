use super::render::{
    ALT_SCREEN_ACTIVE_BIT, LAST_CONTENT_ROW_NONE_BITS, RENDER_SAMPLE_FAILURE_BITS,
    SURFACE_INVALIDATED_BIT, pack_render_sample,
};

/// 位 33..42：已删除的光标行位形，保留 MUST 恒为 0。
const RESERVED_ROW_BITS: i64 = 0x3FF << 33;

/// 位 33..42 之外全部在用的位（计数 0..31、输出 32、内容下沿 43..52、
/// 失效 53、备用屏 54）。它与保留位相交为空即证明没有任何字段占用 33..42。
const ALLOCATED_SAMPLE_BITS: i64 = 0xFFFF_FFFF
    | (1 << 32)
    | (LAST_CONTENT_ROW_NONE_BITS << 43)
    | SURFACE_INVALIDATED_BIT
    | ALT_SCREEN_ACTIVE_BIT;

#[test]
fn no_allocated_field_overlaps_the_reserved_row_bits() {
    // 位 33..42 是已删除的光标行位形。Kotlin 侧不读它，故一旦有字段落进去，
    // 两侧都不会判红——这里是唯一的闸。
    assert_eq!(
        ALLOCATED_SAMPLE_BITS & RESERVED_ROW_BITS,
        0,
        "已分配字段与保留位 33..42 重叠",
    );
}

#[test]
fn packing_never_sets_the_reserved_row_bits() {
    // 直接对**打包表达式**取值，而不是对常量：逐字段取遍有标志与哨兵的组合。
    for count in [0i64, -1, 7] {
        for new_output in [0i64, 1] {
            for content in [0i64, 5, LAST_CONTENT_ROW_NONE_BITS] {
                for surface in [0i64, SURFACE_INVALIDATED_BIT] {
                    for alt in [0i64, ALT_SCREEN_ACTIVE_BIT] {
                        let packed = pack_render_sample(
                            count & 0xFFFF_FFFF,
                            new_output,
                            content,
                            surface,
                            alt,
                        );
                        assert_eq!(
                            packed & RESERVED_ROW_BITS,
                            0,
                            "保留位 33..42 被污染：count={count} new_output={new_output} content={content} surface={surface} alt={alt}",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn packed_fields_reach_their_own_bits() {
    // 反向：每个字段确实出现在自己的位上（纯 0 打包恒真，故要非 0 输入）。
    let packed = pack_render_sample(7, 1, 5, SURFACE_INVALIDATED_BIT, ALT_SCREEN_ACTIVE_BIT);
    assert_eq!(packed & 0xFFFF_FFFF, 7);
    assert_eq!(packed >> 32 & 0x1, 1);
    assert_eq!(packed >> 43 & 0x3FF, 5);
    assert_eq!(packed >> 53 & 0x1, 1);
    assert_eq!(packed >> 54 & 0x1, 1);
}

#[test]
fn sample_failure_bits_report_failure_count_and_unknown_rows() {
    // 渲染计数位（0..31）全 1 即 Kotlin 侧读到的 -1；内容下沿取未知哨兵。
    assert_eq!(
        RENDER_SAMPLE_FAILURE_BITS & 0xFFFF_FFFF,
        0xFFFF_FFFF,
        "渲染计数必须为负，否则 panic 会被读成空闲帧",
    );
    assert_eq!(
        (RENDER_SAMPLE_FAILURE_BITS >> 43) & 0x3FF,
        LAST_CONTENT_ROW_NONE_BITS,
    );
    assert_eq!(RENDER_SAMPLE_FAILURE_BITS >> 32 & 0x1, 0);
    assert_eq!(RENDER_SAMPLE_FAILURE_BITS >> 53 & 0x1, 0);
    assert_eq!(RENDER_SAMPLE_FAILURE_BITS >> 54 & 0x1, 0);
}

#[test]
fn alt_screen_bit_is_distinct_and_outside_the_other_fields() {
    // 备用屏位必须落在所有既有字段之外，否则会被任一掩码读成别的含义
    // （例如被内容下沿的 10 位掩码读成行号，或让失败位形带上脏位）。
    assert_eq!(ALT_SCREEN_ACTIVE_BIT, 1 << 54);
    assert_eq!(RENDER_SAMPLE_FAILURE_BITS >> 54 & 0x1, 0);
    // 与内容下沿（43..52）、失效位（53）、输出位（32）均不重叠。
    assert_eq!(ALT_SCREEN_ACTIVE_BIT >> 32 & 0x1, 0);
    assert_eq!((ALT_SCREEN_ACTIVE_BIT >> 43) & 0x3FF, 0);
    assert_eq!(ALT_SCREEN_ACTIVE_BIT >> 53 & 0x1, 0);
}
