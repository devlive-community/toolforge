use std::io::Read;
use std::sync::Mutex;

use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx {
    progress: Mutex<u64>,
}

impl TaskContext for Ctx {
    fn log(&self, _level: LogLevel, _code: &str, _params: Value) {}
    fn progress(&self, done: u64, _total: u64) {
        *self.progress.lock().unwrap() = done;
    }
    fn stage(&self, _code: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        Ok(Box::new(std::fs::File::open(path).unwrap()))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        Ok(std::fs::metadata(path).unwrap().len())
    }
}

#[test]
fn shannon_bounds() {
    let mut counts = [0u64; 256];
    counts[b'a' as usize] = 100;
    assert_eq!(shannon(&counts, 100), 0.0);
    let uniform = [4u64; 256];
    assert_eq!(shannon(&uniform, 1024), 8.0);
    counts[b'b' as usize] = 100;
    assert_eq!(shannon(&counts, 200), 1.0);
}

#[test]
fn block_size_rounds_and_caps_points() {
    assert_eq!(block_size(256, 10_000), 256);
    assert_eq!(block_size(300, 10_000), 512);
    assert_eq!(block_size(1, 10_000), 64);
    // 10 MB / 256 B 超过 4096 点时放大
    let block = block_size(256, 10 * 1024 * 1024);
    assert!((10 * 1024 * 1024u64).div_ceil(block) <= MAX_POINTS);
    assert_eq!(block, 4096);
}

#[test]
fn merges_high_regions() {
    let values = [1.0, 7.5, 7.9, 3.0, 7.3, 7.3];
    let (found, truncated) = regions(&values, 100, 550);
    assert!(!truncated);
    assert_eq!(
        found,
        vec![
            Region {
                start: 100,
                end: 300,
                average: 7.7
            },
            Region {
                start: 400,
                end: 550,
                average: 7.3
            },
        ]
    );
}

#[test]
fn finds_random_section_in_file() {
    let path = std::env::temp_dir().join(format!("tf-hex-entropy-{}", std::process::id()));
    let mut data = vec![b'A'; 4096];
    // 伪随机字节（xorshift），熵接近 8
    let mut x: u32 = 2463534242;
    for _ in 0..8192 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        data.push((x >> 24) as u8);
    }
    data.extend(vec![0u8; 4096]);
    std::fs::write(&path, &data).unwrap();
    let ctx = Ctx::default();
    let report = run(
        Args {
            path: path.to_string_lossy().into_owned(),
            block: 1024,
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(report.block, 1024);
    assert_eq!(report.values.len(), 16);
    assert_eq!(report.values[0], 0.0);
    assert!(report.values[6] > 7.5);
    assert_eq!(report.regions.len(), 1);
    assert_eq!(
        (report.regions[0].start, report.regions[0].end),
        (4096, 12288)
    );
    assert_eq!(*ctx.progress.lock().unwrap(), data.len() as u64);
    std::fs::remove_file(path).unwrap();
}
