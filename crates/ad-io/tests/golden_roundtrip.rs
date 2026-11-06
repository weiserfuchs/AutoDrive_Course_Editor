use std::path::PathBuf;

use ad_io::config_xml::{GameVersion, load, save};

#[test]
fn fixture_roundtrips_byte_identically() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../AutoDrive_config.xml");
    let original = std::fs::read(&fixture)?;
    let cfg = load(&original)?;
    assert_eq!(cfg.game_version(), GameVersion::FS25);
    assert!(cfg.read_only().is_none());
    assert_eq!(cfg.network.len(), 35652);
    assert_eq!(cfg.network.markers().len(), 223);

    let out = std::env::temp_dir().join(format!("ad_golden_{}.xml", std::process::id()));
    std::fs::write(&out, save(&cfg)?)?;
    let written = std::fs::read(&out)?;
    std::fs::remove_file(&out)?;
    assert!(written == original, "saved bytes differ from the fixture");
    Ok(())
}
