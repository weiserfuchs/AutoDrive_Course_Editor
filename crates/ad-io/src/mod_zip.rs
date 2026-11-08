//! Map mod zip reading for the image import (AD-11, AD-12): `modDesc.xml`, the map config,
//! the DDS map image and `dem.png`, each read by name into memory, never extracted.

use std::io::{Read, Seek};

use ad_core::heightmap::Heightmap;
use image::RgbaImage;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use zip::ZipArchive;
use zip::result::ZipError;

use crate::config_xml::GameVersion;
use crate::dds::{self, DdsError};
use crate::map_image;

/// Largest DDS entry read.
pub const MAX_DDS: u64 = 128 << 20;
/// Largest XML or PNG entry read.
pub const MAX_OTHER: u64 = 64 << 20;

const MOD_DESC: &str = "modDesc.xml";
const DEM: &str = "dem.png";

#[derive(Debug, thiserror::Error)]
pub enum ModZipError {
    #[error("not a readable zip ({0})")]
    Zip(String),
    #[error("{what} {name} is missing in the zip")]
    Missing { what: &'static str, name: String },
    #[error("{0}")]
    NoValue(String),
    #[error("entry name {0} is absolute or leaves the zip; refused")]
    Unsafe(String),
    #[error("{name} is larger than {limit} bytes; refused")]
    TooLarge { name: String, limit: u64 },
    #[error("{name} is not valid XML ({why})")]
    Xml { name: String, why: String },
    #[error("modDesc.xml descVersion {0} is not FS19 (40-53), FS22 (60-80) or FS25 (90+)")]
    Unsupported(String),
    #[error("{name}: {source}")]
    Dds { name: String, source: DdsError },
}

/// The heightmap `dem.png`: its bytes, saved unchanged, and their decode.
#[derive(Debug, Clone, PartialEq)]
pub struct Dem {
    pub png: Vec<u8>,
    pub heightmap: Heightmap,
}

/// What a mod zip gives the import.
#[derive(Debug, Clone, PartialEq)]
pub struct ModZip {
    /// The game the `descVersion` names.
    pub version: GameVersion,
    /// The `<map><title>` texts as (language, text), in file order.
    pub titles: Vec<(String, String)>,
    /// The decoded DDS, not yet cropped: the crop follows the game chosen at import.
    pub image: RgbaImage,
    pub dem: Option<Dem>,
    /// Why there is no heightmap, if there is none.
    pub notice: Option<String>,
}

/// The game of a `descVersion` (Java `FSUtils.getFSVersion`); `None` when unsupported.
pub fn game_for(desc_version: i64) -> Option<GameVersion> {
    match desc_version {
        40..=53 => Some(GameVersion::FS19),
        60..=80 => Some(GameVersion::FS22),
        90.. => Some(GameVersion::FS25),
        _ => None,
    }
}

/// The name the preview starts with: the open config's map name, else the English title,
/// else the first title, else empty.
pub fn default_name(titles: &[(String, String)], map_name: Option<&str>) -> String {
    if let Some(name) = map_name.filter(|n| !n.is_empty()) {
        return name.to_owned();
    }
    titles
        .iter()
        .find(|(lang, _)| lang == "en")
        .or(titles.first())
        .map(|(_, text)| text.clone())
        .unwrap_or_default()
}

/// The `mapImages` folder and file name for `name`: trimmed, spaces and dots replaced by
/// `_` (as Java). Empty names and characters a file name cannot hold are refused.
pub fn folder_name(name: &str) -> Result<String, &'static str> {
    let name = name.trim();
    if name.is_empty() {
        return Err("The map name is empty");
    }
    if name
        .chars()
        .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        return Err("The map name may not contain / \\ : * ? \" < > |");
    }
    Ok(name.replace([' ', '.'], "_"))
}

/// Reads the zip with the default entry caps.
pub fn read<R: Read + Seek>(reader: R) -> Result<ModZip, ModZipError> {
    read_with(reader, MAX_DDS, MAX_OTHER)
}

fn read_with<R: Read + Seek>(
    reader: R,
    max_dds: u64,
    max_other: u64,
) -> Result<ModZip, ModZipError> {
    let mut zip = ZipArchive::new(reader).map_err(|e| ModZipError::Zip(e.to_string()))?;
    let desc = entry(&mut zip, "modDesc.xml", MOD_DESC, max_other)?;
    let desc = parse_mod_desc(&desc).map_err(|why| ModZipError::Xml {
        name: MOD_DESC.to_owned(),
        why,
    })?;
    let text = desc
        .desc_version
        .ok_or_else(|| ModZipError::NoValue("modDesc.xml has no descVersion".to_owned()))?;
    let version = text
        .trim()
        .parse::<i64>()
        .ok()
        .and_then(game_for)
        .ok_or(ModZipError::Unsupported(text))?;
    let config_name = desc.config.ok_or_else(|| {
        ModZipError::NoValue("modDesc.xml names no map config (<map configFilename>)".to_owned())
    })?;
    let config = entry(&mut zip, "The map config", &config_name, max_other)?;
    let image_name = first_map_attr(&config, "imageFilename")
        .map_err(|why| ModZipError::Xml {
            name: config_name.clone(),
            why,
        })?
        .ok_or_else(|| {
            ModZipError::NoValue(format!(
                "{config_name} names no map image (<map imageFilename>)"
            ))
        })?;
    let dds_name = match image_name.strip_suffix(".png") {
        Some(stem) => format!("{stem}.dds"),
        None => image_name,
    };
    let bytes = entry(&mut zip, "The map image", &dds_name, max_dds)?;
    let image = dds::decode(&bytes).map_err(|source| ModZipError::Dds {
        name: dds_name.clone(),
        source,
    })?;
    drop(bytes);
    let (dem, notice) = read_dem(&mut zip, max_other)?;
    Ok(ModZip {
        version,
        titles: desc.titles,
        image,
        dem,
        notice,
    })
}

/// `dem.png` anywhere in the zip (Java `findFileInZip`). A missing or unreadable one gives
/// no heightmap and a notice; an unsafe or oversized one is an error.
fn read_dem<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    max: u64,
) -> Result<(Option<Dem>, Option<String>), ModZipError> {
    let found = zip
        .file_names()
        .filter_map(Result::ok)
        .find(|n| n.rsplit(['/', '\\']).next() == Some(DEM))
        .map(|n| n.into_owned());
    let Some(name) = found else {
        return Ok((
            None,
            Some("The zip has no dem.png; only the map image is imported".to_owned()),
        ));
    };
    let png = entry(zip, "The heightmap", &name, max)?;
    match map_image::decode_heightmap(&png) {
        Ok(heightmap) => Ok((Some(Dem { png, heightmap }), None)),
        Err(why) => Ok((
            None,
            Some(format!(
                "{name} could not be read ({why}); only the map image is imported"
            )),
        )),
    }
}

/// `name` with `\` as `/`; absolute names, drive letters and `..` are refused.
fn safe_name(name: &str) -> Result<String, ModZipError> {
    let normal = name.replace('\\', "/");
    let bytes = normal.as_bytes();
    let drive = bytes.len() >= 2 && bytes[1] == b':';
    if normal.starts_with('/') || drive || normal.split('/').any(|part| part == "..") {
        return Err(ModZipError::Unsafe(name.to_owned()));
    }
    Ok(normal)
}

/// Reads the entry `name` into memory, at most `max` bytes, whatever its header claims.
fn entry<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    what: &'static str,
    name: &str,
    max: u64,
) -> Result<Vec<u8>, ModZipError> {
    let normal = safe_name(name)?;
    let too_large = || ModZipError::TooLarge {
        name: name.to_owned(),
        limit: max,
    };
    let mut file = match zip.by_name(&normal) {
        Ok(file) => file,
        Err(ZipError::FileNotFound) => {
            return Err(ModZipError::Missing {
                what,
                name: name.to_owned(),
            });
        }
        Err(e) => return Err(ModZipError::Zip(format!("{name}: {e}"))),
    };
    if file.size() > max {
        return Err(too_large());
    }
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.by_ref()
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| ModZipError::Zip(format!("{name}: {e}")))?;
    if bytes.len() as u64 > max {
        return Err(too_large());
    }
    Ok(bytes)
}

#[derive(Default)]
struct ModDesc {
    desc_version: Option<String>,
    config: Option<String>,
    titles: Vec<(String, String)>,
}

fn reader(bytes: &[u8]) -> Reader<&[u8]> {
    Reader::from_reader(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
}

fn attr(e: &BytesStart, name: &str) -> Result<Option<String>, String> {
    e.try_get_attribute(name)
        .map_err(|e| e.to_string())?
        .map(|a| {
            a.normalized_value(XmlVersion::Implicit1_0)
                .map(|v| v.into_owned())
                .map_err(|e| e.to_string())
        })
        .transpose()
}

/// The root `descVersion`, and of the first `<map>` its `configFilename` and the children
/// of its first `<title>`.
fn parse_mod_desc(bytes: &[u8]) -> Result<ModDesc, String> {
    let mut reader = reader(bytes);
    let mut out = ModDesc::default();
    let mut depth = 0_usize;
    // Depths of the first map and its first title while open; `done` once each closed.
    let (mut map, mut title, mut lang): (Option<usize>, Option<usize>, Option<(String, String)>) =
        (None, None, None);
    let (mut map_done, mut title_done) = (false, false);
    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        let (e, empty) = match &event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::Text(t) => {
                if let Some((_, text)) = &mut lang {
                    text.push_str(&t.xml_content(XmlVersion::Implicit1_0));
                }
                continue;
            }
            Event::CData(t) => {
                if let Some((_, text)) = &mut lang {
                    text.push_str(&t.xml_content(XmlVersion::Implicit1_0));
                }
                continue;
            }
            Event::GeneralRef(r) => {
                if let Some((_, text)) = &mut lang {
                    match r.resolve_char_ref().map_err(|e| e.to_string())? {
                        Some(c) => text.push(c),
                        None => {
                            let n = r.borrow().into_inner();
                            let s = quick_xml::escape::resolve_predefined_entity(&n)
                                .ok_or_else(|| format!("unknown entity &{n};"))?;
                            text.push_str(s);
                        }
                    }
                }
                continue;
            }
            Event::End(_) => {
                if lang.is_some() && title.is_some_and(|t| depth == t + 2) {
                    if let Some((l, text)) = lang.take() {
                        out.titles.push((l, text.trim().to_owned()));
                    }
                } else if title.is_some_and(|t| depth == t + 1) {
                    title = None;
                    title_done = true;
                } else if map.is_some_and(|m| depth == m + 1) {
                    map = None;
                    map_done = true;
                }
                depth -= 1;
                continue;
            }
            Event::Eof if depth == 0 => return Ok(out),
            Event::Eof => return Err("unexpected end of file".to_owned()),
            _ => continue,
        };
        let name = e.name();
        let name: &str = name.as_ref();
        if depth == 0 && name == "modDesc" {
            out.desc_version = attr(e, "descVersion")?;
        } else if name == "map" && map.is_none() && !map_done {
            out.config = attr(e, "configFilename")?;
            map = (!empty).then_some(depth);
            map_done = empty;
        } else if name == "title" && map.is_some_and(|m| depth == m + 1) && !title_done {
            title = (!empty).then_some(depth);
            title_done = empty;
        } else if title.is_some_and(|t| depth == t + 1) {
            if empty {
                out.titles.push((name.to_owned(), String::new()));
            } else {
                lang = Some((name.to_owned(), String::new()));
            }
        }
        if !empty {
            depth += 1;
        }
    }
}

/// The attribute `name` of the first `<map>` element.
fn first_map_attr(bytes: &[u8], name: &str) -> Result<Option<String>, String> {
    let mut reader = reader(bytes);
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "map" => {
                return attr(&e, name);
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    /// An 8x8 A8R8G8B8 DDS filled with `rgba`.
    fn test_dds([r, g, b, a]: [u8; 4]) -> Vec<u8> {
        let mut file = Vec::new();
        let mut put = |v: u32| file.extend_from_slice(&v.to_le_bytes());
        put(u32::from_le_bytes(*b"DDS "));
        put(124);
        put(0x1007);
        put(8);
        put(8);
        (0..14).for_each(|_| put(0));
        put(32);
        put(0x41);
        put(0);
        put(32);
        [0xff_0000, 0xff00, 0xff, 0xff00_0000]
            .into_iter()
            .for_each(&mut put);
        put(0x1000);
        (0..4).for_each(|_| put(0));
        (0..64).for_each(|_| file.extend_from_slice(&[b, g, r, a]));
        file
    }

    fn dem() -> Vec<u8> {
        let img = image::ImageBuffer::<image::Luma<u16>, _>::from_pixel(4, 4, image::Luma([25600]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .expect("encode");
        out.into_inner()
    }

    fn mod_desc(version: &str, config: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<modDesc descVersion=\"{version}\">\
             <title><en>Mod title</en></title>\
             <maps><map id=\"M\" configFilename=\"{config}\">\
             <title><en>Foo Bar</en><de>Foo</de></title></map></maps></modDesc>"
        )
    }

    fn map_xml(image: &str) -> String {
        format!("<map width=\"2048\" imageFilename=\"{image}\"><foo/></map>")
    }

    fn build(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("start");
            writer.write_all(bytes).expect("write");
        }
        writer.finish().expect("finish").into_inner()
    }

    /// A complete FS25 zip, with `dem.png` unless `with_dem` is false.
    fn fs25(with_dem: bool) -> Vec<u8> {
        let desc = mod_desc("92", "maps/map.xml");
        let map = map_xml("maps/overview.png");
        let dds = test_dds([1, 2, 3, 255]);
        let dem = dem();
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("modDesc.xml", desc.as_bytes()),
            ("maps/map.xml", map.as_bytes()),
            ("maps/overview.dds", &dds),
        ];
        if with_dem {
            entries.push(("maps/data/dem.png", &dem));
        }
        build(&entries)
    }

    fn read_bytes(bytes: Vec<u8>) -> Result<ModZip, ModZipError> {
        read(Cursor::new(bytes))
    }

    #[test]
    fn a_complete_zip_gives_version_titles_image_and_dem() {
        let zip = read_bytes(fs25(true)).expect("reads");
        assert_eq!(zip.version, GameVersion::FS25);
        assert_eq!(
            zip.titles,
            [
                ("en".to_owned(), "Foo Bar".to_owned()),
                ("de".to_owned(), "Foo".to_owned())
            ]
        );
        assert_eq!(zip.image.dimensions(), (8, 8));
        assert_eq!(zip.image.get_pixel(0, 0).0, [1, 2, 3, 255]);
        let dem = zip.dem.expect("dem");
        assert_eq!(dem.png, super::tests::dem());
        assert_eq!(dem.heightmap.sample_raw(0, 0), 25600);
        assert_eq!(zip.notice, None);
    }

    #[test]
    fn versions_map_to_games_and_old_ones_are_refused() {
        assert_eq!(game_for(45), Some(GameVersion::FS19));
        assert_eq!(game_for(70), Some(GameVersion::FS22));
        assert_eq!(game_for(95), Some(GameVersion::FS25));
        assert_eq!(game_for(30), None);
        assert_eq!(game_for(55), None);
        for (text, game) in [
            ("45", GameVersion::FS19),
            ("70", GameVersion::FS22),
            ("95", GameVersion::FS25),
        ] {
            let desc = mod_desc(text, "map.xml");
            let map = map_xml("o.png");
            let dds = test_dds([0; 4]);
            let bytes = build(&[
                ("modDesc.xml", desc.as_bytes()),
                ("map.xml", map.as_bytes()),
                ("o.dds", &dds),
            ]);
            assert_eq!(read_bytes(bytes).expect("reads").version, game);
        }
        for text in ["30", "abc"] {
            let desc = mod_desc(text, "map.xml");
            let error = read_bytes(build(&[("modDesc.xml", desc.as_bytes())])).expect_err(text);
            assert!(matches!(error, ModZipError::Unsupported(_)), "{error}");
            assert!(error.to_string().contains(text), "{error}");
        }
    }

    #[test]
    fn the_name_defaults_to_the_map_name_then_english_then_first_title() {
        let titles = vec![
            ("de".to_owned(), "Foo".to_owned()),
            ("en".to_owned(), "Foo Bar".to_owned()),
        ];
        assert_eq!(default_name(&titles, None), "Foo Bar");
        assert_eq!(
            folder_name(&default_name(&titles, None)),
            Ok("Foo_Bar".to_owned())
        );
        assert_eq!(default_name(&titles, Some("NF_Marsch")), "NF_Marsch");
        assert_eq!(default_name(&titles, Some("")), "Foo Bar");
        assert_eq!(default_name(&titles[..1], None), "Foo");
        assert_eq!(default_name(&[], None), "");
        assert_eq!(folder_name(" My.Map v1.2 "), Ok("My_Map_v1_2".to_owned()));
        assert!(folder_name("  ").is_err());
        assert!(folder_name("a/b").is_err() && folder_name("..\\x").is_err());
        // Dots become underscores, so `..` cannot leave the folder.
        assert_eq!(folder_name(".."), Ok("__".to_owned()));
    }

    #[test]
    fn no_dem_gives_the_image_alone_with_a_notice() {
        let zip = read_bytes(fs25(false)).expect("reads");
        assert!(zip.dem.is_none());
        assert!(zip.notice.expect("notice").contains("dem.png"));
        // An unreadable dem.png is treated the same.
        let desc = mod_desc("92", "map.xml");
        let map = map_xml("o.png");
        let dds = test_dds([0; 4]);
        let bytes = build(&[
            ("modDesc.xml", desc.as_bytes()),
            ("map.xml", map.as_bytes()),
            ("o.dds", &dds),
            ("data/dem.png", b"not a png"),
        ]);
        let zip = read_bytes(bytes).expect("reads");
        assert!(zip.dem.is_none());
        assert!(zip.notice.expect("notice").contains("data/dem.png"));
    }

    #[test]
    fn hostile_names_and_oversized_entries_are_refused() {
        let dds = test_dds([0; 4]);
        for image in [
            "../x.png",
            "/x.png",
            "\\x.png",
            "C:/x.png",
            "maps/../../x.png",
        ] {
            let desc = mod_desc("92", "map.xml");
            let map = map_xml(image);
            let dds_name = image.replace(".png", ".dds");
            let bytes = build(&[
                ("modDesc.xml", desc.as_bytes()),
                ("map.xml", map.as_bytes()),
                (&dds_name, &dds),
            ]);
            let error = read_bytes(bytes).expect_err(image);
            assert!(matches!(error, ModZipError::Unsafe(_)), "{image}: {error}");
        }
        let desc = mod_desc("92", "../map.xml");
        let error = read_bytes(build(&[("modDesc.xml", desc.as_bytes())])).expect_err("config");
        assert!(matches!(error, ModZipError::Unsafe(_)), "{error}");
        // A hostile dem.png entry.
        let desc = mod_desc("92", "map.xml");
        let map = map_xml("o.png");
        let dem = dem();
        let bytes = build(&[
            ("modDesc.xml", desc.as_bytes()),
            ("map.xml", map.as_bytes()),
            ("o.dds", &dds),
            ("../dem.png", &dem),
        ]);
        let error = read_bytes(bytes).expect_err("dem");
        assert!(matches!(error, ModZipError::Unsafe(_)), "{error}");
        // Oversized: the DDS over its cap, then the XML over its cap.
        let error = read_with(Cursor::new(fs25(true)), 100, MAX_OTHER).expect_err("dds");
        assert!(
            matches!(&error, ModZipError::TooLarge { name, .. } if name == "maps/overview.dds"),
            "{error}"
        );
        let error = read_with(Cursor::new(fs25(true)), MAX_DDS, 50).expect_err("xml");
        assert!(
            matches!(&error, ModZipError::TooLarge { name, .. } if name == "modDesc.xml"),
            "{error}"
        );
    }

    #[test]
    fn missing_parts_are_errors_naming_them() {
        let error = read_bytes(build(&[("other.txt", b"x")])).expect_err("modDesc");
        assert!(error.to_string().contains("modDesc.xml"), "{error}");
        let desc = mod_desc("92", "maps/map.xml");
        let error = read_bytes(build(&[("modDesc.xml", desc.as_bytes())])).expect_err("config");
        assert!(error.to_string().contains("maps/map.xml"), "{error}");
        let map = map_xml("maps/overview.png");
        let bytes = build(&[
            ("modDesc.xml", desc.as_bytes()),
            ("maps/map.xml", map.as_bytes()),
        ]);
        let error = read_bytes(bytes).expect_err("dds");
        assert!(error.to_string().contains("maps/overview.dds"), "{error}");
        // Missing attributes, malformed XML and a non-zip.
        let bare = "<modDesc descVersion=\"92\"><maps><map id=\"M\"/></maps></modDesc>";
        let error = read_bytes(build(&[("modDesc.xml", bare.as_bytes())])).expect_err("attr");
        assert!(error.to_string().contains("configFilename"), "{error}");
        let error = read_bytes(build(&[(
            "modDesc.xml",
            b"<modDesc descVersion=\"92\"><maps>",
        )]))
        .expect_err("truncated");
        assert!(matches!(error, ModZipError::Xml { .. }), "{error}");
        let none = "<modDesc><maps/></modDesc>";
        let error = read_bytes(build(&[("modDesc.xml", none.as_bytes())])).expect_err("version");
        assert!(error.to_string().contains("descVersion"), "{error}");
        let error = read_bytes(b"not a zip".to_vec()).expect_err("zip");
        assert!(matches!(error, ModZipError::Zip(_)), "{error}");
    }
}
