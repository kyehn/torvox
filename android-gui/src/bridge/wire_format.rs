//! Wire format serialization/deserialization for the FFI bridge.
//!
//! # Responsibilities
//! - Read strings from raw pointers (unsafe, FFI boundary)
//! - Read u32 values from little-endian byte buffers
//! - Read length-prefixed strings from wire format
//! - Convert f32 color values to u32 ARGB format
//! - Create safe CString values from Rust strings
//!
//! Extracted from `ffi.rs` to improve testability and locality.

/// # Safety
///
/// `ptr` must point to a readable buffer of at least `len` bytes.
pub unsafe fn read_string(ptr: *const u8, len: i32) -> String {
    if ptr.is_null() || len <= 0 {
        String::new()
    } else {
        // SAFETY: The caller guarantees ptr is valid for reads of len bytes
        // and is properly aligned for u8 access. The returned slice is
        // immediately converted to an owned String, so no aliasing issues.
        let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        String::from_utf8_lossy(slice).to_string()
    }
}

/// Create a CString from a Rust string, stripping interior NULs.
pub fn safe_cstring(string: String) -> Option<std::ffi::CString> {
    if string.contains('\0') {
        log::warn!(
            "wire_format: safe_cstring encountered interior NUL(s) — data truncated. Original length: {}",
            string.len()
        );
    }
    let stripped: String = string
        .chars()
        .filter(|&character| character != '\0')
        .collect();
    if stripped.is_empty() {
        None
    } else {
        std::ffi::CString::new(stripped).ok()
    }
}

/// Read a u32 from a little-endian byte buffer at the given position.
pub fn read_u32_le(bytes: &[u8], pos: usize) -> Option<u32> {
    if pos + 4 > bytes.len() {
        log::error!(
            "wire_format: buffer too short at pos={pos}, len={}",
            bytes.len()
        );
        return None;
    }
    let slice: [u8; 4] = bytes[pos..pos + 4].try_into().ok()?;
    Some(u32::from_le_bytes(slice))
}

/// Read a length-prefixed string from wire format.
///
/// Format: [u32_le length][bytes...]
pub fn read_wire_string(bytes: &[u8], pos: &mut usize) -> Option<String> {
    let len_val = read_u32_le(bytes, *pos)?;
    let len = len_val as usize;
    *pos += 4;
    if *pos + len > bytes.len() {
        log::error!(
            "wire_format: string length {len} exceeds buffer at pos={}",
            *pos
        );
        return None;
    }
    let string_value = String::from_utf8_lossy(&bytes[*pos..*pos + len]).to_string();
    *pos += len;
    Some(string_value)
}

/// Convert f32 RGBA color values to u32 ARGB format.
#[inline]
pub fn to_argb(color: &[f32; 4]) -> u32 {
    let r = (color[0].clamp(0.0, 1.0) * 255.0) as u32;
    let g = (color[1].clamp(0.0, 1.0) * 255.0) as u32;
    let b = (color[2].clamp(0.0, 1.0) * 255.0) as u32;
    let a = (color[3].clamp(0.0, 1.0) * 255.0) as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_u32_le_basic() {
        let bytes = [0x01, 0x00, 0x00, 0x00];
        assert_eq!(read_u32_le(&bytes, 0), Some(1));
    }

    #[test]
    fn read_u32_le_boundary() {
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF];
        assert_eq!(read_u32_le(&bytes, 0), Some(u32::MAX));
    }

    #[test]
    fn read_u32_le_buffer_too_short() {
        let bytes = [0x01, 0x02];
        assert_eq!(read_u32_le(&bytes, 0), None);
    }

    #[test]
    fn read_wire_string_basic() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.extend_from_slice(b"hello");
        let mut pos = 0;
        assert_eq!(
            read_wire_string(&bytes, &mut pos),
            Some("hello".to_string())
        );
        assert_eq!(pos, 9);
    }

    #[test]
    fn read_wire_string_empty() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let mut pos = 0;
        assert_eq!(read_wire_string(&bytes, &mut pos), Some(String::new()));
    }

    #[test]
    fn read_wire_string_buffer_too_short() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&100u32.to_le_bytes());
        let mut pos = 0;
        assert_eq!(read_wire_string(&bytes, &mut pos), None);
    }

    #[test]
    fn to_argb_white() {
        assert_eq!(to_argb(&[1.0, 1.0, 1.0, 1.0]), 0xFFFF_FFFF);
    }

    #[test]
    fn to_argb_black() {
        assert_eq!(to_argb(&[0.0, 0.0, 0.0, 1.0]), 0xFF_00_00_00);
    }

    #[test]
    fn to_argb_transparent() {
        assert_eq!(to_argb(&[1.0, 0.0, 0.0, 0.0]), 0x00_FF_00_00);
    }

    #[test]
    fn to_argb_clamping() {
        assert_eq!(to_argb(&[2.0, -1.0, 0.5, 1.0]), 0xFF_FF_00_7F);
    }

    #[test]
    fn safe_cstring_basic() {
        let cstr = safe_cstring("hello".to_string());
        assert!(cstr.is_some());
        assert_eq!(cstr.unwrap().to_str().unwrap(), "hello");
    }

    #[test]
    fn safe_cstring_empty() {
        assert!(safe_cstring(String::new()).is_none());
    }

    #[test]
    fn safe_cstring_strips_nuls() {
        let cstr = safe_cstring("hel\0lo".to_string());
        assert!(cstr.is_some());
        assert_eq!(cstr.unwrap().to_str().unwrap(), "hello");
    }

    #[test]
    fn safe_cstring_all_nuls() {
        assert!(safe_cstring("\0\0".to_string()).is_none());
    }
}
