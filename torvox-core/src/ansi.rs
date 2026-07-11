//! ANSI 256-color table — color cube and grayscale ramp.
pub const ANSI_256: [[u8; 3]; 256] = {
    let mut table = [[0u8; 3]; 256];
    let mut i = 0;
    while i < 16 {
        table[i] = ANSI_16[i];
        i += 1;
    }
    // 6x6x6 color cube: indices 16..=231
    // Each channel step = 255 / (CUBE_SIDE - 1) = 51
    let mut v = 0;
    while v < CUBE_COLORS {
        let r = (v / CUBE_SQUARE) * CHANNEL_STEP;
        let g = ((v % CUBE_SQUARE) / CUBE_SIDE) * CHANNEL_STEP;
        let b = (v % CUBE_SIDE) * CHANNEL_STEP;
        table[16 + v] = [r as u8, g as u8, b as u8];
        v += 1;
    }
    // 24-shade grayscale: indices 232..=255
    // Values range from 8 to 238 in steps of 10
    let mut shade = 0;
    while shade < GRAYSCALE_SHADES {
        let value = GRAYSCALE_MIN + shade * GRAYSCALE_STEP;
        table[GRAYSCALE_OFFSET + shade] = [value as u8, value as u8, value as u8];
        shade += 1;
    }
    table
};

/// Number of colors in the 6x6x6 color cube (indices 16..=231)
const CUBE_COLORS: usize = 216;
/// Side length of the 6x6x6 color cube
const CUBE_SIDE: usize = 6;
/// Square of the cube side (6*6 = 36), used for red channel indexing
const CUBE_SQUARE: usize = CUBE_SIDE * CUBE_SIDE;
/// Step between adjacent channel values: 255 / (CUBE_SIDE - 1) = 51
const CHANNEL_STEP: usize = 255 / (CUBE_SIDE - 1);
/// Number of grayscale shades (indices 232..=255)
const GRAYSCALE_SHADES: usize = 24;
/// Index where grayscale begins in the 256-color table
const GRAYSCALE_OFFSET: usize = 232;
/// Minimum grayscale value (lightest black)
const GRAYSCALE_MIN: usize = 8;
/// Step between adjacent grayscale values
const GRAYSCALE_STEP: usize = 10;

const ANSI_16: [[u8; 3]; 16] = [
    [0, 0, 0],
    [128, 0, 0],
    [0, 128, 0],
    [128, 128, 0],
    [0, 0, 128],
    [128, 0, 128],
    [0, 128, 128],
    [192, 192, 192],
    [128, 128, 128],
    [255, 0, 0],
    [0, 255, 0],
    [255, 255, 0],
    [0, 0, 255],
    [255, 0, 255],
    [0, 255, 255],
    [255, 255, 255],
];

/// Map an ANSI 256-color index (0-255) to an RGBA byte triple.
pub fn ansi_to_rgb(index: u8) -> [u8; 3] {
    ANSI_256[index as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_16_first_entry_is_black() {
        assert_eq!(ANSI_256[0], [0, 0, 0]);
    }

    #[test]
    fn ansi_16_last_entry_is_white() {
        assert_eq!(ANSI_256[15], [255, 255, 255]);
    }

    #[test]
    fn ansi_216_color_cube() {
        assert_eq!(ANSI_256[16], [0, 0, 0]);
        assert_eq!(ANSI_256[231], [255, 255, 255]);
    }

    #[test]
    fn ansi_grayscale_range() {
        assert_eq!(ANSI_256[232], [8, 8, 8]);
        assert_eq!(ANSI_256[255], [238, 238, 238]);
    }

    #[test]
    fn ansi_to_rgb_index() {
        assert_eq!(ansi_to_rgb(1), [128, 0, 0]);
        assert_eq!(ansi_to_rgb(9), [255, 0, 0]);
    }

    #[test]
    fn ansi_256_table_size() {
        assert_eq!(ANSI_256.len(), 256);
    }

    #[test]
    fn ansi_16_complete() {
        for i in 0..16 {
            assert_eq!(ANSI_256[i], ANSI_16[i]);
        }
    }

    #[test]
    fn ansi_216_cube_starts_black() {
        for v in 0..CUBE_COLORS {
            let r = ((v / CUBE_SQUARE) * CHANNEL_STEP) as u8;
            let g = (((v % CUBE_SQUARE) / CUBE_SIDE) * CHANNEL_STEP) as u8;
            let b = ((v % CUBE_SIDE) * CHANNEL_STEP) as u8;
            assert_eq!(ANSI_256[16 + v], [r, g, b]);
        }
    }

    #[test]
    fn ansi_216_cube_first_index() {
        assert_eq!(ANSI_256[16], [0, 0, 0]);
    }

    #[test]
    fn ansi_216_cube_last_index() {
        assert_eq!(ANSI_256[231], [255, 255, 255]);
    }

    #[test]
    fn ansi_216_cube_known_color() {
        assert_eq!(ANSI_256[16 + 36 + 6 + 1], [51, 51, 51]);
        assert_eq!(ANSI_256[16 + 5 * 36 + 5 * 6 + 5], [255, 255, 255]);
    }

    #[test]
    fn ansi_grayscale_starts_at_8() {
        for i in 0..24 {
            let v = 8 + i * 10;
            assert_eq!(ANSI_256[232 + i], [v as u8, v as u8, v as u8]);
        }
    }

    #[test]
    fn ansi_grayscale_first_is_8() {
        assert_eq!(ANSI_256[232], [8, 8, 8]);
    }

    #[test]
    fn ansi_grayscale_last_is_238() {
        assert_eq!(ANSI_256[255], [238, 238, 238]);
    }

    #[test]
    fn ansi_to_rgb_passes_through_table() {
        for i in 0..=255u8 {
            assert_eq!(ansi_to_rgb(i), ANSI_256[i as usize]);
        }
    }

    #[test]
    fn ansi_pure_colors() {
        let cases = [
            (9, [255, 0, 0]),    // red
            (10, [0, 255, 0]),   // green
            (12, [0, 0, 255]),   // blue
            (11, [255, 255, 0]), // yellow
            (14, [0, 255, 255]), // cyan
            (13, [255, 0, 255]), // magenta
        ];
        for (idx, expected) in cases {
            assert_eq!(ansi_to_rgb(idx), expected, "ANSI index {}", idx);
        }
    }

    #[test]
    fn ansi_256_all_values_map_to_valid_rgb() {
        for i in 0..=255u8 {
            let [r, g, b] = ansi_to_rgb(i);
            // All values are u8, guaranteed 0-255 by type system
            let _ = (r, g, b);
        }
    }

    #[test]
    fn ansi_16_indexed_match() {
        for i in 0..=15u8 {
            assert_eq!(
                ansi_to_rgb(i),
                ANSI_16[i as usize],
                "ANSI 16 color {i} mismatch"
            );
        }
    }

    #[test]
    fn ansi_color_cube_symmetry() {
        let valid_values = [0, 51, 102, 153, 204, 255];
        for i in 16u8..=231 {
            let [r, g, b] = ansi_to_rgb(i);
            assert!(
                valid_values.contains(&r),
                "Red {r} not in valid set for index {i}"
            );
            assert!(
                valid_values.contains(&g),
                "Green {g} not in valid set for index {i}"
            );
            assert!(
                valid_values.contains(&b),
                "Blue {b} not in valid set for index {i}"
            );
        }
    }

    #[test]
    fn ansi_216_cube_red_values() {
        // First column (red=0): indices 16..=51
        for i in 16..52 {
            assert_eq!(ANSI_256[i][0], 0, "index {i} should have red=0");
        }
        // Column 1 (red=51): indices 52..=87
        for i in 52..88 {
            assert_eq!(ANSI_256[i][0], 51, "index {i} should have red=51");
        }
    }

    #[test]
    fn prop_ansi_all_indices_valid() {
        for index in 0..=255u8 {
            let [r, g, b] = ansi_to_rgb(index);
            assert!(
                (r, g, b) != (0, 0, 0) || index == 0 || index == 16,
                "index {index} returned [{r},{g},{b}]"
            );
        }
    }

    #[test]
    fn prop_ansi_16_matches_table() {
        for index in 0..16u8 {
            assert_eq!(
                ansi_to_rgb(index),
                ANSI_16[index as usize],
                "index {index} should match ANSI_16 table"
            );
        }
    }

    #[test]
    fn ansi_grayscale_values_increasing() {
        for i in 0..23 {
            let v1 = ANSI_256[232 + i][0];
            let v2 = ANSI_256[232 + i + 1][0];
            assert!(
                v2 > v1,
                "grayscale values must increase: index {} has {v1}, {} has {v2}",
                232 + i,
                232 + i + 1
            );
        }
    }
}
