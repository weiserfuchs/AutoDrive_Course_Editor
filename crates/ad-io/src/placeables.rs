//! The placed buildings of the savegame's `placeables.xml`, read only for display.
//!
//! Each top-level `placeable` with a `position` becomes one [`Placeable`]; the file is never
//! written. Preplaced map objects without a position are skipped.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlaceablesError {
    #[error("XML error: {0}")]
    Xml(String),
    #[error("the root element is not <placeables>")]
    NotPlaceables,
}

fn xml_err(e: impl std::fmt::Display) -> PlaceablesError {
    PlaceablesError::Xml(e.to_string())
}

/// A footprint part: four corners (x, z), in order around it.
pub type Quad = [(f64, f64); 4];

/// A building's areas: its outline (clear areas, else level areas, else test areas; often
/// a base plate around the building) and its indoor areas (the building under its roof).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Footprint {
    pub outline: Vec<Quad>,
    pub indoor: Vec<Quad>,
}

impl Footprint {
    pub fn is_empty(&self) -> bool {
        self.outline.is_empty() && self.indoor.is_empty()
    }
}

/// One placed building.
#[derive(Debug, Clone, PartialEq)]
pub struct Placeable {
    /// The stem of its XML file, e.g. "gasStation01"; the unique id when there is no file.
    pub name: String,
    /// The savegame's `uniqueId`.
    pub unique_id: String,
    /// The mod it comes from, if any.
    pub mod_name: Option<String>,
    /// The `filename` attribute as written, e.g. "$moddir$Mod/placeables/x.xml".
    pub file: String,
    /// World position (x, z); y is not needed on the map.
    pub x: f64,
    pub z: f64,
    /// Rotation about the vertical axis in degrees, as written.
    pub rotation_y: f64,
    /// The owning farm: 0 is the map, 1 the player's farm in a single-player game.
    pub farm_id: u32,
    /// Its footprint in the world, filled from the mods folder ([`crate::footprint`]);
    /// empty draws a square.
    pub footprint: Footprint,
}

/// The value of attribute `key` of `e`, unescaped.
fn attr(e: &BytesStart, key: &str) -> Result<Option<String>, PlaceablesError> {
    for a in e.attributes() {
        let a = a.map_err(xml_err)?;
        if a.key.as_ref() == key {
            let raw: &str = &a.value;
            return Ok(Some(
                quick_xml::escape::unescape(raw)
                    .map_err(xml_err)?
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

/// The numbers of a "x y z" attribute; `None` unless there are exactly three.
fn triple(text: &str) -> Option<(f64, f64, f64)> {
    let mut it = text.split_whitespace().map(str::parse::<f64>);
    match (it.next(), it.next(), it.next(), it.next()) {
        (Some(Ok(x)), Some(Ok(y)), Some(Ok(z)), None) => Some((x, y, z)),
        _ => None,
    }
}

/// The stem of a placeable's file name: the part after the last '/' without ".xml".
fn file_stem(file: &str) -> &str {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    base.strip_suffix(".xml").unwrap_or(base)
}

fn placeable(e: &BytesStart) -> Result<Option<Placeable>, PlaceablesError> {
    let Some((x, _, z)) = attr(e, "position")?.as_deref().and_then(triple) else {
        return Ok(None);
    };
    let rotation_y = attr(e, "rotation")?
        .as_deref()
        .and_then(triple)
        .map_or(0.0, |(_, y, _)| y);
    let file = attr(e, "filename")?.unwrap_or_default();
    let unique_id = attr(e, "uniqueId")?.unwrap_or_default();
    let name = match file_stem(&file) {
        "" => unique_id.clone(),
        stem => stem.to_owned(),
    };
    let farm_id = attr(e, "farmId")?
        .and_then(|f| f.trim().parse().ok())
        .unwrap_or(0);
    Ok(Some(Placeable {
        name,
        unique_id,
        mod_name: attr(e, "modName")?,
        file,
        x,
        z,
        rotation_y,
        farm_id,
        footprint: Footprint::default(),
    }))
}

/// Reads the placeables of a `placeables.xml`.
pub fn load(bytes: &[u8]) -> Result<Vec<Placeable>, PlaceablesError> {
    let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let mut reader = Reader::from_reader(bytes);
    let mut depth = 0usize;
    let mut out = Vec::new();
    loop {
        let event = reader.read_event().map_err(xml_err)?;
        let (e, empty) = match &event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match depth {
            0 if e.name().as_ref() != "placeables" => return Err(PlaceablesError::NotPlaceables),
            1 if e.name().as_ref() == "placeable" => out.extend(placeable(e)?),
            _ => {}
        }
        if !empty {
            depth += 1;
        }
    }
    if depth != 0 {
        return Err(xml_err("unexpected end of file"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn placeables_with_a_position_are_read() {
        let xml = "\u{feff}<?xml version=\"1.0\"?>\n<placeables version=\"1\">\n\
            <placeable isPreplaced=\"true\" uniqueId=\"pre\" farmId=\"0\"><x/></placeable>\n\
            <placeable modName=\"M\" filename=\"$moddir$M/p/gasStation01.xml\" uniqueId=\"Tanke\" \
              position=\"788.883 100.000 -847.008\" rotation=\"0.00 -76.00 0.00\" farmId=\"1\">\n\
              <windTurbine headRotation=\"3\"/><placeable position=\"1 2 3\"/>\n\
            </placeable>\n\
            <placeable filename=\"$data/a&amp;b.xml\" uniqueId=\"u\" position=\"1 2 3\"/>\n\
            <placeable uniqueId=\"onlyId\" position=\"4 5 6\" farmId=\"x\"/>\n\
            <placeable uniqueId=\"bad\" position=\"1 2\"/>\n\
            </placeables>";
        let got = load(xml.as_bytes()).expect("loads");
        assert_eq!(got.len(), 3);
        assert_eq!(
            got[0],
            Placeable {
                name: "gasStation01".to_owned(),
                unique_id: "Tanke".to_owned(),
                mod_name: Some("M".to_owned()),
                file: "$moddir$M/p/gasStation01.xml".to_owned(),
                x: 788.883,
                z: -847.008,
                rotation_y: -76.0,
                farm_id: 1,
                footprint: Footprint::default(),
            }
        );
        assert_eq!((got[1].name.as_str(), got[1].farm_id), ("a&b", 0));
        assert_eq!((got[1].rotation_y, got[1].mod_name.clone()), (0.0, None));
        assert_eq!((got[2].name.as_str(), got[2].farm_id), ("onlyId", 0));
    }

    #[test]
    fn a_wrong_root_or_broken_xml_is_an_error() {
        assert_eq!(load(b"<vehicles/>"), Err(PlaceablesError::NotPlaceables));
        assert!(load(b"<placeables><placeable position=\"1 2 3\">").is_err());
        assert_eq!(load(b"<placeables/>"), Ok(Vec::new()));
    }

    #[test]
    fn the_fixture_reads() {
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../placeables.xml"))
            .expect("fixture");
        let got = load(&bytes).expect("loads");
        // 118 placeables, one preplaced without a position.
        assert_eq!(got.len(), 117);
        assert!(got.iter().all(|p| p.x.abs() < 2048.0 && p.z.abs() < 2048.0));
        let tanke = got
            .iter()
            .find(|p| p.unique_id == "TankeNord")
            .expect("TankeNord");
        assert_eq!(tanke.name, "gasStation01");
        let own = got.iter().filter(|p| p.farm_id == 1).count();
        assert_eq!(own, 61);
    }
}
