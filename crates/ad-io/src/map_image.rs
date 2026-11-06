//! Map image lookup, decoding and scaling (AD-14). Runs on the load job, never on the UI thread.

use std::path::{Path, PathBuf};

use ad_core::heightmap::Heightmap;
use image::imageops::FilterType;

const SIDE: u32 = 2048;
const BLANK_PNG: &[u8] = include_bytes!("../assets/Blank.png");

/// A map image as RGBA8, always 2048x2048.
#[derive(Debug, Clone, PartialEq)]
pub struct MapImage {
    pub rgba: Vec<u8>,
}

/// `<exe dir>/mapImages/<name>/<name>.png`, else the same beside the config. First existing wins.
pub fn find_map_image(
    name: &str,
    exe_dir: Option<&Path>,
    config_dir: Option<&Path>,
) -> Option<PathBuf> {
    find_in_map_images(name, "", exe_dir, config_dir)
}

/// `<config dir>/terrain.heightmap.png`, else `mapImages/<name>/<name>_HeightMap.png` as for
/// the map image. Without a name only the first applies.
pub fn find_heightmap(
    name: Option<&str>,
    exe_dir: Option<&Path>,
    config_dir: Option<&Path>,
) -> Option<PathBuf> {
    let beside = config_dir.map(|dir| dir.join("terrain.heightmap.png"));
    if let Some(path) = beside.filter(|p| p.is_file()) {
        return Some(path);
    }
    find_in_map_images(name?, "_HeightMap", exe_dir, config_dir)
}

fn find_in_map_images(
    name: &str,
    suffix: &str,
    exe_dir: Option<&Path>,
    config_dir: Option<&Path>,
) -> Option<PathBuf> {
    [exe_dir, config_dir].into_iter().flatten().find_map(|dir| {
        let path = dir
            .join("mapImages")
            .join(name)
            .join(format!("{name}{suffix}.png"));
        path.is_file().then_some(path)
    })
}

/// Decodes a PNG heightmap: 16-bit grey as is, 8-bit RGB(A) as `(R << 8) + G` (Java). Any
/// other format, or a file that does not decode, is an error naming the file.
pub fn load_heightmap(path: &Path) -> Result<Heightmap, String> {
    let fail = |why: &dyn std::fmt::Display| {
        format!("Heightmap {} could not be loaded ({why})", path.display())
    };
    let bytes = std::fs::read(path).map_err(|e| fail(&e))?;
    // PNG colour type 3 is a palette; the decoder would expand it to RGB.
    if bytes.get(25) == Some(&3) {
        return Err(fail(&"unsupported format: palette"));
    }
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| fail(&e))?;
    let (w, h) = (img.width(), img.height());
    let raw: Vec<u16> = match img.color() {
        image::ColorType::L16 => img.into_luma16().into_raw(),
        image::ColorType::Rgb8 | image::ColorType::Rgba8 => img
            .into_rgb8()
            .pixels()
            .map(|p| (u16::from(p[0]) << 8) + u16::from(p[1]))
            .collect(),
        other => return Err(fail(&format!("unsupported format {other:?}"))),
    };
    Heightmap::new(w, h, raw).ok_or_else(|| fail(&"empty image"))
}

/// The embedded Blank image.
pub fn blank() -> MapImage {
    match decode(BLANK_PNG) {
        Ok((image, _)) => image,
        // Unreachable for the shipped asset; a grey square keeps the view usable.
        Err(_) => MapImage {
            rgba: vec![128; (SIDE * SIDE * 4) as usize],
        },
    }
}

/// Decodes and scales the file. A broken file gives Blank and a notice; a wrong size keeps the
/// image, resized to 2048x2048, with a notice naming the original size.
pub fn load_map_image(path: &Path) -> (MapImage, Option<String>) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return (blank(), Some(failed(path, &e))),
    };
    match decode(&bytes) {
        Ok((image, None)) => (image, None),
        Ok((image, Some((w, h)))) => (
            image,
            Some(format!(
                "Map image {} is {w}x{h} and was scaled to {SIDE}x{SIDE}",
                path.display()
            )),
        ),
        Err(e) => (blank(), Some(failed(path, &e))),
    }
}

fn failed(path: &Path, error: &dyn std::fmt::Display) -> String {
    format!(
        "Map image {} could not be loaded ({error}); showing a blank map",
        path.display()
    )
}

/// The image and, if it was resized, its original size.
fn decode(bytes: &[u8]) -> Result<(MapImage, Option<(u32, u32)>), image::ImageError> {
    let rgba = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.into_rgba8();
    let (w, h) = rgba.dimensions();
    if (w, h) == (SIDE, SIDE) {
        return Ok((
            MapImage {
                rgba: rgba.into_raw(),
            },
            None,
        ));
    }
    let scaled = image::imageops::resize(&rgba, SIDE, SIDE, FilterType::Triangle);
    Ok((
        MapImage {
            rgba: scaled.into_raw(),
        },
        Some((w, h)),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ad_io_map_image_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn put(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let folder = dir.join("mapImages").join(name);
        std::fs::create_dir_all(&folder).expect("folder");
        let path = folder.join(format!("{name}.png"));
        std::fs::write(&path, bytes).expect("write");
        path
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .expect("encode");
        out.into_inner()
    }

    #[test]
    fn exe_dir_wins_then_config_dir() {
        let (exe, cfg) = (temp("exe"), temp("cfg"));
        let in_cfg = put(&cfg, "M", b"x");
        assert_eq!(find_map_image("M", Some(&exe), Some(&cfg)), Some(in_cfg));
        let in_exe = put(&exe, "M", b"x");
        assert_eq!(find_map_image("M", Some(&exe), Some(&cfg)), Some(in_exe));
        assert_eq!(find_map_image("Other", Some(&exe), Some(&cfg)), None);
        assert_eq!(find_map_image("M", None, None), None);
        let _ = std::fs::remove_dir_all(exe);
        let _ = std::fs::remove_dir_all(cfg);
    }

    #[test]
    fn right_size_has_no_notice() {
        let dir = temp("right");
        let path = put(&dir, "M", &png(SIDE, SIDE));
        let (img, notice) = load_map_image(&path);
        assert_eq!((img.rgba.len(), notice), ((SIDE * SIDE * 4) as usize, None));
        assert_eq!(&img.rgba[..4], &[10, 20, 30, 255]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wrong_size_is_scaled_with_notice() {
        let dir = temp("wrong");
        let path = put(&dir, "M", &png(1000, 800));
        let (img, notice) = load_map_image(&path);
        assert_eq!(img.rgba.len(), (SIDE * SIDE * 4) as usize);
        let notice = notice.expect("notice");
        assert!(
            notice.contains("M.png") && notice.contains("1000x800"),
            "{notice}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn broken_file_gives_blank_with_notice() {
        let dir = temp("broken");
        let path = put(&dir, "M", b"not a png");
        let (img, notice) = load_map_image(&path);
        assert_eq!(img, blank());
        assert!(notice.expect("notice").contains("M.png"));
        let _ = std::fs::remove_dir_all(dir);
    }

    fn encode<P: image::Pixel + image::PixelWithColorType>(
        img: &image::ImageBuffer<P, Vec<P::Subpixel>>,
    ) -> Vec<u8>
    where
        [P::Subpixel]: image::EncodableLayout,
    {
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .expect("encode");
        out.into_inner()
    }

    fn put_heightmap(dir: &Path, name: &str) -> PathBuf {
        let folder = dir.join("mapImages").join(name);
        std::fs::create_dir_all(&folder).expect("folder");
        let path = folder.join(format!("{name}_HeightMap.png"));
        std::fs::write(&path, b"x").expect("write");
        path
    }

    #[test]
    fn heightmap_lookup_order_and_fallback() {
        let (exe, cfg) = (temp("hm_exe"), temp("hm_cfg"));
        assert_eq!(find_heightmap(Some("M"), Some(&exe), Some(&cfg)), None);
        let in_cfg = put_heightmap(&cfg, "M");
        assert_eq!(
            find_heightmap(Some("M"), Some(&exe), Some(&cfg)),
            Some(in_cfg)
        );
        let in_exe = put_heightmap(&exe, "M");
        assert_eq!(
            find_heightmap(Some("M"), Some(&exe), Some(&cfg)),
            Some(in_exe)
        );
        let beside = cfg.join("terrain.heightmap.png");
        std::fs::write(&beside, b"x").expect("write");
        assert_eq!(
            find_heightmap(Some("M"), Some(&exe), Some(&cfg)),
            Some(beside.clone())
        );
        // No map name: only the file beside the config.
        assert_eq!(find_heightmap(None, Some(&exe), Some(&cfg)), Some(beside));
        assert_eq!(find_heightmap(None, Some(&cfg), Some(&exe)), None);
        let _ = std::fs::remove_dir_all(exe);
        let _ = std::fs::remove_dir_all(cfg);
    }

    #[test]
    fn heightmap_decodes_grey16_and_rgb8() {
        let dir = temp("hm_decode");
        let grey = dir.join("grey.png");
        let img = image::ImageBuffer::<image::Luma<u16>, _>::from_pixel(4, 4, image::Luma([25600]));
        std::fs::write(&grey, encode(&img)).expect("write");
        let hm = load_heightmap(&grey).expect("grey16");
        assert_eq!(
            (hm.width(), hm.height(), hm.sample_raw(3, 3)),
            (4, 4, 25600)
        );
        let rgb = dir.join("rgb.png");
        let img = image::RgbImage::from_pixel(4, 4, image::Rgb([100, 128, 0]));
        std::fs::write(&rgb, encode(&img)).expect("write");
        assert_eq!(load_heightmap(&rgb).expect("rgb8").sample_raw(0, 0), 25728);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn broken_or_unsupported_heightmap_is_an_error_naming_the_file() {
        let dir = temp("hm_broken");
        let bad = dir.join("bad.png");
        std::fs::write(&bad, b"not a png").expect("write");
        let message = load_heightmap(&bad).expect_err("garbage");
        assert!(message.contains("bad.png"), "{message}");
        let grey8 = dir.join("grey8.png");
        image::GrayImage::from_pixel(4, 4, image::Luma([7]))
            .save(&grey8)
            .expect("png");
        let message = load_heightmap(&grey8).expect_err("grey8");
        assert!(message.contains("grey8.png"), "{message}");
        // A palette PNG: IHDR colour type (byte 25) is 3.
        let palette = dir.join("palette.png");
        let mut bytes = encode(&image::RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3])));
        bytes[25] = 3;
        std::fs::write(&palette, bytes).expect("write");
        let message = load_heightmap(&palette).expect_err("palette");
        assert!(message.contains("palette.png"), "{message}");
        assert!(load_heightmap(&dir.join("missing.png")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn blank_is_the_full_size() {
        assert_eq!(blank().rgba.len(), (SIDE * SIDE * 4) as usize);
        assert_ne!(blank().rgba[0..4], [128, 128, 128, 128]);
    }
}
