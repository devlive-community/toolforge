use super::*;
use crate::reg::Memory;

const CLSID: &str = "{B41DB860-64E4-11D2-9906-E49FADC173CA}";

/// 当前用户下一个命令、本机下一个命令与一个外壳扩展
fn fixture() -> Memory {
    let reg = Memory::default();
    reg.put(
        Hive::User,
        "Software\\Classes\\*\\shell\\VSCode",
        "",
        Value::Str("Open with &Code".into()),
    );
    reg.put(
        Hive::User,
        "Software\\Classes\\*\\shell\\VSCode\\command",
        "",
        Value::Str("\"C:\\Code.exe\" \"%1\"".into()),
    );
    reg.put(
        Hive::Machine,
        "Software\\Classes\\*\\shell\\Notepad",
        "MUIVerb",
        Value::Str("@notepad.exe,-470".into()),
    );
    reg.put(
        Hive::Machine,
        "Software\\Classes\\*\\shell\\Notepad",
        "Extended",
        Value::Str(String::new()),
    );
    reg.put(
        Hive::Machine,
        "Software\\Classes\\*\\shellex\\ContextMenuHandlers\\WinRAR",
        "",
        Value::Str(CLSID.into()),
    );
    reg.put(
        Hive::Machine,
        &format!("Software\\Classes\\CLSID\\{CLSID}"),
        "",
        Value::Str("WinRAR shell extension".into()),
    );
    reg.put(
        Hive::Machine,
        &format!("Software\\Classes\\CLSID\\{CLSID}\\InprocServer32"),
        "",
        Value::Str("C:\\WinRAR\\RarExt.dll".into()),
    );
    // 名称不是 CLSID、默认值也不是的扩展会被忽略
    reg.put(
        Hive::Machine,
        "Software\\Classes\\*\\shellex\\ContextMenuHandlers\\Broken",
        "",
        Value::Str("nothing".into()),
    );
    reg
}

#[test]
fn lists_commands_and_handlers() {
    let reg = fixture();
    let entries = list(&reg, Scope::Files);
    assert_eq!(entries.len(), 3, "{entries:#?}");
    let code = entries.iter().find(|e| e.key == "vscode").unwrap();
    assert_eq!(
        (code.hive, code.kind, code.name.as_str()),
        (Hive::User, Kind::Command, "Open with Code")
    );
    assert_eq!(code.target.as_deref(), Some("\"C:\\Code.exe\" \"%1\""));
    assert!(code.enabled && !code.extended);
    let notepad = entries.iter().find(|e| e.key == "notepad").unwrap();
    assert_eq!(
        notepad.name, "@notepad.exe,-470",
        "resolved only on Windows"
    );
    assert!(notepad.extended);
    let rar = entries.iter().find(|e| e.kind == Kind::Handler).unwrap();
    assert_eq!(rar.name, "WinRAR shell extension");
    assert_eq!(rar.target.as_deref(), Some("C:\\WinRAR\\RarExt.dll"));
    assert_eq!(rar.clsid.as_deref(), Some(CLSID));
    assert!(list(&reg, Scope::Drives).is_empty());
}

#[test]
fn disables_and_enables_without_deleting_keys() {
    let reg = fixture();
    let code = Target {
        hive: Hive::User,
        path: "Software\\Classes\\*\\shell\\VSCode".into(),
        kind: Kind::Command,
        clsid: None,
    };
    set_enabled(&reg, &code, false).unwrap();
    let entry = |key: &str| {
        list(&reg, Scope::Files)
            .into_iter()
            .find(|e| e.key.eq_ignore_ascii_case(key))
            .unwrap()
    };
    assert!(!entry("vscode").enabled);
    set_enabled(&reg, &code, true).unwrap();
    assert!(entry("vscode").enabled);

    let rar = Target {
        hive: Hive::Machine,
        path: String::new(),
        kind: Kind::Handler,
        clsid: Some(CLSID.into()),
    };
    set_enabled(&reg, &rar, false).unwrap();
    assert!(reg.value(Hive::Machine, BLOCKED, CLSID).is_some());
    assert!(!entry("winrar").enabled);
    // 当前用户下的阻止记录（其他工具写的）在启用时一起删掉
    reg.put(Hive::User, BLOCKED, CLSID, Value::Str(String::new()));
    set_enabled(&reg, &rar, true).unwrap();
    assert!(entry("winrar").enabled);
    assert!(reg.exists(
        Hive::Machine,
        "Software\\Classes\\*\\shellex\\ContextMenuHandlers\\WinRAR"
    ));
}

#[test]
fn reports_missing_keys_and_admin_rights() {
    let reg = Memory {
        read_only_machine: true,
        ..fixture()
    };
    let gone = Target {
        hive: Hive::User,
        path: "Software\\Classes\\*\\shell\\Gone".into(),
        kind: Kind::Command,
        clsid: None,
    };
    assert_eq!(
        set_enabled(&reg, &gone, false).unwrap_err().code,
        "ctxmenu.not_found"
    );
    let rar = Target {
        hive: Hive::Machine,
        path: String::new(),
        kind: Kind::Handler,
        clsid: Some(CLSID.into()),
    };
    assert_eq!(
        set_enabled(&reg, &rar, false).unwrap_err().code,
        "ctxmenu.needs_admin"
    );
}

#[test]
fn recognises_handlers_disabled_with_a_dash() {
    let reg = Memory::default();
    reg.put(
        Hive::User,
        "Software\\Classes\\Directory\\shellex\\ContextMenuHandlers\\Tool",
        "",
        Value::Str(format!("-{CLSID}")),
    );
    let entries = list(&reg, Scope::Folders);
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].enabled);
    assert_eq!(entries[0].clsid.as_deref(), Some(CLSID));
}

#[test]
fn toggles_the_windows_11_classic_menu() {
    let reg = Memory::default();
    assert!(!classic_enabled(&reg));
    set_classic(&reg, true).unwrap();
    assert!(classic_enabled(&reg));
    assert_eq!(
        reg.value(Hive::User, &format!("{CLASSIC}\\InprocServer32"), ""),
        Some(Value::Str(String::new()))
    );
    set_classic(&reg, false).unwrap();
    assert!(!classic_enabled(&reg) && !reg.exists(Hive::User, CLASSIC));
}
