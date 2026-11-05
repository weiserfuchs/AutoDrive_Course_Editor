# Review: versions and reality-check (lens)

Date: 2026-11-05. Checked: `cargo search --limit 1` for every Stack crate plus `toml`, rustc 1.99.1 local, crates.io API (features, deps, rust_version), the repo fixture (`AutoDrive_config.xml` starts with EF BB BF, then `<?xml ... encoding="utf-8" standalone=...`), and Java `DDSReader.java`.

## Verdict
Every version number in the Stack table matches crates.io today, but several FIT claims were asserted rather than checked, and four crate-level gaps (feature flags, duplicate resvg, BOM handling in quick-xml, DDS format coverage) would bite in the first stories.

## Version table (all confirmed current via cargo search)
eframe/egui_extras 0.37.0, quick-xml 0.42.0, image 0.25.10, image_dds 0.7.2, zip 9.4.0, slotmap 1.1.1, rstar 0.14.0, rfd 0.17.2, ureq 3.5.1, resvg 0.49.0, serde 1.0.229, thiserror 2.0.21, anyhow 1.0.104, tracing 0.1.44, tracing-subscriber 0.3.23, directories 6.0.0. rustc 1.99.1 satisfies eframe 0.37 (rust_version 1.95) and resvg 0.49 (1.85); edition 2024 needs 1.85, fine. Note image_dds 0.7.2 dates from 2025-03 (no newer release; stale but not abandoned).

## Findings

### F1 (High) Stack names crates without feature flags; live defaults are not what the spine assumes
- image_dds 0.7.2 default features are `ddsfile, image, encode, strum`; `encode` pulls `intel_tex_2` (ISPC). "encode feature off" requires `default-features = false, features = ["ddsfile","image"]`. Not stated.
- eframe 0.37 default features include `wgpu` and not `glow`. The spine says nothing about the renderer. wgpu affects binary size, build time, and GPU compatibility on old machines. Decide and pin: `default-features = false` with an explicit list (`wgpu` or `glow`, `accesskit`, `default_fonts`, `wayland`/`x11` irrelevant on Windows).
- zip 9.4 default features pull aes-crypto, bzip2, lzma, ppmd, zstd, xz, etc. Mod zips only need deflate (maybe stored). Use `default-features = false, features = ["deflate"]` to cut build time and C dependencies.
- ureq 3 defaults: rustls + gzip. Timeouts are off by default; AD-9 must name them (`timeout_connect`, `timeout_recv_response`/`timeout_global` on the `Agent` config in v3; v2 API `.timeout_read` no longer applies).
- Fix: add a "features" column to the Stack table and a workspace `[workspace.dependencies]` rule.

### F2 (High) quick-xml does not by itself give a byte-identical round-trip including the BOM
- quick-xml's reader strips a leading UTF-8 BOM silently; it is not emitted as an event. The writer will not restore it. The fixture does start with EF BB BF. AD-6 says "BOM ... follow the source file" but the mechanism is unconfirmed.
- Events also normalise: `Empty` vs `Start/End` is preserved, but writing back via `Writer` re-serialises attributes/quotes unless raw bytes are kept; trim_text must stay off; line endings are not normalised on read but entity text must not be unescaped/re-escaped.
- Fix (simpler and more robust): do not re-emit events. Use quick-xml only to locate byte ranges (`buffer_position()` before/after the `<waypoints>` and `<mapmarker>` elements) and splice: output = original bytes[..start] + generated block + original bytes[end..]. BOM, declaration, indent, CRLF then survive by construction. Record the BOM/indent/EOL for the generated blocks by sniffing. State this in AD-6 and keep the golden test. The Stack row can stay.

### F3 (Medium) image_dds coverage vs what Java DDSReader supports
- image_dds decodes BC1/BC2/BC3 (and BC4-7) with pure-Rust `bcdec_rs` once `encode` is off, so DXT1/3/5 on Windows x64 is fine and needs no C/ISPC toolchain. Confirmed from the crate's dependency list (bcdec_rs required, intel_tex_2 optional).
- Not confirmed: legacy DXT2/DXT4 (premultiplied FourCC, handled in Java `DDSReader` lines 147-176) and old-style uncompressed masks (24-bit RGB, A1R5G5B5, etc.). image_dds covers RGBA8, BGRA8, R8 and similar; 24-bit and 16-bit masked formats are doubtful.
- Fix: before committing, test against real FS19/FS22/FS25 mod zip DDS samples; if any are exotic, add a small fallback decoder in ad-io (header + masks, ~100 lines) or restrict support and say so in AD-11. Add a note in the spine that DDS formats outside BC1-3/RGBA8 are Deferred or covered by fallback.

### F4 (Medium) egui_extras 0.37 and resvg 0.49 conflict; egui_extras need is unjustified
- egui_extras's `svg` feature depends on resvg `^0.45.1`. Using resvg 0.49 directly alongside it builds two resvg/usvg/tiny-skia trees.
- Is egui_extras needed at all? Only for `Table`/`TableBuilder`, `install_image_loaders` (svg/image loaders), `DatePicker`. Marker list and scan-result tables could use it, but nothing in the spine names a requirement.
- Fix: either (a) drop direct resvg and use egui_extras with `svg` + `image` features (`egui::Image::new(include_image!(...))`), or (b) drop egui_extras and rasterise icons once with resvg 0.49 at startup (or pre-render PNGs at build time). Recommend (a) if tables are wanted, else (b) with no egui_extras. Pin whichever; do not list both.

### F5 (Low-Medium) Unchecked performance and fit claims
- rstar 0.14: insert and remove are O(log n); remove requires an object equal to the stored one (use `GeomWithData<[f64;2], NodeId>`), so a move is remove(old pos)+insert(new pos). 10^5 points is comfortable; bulk-load with `RTree::bulk_load` at file load rather than 10^5 inserts. Document this in AD-5 (the Network must keep the old position to remove).
- egui 2048x2048 RGBA texture is 16 MB; fine for wgpu (default max 8192) and glow (>=4096 nearly everywhere). Painter per-frame cost is dominated by shape tessellation, not the texture; for ~10^5 visible edges use batched `Shape::LineSegment` vectors or a `Mesh`, and rely on the already-listed LOD rule. AD-8 "dropped CPU copy" is fine (`ColorImage` moved into `load_texture`).
- zip 9.x reads mod zips fine (`ZipArchive::new(Cursor/File)`); 9.x is a recent major, API check at first story.
- Missing from Stack but implied: `toml` (1.2.0 current) and serde `derive` feature for settings; `tracing-appender` (current, updated 2026-04) for the "rolling log file" in Logging, otherwise tracing-subscriber cannot roll files; `rfd` default backend is fine on Windows.

## Not checked
No web docs were fetched for egui painter performance; the Painter/rstar notes above come from crate metadata and API knowledge, not benchmarks. Recommend a one-day spike on the fixture (1.8 MB, node count unknown) before AD-5/AD-8 are locked.
