use super::*;

fn workspace(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tfp-disk-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, name: &str, size: usize) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![b'x'; size]).unwrap();
}

fn scan(dir: &Path, hidden: bool) -> Tree {
    Tree::scan(dir, hidden, true, &AtomicU64::new(0), &|| false).unwrap()
}

fn child<'a>(tree: &'a Tree, parent: NodeId, name: &str) -> (NodeId, &'a Node) {
    let id = *tree.nodes[parent as usize]
        .children
        .iter()
        .find(|c| &*tree.nodes[**c as usize].name == name)
        .unwrap_or_else(|| panic!("{name} not found"));
    (id, &tree.nodes[id as usize])
}

#[test]
fn sums_sizes_and_sorts_children() {
    let dir = workspace("sum");
    write(&dir, "small.txt", 10);
    write(&dir, "photos/a.jpg", 1000);
    write(&dir, "photos/b.png", 500);
    write(&dir, "photos/raw/c.dng", 3000);
    write(&dir, ".cache/tmp.bin", 99);
    let tree = scan(&dir, false);
    let root = &tree.nodes[ROOT as usize];
    assert_eq!((root.size, root.files), (4510, 4));
    let names: Vec<&str> = root
        .children
        .iter()
        .map(|c| &*tree.nodes[*c as usize].name)
        .collect();
    assert_eq!(names, ["photos", "small.txt"]);
    let (photos, node) = child(&tree, ROOT, "photos");
    assert_eq!((node.size, node.files), (4500, 3));
    let (raw, _) = child(&tree, photos, "raw");
    assert_eq!(
        tree.path(raw),
        dir.canonicalize().unwrap().join("photos/raw")
    );
    let (jpg, file) = child(&tree, photos, "a.jpg");
    assert_eq!(file.category, Category::Images);
    assert_eq!(
        tree.ancestors(jpg)
            .iter()
            .map(|(_, n)| *n)
            .skip(1)
            .collect::<Vec<_>>(),
        ["photos", "a.jpg"]
    );
    assert_eq!(scan(&dir, true).nodes[ROOT as usize].size, 4609);
}

#[test]
fn removes_nodes_and_updates_totals() {
    let dir = workspace("remove");
    write(&dir, "a/one.bin", 100);
    write(&dir, "a/two.bin", 50);
    write(&dir, "b.bin", 10);
    let mut tree = scan(&dir, false);
    let (a, _) = child(&tree, ROOT, "a");
    let (one, _) = child(&tree, a, "one.bin");
    tree.remove(one);
    assert_eq!(tree.nodes[a as usize].size, 50);
    assert_eq!(
        (
            tree.nodes[ROOT as usize].size,
            tree.nodes[ROOT as usize].files
        ),
        (60, 2)
    );
    assert_eq!(tree.get(one).unwrap_err().code, "disk.node_missing");
    tree.remove(one);
    assert_eq!(
        tree.nodes[ROOT as usize].size, 60,
        "removing twice changes nothing"
    );
    tree.remove(ROOT);
    assert!(tree.get(ROOT).is_ok());
    assert_eq!(tree.files_under(ROOT).count(), 2);
}

#[cfg(unix)]
#[test]
fn skips_symlinks_and_counts_hard_links_once() {
    let dir = workspace("links");
    write(&dir, "data.bin", 1000);
    std::os::unix::fs::symlink(dir.join("data.bin"), dir.join("link.bin")).unwrap();
    std::fs::hard_link(dir.join("data.bin"), dir.join("hard.bin")).unwrap();
    let tree = scan(&dir, false);
    assert_eq!(tree.nodes[ROOT as usize].size, 1000);
    assert_eq!(tree.nodes[ROOT as usize].files, 2);
}

#[test]
fn rejects_missing_folders_and_honours_cancel() {
    let missing = Tree::scan(
        Path::new("/definitely/missing"),
        false,
        true,
        &AtomicU64::new(0),
        &|| false,
    );
    assert_eq!(missing.err().unwrap().code, "disk.not_a_folder");
    let dir = workspace("cancel");
    write(&dir, "a/b.txt", 1);
    let cancelled = Tree::scan(&dir, false, true, &AtomicU64::new(0), &|| true);
    assert_eq!(cancelled.err().unwrap().code, "task.cancelled");
}

/// `TOOLFORGE_DISK_SAMPLE=<目录> cargo test --release -p tfp-disk-usage -- --ignored --nocapture`
#[test]
#[ignore]
fn benchmark_scan() {
    let Some(dir) = std::env::var_os("TOOLFORGE_DISK_SAMPLE") else {
        return;
    };
    let started = std::time::Instant::now();
    let tree = Tree::scan(Path::new(&dir), true, true, &AtomicU64::new(0), &|| false).unwrap();
    let root = &tree.nodes[ROOT as usize];
    println!(
        "{} files, {} nodes, {} bytes, {} denied in {} ms",
        root.files,
        tree.nodes.len(),
        root.size,
        tree.denied,
        started.elapsed().as_millis()
    );
    let started = std::time::Instant::now();
    let tiles = crate::treemap::tiles(&tree, ROOT, 1200.0, 700.0);
    println!(
        "treemap: {} tiles in {} µs",
        tiles.len(),
        started.elapsed().as_micros()
    );
}
