use serde_json::json;

use super::*;

#[test]
fn reads_shortcut_preferences() {
    assert_eq!(shortcut_from(&json!({})), Some(DEFAULT_SHORTCUT.to_owned()));
    assert_eq!(
        shortcut_from(&json!({ "globalShortcut": "Command+Shift+K" })),
        Some("Command+Shift+K".to_owned())
    );
    assert_eq!(shortcut_from(&json!({ "globalShortcut": null })), None);
    assert_eq!(shortcut_from(&json!({ "globalShortcut": " " })), None);
    assert!(!background_from(&json!({})));
    assert!(background_from(&json!({ "runInBackground": true })));
}

#[test]
fn parses_recorded_shortcuts() {
    for shortcut in [
        DEFAULT_SHORTCUT,
        "Command+Shift+K",
        "Control+Alt+1",
        "Alt+Comma",
        "Shift+F5",
        "Super+ArrowUp",
    ] {
        assert!(parse_shortcut(shortcut).is_ok(), "{shortcut}");
    }
    assert_eq!(
        parse_shortcut("Alt+Nope").unwrap_err().code,
        "shortcut.invalid"
    );
    assert_eq!(parse_shortcut("").unwrap_err().code, "shortcut.invalid");
}

#[test]
fn localizes_tray_menu() {
    assert_eq!(tray_labels("zh-CN")[1], "命令面板");
    assert_eq!(tray_labels("en-US")[2], "Quit ToolForge");
}
