use super::*;
use crate::reg::Memory;

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tfp-context-menu-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn read_utf16(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).unwrap();
    let units: Vec<u16> = bytes[2..]
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).unwrap()
}

#[test]
fn writes_an_undo_file_before_each_change() {
    let reg = Memory::default();
    reg.put(
        Hive::User,
        "Software\\Classes\\*\\shell\\Code",
        "",
        reg::Value::Str("Code".into()),
    );
    let dir = dir("set");
    let args = SetArgs {
        target: Target {
            hive: Hive::User,
            path: "Software\\Classes\\*\\shell\\Code".into(),
            kind: Kind::Command,
            clsid: None,
        },
        enabled: false,
        name: Some("Code".into()),
    };
    let file = set_entry(&reg, &args, &dir).unwrap();
    assert!(
        reg.value(
            Hive::User,
            "Software\\Classes\\*\\shell\\Code",
            "LegacyDisable"
        )
        .is_some()
    );
    let script = read_utf16(&file);
    assert!(
        script.contains(
            "[HKEY_CURRENT_USER\\Software\\Classes\\*\\shell\\Code]\r\n\"LegacyDisable\"=-"
        ),
        "{script}"
    );

    let classic = set_classic(&reg, true, &dir).unwrap();
    assert!(read_utf16(&classic).contains(
        "[-HKEY_CURRENT_USER\\Software\\Classes\\CLSID\\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}]"
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn removes_the_undo_file_when_the_change_fails() {
    let reg = Memory {
        read_only_machine: true,
        ..Memory::default()
    };
    let dir = dir("fail");
    let args = SetArgs {
        target: Target {
            hive: Hive::Machine,
            path: String::new(),
            kind: Kind::Handler,
            clsid: Some("{B41DB860-64E4-11D2-9906-E49FADC173CA}".into()),
        },
        enabled: false,
        name: None,
    };
    assert_eq!(
        set_entry(&reg, &args, &dir).unwrap_err().code,
        "ctxmenu.needs_admin"
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(not(windows))]
#[test]
fn only_runs_on_windows() {
    let tool = ContextMenu::default();
    assert_eq!(tool.manifest().platforms, ["windows"]);
    assert_eq!(
        tool.call("list", json!({ "scope": "files" }))
            .unwrap_err()
            .code,
        "ctxmenu.windows_only"
    );
}
