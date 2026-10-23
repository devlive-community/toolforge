use super::*;

#[test]
fn names_follow_the_tauri_sidecar_convention() {
    assert_eq!(
        sidecar_name("aarch64-apple-darwin"),
        "tfp-aarch64-apple-darwin"
    );
    assert_eq!(
        sidecar_name("x86_64-pc-windows-msvc"),
        "tfp-x86_64-pc-windows-msvc.exe"
    );
}

#[test]
fn reads_the_host_target_from_rustc() {
    let output = "rustc 1.97.0 (abc 2026-09-01)\nbinary: rustc\nhost: x86_64-unknown-linux-gnu\nrelease: 1.97.0\n";
    assert_eq!(
        parse_host(output).as_deref(),
        Some("x86_64-unknown-linux-gnu")
    );
    assert_eq!(parse_host("nothing"), None);
}

/// 发布配置里的 externalBin 必须指向 sidecar 目录下的 tfp
#[test]
fn release_config_bundles_the_sidecar() {
    let config = include_str!("../../../apps/desktop/src-tauri/tauri.release.conf.json");
    let config: serde_json::Value = serde_json::from_str(config).unwrap();
    let dir = BINARIES_DIR.trim_start_matches("apps/desktop/src-tauri/");
    assert_eq!(config["bundle"]["externalBin"][0], format!("{dir}/tfp"));
}
