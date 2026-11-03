use super::*;
use serde_json::json;

fn items() -> Vec<Item> {
    vec![
        Item {
            id: "t1".into(),
            name: "白衬衫".into(),
            category: "top".into(),
            color: Some("white".into()),
            season: Some("all".into()),
            state: Some("clean".into()),
            last_worn: None,
            wear_count: 0,
        },
        Item {
            id: "t2".into(),
            name: "灰卫衣".into(),
            category: "top".into(),
            color: Some("gray".into()),
            season: Some("winter".into()),
            state: Some("dirty".into()),
            last_worn: None,
            wear_count: 0,
        },
        Item {
            id: "b1".into(),
            name: "牛仔裤".into(),
            category: "bottom".into(),
            color: Some("blue".into()),
            season: Some("all".into()),
            state: Some("clean".into()),
            last_worn: None,
            wear_count: 0,
        },
    ]
}

fn outfits() -> Vec<Outfit> {
    vec![
        Outfit {
            id: "o1".into(),
            name: "通勤".into(),
            item_ids: vec!["t1".into(), "b1".into()],
        },
        Outfit {
            id: "o2".into(),
            name: "居家".into(),
            item_ids: vec!["t2".into(), "b1".into()],
        },
    ]
}

#[test]
fn suggest_marks_dirty_and_filters_season() {
    let out = suggest(SuggestArgs {
        items: items(),
        outfits: outfits(),
        season: None,
    })
    .unwrap();
    assert_eq!(out.suggestions.len(), 2);
    let home = out.suggestions.iter().find(|s| s.name == "居家").unwrap();
    assert_eq!(home.status, "has_dirty");
    assert_eq!(home.dirty, vec!["灰卫衣".to_string()]);
    // 冬季筛选：all/冬季兼容的搭配都保留
    let out = suggest(SuggestArgs {
        items: items(),
        outfits: outfits(),
        season: Some("winter".into()),
    })
    .unwrap();
    assert_eq!(out.suggestions.len(), 2);
}

#[test]
fn suggest_errors_on_empty() {
    let err = suggest(SuggestArgs {
        items: items(),
        outfits: vec![],
        season: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "wardrobe.no_outfits");
}

#[test]
fn stats_aggregate_wear_and_dirty() {
    let mut wear_log = std::collections::HashMap::new();
    wear_log.insert("o1".to_string(), 3u64);
    wear_log.insert("o2".to_string(), 1u64);
    let out = stats(StatsArgs {
        items: items(),
        outfits: outfits(),
        wear_log,
    })
    .unwrap();
    assert_eq!(out.dirty_count, 1);
    let shirt = out.items.iter().find(|s| s.name == "白衬衫").unwrap();
    assert_eq!(shirt.wear_count, 3);
    assert_eq!(shirt.outfits, vec!["通勤".to_string()]);
    // 排序：穿着多的在前（牛仔裤参与 o1+o2 共 4 次）
    assert_eq!(out.items[0].name, "牛仔裤");
    assert_eq!(out.items[0].wear_count, 4);
}

#[test]
fn dispatches_via_plugin() {
    let out = WardrobeJournal::default()
        .call("stats", json!({"items": [], "outfits": [], "wearLog": {}}))
        .unwrap();
    assert_eq!(out["dirtyCount"], 0);
    assert_eq!(
        WardrobeJournal::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&WardrobeJournal::default()).unwrap();
    assert_eq!(checked, 2);
}
