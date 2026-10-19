//! 纳秒时间 + 原子计数器生成 id（WASM 无 rand）

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn new_id() -> serde_json::Value {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    serde_json::json!({ "id": format!("{nanos}-{counter:04}") })
}
