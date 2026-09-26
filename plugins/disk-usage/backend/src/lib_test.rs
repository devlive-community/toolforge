use std::io::Read;
use std::path::PathBuf;

use super::*;

struct Ctx;

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

#[test]
fn scans_lists_maps_and_summarizes() {
    let dir = std::env::temp_dir().join(format!("tfp-disk-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (name, size) in [
        ("videos/trip.mp4", 8000),
        ("videos/clip.mov", 2000),
        ("docs/a.pdf", 500),
        ("notes.txt", 100),
    ] {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; size]).unwrap();
    }
    let plugin = DiskUsage::default();
    let scanned = plugin
        .run_task("scan", json!({ "root": dir }), &Ctx)
        .unwrap();
    let id = scanned["id"].as_u64().unwrap();
    assert_eq!(scanned["root"]["size"], 10_600);

    let listing = plugin.call("list", json!({ "id": id })).unwrap();
    assert_eq!(listing["children"][0]["name"], "videos");
    let videos = listing["children"][0]["id"].as_u64().unwrap();
    let inner = plugin
        .call("list", json!({ "id": id, "node": videos }))
        .unwrap();
    assert_eq!(inner["crumbs"].as_array().unwrap().len(), 2);
    assert!(inner["path"].as_str().unwrap().ends_with("videos"));

    let tiles = plugin
        .call("treemap", json!({ "id": id, "width": 400, "height": 300 }))
        .unwrap();
    let tiles = tiles.as_array().unwrap();
    let top: f64 = tiles
        .iter()
        .filter(|t| t["depth"] == 0)
        .map(|t| t["w"].as_f64().unwrap() * t["h"].as_f64().unwrap())
        .sum();
    assert!((top - 120_000.0).abs() < 1.0, "{top}");
    assert!(
        tiles
            .iter()
            .any(|t| t["depth"] == 1 && t["name"] == "trip.mp4")
    );

    let types = plugin.call("types", json!({ "id": id })).unwrap();
    assert_eq!(types[0]["category"], "videos");
    assert_eq!(types[0]["size"], 10_000);
    let largest = plugin
        .call("largest", json!({ "id": id, "limit": 2 }))
        .unwrap();
    assert_eq!(largest.as_array().unwrap().len(), 2);
    assert_eq!(largest[0]["name"], "trip.mp4");

    assert_eq!(
        plugin
            .run_task("trash", json!({ "id": id, "nodes": [0] }), &Ctx)
            .unwrap_err()
            .code,
        "disk.cannot_remove_root"
    );
    assert_eq!(
        plugin.call("list", json!({ "id": 999 })).unwrap_err().code,
        "disk.scan_expired"
    );
}
