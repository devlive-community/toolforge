use super::*;
use crate::keymap::{META, SHIFT};

fn display() -> Display {
    Display {
        x: 0.0,
        y: 0.0,
        width: 1600.0,
        height: 900.0,
        mm_per_px: 0.25,
    }
}

fn recorder() -> Recorder {
    let mut r = Recorder::default();
    r.set_clock("2026-10-13", 9);
    r.set_displays(vec![display()]);
    r.set_app(Some(App {
        id: "com.example.editor".into(),
        name: "Editor".into(),
    }));
    r
}

fn key(code: &'static str, mods: u8) -> Event {
    Event::Key { code, mods }
}

#[test]
fn counts_keys_combos_and_attributes_them_to_the_app() {
    let mut r = recorder();
    r.record(key("KeyA", 0), 1);
    r.record(key("KeyA", SHIFT), 1);
    r.record(key("KeyC", META), 2);
    let p = r.take();
    assert_eq!(p.keys[&("2026-10-13".into(), "KeyA")], 2);
    assert_eq!(p.keys[&("2026-10-13".into(), "KeyC")], 1);
    assert_eq!(p.combos.len(), 1);
    assert_eq!(p.combos[&("2026-10-13".into(), "Meta+KeyC".into())], 1);
    assert_eq!(p.hours[&("2026-10-13".into(), 9)].keys, 3);
    let app = &p.apps[&("2026-10-13".into(), "com.example.editor".into())];
    assert_eq!((app.name.as_str(), app.keys), ("Editor", 3));
    // 取走后清空
    assert!(r.take().is_empty());
}

#[test]
fn records_click_cells_on_the_right_display() {
    let mut r = recorder();
    let second = Display {
        x: 1600.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
        mm_per_px: 0.3,
    };
    r.set_displays(vec![display(), second.clone()]);
    r.record(
        Event::Button {
            button: Button::Left,
            x: 0.0,
            y: 0.0,
        },
        1,
    );
    r.record(
        Event::Button {
            button: Button::Right,
            x: 1599.9,
            y: 899.9,
        },
        1,
    );
    r.record(
        Event::Button {
            button: Button::Left,
            x: 1600.0 + 960.0,
            y: 540.0,
        },
        1,
    );
    let p = r.take();
    let first = display().id();
    assert_eq!(first, "1600x900@0,0");
    assert_eq!(display().grid(), (64, 36));
    assert_eq!(p.clicks[&("2026-10-13".into(), first.clone(), 0, 0)], 1);
    assert_eq!(p.clicks[&("2026-10-13".into(), first, 63, 35)], 1);
    assert_eq!(p.clicks[&("2026-10-13".into(), second.id(), 32, 18)], 1);
    assert_eq!(p.displays.len(), 2);
    assert_eq!(p.buttons[&("2026-10-13".into(), "left")], 2);
    assert_eq!(p.buttons[&("2026-10-13".into(), "right")], 1);
    assert_eq!(p.hours[&("2026-10-13".into(), 9)].clicks, 3);
}

#[test]
fn measures_mouse_distance_and_ignores_jumps() {
    let mut r = recorder();
    r.record(Event::Move { x: 0.0, y: 0.0 }, 1);
    r.record(Event::Move { x: 30.0, y: 40.0 }, 1);
    // 光标被程序移动到远处：不计入
    r.record(
        Event::Move {
            x: 1500.0,
            y: 2500.0,
        },
        1,
    );
    r.record(Event::Scroll { lines: -3.0 }, 1);
    let hour = &r.take().hours[&("2026-10-13".into(), 9)];
    assert!((hour.distance - 50.0 * 0.25).abs() < 1e-9);
    assert_eq!(hour.scroll, 3.0);
}

#[test]
fn active_time_stops_after_idle() {
    let mut r = recorder();
    r.tick(0, 1);
    assert!(r.take().is_empty(), "no input yet");
    r.record(key("KeyA", 0), 10);
    r.tick(11, 1);
    r.tick(10 + IDLE_SECS, 1);
    r.tick(11 + IDLE_SECS, 1);
    let p = r.take();
    assert_eq!(p.hours[&("2026-10-13".into(), 9)].active, 2);
    assert_eq!(
        p.apps[&("2026-10-13".into(), "com.example.editor".into())].active,
        2
    );
}

#[test]
fn excluded_apps_are_not_counted() {
    let mut r = recorder();
    r.set_excluded(["com.example.editor".to_owned()].into());
    r.record(key("KeyA", 0), 1);
    r.record(Event::Move { x: 0.0, y: 0.0 }, 1);
    r.tick(1, 1);
    assert!(r.take().is_empty());
    // 切回其他应用后，移动距离从排除期间最后的位置算起
    r.set_app(None);
    r.record(Event::Move { x: 3.0, y: 4.0 }, 2);
    let p = r.take();
    assert!((p.hours[&("2026-10-13".into(), 9)].distance - 5.0 * 0.25).abs() < 1e-9);
    assert!(p.apps.is_empty());
}

#[test]
fn day_changes_split_buckets() {
    let mut r = recorder();
    r.record(key("KeyA", 0), 1);
    r.set_clock("2026-10-14", 0);
    r.record(key("KeyA", 0), 2);
    let p = r.take();
    assert_eq!(p.keys[&("2026-10-13".into(), "KeyA")], 1);
    assert_eq!(p.keys[&("2026-10-14".into(), "KeyA")], 1);
    assert_eq!(p.hours[&("2026-10-14".into(), 0)].keys, 1);
}

#[test]
fn nothing_is_recorded_before_the_clock_is_set() {
    let mut r = Recorder::default();
    r.record(key("KeyA", 0), 1);
    assert!(r.take().is_empty());
}

#[test]
fn held_keys_filter_auto_repeat() {
    let mut held = HeldKeys::default();
    assert!(held.press("KeyA"));
    assert!(!held.press("KeyA"));
    assert!(held.press("ControlLeft"));
    assert!(held.press("ShiftRight"));
    assert_eq!(held.mods(), crate::keymap::CTRL | SHIFT);
    held.release("KeyA");
    held.release("ControlLeft");
    assert!(held.press("KeyA"));
    assert_eq!(held.mods(), SHIFT);
}
