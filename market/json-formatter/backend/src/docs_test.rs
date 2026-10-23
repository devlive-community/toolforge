use super::*;
use serde_json::json;

#[test]
fn evicts_oldest_documents() {
    let cache = DocCache::default();
    let ids: Vec<u64> = (0..=CAPACITY).map(|i| cache.insert(json!(i))).collect();
    assert!(cache.get(ids[0]).is_none());
    assert_eq!(*cache.get(ids[CAPACITY]).unwrap(), json!(CAPACITY));
}
