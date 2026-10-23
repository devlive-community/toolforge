use super::*;

#[test]
fn identifier_matches_tauri_config() {
    let config = include_str!("../../../apps/desktop/src-tauri/tauri.conf.json");
    let config: serde_json::Value = serde_json::from_str(config).unwrap();
    assert_eq!(config["identifier"], APP_IDENTIFIER);
}

#[test]
fn data_dir_ends_with_identifier() {
    if let Some(dir) = app_data_dir() {
        assert!(dir.ends_with(APP_IDENTIFIER));
    }
}
