use serde_json::json;

use super::*;

#[test]
fn lookup_handles_nested_and_dotted_keys() {
    let messages = json!({
        "errors": { "hash.no_files": "none" },
        "logs": { "hash": { "scanned": "{{count}} files" } },
        "api": { "convert": { "summary": "Convert" } }
    });
    assert_eq!(
        lookup(&messages, "errors.hash.no_files"),
        Some(&json!("none"))
    );
    assert_eq!(
        lookup(&messages, "logs.hash.scanned"),
        Some(&json!("{{count}} files"))
    );
    assert_eq!(
        lookup(&messages, "api.convert.summary"),
        Some(&json!("Convert"))
    );
    assert_eq!(lookup(&messages, "api.convert.missing"), None);
}

#[test]
fn interpolates_params_and_ignores_formats() {
    let params = json!({ "count": 3, "bytes": 1024, "file": "a.txt" });
    let params = params.as_object().unwrap();
    assert_eq!(
        interpolate(
            "{{count}} files, {{bytes, number}} bytes in {{file}}",
            params
        ),
        "3 files, 1024 bytes in a.txt"
    );
    assert_eq!(interpolate("missing {{gone}}", params), "missing {{gone}}");
    assert_eq!(interpolate("open {{ only", params), "open {{ only");
}

#[test]
fn plugin_messages_win_over_common() {
    let plugin = json!({"name": "Hash", "errors": {"fs.not_found": "plugin wording"}});
    let messages = Messages::new("en-US").with_plugin(plugin);
    assert_eq!(messages.manifest_text("i18n:name"), "Hash");
    assert_eq!(messages.manifest_text("plain"), "plain");
    assert_eq!(
        messages.error("fs.not_found", &Map::new()),
        "plugin wording"
    );
    assert_eq!(messages.error("no.such_code", &Map::new()), "no.such_code");
    assert_eq!(messages.text("usage"), "Usage", "cli messages are bundled");
    let shadowing =
        Messages::new("en-US").with_plugin(json!({"usage": "plugin", "options": {"a": "b"}}));
    assert_eq!(
        shadowing.text("usage"),
        "Usage",
        "plugin keys never replace cli labels"
    );
    assert_eq!(shadowing.heading("options"), "Options:");
    assert_eq!(Messages::new("zh-CN").heading("options"), "选项：");
    assert_ne!(
        messages.error("plugin.invalid_args", &Map::new()),
        "plugin.invalid_args",
        "app common errors are bundled"
    );
}

/// 两种语言的命令行文案必须有相同的键
#[test]
fn cli_locales_have_the_same_keys() {
    fn keys(node: &Value, prefix: &str, out: &mut Vec<String>) {
        if let Value::Object(map) = node {
            for (k, v) in map {
                keys(v, &format!("{prefix}{k}."), out);
            }
        } else {
            out.push(prefix.trim_end_matches('.').to_owned());
        }
    }
    let (mut en, mut zh) = (Vec::new(), Vec::new());
    keys(&serde_json::from_str(CLI_EN).unwrap(), "", &mut en);
    keys(&serde_json::from_str(CLI_ZH).unwrap(), "", &mut zh);
    en.sort();
    zh.sort();
    assert_eq!(en, zh);
}

#[test]
fn language_flag_maps_to_supported_locales() {
    assert_eq!(detect_lang(Some("zh")), "zh-CN");
    assert_eq!(detect_lang(Some("zh_TW.UTF-8")), "zh-CN");
    assert_eq!(detect_lang(Some("en")), "en-US");
    assert_eq!(detect_lang(Some("fr-FR")), "en-US");
}
