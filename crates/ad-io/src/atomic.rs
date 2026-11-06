//! Atomic file replacement: temp file in the target's directory, fsync, rename (AD-6).

use std::path::Path;

/// Writes `bytes` to `.{file name}.{pid}.tmp` beside `target`, flushes it to disk and
/// renames it over `target`. On any error the target is untouched and the temp file is
/// removed.
pub(crate) fn write(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = match target.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let name = target.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name")
    })?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = dir.join(tmp_name);
    let result = std::fs::File::create(&tmp)
        .and_then(|mut file| {
            std::io::Write::write_all(&mut file, bytes)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&tmp, target));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
