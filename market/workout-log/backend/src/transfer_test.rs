use super::*;
#[test]
fn recovery_rejects_malformed_or_oversized_storage_before_any_write() {
    assert!(
        recover(Recovery {
            writes: BTreeMap::from([("outside".into(), Value::Null)])
        })
        .is_err()
    );
    assert!(
        recover(Recovery {
            writes: BTreeMap::from([("head".into(), json!({"version":99}))])
        })
        .is_err()
    );
}
#[test]
fn empty_backup_roundtrip_and_repeat_are_idempotent() {
    let output = export(ExportArgs {
        head: None,
        workouts: vec![],
        format: "json".into(),
        offset: 0,
        directory: None,
        stamp: None,
    })
    .unwrap();
    let text = output["content"].as_str().unwrap().to_owned();
    let first = import(ImportArgs {
        text: text.clone(),
        head: None,
        records: BTreeMap::new(),
    })
    .unwrap();
    assert_eq!(first.need.len(), 1);
    let receipt = first.need[0].clone();
    let next = import(ImportArgs {
        text: text.clone(),
        head: None,
        records: BTreeMap::from([(receipt.clone(), Value::Null)]),
    })
    .unwrap();
    assert!(next.writes.contains_key("head"));
    let head = serde_json::from_value(next.writes["head"].clone()).unwrap();
    let repeat = import(ImportArgs {
        text,
        head: Some(head),
        records: BTreeMap::from([(receipt, Value::Bool(true))]),
    })
    .unwrap();
    assert!(repeat.writes.is_empty());
}
