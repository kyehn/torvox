use super::font::{RASTER_SCALE_MAX, RASTER_SCALE_MIN, font_size_cap_sp, is_font_size_selectable};
use super::render::ATLAS_SIZE;

/// Kotlin `SettingsRepository.fontSizeMaxSp` 的**权威取值**，即 Termux 自身的
/// 公开常量：像素上限 256、调整步长 2sp、下限 4sp。这里刻意写死外部数值而非
/// 复刻 Kotlin 公式——后者会随 Kotlin 侧一起漂移，就等于没测。
const TERMUX_MAX_PX: f32 = 256.0;
const TERMUX_STEP_SP: f32 = 2.0;
const SLIDER_MIN_SP: f32 = 4.0;

/// Termux 的可选上界（sp）：256px 换算后按步长向下取整。
fn termux_selectable_max_sp(raster_scale: f32) -> f32 {
    let raw = TERMUX_MAX_PX / raster_scale;
    let stepped = (raw / TERMUX_STEP_SP).floor() * TERMUX_STEP_SP;
    stepped.max(SLIDER_MIN_SP + TERMUX_STEP_SP)
}

use super::font::is_raster_scale_acceptable;

/// `coerceSpToPxScale` 允许的全系数区间即 `ffi.rs` 的唯一真源
/// `RASTER_SCALE_MIN/MAX`：Kotlin 侧的钳位区间经 `getRasterScaleRange` 取同一
/// 常量，两端不再各存一份字面量，测试再抄一份只会掩盖漂移。
const RASTER_SCALE_SAMPLES: usize = 64;

/// 在合法区间内取第 `i` 个采样系数（含两端）。
fn sampled_scale(i: usize) -> f32 {
    RASTER_SCALE_MIN
        + (RASTER_SCALE_MAX - RASTER_SCALE_MIN) * i as f32 / RASTER_SCALE_SAMPLES as f32
}

/// 守卫（`setRasterScale`）实际调用的那个判定：区间外的相邻可表示值必须被拒收。
///
/// 断言取具体边界而非「判定 == 区间包含」这个定义式——后者只复刻实现。
#[test]
fn the_guard_accepts_the_whole_interval_and_rejects_just_outside() {
    assert!(is_raster_scale_acceptable(RASTER_SCALE_MIN));
    assert!(is_raster_scale_acceptable(RASTER_SCALE_MAX));
    let just_below = f32::from_bits(RASTER_SCALE_MIN.to_bits() - 1);
    let just_above = f32::from_bits(RASTER_SCALE_MAX.to_bits() + 1);
    assert!(!is_raster_scale_acceptable(just_below));
    assert!(!is_raster_scale_acceptable(just_above));
    // 非有限值不得落进区间：NaN 的 `contains` 为 false，但 Kotlin 侧
    // `coerceIn` 会把 NaN 原样送出，故此处只保证原生拒收。
    assert!(!is_raster_scale_acceptable(f32::NAN));
}

#[test]
fn cap_never_rejects_a_selectable_font_size() {
    // 核心不变量：滑块能划到的每个字号，原生都必须接受。
    // 图集一旦缩小到接近 256px，这条即失效——那正是「范围与实际可设置范围不一致」。
    //
    // Kotlin 侧的上界实际是两条约束取紧后再兜底：
    //   min(Termux 像素上限, 「至少放得下 MIN_USABLE_COLUMNS 列」)
    //   .coerceAtLeast(自适应默认值)
    // 于是真正需要覆盖的不是 Termux 上界本身，而是它的**上包界**。
    // Termux 上界恒不小于那條兜底值（故此处仍是全覆盖）由 JVM 侧断言
    // `NativeBridgeSmokeTest` 的用例钉住——那边能直接读 Kotlin 的
    // `ADAPTIVE_DEFAULT_MAX_SP` 与经 `getRasterScaleRange` 导出的原生端点，
    // 不需要在本文件再抄一份 Kotlin 私有策略值。抄一份的后果：Kotlin 改 24→28
    // 时本文件仍全绿，覆盖面证明静默失效。
    for sample_index in 0..=RASTER_SCALE_SAMPLES {
        let scale = sampled_scale(sample_index);
        let cap = font_size_cap_sp(ATLAS_SIZE as f32, scale);
        let selectable_max = termux_selectable_max_sp(scale);
        assert!(
            is_font_size_selectable(selectable_max, cap),
            "raster_scale={scale}：滑块上界 {selectable_max}sp 超过原生上界 {cap}sp"
        );
        assert!(
            is_font_size_selectable(SLIDER_MIN_SP, cap),
            "raster_scale={scale}：原生拒绝滑块下界 {SLIDER_MIN_SP}sp"
        );
    }
}

/// 余量断言：图集边长在**最大**合法系数下仍须显著高于 Termux 的像素上限。
///
/// 与 `cap_never_rejects_a_selectable_font_size` 互补：那条只要求「上界不低于可选
/// 上界」，把 `ATLAS_SIZE` 砍到 300 它仍全绿（300/8 = 37.5 ≥ 32），但那时字形位图
/// 只剩 Termux 上限的四倍余量，容错已经很小——这条守住余量本身。
#[test]
fn atlas_edge_keeps_headroom_over_the_termux_ceiling() {
    let cap = font_size_cap_sp(ATLAS_SIZE as f32, RASTER_SCALE_MAX);
    assert!(
        cap * RASTER_SCALE_MAX > TERMUX_MAX_PX * 2.0,
        "图集 {ATLAS_SIZE}px 在最大系数 {RASTER_SCALE_MAX} 下换算出的字号上限 {cap}sp 未留出两倍余量"
    );
}

#[test]
fn cap_is_the_atlas_edge_expressed_in_sp() {
    // 对照**字面量**而非「cap × scale == ATLAS_SIZE」这个定义式：后者只是把除法
    // 还原，边长取错或系数取错都照样通过。2048 是图集边长的具体取值，
    // 2048 / 0.5 = 4096 是它在最小系数下的具体上界。
    assert_eq!(font_size_cap_sp(2048.0, 0.5), 4096.0);
    assert_eq!(font_size_cap_sp(2048.0, 8.0), 256.0);
    // 而生产路径用的边长必须就是 2048：cap 是 `atlas_width / raster_scale`，
    // 图集边长一旦不是 2048，上面的字面量与真实行为就分家了。
    assert_eq!(
        ATLAS_SIZE, 2048,
        "图集边长改变时须同步更新本模块的字面量断言"
    );
}

#[test]
fn non_finite_and_non_positive_sizes_are_rejected() {
    let cap = font_size_cap_sp(ATLAS_SIZE as f32, 1.0);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(
            !is_font_size_selectable(bad, cap),
            "非法字号 {bad} 竟被接受"
        );
    }
}
