# The Document lives in ad-app and owns the loaded config

**Context:** The architecture's Structural Seed puts `Document` in `ad-core/src/document.rs`, and AD-11 stores the game version on it. But the loaded config, `ad_io::config_xml::LoadedConfig`, holds both the `Network` and the original file bytes and layout that the byte-splice save needs (AD-6). `ad-core` cannot depend on `ad-io` (AD-1).

**Decision:** The user decided this on 2026-11-06, in story 1.7 ("Loading and saving run in the background and never corrupt the open document"), commit `9cf8f34`.
- `Document` lives in `crates/ad-app/src/document.rs`.
- It holds `LoadedConfig` (and through it the network, game version and read-only state), its `History`, the path and the epoch.
- `replace_document` is a free function in `crates/ad-app/src/jobs.rs`.

**Why:** The other option was to split `LoadedConfig` into a layout part and a `Network`, with `save(&layout, &network)`, so that a `Document` in `ad-core` could own the network. That would have rewritten `ad-io`'s API and every save caller and test just to match the seed. The plan's Open Questions record both options; the user chose `ad-app`.

**Consequence:** AD-4's rule that the network changes only through `History` is not enforced by types while `Document.config`, `LoadedConfig.network` and the `Network` mutators are `pub` (retrospective finding A4).
