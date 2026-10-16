use super::*;
use serde_json::json;

#[test]
fn manifest_and_dispatch_match_the_public_contract() {
    let tool = Gitignore::default();
    assert_eq!(tool.manifest().id, "org.devlive.toolforge.gitignore");
    for function in ["catalog", "generate", "library"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
    let catalog = tool.call("catalog", json!({})).unwrap();
    assert_eq!(catalog["templates"].as_array().unwrap().len(), 23);
    assert_eq!(catalog["source"]["license"], "CC0-1.0");
    let result = tool
        .call("generate", json!({"templateIds":["node", "rust"]}))
        .unwrap();
    assert_eq!(result["stats"]["templateCount"], 2);
    assert!(result["output"].as_str().unwrap().contains("node_modules/"));
    assert!(result["output"].as_str().unwrap().contains("target"));
    assert!(tool.call("unknown", json!({})).is_err());
    assert!(
        tool.call("catalog", json!({"category":"unexpected"}))
            .is_err()
    );
    assert!(
        tool.call("generate", json!({"lineEnding":"invalid"}))
            .is_err()
    );
}

#[test]
fn custom_library_roundtrips_and_drives_search_and_ordered_generation() {
    let tool = Gitignore::default();
    let created = tool.call("library", json!({
        "operation":"save", "name":"团队 Rust \"rules\"", "content":"target/\n*.log\n!keep.log\n"
    })).unwrap();
    assert_eq!(created["templateId"], "custom-1");
    let stored = serde_json::to_string(&created["library"]).unwrap();
    let library: Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(
        tool.call("library", json!({"operation":"validate","library":library}))
            .unwrap()["library"],
        created["library"]
    );
    let custom_catalog = tool
        .call(
            "catalog",
            json!({
                "library":library, "category":"custom", "query":"团队 rust"
            }),
        )
        .unwrap();
    assert_eq!(custom_catalog["templates"].as_array().unwrap().len(), 1);
    assert_eq!(custom_catalog["templates"][0]["id"], "custom-1");
    assert_eq!(custom_catalog["templates"][0]["category"], "custom");
    assert_eq!(custom_catalog["templates"][0]["ruleCount"], 3);
    let generated = tool
        .call(
            "generate",
            json!({
                "library":library, "templateIds":["windows","custom-1","node","custom-1"]
            }),
        )
        .unwrap();
    assert_eq!(generated["stats"]["templateCount"], 3);
    assert_eq!(generated["sections"][0]["id"], "windows");
    assert_eq!(generated["sections"][1]["id"], "custom-1");
    assert_eq!(generated["sections"][2]["id"], "node");
    assert!(
        generated["output"]
            .as_str()
            .unwrap()
            .contains("# --- 团队 Rust \"rules\" ---\n")
    );
    let deleted = tool
        .call(
            "library",
            json!({"operation":"delete","library":library,"id":"custom-1"}),
        )
        .unwrap();
    assert!(
        tool.call(
            "generate",
            json!({"library":deleted["library"],"templateIds":["node","custom-1"]})
        )
        .is_err()
    );
}

#[test]
fn all_public_calls_reject_invalid_persisted_libraries() {
    let tool = Gitignore::default();
    for function in ["library", "catalog", "generate"] {
        let error = tool
            .call(
                function,
                json!({
                    "operation":"validate", "library":{"version":2,"nextId":1,"templates":[]}
                }),
            )
            .unwrap_err();
        assert_eq!(
            error.code, "gitignore.libraryUnsupportedVersion",
            "{function}"
        );
        assert!(tool.call(function, json!({
            "operation":"validate", "library":{"version":1,"nextId":1,"templates":[{"id":"node","name":"Wrong ID","content":"x/"}]}
        })).is_err());
    }
}
