//! Fails when `ad-core` gains a dependency that is not on the allowlist (AD-1).

/// Crates `ad-core` may depend on. Later stories extend this (for example `rstar`).
const ALLOWED: &[&str] = &["rstar", "thiserror"];

/// Crates that must never appear in `ad-core`.
const FORBIDDEN: &[&str] = &[
    "egui",
    "eframe",
    "quick-xml",
    "image",
    "image_dds",
    "zip",
    "ureq",
    "rfd",
    "resvg",
];

const DEP_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

/// Removes a trailing `# comment` that is not inside a quoted string.
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '#') => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Splits a dotted TOML key, keeping quoted segments such as `'cfg(windows)'` whole.
fn split_key(key: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut quote = None;
    for c in key.chars() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '.') => parts.push(String::new()),
            _ => {
                if let Some(last) = parts.last_mut() {
                    last.push(c);
                }
            }
        }
    }
    parts.into_iter().map(|p| p.trim().to_string()).collect()
}

/// The value of `package = "..."` inside `line`, if any (a renamed dependency).
fn package_rename(line: &str) -> Option<String> {
    let rest = &line[line.find("package")? + "package".len()..];
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// Every crate name `manifest` depends on, in any dependency table, including
/// build, dev and target-specific tables and the real name behind `package = ...`.
fn dependency_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    // `Some(true)`: keys are dependency names; `Some(false)`: keys are options of one dependency.
    let mut section = None;
    for raw in manifest.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let path = split_key(header.trim_end_matches(']'));
            // [dependencies] or [target.<cfg>.dependencies]; [dependencies.foo] names `foo`.
            let dep_at = path.iter().position(|p| DEP_TABLES.contains(&p.as_str()));
            let in_target = dep_at.is_some_and(|i| i == 0 || (i == 2 && path[0] == "target"));
            section = match (in_target, dep_at) {
                (true, Some(i)) if path.len() == i + 1 => Some(true),
                (true, Some(i)) if path.len() == i + 2 => {
                    names.push(path[i + 1].clone());
                    Some(false)
                }
                _ => None,
            };
            continue;
        }
        let Some(is_list) = section else { continue };
        if let Some(real) = package_rename(line) {
            names.push(real);
        }
        if is_list
            && let Some((key, _)) = line.split_once('=')
            && let Some(name) = split_key(key).into_iter().next()
        {
            names.push(name);
        }
    }
    names
}

/// One message per dependency that breaks the boundary, naming the crate.
fn violations(manifest: &str) -> Vec<String> {
    dependency_names(manifest)
        .into_iter()
        .filter_map(|name| {
            if FORBIDDEN.contains(&name.as_str()) {
                Some(format!(
                    "ad-core must not depend on `{name}` (AD-1: core has no UI or I/O crates)"
                ))
            } else if !ALLOWED.contains(&name.as_str()) {
                Some(format!(
                    "ad-core depends on `{name}`, which is not on the allowlist in dependency_boundary.rs"
                ))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn ad_core_has_only_allowed_dependencies() -> std::io::Result<()> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest = std::fs::read_to_string(path)?;
    let found = violations(&manifest);
    assert!(found.is_empty(), "{}", found.join("\n"));
    Ok(())
}

#[test]
fn parser_sees_build_target_renamed_and_commented_dependencies() {
    let manifest = "\
[build-dependencies] # tools
zip = \"9\"
[target.'cfg(windows)'.dependencies]
ureq = \"3\"
[dependencies]
gui = { package = \"eframe\", version = \"0.37\" }
[dependencies.pic]
package = \"image\"
[package.metadata.dependencies]
not_a_dep = 1
";
    let names = dependency_names(manifest);
    for crate_name in ["zip", "ureq", "eframe", "image"] {
        assert!(
            names.iter().any(|n| n == crate_name),
            "{crate_name} missing in {names:?}"
        );
    }
    assert!(!names.iter().any(|n| n == "not_a_dep"), "{names:?}");
}

#[test]
fn forbidden_dependency_is_reported_by_name() {
    let found = violations("[dependencies]\nquick-xml = \"0.42\"\n");
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("`quick-xml`"), "{}", found[0]);
}

#[test]
fn parser_detects_forbidden_dependency() {
    let manifest =
        "[dependencies]\nquick-xml = \"0.42\"\n[dev-dependencies.eframe]\nversion = \"1\"\n";
    assert_eq!(dependency_names(manifest), ["quick-xml", "eframe"]);
}
