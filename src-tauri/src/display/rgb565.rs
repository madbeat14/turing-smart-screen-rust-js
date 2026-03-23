/// Image format conversion utilities.
///
/// Ports `library/lcd/serialize.py` from the Python implementation.

/// Convert RGBA pixel buffer to RGB565 little-endian bytes (for Rev A, WeAct)
pub fn rgba_to_rgb565_le(rgba: &[u8]) -> Vec<u8> {
    let pixel_count = rgba.len() / 4;
    let mut out = Vec::with_capacity(pixel_count * 2);

    for pixel in rgba.chunks_exact(4) {
        let r = (pixel[0] >> 3) as u16;
        let g = (pixel[1] >> 2) as u16;
        let b = (pixel[2] >> 3) as u16;
        let color: u16 = (r << 11) | (g << 5) | b;
        out.extend_from_slice(&color.to_le_bytes());
    }

    out
}

/// Convert RGBA pixel buffer to RGB565 big-endian bytes (for Rev B, Rev D)
pub fn rgba_to_rgb565_be(rgba: &[u8]) -> Vec<u8> {
    let pixel_count = rgba.len() / 4;
    let mut out = Vec::with_capacity(pixel_count * 2);

    for pixel in rgba.chunks_exact(4) {
        let r = (pixel[0] >> 3) as u16;
        let g = (pixel[1] >> 2) as u16;
        let b = (pixel[2] >> 3) as u16;
        let color: u16 = (r << 11) | (g << 5) | b;
        out.extend_from_slice(&color.to_be_bytes());
    }

    out
}

/// Convert RGBA pixel buffer to BGR byte order (for Rev C)
#[allow(dead_code)] // Available for Rev C partial update format
pub fn rgba_to_bgr(rgba: &[u8]) -> Vec<u8> {
    let pixel_count = rgba.len() / 4;
    let mut out = Vec::with_capacity(pixel_count * 3);

    for pixel in rgba.chunks_exact(4) {
        out.push(pixel[2]); // B
        out.push(pixel[1]); // G
        out.push(pixel[0]); // R
    }

    out
}

/// Convert RGBA pixel buffer to BGRA byte order (for Rev C full-screen)
pub fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    let pixel_count = rgba.len() / 4;
    let mut out = Vec::with_capacity(pixel_count * 4);

    for pixel in rgba.chunks_exact(4) {
        out.push(pixel[2]); // B
        out.push(pixel[1]); // G
        out.push(pixel[0]); // R
        out.push(pixel[3]); // A
    }

    out
}

/// Convert RGBA to compressed BGRA (for Rev C partial updates)
/// Alpha is reduced to 4 bits and packed into the B and G channels' lower bits
#[allow(dead_code)] // Planned for Rev C optimized partial updates
pub fn rgba_to_compressed_bgra(rgba: &[u8]) -> Vec<u8> {
    let pixel_count = rgba.len() / 4;
    let mut out = Vec::with_capacity(pixel_count * 3);

    for pixel in rgba.chunks_exact(4) {
        let a = pixel[3] >> 4; // 4-bit alpha
        out.push((pixel[2] & 0xFC) | (a >> 2));    // B with alpha high bits
        out.push((pixel[1] & 0xFC) | (a & 0x02));  // G with alpha low bit
        out.push(pixel[0]);                          // R
    }

    out
}

/// Convert RGB pixel buffer (no alpha) to RGB565 little-endian
#[allow(dead_code)] // Available for non-RGBA input sources
pub fn rgb_to_rgb565_le(rgb: &[u8]) -> Vec<u8> {
    let pixel_count = rgb.len() / 3;
    let mut out = Vec::with_capacity(pixel_count * 2);

    for pixel in rgb.chunks_exact(3) {
        let r = (pixel[0] >> 3) as u16;
        let g = (pixel[1] >> 2) as u16;
        let b = (pixel[2] >> 3) as u16;
        let color: u16 = (r << 11) | (g << 5) | b;
        out.extend_from_slice(&color.to_le_bytes());
    }

    out
}

/// Iterator that yields fixed-size chunks from a byte slice
pub fn chunked(data: &[u8], chunk_size: usize) -> impl Iterator<Item = &[u8]> {
    data.chunks(chunk_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba_to_rgb565_le_black() {
        let rgba = [0u8, 0, 0, 255]; // black, full alpha
        let result = rgba_to_rgb565_le(&rgba);
        assert_eq!(result, [0x00, 0x00]); // RGB565 black = 0x0000
    }

    #[test]
    fn test_rgba_to_rgb565_le_white() {
        let rgba = [255u8, 255, 255, 255]; // white
        let result = rgba_to_rgb565_le(&rgba);
        // R=31, G=63, B=31 => (31<<11)|(63<<5)|31 = 0xFFFF
        assert_eq!(result, [0xFF, 0xFF]);
    }

    #[test]
    fn test_rgba_to_rgb565_le_red() {
        let rgba = [255u8, 0, 0, 255]; // pure red
        let result = rgba_to_rgb565_le(&rgba);
        // R=31, G=0, B=0 => (31<<11) = 0xF800 => LE: [0x00, 0xF8]
        assert_eq!(result, [0x00, 0xF8]);
    }

    #[test]
    fn test_rgba_to_rgb565_be_red() {
        let rgba = [255u8, 0, 0, 255];
        let result = rgba_to_rgb565_be(&rgba);
        // 0xF800 => BE: [0xF8, 0x00]
        assert_eq!(result, [0xF8, 0x00]);
    }

    #[test]
    fn test_rgba_to_bgr() {
        let rgba = [10u8, 20, 30, 255];
        let result = rgba_to_bgr(&rgba);
        assert_eq!(result, [30, 20, 10]); // B, G, R
    }

    #[test]
    fn test_chunked() {
        let data = [1, 2, 3, 4, 5];
        let chunks: Vec<&[u8]> = chunked(&data, 2).collect();
        assert_eq!(chunks, vec![&[1, 2][..], &[3, 4][..], &[5][..]]);
    }

    #[test]
    fn test_multiple_pixels() {
        // Two pixels: red, green
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255];
        let result = rgba_to_rgb565_le(&rgba);
        assert_eq!(result.len(), 4); // 2 pixels * 2 bytes each
    }
}
