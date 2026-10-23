use serde_json::json;

use super::*;

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn hmac_params() -> Value {
    json!({
        "type": "object",
        "required": ["input", "key"],
        "properties": {
            "input": { "type": "string", "x-positional": 0, "x-stdin": true },
            "key": { "type": "string" },
            "keyEncoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text" },
            "algorithms": { "type": "array", "items": { "type": "string", "enum": ["md5", "sha256"] }, "default": ["sha256"] },
            "uppercase": { "type": "boolean", "default": false },
            "limit": { "type": "integer" }
        }
    })
}

/// 当前平台上的一个绝对路径
fn absolute(name: &str) -> String {
    std::env::temp_dir()
        .join("tfp-args")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn parse_args(params: &Value, items: &[&str]) -> AppResult<Parsed> {
    parse(params, &argv(items), None, Path::new("/work"), || None)
}

#[test]
fn splits_global_options() {
    let (global, rest) = split_global(&argv(&[
        "hash_text",
        "--json",
        "--lang=zh",
        "x",
        "-h",
        "--",
        "--json",
    ]))
    .unwrap();
    assert!(global.json && global.help);
    assert_eq!(global.lang.as_deref(), Some("zh"));
    assert_eq!(rest, ["hash_text", "x", "--", "--json"]);
    assert_eq!(
        split_global(&argv(&["--args"])).unwrap_err().code,
        "cli.missing_value"
    );
}

#[test]
fn maps_flags_positionals_and_defaults() {
    let parsed = parse_args(
        &hmac_params(),
        &[
            "hello",
            "--key",
            "s3cret",
            "--key-encoding=hex",
            "--algorithms",
            "md5,sha256",
            "--uppercase",
            "--limit",
            "5",
        ],
    )
    .unwrap();
    assert_eq!(
        parsed.args,
        json!({
            "input": "hello", "key": "s3cret", "keyEncoding": "hex",
            "algorithms": ["md5", "sha256"], "uppercase": true, "limit": 5
        })
    );
    let defaults = parse_args(&hmac_params(), &["hello", "--key", "k", "--no-uppercase"]).unwrap();
    assert_eq!(defaults.args["keyEncoding"], "text");
    assert_eq!(defaults.args["algorithms"], json!(["sha256"]));
    assert_eq!(defaults.args["uppercase"], false);
}

#[test]
fn repeated_array_options_append() {
    let parsed = parse_args(
        &hmac_params(),
        &[
            "x",
            "--key",
            "k",
            "--algorithms",
            "md5",
            "--algorithms",
            "sha256",
        ],
    )
    .unwrap();
    assert_eq!(parsed.args["algorithms"], json!(["md5", "sha256"]));
}

#[test]
fn reads_stdin_only_when_needed() {
    let params = hmac_params();
    let parsed = parse(
        &params,
        &argv(&["--key", "k"]),
        None,
        Path::new("/"),
        || Some("from stdin\n".to_owned()),
    )
    .unwrap();
    assert_eq!(
        parsed.args["input"], "from stdin\n",
        "stdin is passed through unchanged"
    );
    let given = parse(
        &params,
        &argv(&["arg", "--key", "k"]),
        None,
        Path::new("/"),
        || panic!("stdin must not be read when the argument is given"),
    )
    .unwrap();
    assert_eq!(given.args["input"], "arg");
}

#[test]
fn reports_usage_errors() {
    let params = hmac_params();
    assert_eq!(
        parse_args(&params, &["x"]).unwrap_err().code,
        "cli.missing_required"
    );
    assert_eq!(
        parse_args(&params, &["x", "--key"]).unwrap_err().code,
        "cli.missing_value"
    );
    assert_eq!(
        parse_args(&params, &["x", "--key", "k", "--bogus"])
            .unwrap_err()
            .code,
        "cli.unknown_option"
    );
    assert_eq!(
        parse_args(&params, &["x", "y", "--key", "k"])
            .unwrap_err()
            .code,
        "cli.unexpected_argument"
    );
    assert_eq!(
        parse_args(&params, &["x", "--key", "k", "--limit", "many"])
            .unwrap_err()
            .code,
        "cli.invalid_value"
    );
    let bad_enum = parse_args(&params, &["x", "--key", "k", "--algorithms", "crc"]).unwrap_err();
    assert_eq!(bad_enum.code, "cli.invalid_choice");
    assert_eq!(bad_enum.params["allowed"], "md5, sha256");
}

#[test]
fn double_dash_keeps_values_that_look_like_options() {
    let parsed = parse_args(&hmac_params(), &["--key", "k", "--", "--literal"]).unwrap();
    assert_eq!(parsed.args["input"], "--literal");
    let negative = parse_args(&hmac_params(), &["-12", "--key", "k"]).unwrap();
    assert_eq!(negative.args["input"], "-12");
}

#[test]
fn args_json_is_the_base_and_options_override_it() {
    let params = hmac_params();
    let parsed = parse(
        &params,
        &argv(&["--algorithms", "md5"]),
        Some(r#"{"input":"i","key":"k","algorithms":["sha256"],"extra":1}"#),
        Path::new("/"),
        || None,
    )
    .unwrap();
    assert_eq!(parsed.args["algorithms"], json!(["md5"]));
    assert_eq!(parsed.args["extra"], 1);
    let bad = parse(&params, &[], Some("[1]"), Path::new("/"), || None).unwrap_err();
    assert_eq!(bad.code, "cli.args_json");
}

#[test]
fn path_params_become_absolute_and_are_recorded() {
    let params = json!({
        "type": "object",
        "required": ["paths", "output"],
        "properties": {
            "paths": { "type": "array", "items": { "type": "string", "format": "path", "x-access": "read" }, "x-positional": 0 },
            "output": { "type": "string", "format": "path", "x-access": "write" }
        }
    });
    // 按平台构造路径：Windows 上的分隔符与绝对路径形式都不同
    let absolute = absolute("b.txt");
    let parsed = parse_args(&params, &["a.txt", &absolute, "--output", "out/sums"]).unwrap();
    let joined = |relative: &str| {
        Path::new("/work")
            .join(relative)
            .to_string_lossy()
            .into_owned()
    };
    assert_eq!(parsed.args["paths"], json!([joined("a.txt"), absolute]));
    assert_eq!(parsed.args["output"], joined("out/sums"));
    assert_eq!(parsed.paths.len(), 3);
    assert!(parsed.paths.iter().any(|p| p.write && p.param == "output"));
}

#[test]
fn kebab_converts_camel_case() {
    assert_eq!(kebab("keyEncoding"), "key-encoding");
    assert_eq!(kebab("input"), "input");
}

#[test]
fn complete_requires_absolute_paths_without_a_working_directory() {
    let params = json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "format": "path", "x-access": "read" },
            "mode": { "type": "string", "enum": ["a", "b"], "default": "a" }
        }
    });
    let args = |path: &str| json!({ "path": path }).as_object().unwrap().clone();
    let file = absolute("file");
    let done = complete(&params, args(&file), None).unwrap();
    assert_eq!(done.args["mode"], "a");
    assert_eq!(done.paths[0].path, Path::new(&file));
    let err = complete(&params, args("relative/file"), None).unwrap_err();
    assert_eq!(err.code, "agent.relative_path");
}
