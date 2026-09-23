use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

/// 最近解析过的文档；树形视图按需从这里读取子节点，避免把整棵树传给前端
const CAPACITY: usize = 4;

#[derive(Default)]
pub struct DocCache {
    next_id: AtomicU64,
    docs: Mutex<VecDeque<(u64, Arc<Value>)>>,
}

impl DocCache {
    pub fn insert(&self, value: Value) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut docs = self.docs.lock().unwrap_or_else(|e| e.into_inner());
        docs.push_back((id, Arc::new(value)));
        while docs.len() > CAPACITY {
            docs.pop_front();
        }
        id
    }

    pub fn get(&self, id: u64) -> Option<Arc<Value>> {
        let docs = self.docs.lock().unwrap_or_else(|e| e.into_inner());
        docs.iter()
            .find(|(doc_id, _)| *doc_id == id)
            .map(|(_, v)| v.clone())
    }
}

#[cfg(test)]
#[path = "docs_test.rs"]
mod tests;
