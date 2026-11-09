//! The update check: the latest release of this fork on GitHub (AD-9: runs on a job).

use std::time::Duration;

use serde::Deserialize;

/// The fork's repository; never the upstream editor's.
pub const REPO_URL: &str = "https://github.com/weiserfuchs/AutoDrive_Course_Editor";
/// The fork's releases page (Help ▸ Version history).
pub const RELEASES_URL: &str = "https://github.com/weiserfuchs/AutoDrive_Course_Editor/releases";
/// The GitHub API endpoint of the latest release.
pub const LATEST_URL: &str =
    "https://api.github.com/repos/weiserfuchs/AutoDrive_Course_Editor/releases/latest";

/// Largest answer read; a release body is a few kB.
const MAX_BYTES: u64 = 1024 * 1024;

/// The latest release as GitHub describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Release {
    /// The tag, e.g. "v1.2.0".
    pub tag_name: String,
    /// The release page.
    pub html_url: String,
    /// The release notes (Markdown); may be missing.
    #[serde(default)]
    pub body: Option<String>,
}

/// The numbers of a version text: "v1.2.10" and "1.2.10-beta" give [1, 2, 10]; `None` when
/// it starts with no number.
pub fn version_numbers(text: &str) -> Option<Vec<u64>> {
    let text = text.trim().trim_start_matches(['v', 'V']);
    let core = text.split(['-', '+', ' ']).next()?;
    let numbers: Vec<u64> = core
        .split('.')
        .map_while(|part| part.parse().ok())
        .collect();
    (!numbers.is_empty()).then_some(numbers)
}

/// Whether `tag` names a newer version than `current` (missing parts count as 0, so
/// "1.2" equals "1.2.0"). A tag that is no version is never newer.
pub fn is_newer(tag: &str, current: &str) -> bool {
    let (Some(mut tag), Some(mut current)) = (version_numbers(tag), version_numbers(current))
    else {
        return false;
    };
    let len = tag.len().max(current.len());
    tag.resize(len, 0);
    current.resize(len, 0);
    tag > current
}

/// Reads the latest release from `url` (normally [`LATEST_URL`]). Any failure is an error
/// text for the log; the check is silent for the user.
pub fn fetch_latest(url: &str) -> Result<Release, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .user_agent("AutoDrive-Course-Editor")
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(format!("HTTP status {status}"));
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("unexpected answer: {e}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::download::test_server::{not_found, ok, serve};

    #[test]
    fn versions_compare_by_their_numbers() {
        assert_eq!(version_numbers("v1.2.10"), Some(vec![1, 2, 10]));
        assert_eq!(version_numbers("1.2.0-beta.1"), Some(vec![1, 2, 0]));
        assert_eq!(version_numbers("latest"), None);
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("v0.1.10", "0.1.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(!is_newer("v0.0.9", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }

    #[test]
    fn the_latest_release_is_read_and_a_bad_answer_is_an_error() {
        let json = br#"{"tag_name":"v0.2.0","html_url":"https://example/r","body":"- new \"menu\"","draft":false}"#;
        let (base, path) = serve(ok(json.to_vec(), "application/json"));
        let release = fetch_latest(&format!("{base}/latest")).expect("release");
        assert_eq!(path.recv().expect("path"), "/mapImages/latest");
        assert_eq!(
            release,
            Release {
                tag_name: "v0.2.0".to_owned(),
                html_url: "https://example/r".to_owned(),
                body: Some("- new \"menu\"".to_owned()),
            }
        );
        let (base, _) = serve(ok(br#"{"tag_name":"v1","html_url":"u"}"#.to_vec(), "x"));
        assert_eq!(fetch_latest(&base).expect("no body").body, None);
        let (base, _) = serve(not_found);
        assert_eq!(fetch_latest(&base), Err("HTTP status 404".to_owned()));
        let (base, _) = serve(ok(b"<html>".to_vec(), "text/html"));
        assert!(fetch_latest(&base).is_err_and(|e| e.starts_with("unexpected answer")));
        // Nothing listening: an error, no panic.
        assert!(fetch_latest("http://127.0.0.1:1/x").is_err());
    }
}
