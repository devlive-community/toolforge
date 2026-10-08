//! 测试用的临时文件夹。

use std::path::{Path, PathBuf};

pub fn dir(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tfp-folder-compare-{name}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 写文件，路径用 / 分隔
pub fn write(root: &Path, rel: &str, text: &str) {
    let path = rel
        .split('/')
        .fold(root.to_path_buf(), |p, part| p.join(part));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// 把修改时间设为固定值，避免测试受写入时间影响
pub fn touch(root: &Path, rel: &str, secs: u64) {
    let path = rel
        .split('/')
        .fold(root.to_path_buf(), |p, part| p.join(part));
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}
