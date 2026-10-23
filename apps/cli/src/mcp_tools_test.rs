use super::*;

fn messages() -> Messages {
    Messages::new("en-US").with_plugin(json!({
        "api": { "run": { "summary": "Hash files", "paths": "Files to hash" } }
    }))
}

fn interface(effect: &str) -> Interface {
    serde_json::from_value(json!({
        "expose": ["agent"],
        "summary": "i18n:api.run.summary",
        "effect": effect,
        "uses": ["fs:user-selected"],
        "params": {
            "type": "object",
            "required": ["paths"],
            "properties": {
                "paths": {
                    "type": "array",
                    "description": "i18n:api.run.paths",
                    "items": { "type": "string", "format": "path", "x-access": "read" },
                    "x-positional": 0
                },
                "key": { "type": "string", "x-sensitive": true, "default": "k" }
            }
        }
    }))
    .unwrap()
}

#[test]
fn tool_names_round_trip() {
    assert_eq!(tool_name("hash", "hash_files"), "hash__hash_files");
    assert_eq!(
        parse_tool_name("hash__hash_files"),
        Some(("hash", "hash_files"))
    );
    assert_eq!(
        parse_tool_name("json-formatter__diff_patch"),
        Some(("json-formatter", "diff_patch"))
    );
    assert_eq!(parse_tool_name("nothing"), None);
    assert_eq!(parse_tool_name("__x"), None);
}

#[test]
fn input_schema_drops_extensions_and_explains_paths() {
    let schema = input_schema(&interface("read").params, &messages());
    assert_eq!(schema["required"], json!(["paths"]));
    assert_eq!(schema["additionalProperties"], false);
    let paths = &schema["properties"]["paths"];
    assert_eq!(paths["items"], json!({ "type": "string" }));
    assert!(paths.get("x-positional").is_none());
    let description = paths["description"].as_str().unwrap();
    assert!(description.starts_with("Files to hash ("), "{description}");
    assert_eq!(
        schema["properties"]["key"],
        json!({ "type": "string", "default": "k" })
    );
}

#[test]
fn tool_annotations_follow_the_effect() {
    let read = tool(
        "hash",
        "Hash",
        "hash_files",
        &interface("read"),
        &messages(),
    );
    assert_eq!(read["name"], "hash__hash_files");
    assert_eq!(read["title"], "Hash: Hash files");
    assert_eq!(read["annotations"]["readOnlyHint"], true);
    assert!(
        read["description"]
            .as_str()
            .unwrap()
            .starts_with("Hash files. ")
    );
    let write = tool("hash", "Hash", "f", &interface("write"), &messages());
    assert_eq!(write["annotations"]["destructiveHint"], true);
    assert_eq!(write["annotations"]["readOnlyHint"], false);
}

#[test]
fn results_carry_structured_content_for_objects() {
    let ok = success(&json!({ "digest": "abc" }));
    assert_eq!(ok["structuredContent"]["digest"], "abc");
    assert_eq!(ok["content"][0]["text"], r#"{"digest":"abc"}"#);
    assert!(success(&json!("text")).get("structuredContent").is_none());
    let err = failure("Not allowed", "agent.plugin_off");
    assert_eq!(err["isError"], true);
    assert_eq!(err["content"][0]["text"], "Not allowed [agent.plugin_off]");
}
