use serde_json::json;

use super::*;
use crate::schema::infer;

#[test]
fn case_conversions() {
    assert_eq!(words("userID_v2"), vec!["user", "ID", "v", "2"]);
    assert_eq!(words("HTTPServer"), vec!["HTTP", "Server"]);
    assert_eq!(pascal("created-at"), "CreatedAt");
    assert_eq!(pascal("2fa"), "N2Fa");
    assert_eq!(camel("Created_At"), "createdAt");
    assert_eq!(snake("createdAt"), "created_at");
    assert_eq!(snake("$"), "field");
    assert_eq!(singular("categories"), "category");
    assert_eq!(singular("boxes"), "box");
    assert_eq!(singular("users"), "user");
    assert_eq!(singular("data"), "dataItem");
}

#[test]
fn names_nested_types_and_dedupes() {
    let value = json!({
        "user": {"address": {"city": "x"}},
        "items": [{"id": 1}],
        "other": {"user": {"age": 1}}
    });
    let model = build(&infer(&value), "Root");
    let names: Vec<_> = model.structs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Root", "User", "Address", "Item", "Other", "User2"]
    );
    assert_eq!(model.root.kind, TypeRef::Struct("Root".into()));
}

#[test]
fn optional_fields_and_root_arrays() {
    let model = build(&infer(&json!([{"a": 1}, {"a": 2, "b": true}])), "Root");
    assert_eq!(model.structs[0].name, "RootItem");
    let b = &model.structs[0].fields[1];
    assert!(b.optional && !model.structs[0].fields[0].optional);
    assert!(matches!(model.root.kind, TypeRef::Array(_)));
}
