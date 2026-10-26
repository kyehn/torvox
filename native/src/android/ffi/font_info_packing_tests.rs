use super::pack_font_info;
use crate::render::font::pipeline::FontPipeline;

/// 字体信息打包：`u32` 标志 + 三个 `f32` + 两个 `u16` 计数 + NUL 分隔名称。
/// 判别序号与浮点按小端摆放，Kotlin 侧按同一布局解包。
#[test]
fn font_info_packs_flags_metrics_and_names() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    pipeline.set_system_locale("en-US");
    let packed = pack_font_info(&pipeline.font_info());
    let flags = u32::from_le_bytes(packed[0..4].try_into().unwrap());
    assert_eq!(
        flags >> 1,
        crate::render::font::pipeline::CjkFallbackState::SkippedNonCjkLocale as u32,
        "非 CJK 系统的回退层按设计跳过，不得报成缺失"
    );
    assert_eq!(
        f32::from_le_bytes(packed[12..16].try_into().unwrap()),
        14.0,
        "字号按 f32 原样传递"
    );
    assert!(f32::from_le_bytes(packed[8..12].try_into().unwrap()) > 0.0);
    let name_count = u16::from_le_bytes(packed[16..18].try_into().unwrap());
    let cjk_count = u16::from_le_bytes(packed[18..20].try_into().unwrap());
    assert_eq!(cjk_count, 0, "无 CJK 回退族");
    assert_eq!(name_count, 1, "只有主字体名，无 CJK 回退族与其他被使用族");
    let names: Vec<&[u8]> = packed[20..]
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .collect();
    assert_eq!(names.len(), 1);
    assert!(!names[0].is_empty(), "主字体族名须非空");
}
