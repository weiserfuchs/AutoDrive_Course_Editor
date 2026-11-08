//! Map image lookup, decoding and scaling (AD-14). Runs on the load job, never on the UI thread.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ad_core::heightmap::Heightmap;
use image::RgbaImage;
use image::imageops::FilterType;

use crate::config_xml::GameVersion;

const SIDE: u32 = 2048;
const BLANK_PNG: &[u8] = include_bytes!("../assets/Blank.png");

/// A map image as RGBA8, always 2048x2048, with PNG bytes that decode to it. The RGBA is
/// dropped after the texture upload; the PNG is kept to save the shown image (AD-8).
#[derive(Debug, Clone, PartialEq)]
pub struct MapImage {
    pub rgba: Vec<u8>,
    pub png: Arc<[u8]>,
}

impl MapImage {
    /// Encodes `rgba` (2048x2048) once for its PNG bytes.
    fn encoded(rgba: Vec<u8>) -> Result<Self, image::ImageError> {
        let png = encode_rgba(&rgba)?.into();
        Ok(Self { rgba, png })
    }
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
    decode_heightmap(&bytes).map_err(|why| fail(&why))
}

/// Decodes PNG heightmap bytes as `load_heightmap` does; the error is the reason alone.
pub fn decode_heightmap(bytes: &[u8]) -> Result<Heightmap, String> {
    // PNG colour type 3 is a palette; the decoder would expand it to RGB.
    if bytes.get(25) == Some(&3) {
        return Err("unsupported format: palette".to_owned());
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    let raw: Vec<u16> = match img.color() {
        image::ColorType::L16 => img.into_luma16().into_raw(),
        image::ColorType::Rgb8 | image::ColorType::Rgba8 => img
            .into_rgb8()
            .pixels()
            .map(|p| (u16::from(p[0]) << 8) + u16::from(p[1]))
            .collect(),
        other => return Err(format!("unsupported format {other:?}")),
    };
    Heightmap::new(w, h, raw).ok_or_else(|| "empty image".to_owned())
}

/// The embedded Blank image.
pub fn blank() -> MapImage {
    match decode(BLANK_PNG) {
        Ok((image, _)) => image,
        // Unreachable for the shipped asset; a grey square keeps the view usable.
        Err(_) => {
            let rgba = vec![128; (SIDE * SIDE * 4) as usize];
            let png = encode_rgba(&rgba).unwrap_or_default().into();
            MapImage { rgba, png }
        }
    }
}

/// Decodes and scales the file. A broken file gives Blank and a notice; a wrong size keeps the
/// image, resized to 2048x2048, with a notice naming the original size.
pub fn load_map_image(path: &Path) -> (MapImage, Option<String>) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return (blank(), Some(failed(path, &e))),
    };
    match decode_map_image(&bytes, &path.display().to_string()) {
        Ok(decoded) => decoded,
        Err(e) => (blank(), Some(failed(path, &e))),
    }
}

/// Decodes PNG bytes as `load_map_image` does; a wrong size is scaled with a notice naming
/// `source`. Bytes that are no PNG are an error.
pub fn decode_map_image(
    bytes: &[u8],
    source: &str,
) -> Result<(MapImage, Option<String>), image::ImageError> {
    Ok(match decode(bytes)? {
        (image, None) => (image, None),
        (image, Some((w, h))) => (
            image,
            Some(format!(
                "Map image {source} is {w}x{h} and was scaled to {SIDE}x{SIDE}"
            )),
        ),
    })
}

/// `<exe dir>/mapImages/<name>/<name>.png`, where an imported image is saved.
pub fn map_image_target(exe_dir: &Path, name: &str) -> PathBuf {
    exe_dir
        .join("mapImages")
        .join(name)
        .join(format!("{name}.png"))
}

/// `<exe dir>/mapImages/<name>/<name>_HeightMap.png`, where an imported heightmap is saved.
pub fn heightmap_target(exe_dir: &Path, name: &str) -> PathBuf {
    exe_dir
        .join("mapImages")
        .join(name)
        .join(format!("{name}_HeightMap.png"))
}

/// The part of a decoded DDS that is the map (AD-11): FS19 the whole image, FS22 and FS25
/// the centre half (`w/4, h/4` to `3w/4, 3h/4`, as Java).
pub fn crop_for(version: GameVersion, image: RgbaImage) -> RgbaImage {
    if version == GameVersion::FS19 {
        return image;
    }
    let (x, y, w, h) = crop_rect(version, image.dimensions());
    image::imageops::crop_imm(&image, x, y, w, h).to_image()
}

/// `crop_for`'s rectangle as x, y, width, height.
fn crop_rect(version: GameVersion, (w, h): (u32, u32)) -> (u32, u32, u32, u32) {
    if version == GameVersion::FS19 {
        return (0, 0, w, h);
    }
    (w / 4, h / 4, (w / 2).max(1), (h / 2).max(1))
}

/// Side of the import previews.
pub const PREVIEW_SIDE: u32 = 256;

/// What `crop_for(version)` keeps of `image`, scaled to 256x256 without copying the image.
pub fn preview(version: GameVersion, image: &RgbaImage) -> RgbaImage {
    let (x, y, w, h) = crop_rect(version, image.dimensions());
    let view = image::imageops::crop_imm(image, x, y, w, h);
    image::imageops::resize(&*view, PREVIEW_SIDE, PREVIEW_SIDE, FilterType::Triangle)
}

/// The heightmap as a 256x256 grey image, nearest samples stretched from lowest to highest.
pub fn heightmap_preview(heightmap: &Heightmap) -> RgbaImage {
    let (w, h) = (u64::from(heightmap.width()), u64::from(heightmap.height()));
    let side = u64::from(PREVIEW_SIDE);
    let sample = |x: u32, y: u32| {
        let px = u64::from(x) * w / side;
        let py = u64::from(y) * h / side;
        heightmap.sample_raw(px as i64, py as i64)
    };
    let grid = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(
        PREVIEW_SIDE,
        PREVIEW_SIDE,
        |x, y| image::Luma([sample(x, y)]),
    );
    let (lo, hi) = grid
        .pixels()
        .fold((u16::MAX, 0), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    let range = u32::from(hi.saturating_sub(lo)).max(1);
    RgbaImage::from_fn(PREVIEW_SIDE, PREVIEW_SIDE, |x, y| {
        let v = (u32::from(grid.get_pixel(x, y)[0] - lo) * 255 / range) as u8;
        image::Rgba([v, v, v, 255])
    })
}

/// Scales to 2048x2048 with Lanczos3 (an image already that size is kept as is) and encodes
/// the result as PNG.
pub fn to_2048(image: RgbaImage) -> Result<MapImage, image::ImageError> {
    let image = if image.dimensions() == (SIDE, SIDE) {
        image
    } else {
        image::imageops::resize(&image, SIDE, SIDE, FilterType::Lanczos3)
    };
    MapImage::encoded(image.into_raw())
}

/// 2048x2048 RGBA8 as PNG bytes.
fn encode_rgba(rgba: &[u8]) -> Result<Vec<u8>, image::ImageError> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::write_buffer_with_format(
        &mut out,
        rgba,
        SIDE,
        SIDE,
        image::ExtendedColorType::Rgba8,
        image::ImageFormat::Png,
    )?;
    Ok(out.into_inner())
}

/// The heightmap as a 16-bit grey PNG, which `decode_heightmap` reads back to the same samples.
pub fn encode_heightmap(heightmap: &Heightmap) -> Result<Vec<u8>, image::ImageError> {
    // The encoder takes 16-bit samples in native byte order.
    let bytes: Vec<u8> = heightmap
        .raw()
        .iter()
        .flat_map(|v| v.to_ne_bytes())
        .collect();
    let mut out = std::io::Cursor::new(Vec::new());
    image::write_buffer_with_format(
        &mut out,
        &bytes,
        heightmap.width(),
        heightmap.height(),
        image::ExtendedColorType::L16,
        image::ImageFormat::Png,
    )?;
    Ok(out.into_inner())
}

/// Writes `png` to `target` atomically, creating its folders.
pub fn save_map_image(target: &Path, png: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::atomic::write(target, png)
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
        // The bytes decode to this image, so they are kept instead of encoding again.
        return Ok((
            MapImage {
                rgba: rgba.into_raw(),
                png: bytes.into(),
            },
            None,
        ));
    }
    let scaled = image::imageops::resize(&rgba, SIDE, SIDE, FilterType::Triangle);
    Ok((MapImage::encoded(scaled.into_raw())?, Some((w, h))))
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

    /// Each pixel holds its own position: R, G the low bytes of x, y; B, A their high bytes.
    fn positions(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba([x as u8, y as u8, (x >> 8) as u8, (y >> 8) as u8])
        })
    }

    fn position(image: &RgbaImage, x: u32, y: u32) -> (u32, u32) {
        let [r, g, b, a] = image.get_pixel(x, y).0;
        (
            u32::from(r) | u32::from(b) << 8,
            u32::from(g) | u32::from(a) << 8,
        )
    }

    #[test]
    fn fs22_and_fs25_crop_the_centre_half() {
        for version in [GameVersion::FS22, GameVersion::FS25] {
            let crop = crop_for(version, positions(4096, 4096));
            assert_eq!(crop.dimensions(), (2048, 2048));
            assert_eq!(position(&crop, 0, 0), (1024, 1024));
            assert_eq!(position(&crop, 2047, 2047), (3071, 3071));
            // Already 2048x2048: kept as is.
            assert_eq!(to_2048(crop.clone()).expect("png").rgba, crop.into_raw());
        }
    }

    #[test]
    fn fs19_keeps_the_whole_image_and_scales_it() {
        let image = RgbaImage::from_pixel(1024, 1024, image::Rgba([10, 20, 30, 255]));
        let crop = crop_for(GameVersion::FS19, image.clone());
        assert_eq!(crop, image);
        let scaled = to_2048(crop).expect("png");
        assert_eq!(scaled.rgba.len(), (SIDE * SIDE * 4) as usize);
        assert!(scaled.rgba.chunks(4).all(|p| p == [10, 20, 30, 255]));
    }

    #[test]
    fn an_odd_size_crops_the_centre_half_and_scales_to_2048() {
        let crop = crop_for(GameVersion::FS25, positions(3000, 2000));
        assert_eq!(crop.dimensions(), (1500, 1000));
        assert_eq!(position(&crop, 0, 0), (750, 500));
        assert_eq!(position(&crop, 1499, 999), (2249, 1499));
        assert_eq!(
            to_2048(crop).expect("png").rgba.len(),
            (SIDE * SIDE * 4) as usize
        );
        // A tiny image still crops to at least one pixel.
        assert_eq!(
            crop_for(GameVersion::FS22, positions(1, 1)).dimensions(),
            (1, 1)
        );
    }

    #[test]
    fn a_saved_image_is_found_and_loaded_at_the_next_load() {
        let exe = temp("saved");
        let target = map_image_target(&exe, "M");
        assert_eq!(target, exe.join("mapImages").join("M").join("M.png"));
        let image = to_2048(RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))).expect("png");
        save_map_image(&target, &image.png).expect("saved");
        assert_eq!(find_map_image("M", Some(&exe), None), Some(target.clone()));
        assert_eq!(load_map_image(&target), (image, None));
        // Saving again replaces it and leaves no temp file behind.
        let other = to_2048(RgbaImage::from_pixel(4, 4, image::Rgba([9, 9, 9, 255]))).expect("png");
        save_map_image(&target, &other.png).expect("saved");
        assert_eq!(load_map_image(&target).0, other);
        let files = std::fs::read_dir(target.parent().expect("dir")).expect("list");
        assert_eq!(files.count(), 1);
        let _ = std::fs::remove_dir_all(exe);
    }

    /// Needs the gitignored `pda_map_H.dds` in the repo root; skipped when it is absent.
    #[test]
    fn fs25_sample_decodes_crops_and_scales() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../pda_map_H.dds");
        let Ok(bytes) = std::fs::read(path) else {
            eprintln!("skipped: {path} is absent");
            return;
        };
        let decoded = crate::dds::decode(&bytes).expect("decodes");
        let centre = *decoded.get_pixel(1024, 1024);
        let crop = crop_for(GameVersion::FS25, decoded);
        assert_eq!(crop.dimensions(), (SIDE, SIDE));
        let image = to_2048(crop).expect("png");
        assert_eq!(image.rgba.len(), (SIDE * SIDE * 4) as usize);
        assert_eq!(&image.rgba[..4], &centre.0);
    }

    #[test]
    fn previews_follow_the_crop_and_stretch_the_heightmap() {
        let image = positions(1024, 1024);
        let whole = preview(GameVersion::FS19, &image);
        let centre = preview(GameVersion::FS25, &image);
        assert_eq!(whole.dimensions(), (PREVIEW_SIDE, PREVIEW_SIDE));
        assert_eq!(centre.dimensions(), (PREVIEW_SIDE, PREVIEW_SIDE));
        assert!(position(&whole, 0, 0).0 < 4);
        assert!((254..262).contains(&position(&centre, 0, 0).0));
        let raw = (0..16).map(|i| 1000 + i * 10).collect();
        let heightmap = Heightmap::new(4, 4, raw).expect("heightmap");
        let grey = heightmap_preview(&heightmap);
        assert_eq!(grey.dimensions(), (PREVIEW_SIDE, PREVIEW_SIDE));
        assert_eq!(grey.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(grey.get_pixel(255, 255).0, [255, 255, 255, 255]);
        let flat = Heightmap::new(2, 2, vec![7; 4]).expect("flat");
        assert_eq!(heightmap_preview(&flat).get_pixel(9, 9).0, [0, 0, 0, 255]);
        let dir = Path::new("exe");
        assert_eq!(
            heightmap_target(dir, "M"),
            dir.join("mapImages").join("M").join("M_HeightMap.png")
        );
    }

    #[test]
    fn blank_is_the_full_size() {
        assert_eq!(blank().rgba.len(), (SIDE * SIDE * 4) as usize);
        assert_ne!(blank().rgba[0..4], [128, 128, 128, 128]);
    }

    #[test]
    fn every_image_carries_png_bytes_that_decode_to_it() {
        let right = png(SIDE, SIDE);
        let (kept, _) = decode_map_image(&right, "right").expect("decodes");
        assert_eq!(&*kept.png, &right[..], "a 2048 PNG is kept byte for byte");
        let (scaled, _) = decode_map_image(&png(100, 50), "small").expect("decodes");
        let blank = blank();
        assert_eq!(&*blank.png, BLANK_PNG);
        let converted = to_2048(RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])));
        for image in [kept, scaled, blank, converted.expect("png")] {
            let (again, notice) = decode_map_image(&image.png, "again").expect("decodes");
            assert_eq!((again.rgba, notice), (image.rgba, None));
        }
    }

    fn heightmap_round_trip(heightmap: &Heightmap) {
        let png = encode_heightmap(heightmap).expect("encodes");
        // IHDR: bit depth 16 (byte 24), colour type 0 grey (byte 25).
        assert_eq!((png[24], png[25]), (16, 0));
        assert_eq!(&decode_heightmap(&png).expect("decodes"), heightmap);
    }

    #[test]
    fn an_encoded_heightmap_reads_back_to_the_same_heights() {
        let raw = (0..12 * 7).map(|i: u32| (i * 797 % 65536) as u16).collect();
        heightmap_round_trip(&Heightmap::new(12, 7, raw).expect("heightmap"));
        heightmap_round_trip(&Heightmap::new(1, 1, vec![u16::MAX]).expect("heightmap"));
    }

    /// Needs the gitignored `dem.png` in the repo root; skipped when it is absent.
    #[test]
    fn the_fs25_dem_round_trips() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dem.png");
        if !Path::new(path).is_file() {
            eprintln!("skipped: {path} is absent");
            return;
        }
        heightmap_round_trip(&load_heightmap(Path::new(path)).expect("dem.png loads"));
    }
}
