use super::*;
use serde_json::json;

#[test]
fn lists_all_and_filters() {
    let out = list(ListArgs::default()).unwrap();
    assert_eq!(out.total, ENTRIES.len());
    assert!(out.entries.iter().all(|e| e.extension.starts_with('.')));

    let out = list(ListArgs {
        query: ".docx".into(),
    })
    .unwrap();
    assert_eq!(out.total, 1);
    assert_eq!(out.entries[0].risk, "office");

    // "exe" 同时命中 .exe 与 executable 类别
    let out = list(ListArgs {
        query: "exe".into(),
    })
    .unwrap();
    assert!(out.total >= 1);
    assert_eq!(out.entries[0].extension, ".exe");
    assert_eq!(out.entries[0].risk, "executable");

    assert!(
        list(ListArgs {
            query: "zzz".into()
        })
        .is_err()
    );
}

#[test]
fn executable_and_office_categories_are_flagged() {
    let out = list(ListArgs {
        query: "executable".into(),
    })
    .unwrap();
    assert!(out.total >= 8);
    // 可执行类别全部带风险提示（jar 等包装了代码的容器也是）
    assert!(
        out.entries
            .iter()
            .all(|e| e.risk == "executable" || e.risk == "script")
    );
    let docx = list(ListArgs {
        query: "docx".into(),
    })
    .unwrap();
    assert_eq!(
        docx.entries[0].mime,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
}

#[test]
fn identify_rejects_missing_files() {
    // 沙箱里 read-file 走宿主；测试环境没有宿主实现，read_file 会返回错误——
    // 只验证错误码不是 panic（具体宿主行为在桌面端手工验证）
    let result = identify(IdentifyArgs {
        path: "/nonexistent/file.bin".into(),
    });
    assert!(result.is_err());
}

#[test]
fn dispatches_via_plugin() {
    let tool = FileExtension::default();
    let out = tool.call("list", json!({"query": "pdf"})).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&FileExtension::default()).unwrap();
    assert_eq!(checked, 2);
}
