use super::*;

fn config(dir: &str, text: &str) -> ConfigInput {
    ConfigInput {
        dir: dir.to_owned(),
        text: text.to_owned(),
    }
}

fn run_one(target: &str, configs: Vec<ConfigInput>) -> TargetResult {
    run(Args {
        targets: vec![target.to_owned()],
        configs,
    })
    .unwrap()
    .targets
    .remove(0)
}

fn value(result: &TargetResult, key: &str) -> Option<String> {
    result
        .properties
        .iter()
        .find(|p| p.key == key)
        .and_then(|p| p.value.clone())
}

#[test]
fn nearer_files_override_parents_and_record_the_trace() {
    let result = run_one(
        "src/app/main.rs",
        vec![
            config(
                "",
                "root = true\n[*]\nindent_style = space\nindent_size = 4\nend_of_line = lf\n",
            ),
            config("src", "[*.rs]\nindent_size = 2\n"),
        ],
    );
    assert_eq!(result.files, ["", "src"]);
    assert_eq!(result.root_dir.as_deref(), Some(""));
    assert!(!result.incomplete);
    let size = result
        .properties
        .iter()
        .find(|p| p.key == "indent_size")
        .unwrap();
    assert_eq!(size.value.as_deref(), Some("2"));
    assert_eq!(size.source.as_ref().unwrap().dir, "src");
    assert_eq!(size.trace.len(), 2);
    assert_eq!(size.trace[0].value, "4");
    // indent_size 为数字且未设 tab_width：推导 tab_width
    assert_eq!(value(&result, "tab_width").as_deref(), Some("2"));
    assert_eq!(
        result.unset_standard,
        [
            "charset",
            "trim_trailing_whitespace",
            "insert_final_newline"
        ]
    );
}

#[test]
fn later_sections_in_the_same_file_win() {
    let result = run_one(
        "a.py",
        vec![config(
            "",
            "root = true\n[*]\nindent_size = 8\n[*.py]\nindent_size = 4\n[*.md]\nindent_size = 1\n",
        )],
    );
    assert_eq!(value(&result, "indent_size").as_deref(), Some("4"));
    let matched: Vec<bool> = result.sections.iter().map(|s| s.matched).collect();
    assert_eq!(matched, [true, true, false]);
}

#[test]
fn root_stops_the_search_upwards() {
    let result = run_one(
        "pkg/lib/x.js",
        vec![
            config("", "[*]\nindent_style = tab\n"),
            config("pkg", "root = true\n[*]\nend_of_line = crlf\n"),
        ],
    );
    assert_eq!(result.files, ["pkg"]);
    assert_eq!(value(&result, "indent_style"), None);
    assert_eq!(value(&result, "end_of_line").as_deref(), Some("crlf"));
}

#[test]
fn unset_removes_inherited_values() {
    let result = run_one(
        "docs/a.md",
        vec![
            config("", "root = true\n[*]\ntrim_trailing_whitespace = true\n"),
            config("docs", "[*.md]\ntrim_trailing_whitespace = unset\n"),
        ],
    );
    let prop = result
        .properties
        .iter()
        .find(|p| p.key == "trim_trailing_whitespace")
        .unwrap();
    assert_eq!(prop.value, None);
    assert!(prop.trace[1].unset);
    assert!(result.unset_standard.contains(&"trim_trailing_whitespace"));
}

#[test]
fn subdirectory_globs_and_files_that_do_not_exist_yet() {
    // 目标可以是尚未创建的文件，只看路径
    let result = run_one(
        "web/src/components/New.tsx",
        vec![config(
            "web",
            "root = true\n[src/**/*.tsx]\nindent_size = 2\n[*.{ts,tsx}]\ncharset = UTF-8\n",
        )],
    );
    assert_eq!(value(&result, "indent_size").as_deref(), Some("2"));
    // 标准属性的值不区分大小写
    assert_eq!(value(&result, "charset").as_deref(), Some("utf-8"));
}

#[test]
fn derives_indent_and_tab_widths() {
    let tabs = run_one(
        "a.go",
        vec![config("", "root=true\n[*]\nindent_style = tab\n")],
    );
    assert_eq!(value(&tabs, "indent_size").as_deref(), Some("tab"));
    let derived = tabs
        .properties
        .iter()
        .find(|p| p.key == "indent_size")
        .unwrap();
    assert_eq!(derived.kind, "derived");
    let width = run_one(
        "a.go",
        vec![config(
            "",
            "root=true\n[*]\nindent_size = tab\ntab_width = 8\n",
        )],
    );
    assert_eq!(value(&width, "indent_size").as_deref(), Some("8"));
}

#[test]
fn separates_invalid_values_from_extension_properties() {
    let result = run_one(
        "a.rs",
        vec![config(
            "",
            "[*]\nindent_style = spaces\nmax_line_length = 100\nij_continuation_indent_size = 8\n",
        )],
    );
    let style = result
        .properties
        .iter()
        .find(|p| p.key == "indent_style")
        .unwrap();
    assert!(style.invalid);
    assert_eq!(style.value.as_deref(), Some("spaces"));
    let ext = result
        .properties
        .iter()
        .find(|p| p.key == "max_line_length")
        .unwrap();
    assert_eq!((ext.kind, ext.invalid), ("extension", false));
    // 到项目根仍没有 root = true
    assert!(result.incomplete);
}

#[test]
fn validates_paths() {
    assert_eq!(normalize("./src//a.rs").unwrap(), "src/a.rs");
    assert_eq!(normalize("src\\a.rs").unwrap(), "src/a.rs");
    assert_eq!(normalize("../a").unwrap_err().code, "ec.outside_root");
    assert_eq!(normalize("/etc/a").unwrap_err().code, "ec.absolute_path");
    assert_eq!(normalize("C:\\a").unwrap_err().code, "ec.absolute_path");
    assert_eq!(ancestors("a/b/c.rs"), ["a/b", "a", ""]);
}
