use super::*;

fn file(name: &str, text: &str) -> FileInput {
    FileInput {
        name: name.to_owned(),
        text: text.to_owned(),
        format: None,
    }
}

fn write(
    base: FileInput,
    target: FileInput,
    edits: &[(&str, &str)],
    fill: Fill,
) -> PluginResult<Output> {
    run(Args {
        base,
        target,
        edits: edits
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        fill,
    })
}

#[test]
fn fills_json_in_base_order_and_keeps_indentation() {
    let base = file(
        "en.json",
        "{\n  \"a\": \"A\",\n  \"nav\": {\"home\": \"Home\", \"about\": \"About\", \"blog\": \"Blog\"},\n  \"z\": \"Z\"\n}\n",
    );
    let target = file(
        "zh.json",
        "{\n    \"nav\": {\n        \"home\": \"首页\",\n        \"blog\": \"博客\"\n    },\n    \"z\": \"Z 中文\"\n}\n",
    );
    let out = write(base, target, &[("nav.home", "主页")], Fill::Empty).unwrap();
    assert_eq!(out.added, ["a", "nav.about"]);
    assert_eq!(out.updated, ["nav.home"]);
    assert_eq!(
        out.text,
        "{\n    \"nav\": {\n        \"home\": \"主页\",\n        \"about\": \"\",\n        \"blog\": \"博客\"\n    },\n    \"z\": \"Z 中文\",\n    \"a\": \"\"\n}\n"
    );
}

#[test]
fn fills_with_base_text_and_creates_nested_objects() {
    let base = file("en.json", r#"{"a":{"b":{"c":"Deep"}}}"#);
    let target = file("de.json", "{}");
    let out = write(base, target, &[], Fill::Base).unwrap();
    assert_eq!(
        out.text,
        "{\n  \"a\": {\n    \"b\": {\n      \"c\": \"Deep\"\n    }\n  }\n}"
    );
}

#[test]
fn edits_properties_line_by_line() {
    let base = file(
        "m.properties",
        "# header\ntitle=Title\nlong=Long\nnew=New 你\n",
    );
    let target = file(
        "m_zh.properties",
        "# 中文\ntitle = Old\nlong=first \\\n  second\n# trailing comment\n",
    );
    let out = write(
        base,
        target,
        &[("title", "标题"), ("long", "长")],
        Fill::Base,
    )
    .unwrap();
    assert_eq!(out.updated, ["long", "title"]);
    assert_eq!(out.added, ["new"]);
    assert_eq!(
        out.text,
        "# 中文\ntitle = 标题\nlong=长\nnew=New 你\n# trailing comment\n"
    );
}

#[test]
fn keeps_unicode_escapes_when_the_file_uses_them() {
    let base = file("m.properties", "a=A\nb=B\n");
    let target = file("m_zh.properties", "a=\\u4F60\n");
    let out = write(base, target, &[("b", "好\n")], Fill::None).unwrap();
    assert_eq!(out.text, "a=\\u4F60\nb=\\u597D\\n\n");
}

#[test]
fn rejects_yaml_and_unknown_keys() {
    let base = file("en.json", r#"{"a":"A"}"#);
    assert_eq!(
        write(base.clone(), file("zh.yml", "a: A"), &[], Fill::Empty)
            .unwrap_err()
            .code,
        "i18n.write_unsupported"
    );
    assert_eq!(
        write(base, file("zh.json", "{}"), &[("nope", "x")], Fill::None)
            .unwrap_err()
            .code,
        "i18n.unknown_key"
    );
}
