pub const ANSI_256: [[u8; 3]; 256] = {
    let mut table = [[0u8; 3]; 256];
    let mut i = 0;
    while i < 16 {
        table[i] = ANSI_16[i];
        i += 1;
    }
    let mut v = 0;
    while v < 216 {
        let r = (v / 36) * 51;
        let g = ((v % 36) / 6) * 51;
        let b = (v % 6) * 51;
        table[16 + v] = [r as u8, g as u8, b as u8];
        v += 1;
    }
    let mut g = 0;
    while g < 24 {
        let v = 8 + g * 10;
        table[232 + g] = [v as u8, v as u8, v as u8];
        g += 1;
    }
    table
};

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
}
