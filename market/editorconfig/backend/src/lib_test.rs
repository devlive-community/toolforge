use super::*;
use serde_json::json;

#[test]
fn lists_config_files_and_relative_paths() {
    let tool = EditorConfigTool::default();
    let files = tool
        .call(
            "chain",
            json!({ "root": "/p/", "targets": ["a/b.rs", "c.rs"] }),
        )
        .unwrap();
    assert_eq!(
        files,
        json!([
            { "dir": "", "path": "/p/.editorconfig" },
            { "dir": "a", "path": "/p/a/.editorconfig" },
        ])
    );
    let win = tool
        .call("chain", json!({ "root": "C:\\p", "targets": ["x\\y.cs"] }))
        .unwrap();
    assert_eq!(win[1]["path"], "C:\\p\\x\\.editorconfig");

    let rel = tool
        .call(
            "relative",
            json!({ "root": "C:\\Proj", "paths": ["c:\\proj\\src\\a.cs"] }),
        )
        .unwrap();
    assert_eq!(rel, json!(["src/a.cs"]));
    assert_eq!(
        tool.call("relative", json!({ "root": "/p", "paths": ["/pq/a"] }))
            .unwrap_err()
            .code,
        "ec.outside_root"
    );
}

#[test]
fn resolves_and_reports() {
    let tool = EditorConfigTool::default();
    let args = json!({
        "targets": ["a.rs"],
        "configs": [{ "dir": "", "text": "root = true\n[*.rs]\nindent_size = 4\n" }],
    });
    let out = tool.call("resolve", args.clone()).unwrap();
    assert_eq!(out["specVersion"], "0.17.2");
    assert_eq!(out["targets"][0]["properties"][0]["value"], "4");
    let mut md_args = args.clone();
    md_args["format"] = json!("markdown");
    let md = tool.call("report", md_args).unwrap();
    assert!(
        md.as_str()
            .unwrap()
            .contains("| `indent_size` | 4 | .editorconfig:3 `[*.rs]` |")
    );
    let mut json_args = args;
    json_args["format"] = json!("json");
    assert!(
        tool.call("report", json_args)
            .unwrap()
            .as_str()
            .unwrap()
            .contains("\"specVersion\"")
    );
    for function in ["chain", "relative", "resolve", "report"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
}
