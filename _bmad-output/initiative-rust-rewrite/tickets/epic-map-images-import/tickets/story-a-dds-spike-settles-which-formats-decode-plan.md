---
title: 'A DDS spike settles which formats decode'
type: 'spike'
ticket: '1'
created: '2026-11-08'
status: 'built'
baseline_revision: '5b613cd'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The architecture defers DDS format coverage beyond BC1–BC3 and RGBA8 to a spike before this epic. Java's `DDSReader` decodes DXT1–DXT5 and ten uncompressed formats (A1R5G5B5, X1R5G5B5, A4R4G4B4, X4R4G4B4, R5G6B5, R8G8B8, A8B8G8R8, X8B8G8R8, A8R8G8B8, X8R8G8B8). Which of these does `image_dds` decode, and which do the games use?

**Approach:**
- Add `image_dds` (decode only, per the Stack) to ad-io behind one function `dds::decode(bytes) -> Result<RgbaImage, ImageError>` that reads the top mip level.
- Decode the user's FS25 `pda_map_H.dds` (DXT1, 4096², gitignored in the repo root) in a test that is skipped when the file is absent.
- Generate a tiny sample (e.g. 8×8, two known colours) for each Java format: a hand-built 128-byte header (plus DX10 header where needed) and pixel/block bytes written in the test, and assert the decoded pixels.
- For each format record: decodes with `image_dds` / needs a small decoder / out (not used by the games). Write a small decoder only for a format that fails and is plausibly used by FS19–FS25 map images (DXT1/DXT5 and 32-bit RGBA are the known ones); others are refused with a clear error.
- Record the result table here and replace the architecture's deferred DDS item with the outcome (moved from Deferred into the AD-14/Stack text or a short "settled" line).

## Boundaries & Constraints

**Always:** decoding never panics on a truncated or hostile file (AD-12): a short header, a size that does not match the data, or an absurd dimension (cap at 16384²) returns an error.

**Never:** no UI, no import flow (story 7.2); no encoder features; no committing of the user's DDS.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| FS25 sample | `pda_map_H.dds` | 4096² RGBA, a few probed pixels match a reference decode | skipped if absent |
| DXT1..DXT5 | generated 8×8 blocks | expected colours (alpha where the format has it) | none |
| Uncompressed | each of the ten masks | expected colours | none |
| Unsupported | a format neither decodes | error naming the format | error |
| Truncated | header only / half the data | error | error |
| Absurd size | 100000² header | error before allocating | error |

</frozen-after-approval>

## Code Map

- `Cargo.toml` (workspace) and `crates/ad-io/Cargo.toml` -- `image_dds` 0.7.2, `default-features = false`, features `ddsfile`, `image`.
- `crates/ad-io/src/dds.rs` (new) -- `decode`, size cap, errors; tests with generated samples and the optional FS25 sample.
- `_bmad-output/.../architecture-rust-rewrite.md` -- the Deferred DDS line settled.

## Tasks & Acceptance

**Execution:**
- [x] dependency and `dds::decode`
- [x] tests for every matrix row; result table in the Implementation Notes
- [x] small decoder only if a needed format fails; architecture updated

**Acceptance Criteria:**
- Given the FS25 sample and the generated samples, when the tests run, then every format the games use decodes, every other format is refused cleanly, and the architecture names the settled coverage.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass (the FS25 sample test runs here, the file is present)

## Implementation Notes

`ad_io::dds::decode(bytes) -> Result<RgbaImage, DdsError>` reads the file with `ddsfile`, refuses a width or height of 0 or above 16384 and any cube map, array or volume texture, and then decodes mip 0 with `image_dds::image_from_dds`. The error is a `DdsError` (thiserror) rather than `image::ImageError`: header, size, unsupported format (named), data.

Result (tests in `crates/ad-io/src/dds.rs`, 8×8 samples built in the test):

| Format | Result |
| --- | --- |
| DXT1 (incl. 1-bit alpha) | decodes with `image_dds` |
| DXT2, DXT3 | decodes with `image_dds` (DXT2 not un-premultiplied, as in Java) |
| DXT4, DXT5 | decodes with `image_dds` (DXT4 not un-premultiplied, as in Java) |
| A8R8G8B8, A8B8G8R8, R8G8B8, A4R4G4B4, A1R5G5B5 | decodes with `image_dds` |
| X8R8G8B8, X8B8G8R8 | own decoder (byte reorder, alpha 255); `image_dds` refuses them |
| X1R5G5B5, X4R4G4B4, R5G6B5 | refused ("unsupported DDS format R5G6B5" etc.); 16-bit, not used for map images |
| unknown FourCC / masks | refused, naming the FourCC, D3D/DXGI format or the masks |

- FS25 `pda_map_H.dds` (DXT1, 4096²) decodes; 68 probed pixels match an independent BC1 reference decode in the test within ±1.
- Truncated input (short header, header only, half the data, every prefix of each sample) and a 100000² header give an error; none panics. The size check runs before any pixel buffer is allocated.
- New crates: `image_dds`, `ddsfile`, `bcdec_rs`, `enum-primitive-derive`, `syn` 1.x (proc-macro of `enum-primitive-derive`). Stack features unchanged.
- FS19 and FS22 samples were not available; DXT1/DXT5 and 32-bit RGBA cover what they are known to use.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Accepted: the error type is `DdsError`, not `ImageError` (named formats need their own variant); X8R8G8B8/X8B8G8R8 count as the 32-bit RGBA family; cube maps, arrays and volumes are refused. Noted for later: CI has no AD-1 dependency check yet (only fmt, clippy, test).
