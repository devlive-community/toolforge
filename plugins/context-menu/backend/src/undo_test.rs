use super::*;

#[test]
fn writes_reg_scripts_that_restore_values() {
    let changes = [
        Change {
            hive: Hive::User,
            path: "Software\\Classes\\*\\shell\\Open \"x\"",
            name: "LegacyDisable",
            before: None,
        },
        Change {
            hive: Hive::Machine,
            path: "Software\\Blocked",
            name: "{A}",
            before: Some(Value::Str("C:\\a".into())),
        },
        Change {
            hive: Hive::User,
            path: "K",
            name: "",
            before: Some(Value::Dword(1)),
        },
        Change {
            hive: Hive::User,
            path: "K",
            name: "Path",
            before: Some(Value::Expand("%A%".into())),
        },
    ];
    let script = script(&changes, &[(Hive::User, "Software\\Classes\\CLSID\\{X}")]);
    let lines: Vec<&str> = script.split("\r\n").collect();
    assert_eq!(lines[0], "Windows Registry Editor Version 5.00");
    assert!(lines.contains(&"[-HKEY_CURRENT_USER\\Software\\Classes\\CLSID\\{X}]"));
    assert!(lines.contains(&"[HKEY_CURRENT_USER\\Software\\Classes\\*\\shell\\Open \"x\"]"));
    assert!(lines.contains(&"\"LegacyDisable\"=-"));
    assert!(lines.contains(&"\"{A}\"=\"C:\\\\a\""));
    assert!(lines.contains(&"@=dword:00000001"));
    assert!(lines.contains(&"\"Path\"=hex(2):25,00,41,00,25,00,00,00"));
}

#[test]
fn saves_utf16_files() {
    let dir = std::env::temp_dir().join(format!("tfp-context-menu-undo-{}", std::process::id()));
    let path = save(&dir, "用 Code 打开", "Windows Registry Editor Version 5.00").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..2], &[0xff, 0xfe]);
    assert!(
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("undo-")
    );
    assert!(path.to_string_lossy().ends_with("用_Code_打开.reg"));
    std::fs::remove_dir_all(dir).unwrap();
}
