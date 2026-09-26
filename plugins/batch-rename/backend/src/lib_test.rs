use super::*;

#[test]
fn scans_folders() {
    let dir = std::env::temp_dir().join(format!("tfp-rename-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    for name in ["b10.txt", "b2.txt", ".hidden", "sub/c.txt"] {
        std::fs::write(dir.join(name), "x").unwrap();
    }
    let names = |recursive: bool| -> Vec<String> {
        scan(&ScanArgs {
            paths: vec![dir.to_string_lossy().into_owned()],
            recursive,
        })
        .paths
        .iter()
        .map(|p| {
            Path::new(p)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
    };
    assert_eq!(names(false), ["b2.txt", "b10.txt"]);
    assert_eq!(names(true), ["b2.txt", "b10.txt", "c.txt"]);
}
