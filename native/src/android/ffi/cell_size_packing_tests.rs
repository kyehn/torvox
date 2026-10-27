use super::state::cell_metrics_packed_from;

/// 宽高按两个 `f32` 位落进同一个 `jlong`：低位宽、高位高。
///
/// 判别力在**零扩展**：`u64::from(width.to_bits())` 这一步若写成 `as u64`，
/// 宽度位模式的最高位会被填成符号位，宽度在小数值上取回一个天文数字。
#[test]
fn width_and_height_occupy_low_and_high_halves() {
    let packed = cell_metrics_packed_from(8.5, 17.25);
    let width = f32::from_bits((packed as u64 & 0xFFFF_FFFF) as u32);
    let height = f32::from_bits(((packed as u64) >> 32) as u32);
    assert_eq!(width, 8.5);
    assert_eq!(height, 17.25);
}

/// 负宽位模式（NaN/负数的位模式带最高位）不得污染高度半区。
#[test]
fn width_bits_do_not_leak_into_the_height_half() {
    let packed = cell_metrics_packed_from(-0.0, 1.0);
    let width = f32::from_bits((packed as u64 & 0xFFFF_FFFF) as u32);
    let height = f32::from_bits(((packed as u64) >> 32) as u32);
    assert_eq!(width, -0.0);
    assert!(width.is_sign_negative());
    assert_eq!(height, 1.0);
}
