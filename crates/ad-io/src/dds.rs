//! DDS map image decoding (AD-14): the top mip level as RGBA8.
//!
//! `image_dds` decodes DXT1–DXT5 (and BC4–BC7) and the masks A8R8G8B8, A8B8G8R8, R8G8B8,
//! A4R4G4B4 and A1R5G5B5. The 32-bit masks without alpha (X8R8G8B8, X8B8G8R8) are decoded here.
//! Every other format is refused with an error naming it.

use image::RgbaImage;
use image_dds::DdsFormatInfo;
use image_dds::ddsfile::Dds;

/// Largest accepted width and height.
pub const MAX_SIDE: u32 = 16384;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DdsError {
    #[error("not a readable DDS file ({0})")]
    Header(String),
    #[error("image size {width}x{height} is not between 1x1 and {MAX_SIDE}x{MAX_SIDE}")]
    Size { width: u32, height: u32 },
    #[error("unsupported DDS format {0}")]
    Unsupported(String),
    #[error("DDS data could not be decoded ({0})")]
    Data(String),
}

/// Decodes the top mip level of a 2D DDS texture.
pub fn decode(bytes: &[u8]) -> Result<RgbaImage, DdsError> {
    let dds = Dds::read(bytes).map_err(|e| DdsError::Header(e.to_string()))?;
    let (width, height) = (dds.get_width(), dds.get_height());
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(DdsError::Size { width, height });
    }
    if dds.get_depth() > 1 || dds.get_num_array_layers() > 1 {
        return Err(DdsError::Unsupported(
            "cube map, array or volume texture".into(),
        ));
    }
    if let Some(order) = rgbx_order(&dds) {
        return decode_rgbx(&dds, order);
    }
    if let Err(info) = image_dds::dds_image_format(&dds) {
        return Err(DdsError::Unsupported(format_name(&dds, &info)));
    }
    image_dds::image_from_dds(&dds, 0).map_err(|e| DdsError::Data(e.to_string()))
}

/// Byte offsets of R, G and B for the 32-bit masks without alpha, which `image_dds` lacks.
fn rgbx_order(dds: &Dds) -> Option<[usize; 3]> {
    let pf = &dds.header.spf;
    if pf.fourcc.is_some() || pf.rgb_bit_count != Some(32) || pf.a_bit_mask.unwrap_or(0) != 0 {
        return None;
    }
    match (pf.r_bit_mask, pf.g_bit_mask, pf.b_bit_mask) {
        (Some(0xff_0000), Some(0xff00), Some(0xff)) => Some([2, 1, 0]), // X8R8G8B8
        (Some(0xff), Some(0xff00), Some(0xff_0000)) => Some([0, 1, 2]), // X8B8G8R8
        _ => None,
    }
}

fn decode_rgbx(dds: &Dds, [r, g, b]: [usize; 3]) -> Result<RgbaImage, DdsError> {
    let (width, height) = (dds.get_width(), dds.get_height());
    let len = width as usize * height as usize * 4;
    let data = dds
        .data
        .get(..len)
        .ok_or_else(|| DdsError::Data(format!("expected {len} bytes, found {}", dds.data.len())))?;
    let rgba = data
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[r], p[g], p[b], 255])
        .collect();
    RgbaImage::from_raw(width, height, rgba).ok_or_else(|| DdsError::Data("size mismatch".into()))
}

fn format_name(dds: &Dds, info: &DdsFormatInfo) -> String {
    let pf = &dds.header.spf;
    if let Some(d3d) = &info.d3d {
        format!("{d3d:?}")
    } else if let Some(dxgi) = &info.dxgi {
        format!("{dxgi:?}")
    } else if let Some(fourcc) = &info.fourcc {
        format!("FourCC {}", fourcc.0.to_le_bytes().escape_ascii())
    } else {
        let mask = |m: Option<u32>| m.unwrap_or(0);
        format!(
            "{}-bit masks R {:#x} G {:#x} B {:#x} A {:#x}",
            pf.rgb_bit_count.unwrap_or(0),
            mask(pf.r_bit_mask),
            mask(pf.g_bit_mask),
            mask(pf.b_bit_mask),
            mask(pf.a_bit_mask),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    const FOURCC: u32 = 0x4;
    const RGB: u32 = 0x40;
    const ALPHA_PIXELS: u32 = 0x1;

    /// A 128-byte header (magic included) for a 2D texture without mipmaps.
    fn header(
        width: u32,
        height: u32,
        pf_flags: u32,
        fourcc: &[u8; 4],
        bits: u32,
        masks: [u32; 4],
    ) -> Vec<u8> {
        let mut h = Vec::with_capacity(128);
        let mut put = |v: u32| h.extend_from_slice(&v.to_le_bytes());
        put(u32::from_le_bytes(*b"DDS "));
        put(124);
        put(0x1007); // CAPS | HEIGHT | WIDTH | PIXELFORMAT
        put(height);
        put(width);
        put(0); // pitch or linear size
        put(0); // depth
        put(0); // mipmap count
        for _ in 0..11 {
            put(0);
        }
        put(32);
        put(pf_flags);
        put(u32::from_le_bytes(*fourcc));
        put(bits);
        masks.into_iter().for_each(&mut put);
        put(0x1000); // caps: TEXTURE
        for _ in 0..4 {
            put(0);
        }
        h
    }

    const RED: u16 = 0xf800;
    const BLUE: u16 = 0x001f;

    fn color_block(c0: u16, c1: u16, index: u32) -> Vec<u8> {
        let indices = (0..16).fold(0u32, |acc, i| acc | index << (2 * i));
        [c0.to_le_bytes(), c1.to_le_bytes()]
            .concat()
            .into_iter()
            .chain(indices.to_le_bytes())
            .collect()
    }

    /// An 8x8 block-compressed file: the left blocks are `left`, the right ones `right`.
    fn blocks(fourcc: &[u8; 4], left: &[u8], right: &[u8]) -> Vec<u8> {
        let mut file = header(8, 8, FOURCC, fourcc, 0, [0; 4]);
        for _ in 0..2 {
            file.extend_from_slice(left);
            file.extend_from_slice(right);
        }
        file
    }

    fn assert_halves(image: &RgbaImage, left: [u8; 4], right: [u8; 4]) {
        assert_eq!(image.dimensions(), (8, 8));
        for (x, y, p) in image.enumerate_pixels() {
            let want = if x < 4 { left } else { right };
            assert_eq!(p.0, want, "pixel {x},{y}");
        }
    }

    fn dxt1() -> Vec<u8> {
        // Right half: c0 <= c1 selects the 1-bit alpha mode, index 3 is transparent black.
        blocks(
            b"DXT1",
            &color_block(RED, BLUE, 0),
            &color_block(BLUE, RED, 3),
        )
    }

    #[test]
    fn dxt1_decodes_with_punch_through_alpha() {
        let image = decode(&dxt1()).expect("decodes");
        assert_halves(&image, [255, 0, 0, 255], [0, 0, 0, 0]);
    }

    #[test]
    fn dxt2_and_dxt3_decode_explicit_alpha() {
        // Explicit alpha: 4 bits per pixel, 0xf on the left, 0x8 (136) on the right.
        let left = [vec![0xff; 8], color_block(RED, BLUE, 0)].concat();
        let right = [vec![0x88; 8], color_block(RED, BLUE, 1)].concat();
        for fourcc in [b"DXT2", b"DXT3"] {
            let image = decode(&blocks(fourcc, &left, &right)).expect("decodes");
            assert_halves(&image, [255, 0, 0, 255], [0, 0, 255, 136]);
        }
    }

    #[test]
    fn dxt4_and_dxt5_decode_interpolated_alpha() {
        // Alpha endpoints 200 and 50; index 0 on the left, index 1 (all bits 001) on the right.
        let alpha = |bits: [u8; 6]| [vec![200, 50], bits.to_vec()].concat();
        let left = [alpha([0; 6]), color_block(RED, BLUE, 0)].concat();
        let right = [
            alpha([0x49, 0x92, 0x24, 0x49, 0x92, 0x24]),
            color_block(RED, BLUE, 1),
        ]
        .concat();
        for fourcc in [b"DXT4", b"DXT5"] {
            let image = decode(&blocks(fourcc, &left, &right)).expect("decodes");
            assert_halves(&image, [255, 0, 0, 200], [0, 0, 255, 50]);
        }
    }

    /// The ten uncompressed Java formats: name, bits, R/G/B/A masks, decodes.
    const UNCOMPRESSED: [(&str, u32, [u32; 4], bool); 10] = [
        ("A1R5G5B5", 16, [0x7c00, 0x03e0, 0x001f, 0x8000], true),
        ("X1R5G5B5", 16, [0x7c00, 0x03e0, 0x001f, 0], false),
        ("A4R4G4B4", 16, [0x0f00, 0x00f0, 0x000f, 0xf000], true),
        ("X4R4G4B4", 16, [0x0f00, 0x00f0, 0x000f, 0], false),
        ("R5G6B5", 16, [0xf800, 0x07e0, 0x001f, 0], false),
        ("R8G8B8", 24, [0xff_0000, 0xff00, 0xff, 0], true),
        ("A8B8G8R8", 32, [0xff, 0xff00, 0xff_0000, 0xff00_0000], true),
        ("X8B8G8R8", 32, [0xff, 0xff00, 0xff_0000, 0], true),
        ("A8R8G8B8", 32, [0xff_0000, 0xff00, 0xff, 0xff00_0000], true),
        ("X8R8G8B8", 32, [0xff_0000, 0xff00, 0xff, 0], true),
    ];

    /// An 8x8 file: red and opaque on the left, cyan and (if the format has alpha)
    /// transparent on the right.
    fn uncompressed(bits: u32, [r, g, b, a]: [u32; 4]) -> Vec<u8> {
        let flags = if a == 0 { RGB } else { RGB | ALPHA_PIXELS };
        let mut file = header(8, 8, flags, b"\0\0\0\0", bits, [r, g, b, a]);
        for _y in 0..8 {
            for x in 0..8 {
                let pixel = if x < 4 { r | a } else { g | b };
                file.extend_from_slice(&pixel.to_le_bytes()[..bits as usize / 8]);
            }
        }
        file
    }

    #[test]
    fn uncompressed_formats_decode_or_are_refused_by_name() {
        for (name, bits, masks, decodes) in UNCOMPRESSED {
            let result = decode(&uncompressed(bits, masks));
            if decodes {
                let image = result.unwrap_or_else(|e| panic!("{name}: {e}"));
                let alpha = if masks[3] == 0 { 255 } else { 0 };
                assert_halves(&image, [255, 0, 0, 255], [0, 255, 255, alpha]);
            } else {
                assert_eq!(result, Err(DdsError::Unsupported(name.into())), "{name}");
            }
        }
    }

    #[test]
    fn unknown_fourcc_is_refused_by_name() {
        let file = blocks(b"YUY2", &[0; 8], &[0; 8]);
        assert_eq!(decode(&file), Err(DdsError::Unsupported("YUY2".into())));
        let file = blocks(b"ABCD", &[0; 8], &[0; 8]);
        assert_eq!(
            decode(&file),
            Err(DdsError::Unsupported("FourCC ABCD".into()))
        );
    }

    #[test]
    fn truncated_files_are_errors() {
        let dxt1 = dxt1();
        assert!(matches!(decode(&dxt1[..60]), Err(DdsError::Header(_))));
        assert!(matches!(decode(&dxt1[..128]), Err(DdsError::Data(_))));
        assert!(matches!(decode(&dxt1[..128 + 16]), Err(DdsError::Data(_))));
        let x8 = uncompressed(32, UNCOMPRESSED[9].2);
        assert!(matches!(decode(&x8[..128 + 100]), Err(DdsError::Data(_))));
        // No prefix of any sample panics.
        for (_, bits, masks, _) in UNCOMPRESSED {
            let file = uncompressed(bits, masks);
            (0..file.len()).for_each(|n| assert!(decode(&file[..n]).is_err()));
        }
        (0..dxt1.len()).for_each(|n| assert!(decode(&dxt1[..n]).is_err()));
    }

    #[test]
    fn absurd_sizes_are_refused_before_decoding() {
        let file = header(100_000, 100_000, FOURCC, b"DXT1", 0, [0; 4]);
        assert_eq!(
            decode(&file),
            Err(DdsError::Size {
                width: 100_000,
                height: 100_000
            })
        );
        let file = header(MAX_SIDE + 1, 1, RGB, b"\0\0\0\0", 32, UNCOMPRESSED[9].2);
        assert!(matches!(decode(&file), Err(DdsError::Size { .. })));
        let file = header(0, 8, FOURCC, b"DXT1", 0, [0; 4]);
        assert!(matches!(decode(&file), Err(DdsError::Size { .. })));
    }

    /// Reference BC1 decode of one pixel, independent of `image_dds`: exact interpolation of
    /// the 5:6:5 endpoints, rounded.
    fn reference_dxt1(data: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let block = ((y / 4) * (width / 4) + x / 4) as usize * 8;
        let word = |i: usize| u16::from_le_bytes([data[block + i], data[block + i + 1]]);
        let (c0, c1) = (word(0), word(2));
        let bits = u32::from(word(4)) | u32::from(word(6)) << 16;
        let index = (bits >> (2 * ((y % 4) * 4 + x % 4))) & 3;
        let channels = |c: u16| {
            [
                f64::from(c >> 11) * 255.0 / 31.0,
                f64::from((c >> 5) & 0x3f) * 255.0 / 63.0,
                f64::from(c & 0x1f) * 255.0 / 31.0,
            ]
        };
        let (a, b) = (channels(c0), channels(c1));
        let mix = |wa: f64, wb: f64| {
            let p = |i: usize| (a[i] * wa + b[i] * wb).round() as u8;
            [p(0), p(1), p(2), 255]
        };
        match (index, c0 > c1) {
            (0, _) => mix(1.0, 0.0),
            (1, _) => mix(0.0, 1.0),
            (2, true) => mix(2.0 / 3.0, 1.0 / 3.0),
            (3, true) => mix(1.0 / 3.0, 2.0 / 3.0),
            (2, false) => mix(0.5, 0.5),
            _ => [0; 4],
        }
    }

    #[test]
    fn fs25_sample_matches_a_reference_decode() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../pda_map_H.dds");
        let Ok(bytes) = std::fs::read(path) else {
            eprintln!("skipped: {path} is absent");
            return;
        };
        let image = decode(&bytes).expect("decodes");
        assert_eq!(image.dimensions(), (4096, 4096));
        let data = &bytes[128..];
        let probes = (0..64u32).map(|i| {
            (
                i.wrapping_mul(2_654_435_761) % 4096,
                (i * 40503 + 17) % 4096,
            )
        });
        for (x, y) in probes.chain([(0, 0), (4095, 4095), (2048, 2048), (1, 4094)]) {
            let want = reference_dxt1(data, 4096, x, y);
            let got = image.get_pixel(x, y).0;
            let close = got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 1);
            assert!(close, "pixel {x},{y}: {got:?} vs reference {want:?}");
        }
    }
}
