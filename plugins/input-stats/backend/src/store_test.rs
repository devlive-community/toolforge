use super::*;
use crate::keymap::META;
use crate::stats::{App, Button, Display, Event, Recorder};

fn recorder(day: &str, hour: u8) -> Recorder {
    let mut r = Recorder::default();
    r.set_clock(day, hour);
    r.set_displays(vec![Display {
        x: 0.0,
        y: 0.0,
        width: 1280.0,
        height: 800.0,
        mm_per_px: 0.2,
    }]);
    r.set_app(Some(App {
        id: "term".into(),
        name: "Terminal".into(),
    }));
    r
}

fn click(x: f64, y: f64) -> Event {
    Event::Button {
        button: Button::Left,
        x,
        y,
    }
}

#[test]
fn flushes_accumulate_across_batches() {
    let mut store = Store::memory().unwrap();
    for _ in 0..2 {
        let mut r = recorder("2026-10-13", 10);
        r.record(
            Event::Key {
                code: "KeyJ",
                mods: 0,
            },
            1,
        );
        r.record(
            Event::Key {
                code: "KeyV",
                mods: META,
            },
            1,
        );
        r.record(click(10.0, 10.0), 1);
        r.tick(2, 1);
        store.flush(&r.take()).unwrap();
    }
    let s = store.summary("2026-10-13", "2026-10-13").unwrap();
    assert_eq!(s.totals.keys, 4);
    assert_eq!(s.totals.clicks, 2);
    assert_eq!(s.totals.active, 2);
    assert_eq!(
        s.keys,
        vec![
            Count {
                name: "KeyJ".into(),
                count: 2
            },
            Count {
                name: "KeyV".into(),
                count: 2
            }
        ]
    );
    assert_eq!(s.combos[0].name, "Meta+KeyV");
    assert_eq!(s.hours.len(), 24);
    assert_eq!(s.hours[10].keys, 4);
    assert_eq!(s.apps[0].name, "Terminal");
    assert_eq!((s.apps[0].keys, s.apps[0].clicks), (4, 2));

    let maps = store.click_map("2026-10-13", "2026-10-13").unwrap();
    assert_eq!(maps.len(), 1);
    assert_eq!((maps[0].cols, maps[0].rows), (64, 40));
    assert_eq!(maps[0].cells, vec![[0, 0, 2]]);
    assert_eq!((maps[0].max, maps[0].total), (2, 2));
}

#[test]
fn summaries_respect_the_date_range() {
    let mut store = Store::memory().unwrap();
    for (day, n) in [("2026-10-11", 1), ("2026-10-12", 2), ("2026-10-13", 4)] {
        let mut r = recorder(day, 8);
        for _ in 0..n {
            r.record(
                Event::Key {
                    code: "KeyA",
                    mods: 0,
                },
                1,
            );
        }
        store.flush(&r.take()).unwrap();
    }
    let s = store.summary("2026-10-12", "2026-10-13").unwrap();
    assert_eq!(s.totals.keys, 6);
    let days: Vec<(&str, u64)> = s
        .days
        .iter()
        .map(|d| (d.day.as_str(), d.totals.keys))
        .collect();
    assert_eq!(days, vec![("2026-10-12", 2), ("2026-10-13", 4)]);
    assert_eq!(s.first_day.as_deref(), Some("2026-10-11"));
    assert!(
        store
            .click_map("2026-10-12", "2026-10-13")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn settings_survive_clearing_data() {
    let mut store = Store::memory().unwrap();
    assert_eq!(store.setting("enabled").unwrap(), None);
    store.set_setting("enabled", "1").unwrap();
    store.set_setting("enabled", "0").unwrap();
    let mut r = recorder("2026-10-13", 8);
    r.record(
        Event::Key {
            code: "KeyA",
            mods: 0,
        },
        1,
    );
    store.flush(&r.take()).unwrap();
    assert_eq!(store.known_apps().unwrap().len(), 1);
    assert_eq!(store.app_names().unwrap()["term"], "Terminal");

    store.clear().unwrap();
    let s = store.summary("2000-01-01", "2100-01-01").unwrap();
    assert_eq!(s.totals, Totals::default());
    assert!(s.first_day.is_none());
    assert!(store.known_apps().unwrap().is_empty());
    assert_eq!(store.setting("enabled").unwrap().as_deref(), Some("0"));
}

#[test]
fn persists_to_disk() {
    let dir = std::env::temp_dir().join(format!("tf-input-stats-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stats.db");
    {
        let mut store = Store::open(&path).unwrap();
        let mut r = recorder("2026-10-13", 8);
        r.record(Event::Scroll { lines: 5.0 }, 1);
        store.flush(&r.take()).unwrap();
    }
    let store = Store::open(&path).unwrap();
    let s = store.summary("2026-10-13", "2026-10-13").unwrap();
    assert_eq!(s.totals.scroll, 5.0);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}
