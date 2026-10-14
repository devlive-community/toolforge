use super::*;

#[test]
fn maps_letters_on_every_platform_to_the_same_code() {
    // A 键：macOS kVK_ANSI_A、Windows 扫描码 0x1E、X11 键码 38
    assert_eq!(mac(0), "KeyA");
    assert_eq!(windows(0x1E, false), "KeyA");
    assert_eq!(x11(38), "KeyA");
    // 空格与回车
    assert_eq!(
        (mac(49), windows(0x39, false), x11(65)),
        ("Space", "Space", "Space")
    );
    assert_eq!(
        (mac(36), windows(0x1C, false), x11(36)),
        ("Enter", "Enter", "Enter")
    );
}

#[test]
fn distinguishes_left_and_right_modifiers() {
    assert_eq!((mac(55), mac(54)), ("MetaLeft", "MetaRight"));
    assert_eq!(windows(0x1D, false), "ControlLeft");
    assert_eq!(windows(0x1D, true), "ControlRight");
    assert_eq!(
        (windows(0x38, false), windows(0x38, true)),
        ("AltLeft", "AltRight")
    );
    assert_eq!((x11(133), x11(134)), ("MetaLeft", "MetaRight"));
}

#[test]
fn extended_scan_codes_select_the_navigation_cluster() {
    assert_eq!(windows(0x48, false), "Numpad8");
    assert_eq!(windows(0x48, true), "ArrowUp");
    assert_eq!(windows(0x1C, true), "NumpadEnter");
    assert_eq!(
        (windows(0x45, false), windows(0x45, true)),
        ("Pause", "NumLock")
    );
}

#[test]
fn unknown_codes_are_unidentified() {
    assert_eq!(mac(255), UNIDENTIFIED);
    assert_eq!(windows(0xFF, false), UNIDENTIFIED);
    assert_eq!(x11(0), UNIDENTIFIED);
    assert_eq!(x11(7), UNIDENTIFIED);
    assert_eq!((x11(191), x11(202)), ("F13", "F24"));
}

#[test]
fn every_platform_covers_the_main_block() {
    let main: Vec<&str> = (0..=126).map(mac).collect();
    let win: Vec<&str> = (0..=0x7E).map(|s| windows(s, false)).collect();
    let lin: Vec<&str> = (8..=255).map(x11).collect();
    for code in [
        "Backquote",
        "Digit0",
        "Minus",
        "Equal",
        "Backspace",
        "Tab",
        "BracketLeft",
        "BracketRight",
        "Backslash",
        "CapsLock",
        "Semicolon",
        "Quote",
        "ShiftLeft",
        "ShiftRight",
        "Comma",
        "Period",
        "Slash",
        "Escape",
        "F1",
        "F12",
    ] {
        assert!(main.contains(&code), "mac lacks {code}");
        assert!(win.contains(&code), "windows lacks {code}");
        assert!(lin.contains(&code), "x11 lacks {code}");
    }
}

#[test]
fn mac_special_keys_follow_the_ev_keymap_table() {
    assert_eq!(mac_special(0), "AudioVolumeUp");
    assert_eq!(mac_special(1), "AudioVolumeDown");
    assert_eq!(mac_special(2), "BrightnessUp");
    assert_eq!(mac_special(3), "BrightnessDown");
    assert_eq!(mac_special(7), "AudioVolumeMute");
    assert_eq!(mac_special(14), "Eject");
    assert_eq!(mac_special(16), "MediaPlayPause");
    assert_eq!(mac_special(17), "MediaTrackNext");
    assert_eq!(mac_special(18), "MediaTrackPrevious");
    assert_eq!(mac_special(19), "MediaFastForward");
    assert_eq!(mac_special(20), "MediaRewind");
    // 表外的特殊键（对比度、背光等）不编造名字
    assert_eq!(mac_special(11), UNIDENTIFIED);
    assert_eq!(mac_special(21), UNIDENTIFIED);
}

#[test]
fn combos_need_a_command_modifier() {
    assert_eq!(combo(META, "KeyC").as_deref(), Some("Meta+KeyC"));
    assert_eq!(
        combo(CTRL | SHIFT | ALT, "KeyT").as_deref(),
        Some("Ctrl+Alt+Shift+KeyT")
    );
    // 单独的 Shift 只是输入大写
    assert_eq!(combo(SHIFT, "KeyA"), None);
    assert_eq!(combo(0, "KeyA"), None);
    // 只按下修饰键不算组合键
    assert_eq!(combo(CTRL, "ShiftLeft"), None);
    assert_eq!(combo(META, "Fn"), None);
    assert_eq!(modifier_bit("AltRight"), ALT);
    assert_eq!(modifier_bit("CapsLock"), 0);
}
