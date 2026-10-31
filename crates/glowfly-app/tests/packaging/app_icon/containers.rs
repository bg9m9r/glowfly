//! The Windows `.ico` and macOS `.icns` containers, written and read by hand: a header, then
//! one PNG per size.

/// An `.ico` holding `images` as (size in pixels, PNG bytes), in that order.
pub fn write_ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // an icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, png) in images {
        // 256 is written as 0.
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    out
}

fn u16_le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The images of an `.ico` as (size in pixels, PNG bytes).
pub fn read_ico(bytes: &[u8]) -> Vec<(u32, Vec<u8>)> {
    assert_eq!((u16_le(bytes, 0), u16_le(bytes, 2)), (0, 1), "an icon file");
    (0..u16_le(bytes, 4) as usize)
        .map(|i| {
            let e = 6 + 16 * i;
            let size = if bytes[e] == 0 { 256 } else { bytes[e] as u32 };
            let (len, offset) = (u32_le(bytes, e + 8) as usize, u32_le(bytes, e + 12) as usize);
            (size, bytes[offset..offset + len].to_vec())
        })
        .collect()
}

/// An `.icns` holding `chunks` as (type, PNG bytes), in that order.
pub fn write_icns(chunks: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let total: usize = 8 + chunks.iter().map(|(_, png)| 8 + png.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&(total as u32).to_be_bytes());
    for (kind, png) in chunks {
        assert_eq!(kind.len(), 4);
        out.extend_from_slice(kind.as_bytes());
        out.extend_from_slice(&((8 + png.len()) as u32).to_be_bytes());
        out.extend_from_slice(png);
    }
    out
}

/// The chunks of an `.icns` as (type, data).
pub fn read_icns(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    assert_eq!(&bytes[..4], b"icns");
    let be = |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;
    assert_eq!(be(4), bytes.len(), "the header's length");
    let mut chunks = Vec::new();
    let mut at = 8;
    while at < bytes.len() {
        let len = be(at + 4);
        chunks.push((String::from_utf8_lossy(&bytes[at..at + 4]).into_owned(), bytes[at + 8..at + len].to_vec()));
        at += len;
    }
    chunks
}

/// A 24-bit Windows bitmap of `rgb` (`width` x `height`, rows top to bottom): the format WiX's
/// installer dialogs take. BMP rows are stored bottom up, as BGR, padded to 4 bytes.
pub fn write_bmp(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(rgb.len(), (width * height * 3) as usize);
    let stride = (width * 3).div_ceil(4) * 4;
    let size = 54 + stride * height;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(b"BM");
    for v in [size, 0, 54, 40] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
    out.extend_from_slice(&24u16.to_le_bytes()); // bits per pixel
    for v in [0, stride * height, 2835, 2835, 0, 0] {
        // No compression, the pixel data's size, 72 dpi, no palette.
        out.extend_from_slice(&v.to_le_bytes());
    }
    for row in (0..height as usize).rev() {
        let line = &rgb[row * width as usize * 3..(row + 1) * width as usize * 3];
        for px in line.chunks(3) {
            out.extend_from_slice(&[px[2], px[1], px[0]]);
        }
        out.resize(out.len() + (stride - width * 3) as usize, 0);
    }
    out
}

/// The (width, height, RGB rows top to bottom) of a bitmap `write_bmp` wrote.
pub fn read_bmp(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    assert_eq!(&bytes[..2], b"BM");
    let (offset, width, height) = (u32_le(bytes, 10) as usize, u32_le(bytes, 18), u32_le(bytes, 22));
    assert_eq!(u16_le(bytes, 28), 24, "24 bits per pixel");
    let stride = ((width * 3).div_ceil(4) * 4) as usize;
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for row in (0..height as usize).rev() {
        let line = &bytes[offset + row * stride..offset + row * stride + width as usize * 3];
        for px in line.chunks(3) {
            rgb.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }
    (width, height, rgb)
}
