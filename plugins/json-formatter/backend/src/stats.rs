use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub objects: usize,
    pub arrays: usize,
    pub strings: usize,
    pub numbers: usize,
    pub booleans: usize,
    pub nulls: usize,
    pub keys: usize,
    pub max_depth: usize,
    /// 以下为输出文本的统计
    pub lines: usize,
    pub chars: usize,
    pub bytes: usize,
}

impl Stats {
    pub fn collect(value: &Value, output: &str) -> Self {
        let mut stats = Stats {
            lines: output.lines().count().max(1),
            chars: output.chars().count(),
            bytes: output.len(),
            ..Stats::default()
        };
        stats.walk(value, 1);
        stats
    }

    fn walk(&mut self, value: &Value, depth: usize) {
        self.max_depth = self.max_depth.max(depth);
        match value {
            Value::Object(map) => {
                self.objects += 1;
                self.keys += map.len();
                map.values().for_each(|v| self.walk(v, depth + 1));
            }
            Value::Array(items) => {
                self.arrays += 1;
                items.iter().for_each(|v| self.walk(v, depth + 1));
            }
            Value::String(_) => self.strings += 1,
            Value::Number(_) => self.numbers += 1,
            Value::Bool(_) => self.booleans += 1,
            Value::Null => self.nulls += 1,
        }
    }
}

#[cfg(test)]
#[path = "stats_test.rs"]
mod tests;
