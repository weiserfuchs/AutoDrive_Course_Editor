//! Map image lookup, decoding and scaling (AD-14). Runs on the load job, never on the UI thread.

use std::path::{Path, PathBuf};

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
    [exe_dir, config_dir].into_iter().flatten().find_map(|dir| {
        let path = dir.join("mapImages").join(name).join(format!("{name}.png"));
        path.is_file().then_some(path)
    })
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

    #[test]
    fn blank_is_the_full_size() {
        assert_eq!(blank().rgba.len(), (SIDE * SIDE * 4) as usize);
        assert_ne!(blank().rgba[0..4], [128, 128, 128, 128]);
    }
}
