use super::*;

#[test]
fn adds_icon_to_an_empty_desktop_ini() {
    assert_eq!(
        desktop_ini("", Some(".folder-icon.ico")),
        "[.ShellClassInfo]\r\nIconResource=.folder-icon.ico,0\r\n"
    );
}

#[test]
fn replaces_icon_keys_and_keeps_the_rest() {
    let existing = "[.ShellClassInfo]\r\nIconFile=old.ico\r\nIconIndex=0\r\nInfoTip=Photos\r\n[ViewState]\r\nMode=\r\n";
    assert_eq!(
        desktop_ini(existing, Some("new.ico")),
        "[.ShellClassInfo]\r\nInfoTip=Photos\r\nIconResource=new.ico,0\r\n[ViewState]\r\nMode=\r\n"
    );
    assert_eq!(
        desktop_ini(existing, None),
        "[.ShellClassInfo]\r\nInfoTip=Photos\r\n[ViewState]\r\nMode=\r\n"
    );
    let other_only = "[ViewState]\nMode=\n";
    assert_eq!(
        desktop_ini(other_only, Some("a.ico")),
        "[ViewState]\r\nMode=\r\n[.ShellClassInfo]\r\nIconResource=a.ico,0\r\n"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn sets_and_clears_a_folder_icon() {
    let dir = std::env::temp_dir().join(format!("tf-icon-apply-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let master = RgbaImage::from_pixel(64, 64, image::Rgba([20, 120, 220, 255]));
    set(&dir, &master).unwrap();
    // Finder 把文件夹的自定义图标存在名为 "Icon\r" 的文件里
    assert!(dir.join("Icon\r").exists());
    clear(&dir).unwrap();
    assert!(!dir.join("Icon\r").exists());
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(set(&dir, &master).unwrap_err().code, "fs.not_found");
}
