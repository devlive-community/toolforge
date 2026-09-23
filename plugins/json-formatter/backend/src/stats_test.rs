use super::*;
use serde_json::json;

#[test]
fn counts_every_value_type_and_depth() {
    let value = json!({"a": [1, "x", null, true], "b": {"c": 2.5}});
    let stats = Stats::collect(&value, "line1\nline2");
    assert_eq!(stats.objects, 2);
    assert_eq!(stats.arrays, 1);
    assert_eq!(stats.numbers, 2);
    assert_eq!(stats.strings, 1);
    assert_eq!(stats.nulls, 1);
    assert_eq!(stats.booleans, 1);
    assert_eq!(stats.keys, 3);
    assert_eq!(stats.max_depth, 3);
    assert_eq!(stats.lines, 2);
}
