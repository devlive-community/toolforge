use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tf-applog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn formats_utc_times() {
    assert_eq!(format_time(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(format_time(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
    assert_eq!(format_time(951_782_400_000), "2000-02-29T00:00:00.000Z");
    assert_eq!(format_time(-1), "1969-12-31T23:59:59.999Z");
}

#[test]
fn writes_single_lines_and_rotates() {
    let dir = temp_dir("rotate");
    let log = AppLog::new(&dir, 200, 3);
    log.write("INFO", "toolforge", "first\nsecond");
    let text = std::fs::read_to_string(dir.join("app.log")).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.contains("INFO  toolforge first ⏎ second"), "{text}");

    for i in 0..20 {
        log.write(
            "WARN",
            "tf_core",
            &format!("message number {i:02} with some padding"),
        );
    }
    let files = log.files();
    assert_eq!(files.len(), 3, "{files:?}");
    assert_eq!(files[0], dir.join("app.log"));
    assert!(!dir.join("app.3.log").exists());
    for file in &files {
        assert!(std::fs::metadata(file).unwrap().len() <= 200);
    }
    // 最新的日志在 app.log 中
    assert!(
        std::fs::read_to_string(&files[0])
            .unwrap()
            .contains("number 19")
    );
}

#[test]
fn filters_dependency_noise() {
    let log = AppLog::new(temp_dir("filter"), 1024, 2);
    let meta = |level, target| log::Metadata::builder().level(level).target(target).build();
    assert!(log.enabled(&meta(Level::Info, "toolforge_lib::launcher")));
    assert!(log.enabled(&meta(Level::Info, "tfp_hash")));
    assert!(!log.enabled(&meta(Level::Info, "reqwest::connect")));
    assert!(log.enabled(&meta(Level::Warn, "reqwest::connect")));
    assert!(!log.enabled(&meta(Level::Debug, "toolforge")));
}
