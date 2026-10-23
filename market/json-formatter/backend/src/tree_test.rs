use super::*;
use serde_json::json;

fn page(
    value: Value,
    path: Vec<Segment>,
    offset: usize,
    limit: Option<usize>,
) -> PluginResult<Page> {
    let docs = DocCache::default();
    let doc_id = docs.insert(value);
    children(
        &docs,
        Args {
            doc_id,
            path,
            offset,
            limit,
        },
    )
}

#[test]
fn root_children_of_object() {
    let result = page(json!({"a": 1, "b": [1, 2]}), vec![], 0, None).unwrap();
    assert_eq!(result.kind, Kind::Object);
    assert_eq!(result.total, 2);
    assert_eq!(result.nodes[1].kind, Kind::Array);
    assert_eq!(result.nodes[1].size, 2);
}

#[test]
fn pages_large_arrays() {
    let result = page(json!((0..500).collect::<Vec<_>>()), vec![], 200, Some(50)).unwrap();
    assert_eq!(result.total, 500);
    assert_eq!(result.nodes.len(), 50);
    assert_eq!(result.nodes[0].key, Segment::Index(200));
}

#[test]
fn resolves_nested_paths() {
    let value = json!({"a": [{"b": "x"}]});
    let path = vec![Segment::Key("a".into()), Segment::Index(0)];
    let result = page(value, path, 0, None).unwrap();
    assert_eq!(result.nodes[0].preview, "\"x\"");
}

#[test]
fn truncates_long_strings() {
    let result = page(json!(["x".repeat(500)]), vec![], 0, None).unwrap();
    assert!(result.nodes[0].preview.ends_with('…'));
}

#[test]
fn unknown_doc_or_path_is_an_error() {
    let docs = DocCache::default();
    let err = children(
        &docs,
        Args {
            doc_id: 42,
            path: vec![],
            offset: 0,
            limit: None,
        },
    )
    .unwrap_err();
    assert_eq!(err.code, "json.doc_expired");
    let err = page(json!({}), vec![Segment::Key("missing".into())], 0, None).unwrap_err();
    assert_eq!(err.code, "json.path_not_found");
}
