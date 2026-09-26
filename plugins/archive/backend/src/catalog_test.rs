use super::*;

fn meta(path: &str, kind: Kind, size: u64) -> Meta {
    Meta {
        path: path.into(),
        kind,
        size,
        packed: Some(size / 2),
        modified: None,
        mtime: None,
        encrypted: false,
        mode: None,
        safe: true,
    }
}

#[test]
fn builds_folders_and_totals() {
    let catalog = Catalog::new(
        PathBuf::from("/x.zip"),
        Format::Zip,
        None,
        vec![
            meta("b.txt", Kind::File, 10),
            meta("src/main.rs", Kind::File, 100),
            meta("src/lib/mod.rs", Kind::File, 50),
            meta("empty", Kind::Dir, 0),
            meta("Assets/logo.png", Kind::File, 1000),
        ],
    );
    assert_eq!(catalog.total(), (1160, 4));
    let root = catalog.children("").unwrap();
    let names: Vec<&str> = root.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        ["Assets", "empty", "src", "b.txt"],
        "folders first, case-insensitive order"
    );
    let src = root.iter().find(|c| c.name == "src").unwrap();
    assert_eq!((src.kind, src.size, src.files), (Kind::Dir, 150, 2));
    let inner = catalog.children("src").unwrap();
    assert_eq!(
        inner.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(),
        ["src/lib", "src/main.rs"]
    );
    assert_eq!(inner[1].packed, Some(50));
    assert!(catalog.children("src/lib").is_some() && catalog.children("b.txt").is_none());
    assert!(catalog.children("missing").is_none());
    assert!(catalog.children("empty").unwrap().is_empty());
}
