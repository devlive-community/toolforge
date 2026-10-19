use super::*;
use crate::solver::{CELL_BLACK, CELL_UNKNOWN, CELL_WHITE};

/// 插件级链路：生成 → 构造完成棋盘 → check done → hint 在空盘可用
#[test]
fn pipeline() {
    let plugin = Nonogram::default();
    let puzzle = plugin
        .call("generate", serde_json::json!({ "size": 5, "seed": 42 }))
        .unwrap();
    assert_eq!(puzzle["size"], 5);
    let clues = puzzle["clues"].clone();
    let solution: Vec<bool> = puzzle["solution"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_bool().unwrap())
        .collect();
    // 完成棋盘
    let board: Vec<u8> = solution
        .iter()
        .map(|&b| if b { CELL_BLACK } else { CELL_WHITE })
        .collect();
    let check = plugin
        .call(
            "check",
            serde_json::json!({ "clues": clues, "board": board }),
        )
        .unwrap();
    assert_eq!(check["done"], true);
    // 未完成棋盘（全未定）
    let empty = vec![CELL_UNKNOWN; 25];
    let check = plugin
        .call(
            "check",
            serde_json::json!({ "clues": clues, "board": empty }),
        )
        .unwrap();
    assert_eq!(check["done"], false);
    // 空盘提示可找到确定格
    let hint = plugin
        .call(
            "hint",
            serde_json::json!({ "clues": clues, "board": vec![CELL_UNKNOWN; 25] }),
        )
        .unwrap();
    assert_eq!(hint["found"], true);
    assert!(hint["row"].is_number());
    // 题纸导出：题纸 + 答案页
    let paper = plugin
        .call(
            "export_paper",
            serde_json::json!({ "clues": clues, "solution": solution, "title": "demo" }),
        )
        .unwrap();
    let html = paper["html"].as_str().unwrap();
    assert!(html.contains("demo"));
    assert!(html.contains("page-break-after"));
    // 错误棋盘长度
    let err = plugin
        .call(
            "check",
            serde_json::json!({ "clues": clues, "board": [0, 1] }),
        )
        .unwrap_err();
    assert_eq!(err.code, "puz.bad_board");
    let err = plugin
        .call("generate", serde_json::json!({ "size": 7 }))
        .unwrap_err();
    assert_eq!(err.code, "puz.bad_size");
    let err = plugin.call("nope", Value::Null).unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}
