use super::*;

fn dir(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tfp-rename-plan-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn touch(dir: &Path, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .map(|n| {
            let path = dir.join(n);
            std::fs::write(&path, n.as_bytes()).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect()
}

fn plan(paths: Vec<String>, rules: serde_json::Value, sort: &str) -> Plan {
    let args: PlanArgs =
        serde_json::from_value(serde_json::json!({ "paths": paths, "rules": rules, "sort": sort }))
            .unwrap();
    build(&args, &ExifCache::default()).unwrap()
}

fn statuses(plan: &Plan) -> Vec<(String, Status, Option<String>)> {
    plan.items
        .iter()
        .map(|i| (i.to.clone(), i.status, i.reason.clone()))
        .collect()
}

#[test]
fn sorts_naturally() {
    let mut names = vec![
        "file10.txt",
        "File2.txt",
        "file1.txt",
        "file02.txt",
        "a.txt",
    ];
    names.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(
        names,
        [
            "a.txt",
            "file1.txt",
            "File2.txt",
            "file02.txt",
            "file10.txt"
        ]
    );
}

#[test]
fn numbers_in_sorted_order() {
    let dir = dir("sort");
    let paths = touch(&dir, &["b10.jpg", "b2.jpg", "b1.jpg"]);
    let result = plan(
        paths,
        serde_json::json!([{ "kind": "template", "pattern": "photo-{n:2}" }]),
        "name",
    );
    let pairs: Vec<(String, String)> = result
        .items
        .iter()
        .map(|i| (i.from.clone(), i.to.clone()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("b1.jpg".into(), "photo-01.jpg".into()),
            ("b2.jpg".into(), "photo-02.jpg".into()),
            ("b10.jpg".into(), "photo-03.jpg".into())
        ]
    );
    assert_eq!((result.changes, result.problems), (3, 0));
}

#[test]
fn detects_duplicates_existing_files_and_invalid_names() {
    let dir = dir("conflicts");
    let mut paths = touch(&dir, &["a.txt", "b.txt", "keep.txt"]);
    touch(&dir, &["taken.txt"]);
    paths.push(dir.join("missing.txt").to_string_lossy().into_owned());
    // 全部改成同一个名字
    let same = plan(
        paths.clone(),
        serde_json::json!([{ "kind": "template", "pattern": "same" }]),
        "added",
    );
    let s = statuses(&same);
    assert_eq!(s[0].1, Status::Conflict);
    assert_eq!(s[0].2.as_deref(), Some("rename.duplicate"));
    assert_eq!(s[3].1, Status::Missing);
    // 改成已存在的文件名
    let taken = plan(
        vec![paths[0].clone()],
        serde_json::json!([{ "kind": "template", "pattern": "taken" }]),
        "added",
    );
    assert_eq!(statuses(&taken)[0].2.as_deref(), Some("rename.exists"));
    // 目标被批次中不改名的文件占用
    let occupied = plan(
        vec![paths[0].clone(), paths[2].clone()],
        serde_json::json!([{ "kind": "replace", "find": "a", "replace": "keep" }]),
        "added",
    );
    assert_eq!(
        statuses(&occupied)[0].2.as_deref(),
        Some("rename.duplicate")
    );
    assert_eq!(statuses(&occupied)[1].1, Status::Unchanged);
    // 空名字
    let empty = plan(
        vec![paths[0].clone()],
        serde_json::json!([{ "kind": "remove", "count": 99, "part": "full" }]),
        "added",
    );
    assert_eq!(statuses(&empty)[0].2.as_deref(), Some("rename.empty"));
    assert_eq!(empty.problems, 1);
}

#[test]
fn allows_swaps_and_case_changes() {
    let dir = dir("swap");
    let paths = touch(&dir, &["one.txt", "two.txt"]);
    // one → two，two → one：互相腾出位置，不算冲突
    let swap = plan(
        paths.clone(),
        serde_json::json!([
            { "kind": "replace", "find": "one", "replace": "tmp" },
            { "kind": "replace", "find": "two", "replace": "one" },
            { "kind": "replace", "find": "tmp", "replace": "two" }
        ]),
        "added",
    );
    assert_eq!(
        statuses(&swap).iter().map(|s| s.1).collect::<Vec<_>>(),
        [Status::Ok, Status::Ok]
    );
    let upper = plan(
        paths,
        serde_json::json!([{ "kind": "case", "mode": "upper" }]),
        "added",
    );
    assert_eq!(upper.changes, 2);
    assert_eq!(upper.problems, 0);
}

#[test]
fn validates_names() {
    assert_eq!(validate(""), Some("rename.empty"));
    assert_eq!(validate(".."), Some("rename.empty"));
    assert_eq!(validate("a/b"), Some("rename.illegal_chars"));
    assert_eq!(validate(&"x".repeat(256)), Some("rename.too_long"));
    assert_eq!(validate("正常的名字.txt"), None);
    if cfg!(target_os = "windows") {
        assert_eq!(validate("a?.txt"), Some("rename.illegal_chars"));
        assert_eq!(validate("con.txt"), Some("rename.reserved"));
        assert_eq!(validate("name."), Some("rename.trailing_dot"));
    }
    if cfg!(target_os = "macos") {
        assert_eq!(validate("a:b"), Some("rename.illegal_chars"));
    }
}

/// 只含 EXIF 段的最小 JPEG
fn jpeg_with_capture_time(time: &str) -> Vec<u8> {
    let field = exif::Field {
        tag: exif::Tag::DateTimeOriginal,
        ifd_num: exif::In::PRIMARY,
        value: exif::Value::Ascii(vec![time.as_bytes().to_vec()]),
    };
    let mut writer = exif::experimental::Writer::new();
    writer.push_field(&field);
    let mut tiff = std::io::Cursor::new(Vec::new());
    writer.write(&mut tiff, false).unwrap();
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend(tiff.into_inner());
    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
    jpeg.extend(((app1.len() + 2) as u16).to_be_bytes());
    jpeg.extend(app1);
    jpeg.extend([0xff, 0xd9]);
    jpeg
}

#[test]
fn names_photos_by_capture_time() {
    let dir = dir("exif");
    std::fs::write(
        dir.join("IMG_0001.JPG"),
        jpeg_with_capture_time("2024:07:15 18:30:05"),
    )
    .unwrap();
    std::fs::write(dir.join("notes.txt"), "x").unwrap();
    let paths = vec![
        dir.join("IMG_0001.JPG").to_string_lossy().into_owned(),
        dir.join("notes.txt").to_string_lossy().into_owned(),
    ];
    let args: PlanArgs = serde_json::from_value(serde_json::json!({
        "paths": paths,
        "rules": [{ "kind": "template", "pattern": "{exif:%Y%m%d_%H%M%S}" }, { "kind": "extension", "mode": "lower" }]
    }))
    .unwrap();
    let cache = ExifCache::default();
    let result = build(&args, &cache).unwrap();
    assert_eq!(result.items[0].to, "20240715_183005.jpg");
    // 没有 EXIF 的文件使用修改时间
    let modified = std::fs::metadata(dir.join("notes.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let expected = zoned(modified)
        .unwrap()
        .strftime("%Y%m%d_%H%M%S")
        .to_string();
    assert_eq!(result.items[1].to, format!("{expected}.txt"));
    // 第二次从缓存读取
    assert_eq!(
        build(&args, &cache).unwrap().items[0].to,
        "20240715_183005.jpg"
    );
}
