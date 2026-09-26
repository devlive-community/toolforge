use super::*;
use crate::scan::{Options, scan};
use crate::test_support::{workspace, write};

fn run(dir: &Path) -> Vec<Group> {
    let options: Options = serde_json::from_value(serde_json::json!({ "roots": [dir] })).unwrap();
    let files = scan(&options, |_| {}, || false).unwrap();
    find(
        files,
        &Progress {
            stage: &|_| {},
            bytes: &|_, _| {},
            cancelled: &|| false,
        },
    )
    .unwrap()
}

fn names(group: &Group) -> Vec<String> {
    group
        .files
        .iter()
        .map(|f| {
            Path::new(&f.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn groups_identical_files() {
    let dir = workspace("find");
    write(&dir, "a.txt", b"same small content");
    write(&dir, "copy/a (1).txt", b"same small content");
    write(&dir, "other.txt", b"different content!");
    write(&dir, "unique.bin", b"one of a kind");
    // 大小相同、开头 64 KB 相同、结尾不同
    let mut big = vec![7u8; 200_000];
    write(&dir, "big1.bin", &big);
    write(&dir, "big2.bin", &big);
    big[199_999] = 8;
    write(&dir, "big3.bin", &big);
    let groups = run(&dir);
    assert_eq!(groups.len(), 2);
    assert_eq!(names(&groups[0]), ["big1.bin", "big2.bin"]);
    assert_eq!((groups[0].size, groups[0].wasted), (200_000, 200_000));
    assert_eq!(names(&groups[1]), ["a.txt", "a (1).txt"]);
    assert_eq!(
        groups[0].hash,
        blake3::hash(&vec![7u8; 200_000]).to_hex().to_string()
    );
    assert_eq!(
        groups[1].hash,
        blake3::hash(b"same small content").to_hex().to_string()
    );
}

#[cfg(unix)]
#[test]
fn treats_hard_links_as_one_file() {
    let dir = workspace("hardlink");
    write(&dir, "a.txt", b"shared data");
    std::fs::hard_link(dir.join("a.txt"), dir.join("b.txt")).unwrap();
    assert!(run(&dir).is_empty(), "hard links share storage");
    write(&dir, "c.txt", b"shared data");
    let groups = run(&dir);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files.len(), 2);
    assert_eq!(
        groups[0].files.iter().map(|f| f.links.len()).sum::<usize>(),
        1
    );
}

#[test]
fn stops_when_cancelled() {
    let dir = workspace("cancel");
    write(&dir, "a", b"x");
    write(&dir, "b", b"x");
    let options: Options = serde_json::from_value(serde_json::json!({ "roots": [dir] })).unwrap();
    let files = scan(&options, |_| {}, || false).unwrap();
    let result = find(
        files,
        &Progress {
            stage: &|_| {},
            bytes: &|_, _| {},
            cancelled: &|| true,
        },
    );
    assert_eq!(result.unwrap_err().code, "task.cancelled");
}

/// `TOOLFORGE_DUP_SAMPLE=<目录> cargo test --release -p tfp-duplicate-finder -- --ignored --nocapture`
#[test]
#[ignore]
fn benchmark_folder() {
    let Some(dir) = std::env::var_os("TOOLFORGE_DUP_SAMPLE") else {
        return;
    };
    let started = std::time::Instant::now();
    let options: Options =
        serde_json::from_value(serde_json::json!({ "roots": [dir.to_string_lossy()] })).unwrap();
    let files = scan(&options, |_| {}, || false).unwrap();
    let bytes: u64 = files.iter().map(|f| f.size).sum();
    println!(
        "scan: {} files, {} bytes in {} ms",
        files.len(),
        bytes,
        started.elapsed().as_millis()
    );
    let started = std::time::Instant::now();
    let groups = find(
        files,
        &Progress {
            stage: &|_| {},
            bytes: &|_, _| {},
            cancelled: &|| false,
        },
    )
    .unwrap();
    let wasted: u64 = groups.iter().map(|g| g.wasted).sum();
    println!(
        "find: {} groups, {} wasted bytes in {} ms",
        groups.len(),
        wasted,
        started.elapsed().as_millis()
    );
}
