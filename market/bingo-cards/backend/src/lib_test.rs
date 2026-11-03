use super::*;
use serde_json::json;

fn pool(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("词条{i}")).collect()
}

#[test]
fn generates_unique_layouts() {
    let out = generate(GenerateArgs {
        items: pool(24),
        size: 5,
        cards: 6,
        free_center: true,
        seed: 42,
    })
    .unwrap();
    assert_eq!(out.cards.len(), 6);
    assert_eq!(out.items_per_card, 24);
    let free = out.cards[0].free_index.unwrap();
    assert_eq!(out.cards[0].cells[free], "FREE");
    assert_eq!(free, 12);
    // 布局两两不同
    for (i, a) in out.cards.iter().enumerate() {
        for b in out.cards.iter().skip(i + 1) {
            assert_ne!(a.cells, b.cells);
        }
    }
}

#[test]
fn validates_pool_and_params() {
    let err = generate(GenerateArgs {
        items: pool(8),
        size: 5,
        cards: 1,
        free_center: false,
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "bingo.not_enough_items");
    let err = generate(GenerateArgs {
        items: vec![" ".into()],
        size: 3,
        cards: 1,
        free_center: false,
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "bingo.empty_item");
    let err = generate(GenerateArgs {
        items: pool(9),
        size: 4,
        cards: 1,
        free_center: false,
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "bingo.bad_size");
    // 同一批词条重复出现（含空格变体）会被去重
    let items = vec![
        "苹果".into(),
        " 苹果".into(),
        "梨".into(),
        "橙".into(),
        "蕉".into(),
        "莓".into(),
        "瓜".into(),
        "桃".into(),
        "杏".into(),
        "柿".into(),
    ];
    let out = generate(GenerateArgs {
        items,
        size: 3,
        cards: 1,
        free_center: false,
        seed: 1,
    })
    .unwrap();
    assert_eq!(out.items_per_card, 9, "10 raw items dedupe to 9");
    // 苹果只出现一次
    assert_eq!(
        out.cards[0].cells.iter().filter(|c| **c == "苹果").count(),
        1
    );
}

#[test]
fn draw_is_reproducible_and_exhausts() {
    let remaining = vec!["a".into(), "b".into(), "c".into()];
    let out = draw(DrawArgs {
        remaining: remaining.clone(),
        seed: 7,
    })
    .unwrap();
    let again = draw(DrawArgs {
        remaining: remaining.clone(),
        seed: 7,
    })
    .unwrap();
    assert_eq!(out.item, again.item);
    assert!(!out.remaining.contains(&out.item));
    let mut left = remaining.clone();
    for seed in 1..=3u64 {
        let out = draw(DrawArgs {
            remaining: left.clone(),
            seed,
        })
        .unwrap();
        left = out.remaining;
    }
    assert!(left.is_empty());
    let err = draw(DrawArgs {
        remaining: vec![],
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "bingo.pool_exhausted");
}

#[test]
fn check_finds_all_line_kinds() {
    let cells: Vec<String> = (0..9).map(|i| format!("w{i}")).collect();
    // 横线：前三个已叫
    let out = check(CheckArgs {
        size: 3,
        cells: cells.clone(),
        free_center: false,
        called: vec!["w0".into(), "w1".into(), "w2".into()],
    })
    .unwrap();
    assert!(out.has_line);
    assert_eq!(out.lines[0].kind, "row");
    assert_eq!(out.lines[0].index, 0);
    // 对角线
    let called = vec!["w0".into(), "w4".into(), "w8".into()];
    let out = check(CheckArgs {
        size: 3,
        cells: cells.clone(),
        free_center: false,
        called,
    })
    .unwrap();
    assert!(
        out.lines
            .iter()
            .any(|l| l.kind == "diagonal" && l.index == 0)
    );
    // free 中央格自动命中：5×5 对角线过中央即成立
    let cells5: Vec<String> = (0..25).map(|i| format!("x{i}")).collect();
    let called = vec!["x0".into(), "x6".into(), "x18".into(), "x24".into()];
    let out = check(CheckArgs {
        size: 5,
        cells: cells5,
        free_center: true,
        called,
    })
    .unwrap();
    assert!(
        out.lines
            .iter()
            .any(|l| l.kind == "diagonal" && l.index == 0),
        "FREE center completes the diagonal"
    );
    // 未叫项核对
    let out = check(CheckArgs {
        size: 3,
        cells: cells.clone(),
        free_center: false,
        called: vec!["w0".into()],
    })
    .unwrap();
    assert!(out.uncalled_used.contains(&"w1".to_string()));
    assert!(!out.has_line);
}

#[test]
fn dispatches_via_plugin() {
    let tool = BingoCards::default();
    let out = tool.call("generate", json!({"items": ["a", "b", "c", "d", "e", "f", "g", "h", "i"], "size": 3, "cards": 2, "seed": 1})).unwrap();
    assert_eq!(out["cards"].as_array().unwrap().len(), 2);
    let out = tool.call("check", json!({"size": 3, "cells": ["a", "b", "c", "d", "e", "f", "g", "h", "i"], "called": ["a", "b", "c"]})).unwrap();
    assert_eq!(out["hasLine"], true);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&BingoCards::default()).unwrap();
    assert_eq!(checked, 3);
}
