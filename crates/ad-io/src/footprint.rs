//! Footprints of placed buildings: the clear, level or test areas of a placeable's XML as
//! its outline and its indoor areas as the building itself, resolved through the node tree
//! of its `.i3d` and placed with the savegame's position and rotation. Read only, from the mods folder (`<mods>/<mod>.zip` or `<mods>/<mod>/`).
//!
//! Only the rotation about the vertical axis is applied; tilted nodes are rare in area
//! paths and would only skew the outline.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use zip::ZipArchive;

use crate::placeables::Placeable;

pub use crate::placeables::{Footprint, Quad};

/// Largest file read from a mod; placeable XMLs and text i3d files are far smaller.
const MAX_BYTES: u64 = 32 * 1024 * 1024;

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// A 2D affine map of the ground plane: (x, z) -> (a x + b z + tx, c x + d z + tz).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    tz: f64,
}

impl Affine {
    /// Translation, then rotation by `ry` degrees about the vertical axis, then scale, as
    /// the engine places a child point: x' = x cos + z sin, z' = -x sin + z cos.
    fn node(t: (f64, f64), ry_deg: f64, s: (f64, f64)) -> Self {
        let (sin, cos) = ry_deg.to_radians().sin_cos();
        Self {
            a: cos * s.0,
            b: sin * s.1,
            c: -sin * s.0,
            d: cos * s.1,
            tx: t.0,
            tz: t.1,
        }
    }

    fn apply(&self, (x, z): (f64, f64)) -> (f64, f64) {
        (
            self.a * x + self.b * z + self.tx,
            self.c * x + self.d * z + self.tz,
        )
    }

    /// `self` after `inner`: a point goes through `inner` first.
    fn then(&self, inner: &Self) -> Self {
        Self {
            a: self.a * inner.a + self.b * inner.c,
            b: self.a * inner.b + self.b * inner.d,
            c: self.c * inner.a + self.d * inner.c,
            d: self.c * inner.b + self.d * inner.d,
            tx: self.a * inner.tx + self.b * inner.tz + self.tx,
            tz: self.c * inner.tx + self.d * inner.tz + self.tz,
        }
    }
}

/// A node of an i3d scene: its local transform and its children.
#[derive(Debug, Default)]
struct SceneNode {
    local: Option<Affine>,
    children: Vec<usize>,
}

/// The node tree under `<Scene>`.
#[derive(Debug, Default)]
struct Scene {
    nodes: Vec<SceneNode>,
    roots: Vec<usize>,
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.as_ref() == key).then(|| {
            let raw: &str = &a.value;
            quick_xml::escape::unescape(raw)
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| raw.to_owned())
        })
    })
}

fn numbers(text: Option<String>, default: [f64; 3]) -> [f64; 3] {
    let Some(text) = text else {
        return default;
    };
    let mut out = default;
    for (slot, part) in out.iter_mut().zip(text.split_whitespace()) {
        if let Ok(v) = part.parse() {
            *slot = v;
        }
    }
    out
}

fn local_of(e: &BytesStart) -> Affine {
    let [tx, _, tz] = numbers(attr(e, "translation"), [0.0; 3]);
    let [_, ry, _] = numbers(attr(e, "rotation"), [0.0; 3]);
    let [sx, _, sz] = numbers(attr(e, "scale"), [1.0; 3]);
    Affine::node((tx, tz), ry, (sx, sz))
}

fn parse_scene(bytes: &[u8]) -> Result<Scene, String> {
    let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let mut reader = Reader::from_reader(bytes);
    let mut scene = Scene::default();
    // Open nodes, innermost last; `in_scene` while inside <Scene>.
    let mut open: Vec<usize> = Vec::new();
    let mut in_scene = false;
    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        let (e, empty) = match &event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                if e.name().as_ref() == "Scene" {
                    in_scene = false;
                } else if in_scene {
                    open.pop();
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        if !in_scene {
            in_scene = e.name().as_ref() == "Scene" && !empty;
            continue;
        }
        let id = scene.nodes.len();
        scene.nodes.push(SceneNode {
            local: Some(local_of(e)),
            children: Vec::new(),
        });
        match open.last() {
            Some(&parent) => scene.nodes[parent].children.push(id),
            None => scene.roots.push(id),
        }
        if !empty {
            open.push(id);
        }
    }
    Ok(scene)
}

impl Scene {
    /// The placement of the node at index path `path` ("0>1|0|1": component 0, then child
    /// 1, then its child 0, ...) in the placeable's frame.
    fn resolve(&self, path: &str) -> Option<Affine> {
        let (root, rest) = path.split_once('>')?;
        let mut id = *self.roots.get(root.trim().parse::<usize>().ok()?)?;
        let mut m = self.nodes[id].local?;
        for part in rest.split('|').map(str::trim).filter(|p| !p.is_empty()) {
            id = *self.nodes[id].children.get(part.parse::<usize>().ok()?)?;
            m = m.then(&self.nodes[id].local?);
        }
        Some(m)
    }
}

/// What a placeable XML says about its footprint.
#[derive(Debug, Default, PartialEq)]
struct PlaceableXml {
    i3d: Option<String>,
    /// (start, width, height) of each clear area.
    clear: Vec<[String; 3]>,
    /// (start, width, height) of each level area.
    level: Vec<[String; 3]>,
    /// (start, end) of each test area.
    test: Vec<[String; 2]>,
    /// (start, width, height) of each indoor area.
    indoor: Vec<[String; 3]>,
    /// i3d mapping id -> node path.
    mapping: HashMap<String, String>,
}

fn parse_placeable(bytes: &[u8]) -> Result<PlaceableXml, String> {
    let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let mut reader = Reader::from_reader(bytes);
    let mut out = PlaceableXml::default();
    let mut stack: Vec<String> = Vec::new();
    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.name().as_ref().to_owned();
                let three = |e: &BytesStart| {
                    Some([
                        attr(e, "startNode")?,
                        attr(e, "widthNode")?,
                        attr(e, "heightNode")?,
                    ])
                };
                match name.as_str() {
                    "clearArea" => out.clear.extend(three(e)),
                    "levelArea" => out.level.extend(three(e)),
                    "indoorArea" => out.indoor.extend(three(e)),
                    "testArea" => {
                        if let (Some(s), Some(t)) = (attr(e, "startNode"), attr(e, "endNode")) {
                            out.test.push([s, t]);
                        }
                    }
                    "i3dMapping" => {
                        if let (Some(id), Some(node)) = (attr(e, "id"), attr(e, "node")) {
                            out.mapping.insert(id, node);
                        }
                    }
                    _ => {}
                }
                if matches!(event, Event::Start(_)) {
                    stack.push(name);
                }
            }
            Event::Text(t) => {
                let in_base_file = stack.len() >= 2
                    && stack[stack.len() - 1] == "filename"
                    && stack[stack.len() - 2] == "base";
                if in_base_file && out.i3d.is_none() {
                    let raw: &str = t;
                    let text = raw.trim();
                    if !text.is_empty() {
                        out.i3d = Some(text.to_owned());
                    }
                }
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

/// The footprint of a placeable in its own frame, from its XML and its i3d (read through
/// `read_i3d` with the XML's i3d file name); no areas give an empty one.
fn local_footprint(
    placeable_xml: &[u8],
    read_i3d: impl FnOnce(&str) -> Option<Vec<u8>>,
) -> Result<Footprint, String> {
    let xml = parse_placeable(placeable_xml)?;
    if xml.indoor.is_empty() && xml.clear.is_empty() && xml.level.is_empty() && xml.test.is_empty()
    {
        return Ok(Footprint::default());
    }
    let i3d_name = xml.i3d.as_deref().ok_or("no i3d file named")?;
    let i3d = read_i3d(i3d_name).ok_or_else(|| format!("{i3d_name} not found"))?;
    let scene = parse_scene(&i3d)?;
    let node = |id: &str| {
        let path = xml.mapping.get(id).map_or(id, String::as_str);
        scene.resolve(path)
    };
    let origin = |m: Affine| (m.tx, m.tz);
    let parallelogram = |[s, w, h]: &[String; 3]| -> Option<Quad> {
        let (s, w, h) = (origin(node(s)?), origin(node(w)?), origin(node(h)?));
        Some([s, w, (w.0 + h.0 - s.0, w.1 + h.1 - s.1), h])
    };
    let boxed = |[s, e]: &[String; 2]| -> Option<Quad> {
        let start = node(s)?;
        let end = origin(node(e)?);
        let s0 = origin(start);
        // The box is spanned along the start node's own axes.
        let unit = |(x, z): (f64, f64)| {
            let len = x.hypot(z);
            (len > 0.0).then(|| (x / len, z / len))
        };
        let u = unit((start.a, start.c))?;
        let v = unit((start.b, start.d))?;
        let (dx, dz) = (end.0 - s0.0, end.1 - s0.1);
        let (p, q) = (dx * u.0 + dz * u.1, dx * v.0 + dz * v.1);
        let pu = (s0.0 + p * u.0, s0.1 + p * u.1);
        Some([
            s0,
            pu,
            (pu.0 + q * v.0, pu.1 + q * v.1),
            (s0.0 + q * v.0, s0.1 + q * v.1),
        ])
    };
    let outline = if !xml.clear.is_empty() {
        xml.clear.iter().filter_map(parallelogram).collect()
    } else if !xml.level.is_empty() {
        xml.level.iter().filter_map(parallelogram).collect()
    } else {
        xml.test.iter().filter_map(boxed).collect()
    };
    Ok(Footprint {
        outline,
        indoor: xml.indoor.iter().filter_map(parallelogram).collect(),
    })
}

/// `quads` in the world, for a placeable at (x, z) turned by `ry_deg` degrees.
fn place(quads: &[Quad], x: f64, z: f64, ry_deg: f64) -> Vec<Quad> {
    let m = Affine::node((x, z), ry_deg, (1.0, 1.0));
    quads.iter().map(|q| q.map(|p| m.apply(p))).collect()
}

/// The mod and the path inside it of a savegame file name "$moddir$Mod/dir/file.xml".
pub fn mod_path(file: &str) -> Option<(&str, &str)> {
    let rest = file.strip_prefix("$moddir$")?;
    let (name, path) = rest.split_once('/')?;
    (!name.is_empty() && !path.is_empty()).then_some((name, path))
}

/// An open mod zip and its entry names by lower-case name.
type OpenZip = (ZipArchive<File>, HashMap<String, String>);

/// Reads files of the mods in one folder, each a zip or an unpacked folder.
pub struct ModFiles {
    dir: PathBuf,
    /// Per mod: its open zip with a lower-case name index, or none.
    zips: HashMap<String, Option<OpenZip>>,
}

impl ModFiles {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            zips: HashMap::new(),
        }
    }

    /// The bytes of `path` inside mod `name`, if they can be read.
    pub fn read(&mut self, name: &str, path: &str) -> Option<Vec<u8>> {
        let path = path.replace('\\', "/");
        if path.split('/').any(|p| p == "..") {
            return None;
        }
        let folder = self.dir.join(name);
        if folder.is_dir() {
            let file = folder.join(&path);
            let len = std::fs::metadata(&file).ok()?.len();
            return (len <= MAX_BYTES)
                .then(|| std::fs::read(file).ok())
                .flatten();
        }
        let dir = &self.dir;
        let slot = self.zips.entry(name.to_owned()).or_insert_with(|| {
            let zip = ZipArchive::new(File::open(dir.join(format!("{name}.zip"))).ok()?).ok()?;
            let index = zip
                .file_names()
                .filter_map(Result::ok)
                .map(|n| (n.to_lowercase(), n.into_owned()))
                .collect();
            Some((zip, index))
        });
        let (zip, index) = slot.as_mut()?;
        let real = index.get(&path.to_lowercase())?.clone();
        let entry = zip.by_name(&real).ok()?;
        if entry.size() > MAX_BYTES {
            return None;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.take(MAX_BYTES).read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }
}

/// The world footprint of each of `list`, in order; empty where none can be read (no mod,
/// file or areas: the view then draws the square). Each
/// placeable file is read once.
pub fn footprints(list: &[Placeable], mods_dir: &Path) -> Vec<Footprint> {
    let mut files = ModFiles::new(mods_dir);
    let mut local: HashMap<String, Footprint> = HashMap::new();
    list.iter()
        .map(|p| {
            let print = local.entry(p.file.clone()).or_insert_with(|| {
                let Some((name, path)) = mod_path(&p.file) else {
                    return Footprint::default();
                };
                let Some(xml) = files.read(name, path) else {
                    return Footprint::default();
                };
                // Mod files name their i3d relative to the mod; try beside the XML too.
                let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
                let read_i3d = |i3d: &str| {
                    if i3d.starts_with('$') {
                        return None;
                    }
                    files
                        .read(name, i3d)
                        .or_else(|| files.read(name, &format!("{dir}/{i3d}")))
                };
                local_footprint(&xml, read_i3d).unwrap_or_default()
            });
            Footprint {
                outline: place(&print.outline, p.x, p.z, p.rotation_y),
                indoor: place(&print.indoor, p.x, p.z, p.rotation_y),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    const I3D: &str = r#"<?xml version="1.0"?>
<i3D><Files/><Scene>
  <TransformGroup name="root" nodeId="1">
    <TransformGroup name="visual" translation="5 0 5"/>
    <TransformGroup name="areas" translation="1 0 2" rotation="0 90 0">
      <TransformGroup name="clearStart" translation="-2 0 -3">
        <TransformGroup name="clearWidth" translation="4 0 0"/>
        <TransformGroup name="clearHeight" translation="0 0 6"/>
      </TransformGroup>
      <TransformGroup name="testStart" translation="-2 0 -3">
        <TransformGroup name="testEnd" translation="4 0 6"/>
      </TransformGroup>
    </TransformGroup>
  </TransformGroup>
  <TransformGroup name="second" scale="2 1 2">
    <TransformGroup name="s" translation="1 0 1"/>
  </TransformGroup>
</Scene></i3D>"#;

    fn xml(areas: &str) -> String {
        format!(
            "<placeable type=\"x\">\n<base><filename>dir/model.i3d</filename></base>\n\
             {areas}\n<i3dMappings>\n\
             <i3dMapping id=\"cs\" node=\"0>1|0\"/>\n\
             <i3dMapping id=\"cw\" node=\"0>1|0|0\"/>\n\
             <i3dMapping id=\"ch\" node=\"0>1|0|1\"/>\n\
             <i3dMapping id=\"ts\" node=\"0>1|1\"/>\n\
             <i3dMapping id=\"te\" node=\"0>1|1|0\"/>\n\
             </i3dMappings>\n</placeable>"
        )
    }

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    fn read(name: &str) -> Option<Vec<u8>> {
        (name == "dir/model.i3d").then(|| I3D.as_bytes().to_vec())
    }

    #[test]
    fn node_paths_resolve_through_the_tree() {
        let scene = parse_scene(I3D.as_bytes()).expect("scene");
        let at = |p: &str| scene.resolve(p).map(|m| (m.tx, m.tz));
        assert_eq!(at("0>"), Some((0.0, 0.0)));
        assert_eq!(at("0>0"), Some((5.0, 5.0)));
        // areas at (1, 2) turned 90°: local (-2, -3) -> (1 + -3, 2 + 2).
        assert!(close(at("0>1|0").expect("start"), (-2.0, 4.0)));
        // Scale of the second component doubles its child's offset.
        assert!(close(at("1>0").expect("scaled"), (2.0, 2.0)));
        assert_eq!(at("0>7"), None);
        assert_eq!(at("2>"), None);
        assert_eq!(at("nonsense"), None);
    }

    #[test]
    fn clear_areas_win_then_level_then_test_areas() {
        let clear = "<clearAreas><clearArea startNode=\"cs\" widthNode=\"cw\" heightNode=\"ch\"/></clearAreas>";
        let test = "<placement><testAreas><testArea startNode=\"ts\" endNode=\"te\"/></testAreas></placement>";
        let level = "<leveling><levelAreas><levelArea startNode=\"0>1|0\" widthNode=\"0>1|0|0\" heightNode=\"0>1|0|1\"/></levelAreas></leveling>";
        let both = local_footprint(xml(&format!("{test}{clear}")).as_bytes(), read).expect("ok");
        assert_eq!(both.outline.len(), 1);
        assert!(both.indoor.is_empty());
        // The clear area: start (-2, 4); width +4 along local x (turned: z -4); height +6
        // along local z (turned: x +6).
        let want = [(-2.0, 4.0), (-2.0, 0.0), (4.0, 0.0), (4.0, 4.0)];
        for (got, want) in both.outline[0].iter().zip(want) {
            assert!(close(*got, want), "{:?}", both.outline[0]);
        }
        // Direct node paths in a level area give the same rectangle.
        let lv = local_footprint(xml(level).as_bytes(), read).expect("ok");
        assert_eq!(lv.outline.len(), 1);
        for (got, want) in lv.outline[0].iter().zip(want) {
            assert!(close(*got, want), "{:?}", lv.outline[0]);
        }
        // The test area spans the same box along the start node's axes.
        let t = local_footprint(xml(test).as_bytes(), read).expect("ok");
        for (got, want) in t.outline[0].iter().zip(want) {
            assert!(close(*got, want), "{:?}", t.outline[0]);
        }
    }

    #[test]
    fn indoor_areas_are_kept_beside_the_outline() {
        let clear = "<clearAreas><clearArea startNode=\"0>1|1\" widthNode=\"0>1|1|0\" heightNode=\"0>1|1|0\"/></clearAreas>";
        let indoor = "<indoorAreas><indoorArea startNode=\"cs\" widthNode=\"cw\" heightNode=\"ch\"/></indoorAreas>";
        let want = [(-2.0, 4.0), (-2.0, 0.0), (4.0, 0.0), (4.0, 4.0)];
        let both = local_footprint(xml(&format!("{clear}{indoor}")).as_bytes(), read).expect("ok");
        assert_eq!((both.outline.len(), both.indoor.len()), (1, 1));
        for (got, want) in both.indoor[0].iter().zip(want) {
            assert!(close(*got, want), "{:?}", both.indoor[0]);
        }
        // Indoor areas alone: no outline, the building still has a footprint.
        let only = local_footprint(xml(indoor).as_bytes(), read).expect("ok");
        assert!(only.outline.is_empty());
        assert_eq!(only.indoor.len(), 1);
        assert!(!only.is_empty());
    }

    #[test]
    fn no_areas_or_a_missing_i3d_give_no_footprint() {
        assert_eq!(
            local_footprint(xml("").as_bytes(), read),
            Ok(Footprint::default())
        );
        let clear = "<clearAreas><clearArea startNode=\"cs\" widthNode=\"cw\" heightNode=\"ch\"/></clearAreas>";
        assert!(local_footprint(xml(clear).as_bytes(), |_| None).is_err());
        let indoor = "<indoorAreas><indoorArea startNode=\"cs\" widthNode=\"cw\" heightNode=\"ch\"/></indoorAreas>";
        assert!(local_footprint(xml(indoor).as_bytes(), |_| None).is_err());
        // An unknown mapping id drops only that area.
        let bad = "<clearAreas><clearArea startNode=\"zz\" widthNode=\"cw\" heightNode=\"ch\"/></clearAreas>";
        assert!(
            local_footprint(xml(bad).as_bytes(), read)
                .expect("ok")
                .is_empty()
        );
    }

    #[test]
    fn a_footprint_is_placed_with_the_savegame_rotation() {
        let quad: Quad = [(0.0, 0.0), (1.0, 0.0), (1.0, 2.0), (0.0, 2.0)];
        let placed = place(&[quad], 100.0, 50.0, 90.0);
        // Local x turns to -z and local z to +x, as for i3d nodes.
        let want = [(100.0, 50.0), (100.0, 49.0), (102.0, 49.0), (102.0, 50.0)];
        for (got, want) in placed[0].iter().zip(want) {
            assert!(close(*got, want), "{:?}", placed[0]);
        }
    }

    #[test]
    fn mod_paths_split_the_mod_name() {
        assert_eq!(
            mod_path("$moddir$FS25_X/placeables/a.xml"),
            Some(("FS25_X", "placeables/a.xml"))
        );
        assert_eq!(mod_path("$data/placeables/a.xml"), None);
        assert_eq!(mod_path("$moddir$/a.xml"), None);
    }

    #[test]
    fn files_are_read_from_a_zip_or_a_folder() {
        let dir = std::env::temp_dir().join("ad_io_footprint_mods");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Unpacked/dir")).expect("dir");
        std::fs::write(dir.join("Unpacked/dir/a.xml"), "folder").expect("write");
        let mut zip = zip::ZipWriter::new(File::create(dir.join("Zipped.zip")).expect("zip"));
        zip.start_file("Dir/A.xml", zip::write::SimpleFileOptions::default())
            .expect("entry");
        std::io::Write::write_all(&mut zip, b"zipped").expect("write");
        zip.finish().expect("finish");
        let mut files = ModFiles::new(&dir);
        assert_eq!(
            files.read("Unpacked", "dir/a.xml"),
            Some(b"folder".to_vec())
        );
        // Zip names match ignoring case, as the game does on Windows.
        assert_eq!(files.read("Zipped", "dir/a.xml"), Some(b"zipped".to_vec()));
        assert_eq!(files.read("Zipped", "missing.xml"), None);
        assert_eq!(files.read("Missing", "a.xml"), None);
        assert_eq!(files.read("Unpacked", "../Zipped.zip"), None);
    }

    /// Reads footprints from the user's FS25 mods folder for the fixture. Run by hand:
    /// `cargo test -p ad-io -- --ignored real_mods_footprints --nocapture`
    #[test]
    #[ignore = "needs the FS25 mods of the fixture"]
    fn real_mods_footprints() {
        let mods = std::env::var_os("USERPROFILE")
            .map(|home| PathBuf::from(home).join("Documents/My Games/FarmingSimulator2025/mods"))
            .expect("home");
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../placeables.xml"))
            .expect("fixture");
        let list = crate::placeables::load(&bytes).expect("loads");
        let start = std::time::Instant::now();
        let prints = footprints(&list, &mods);
        println!(
            "{} of {} placeables have a footprint, {} with indoor areas, read in {:.0} ms",
            prints.iter().filter(|p| !p.is_empty()).count(),
            list.len(),
            prints.iter().filter(|p| !p.indoor.is_empty()).count(),
            start.elapsed().as_secs_f64() * 1000.0
        );
        let size = |quads: &[Quad]| {
            quads.first().map(|q| {
                let d = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
                (d(q[0], q[1]).round(), d(q[0], q[3]).round())
            })
        };
        for (p, print) in list.iter().zip(&prints).take(40) {
            println!(
                "{:<40} outline {:?}, indoor {:?}",
                p.name,
                size(&print.outline),
                size(&print.indoor)
            );
        }
    }
}
