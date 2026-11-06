//! The marker and warning icons: the Java editor's SVGs, rasterised once per run and
//! uploaded as textures (Java `NodeDrawThread` 110-139, `MapPanel` 204-207).

use eframe::egui;
use resvg::{tiny_skia, usvg};

/// Marker icon edge in px (Java 25).
pub const MARKER_PX: u32 = 25;
/// Edge in px of the parking and warning icons (Java 20).
pub const SMALL_PX: u32 = 20;

const MARKER_SVG: &[u8] = include_bytes!("../assets/icons/marker_icon.svg");
const PARKING_SVG: &[u8] = include_bytes!("../assets/icons/parking_icon.svg");
const WARNING_SVG: &[u8] = include_bytes!("../assets/icons/warning_icon.svg");
const WARNING_Y_SVG: &[u8] = include_bytes!("../assets/icons/warning_y_icon.svg");

/// Rasterises `svg` into a `px` x `px` image. `None` (logged) when the SVG cannot be
/// parsed or the size is zero.
pub fn rasterise(svg: &[u8], px: u32) -> Option<egui::ColorImage> {
    static FONTS: std::sync::OnceLock<std::sync::Arc<usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    let fontdb = FONTS.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        // The parking icon names Lato, which is not installed everywhere: use a sans font.
        db.set_sans_serif_family("Arial");
        std::sync::Arc::new(db)
    });
    let options = usvg::Options {
        font_family: "Arial".to_owned(),
        fontdb: fontdb.clone(),
        ..Default::default()
    };
    let tree = match usvg::Tree::from_data(svg, &options) {
        Ok(tree) => tree,
        Err(e) => {
            tracing::error!("icon SVG does not parse: {e}");
            return None;
        }
    };
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    let size = tree.size();
    let transform =
        tiny_skia::Transform::from_scale(px as f32 / size.width(), px as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(egui::ColorImage::from_rgba_premultiplied(
        [px as usize, px as usize],
        pixmap.data(),
    ))
}

/// The icon textures. A texture is `None` when its SVG failed to rasterise.
pub struct Icons {
    pub marker: Option<egui::TextureHandle>,
    /// Prepared for parked vehicles (a later epic); no node draws it yet.
    #[allow(dead_code)]
    pub parking: Option<egui::TextureHandle>,
    /// Prepared for the overlap scan (a later epic); no node draws it yet.
    #[allow(dead_code)]
    pub warning: Option<egui::TextureHandle>,
    pub warning_y: Option<egui::TextureHandle>,
}

impl Icons {
    /// Rasterises all icons and uploads them.
    pub fn load(ctx: &egui::Context) -> Self {
        let upload = |name: &str, svg: &[u8], px: u32| {
            rasterise(svg, px)
                .map(|image| ctx.load_texture(name, image, egui::TextureOptions::LINEAR))
        };
        Self {
            marker: upload("icon_marker", MARKER_SVG, MARKER_PX),
            parking: upload("icon_parking", PARKING_SVG, SMALL_PX),
            warning: upload("icon_warning", WARNING_SVG, SMALL_PX),
            warning_y: upload("icon_warning_y", WARNING_Y_SVG, SMALL_PX),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn opaque_pixels(image: &egui::ColorImage) -> usize {
        image.pixels.iter().filter(|p| p.a() > 0).count()
    }

    #[test]
    fn icons_rasterise_at_their_size() {
        for (svg, px) in [
            (MARKER_SVG, MARKER_PX),
            (PARKING_SVG, SMALL_PX),
            (WARNING_SVG, SMALL_PX),
            (WARNING_Y_SVG, SMALL_PX),
        ] {
            let image = rasterise(svg, px).expect("rasterises");
            assert_eq!(image.size, [px as usize, px as usize]);
            assert!(opaque_pixels(&image) > 0);
        }
    }

    #[test]
    fn parking_icon_shows_its_letter() {
        let image = rasterise(PARKING_SVG, 20).expect("rasterises");
        // The blue square is dark; the "P" (#fafafa) makes light pixels near the centre.
        let light = (6..14)
            .flat_map(|y| (6..14).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = image.pixels[y * 20 + x];
                p.a() > 200 && p.r() > 200 && p.g() > 200 && p.b() > 200
            })
            .count();
        assert!(light >= 3, "light pixels in the centre: {light}");
    }

    #[test]
    fn broken_svg_gives_none() {
        assert!(rasterise(b"not svg", 20).is_none());
        assert!(rasterise(MARKER_SVG, 0).is_none());
    }
}
