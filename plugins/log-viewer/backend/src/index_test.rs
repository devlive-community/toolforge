use std::io::Write;

use super::*;

fn temp(name: &str, content: &[u8]) -> (std::path::PathBuf, File) {
    let path = std::env::temp_dir().join(format!("tfp-log-index-{name}-{}", std::process::id()));
    std::fs::write(&path, content).unwrap();
    let file = File::open(&path).unwrap();
    (path, file)
}

fn text(file: &mut File, index: &Index, line: usize) -> String {
    let (start, end) = index.range(line, file).unwrap();
    let mut buf = vec![0; (end - start) as usize];
    file.seek(SeekFrom::Start(start)).unwrap();
    file.read_exact(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn indexes_lines_levels_and_tail() {
    let (_, mut file) = temp("basic", b"INFO a\r\nERROR b\n\tat x\n\nWARN tail");
    let mut index = Index::default();
    let len = file.metadata().unwrap().len();
    index.scan(&mut file, len, |_| {}, || false).unwrap();
    assert_eq!(index.complete(), 4);
    assert!(index.has_tail());
    assert_eq!(index.total(), 5);
    assert_eq!(
        index.levels,
        vec![level::INFO, level::ERROR, level::ERROR, level::NONE]
    );
    assert_eq!(index.counts[level::ERROR as usize], 2);
    let lines: Vec<String> = (0..5).map(|i| text(&mut file, &index, i)).collect();
    assert_eq!(lines, ["INFO a", "ERROR b", "\tat x", "", "WARN tail"]);
    assert_eq!(
        index.range(5, &mut file).unwrap_err().code,
        "log.line_out_of_range"
    );
}

#[test]
fn continues_when_the_file_grows() {
    let (path, mut file) = temp("grow", b"INFO one\nERROR par");
    let mut index = Index::default();
    index.scan(&mut file, 18, |_| {}, || false).unwrap();
    assert_eq!((index.complete(), index.total()), (1, 2));

    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"tial\n  continued\n10:00 last\n")
        .unwrap();
    let len = std::fs::metadata(&path).unwrap().len();
    index.scan(&mut file, len, |_| {}, || false).unwrap();
    assert!(!index.has_tail());
    assert_eq!(index.total(), 4);
    assert_eq!(text(&mut file, &index, 1), "ERROR partial");
    assert_eq!(
        index.levels,
        vec![level::INFO, level::ERROR, level::ERROR, level::NONE]
    );
}

#[test]
fn handles_empty_files_and_long_lines() {
    let (_, mut file) = temp("empty", b"");
    let mut index = Index::default();
    index.scan(&mut file, 0, |_| {}, || false).unwrap();
    assert_eq!(index.total(), 0);

    let mut long = vec![b'x'; 3 * CHUNK];
    long.extend_from_slice(b"\nWARN after\n");
    let (_, mut file) = temp("long", &long);
    let mut index = Index::default();
    index
        .scan(&mut file, long.len() as u64, |_| {}, || false)
        .unwrap();
    assert_eq!(index.total(), 2);
    assert_eq!(index.levels[1], level::WARN);
    assert_eq!(index.range(0, &mut file).unwrap(), (0, 3 * CHUNK as u64));
}

#[test]
fn stops_when_cancelled() {
    let (_, mut file) = temp("cancel", b"a\nb\n");
    let mut index = Index::default();
    assert_eq!(
        index.scan(&mut file, 4, |_| {}, || true).unwrap_err().code,
        "task.cancelled"
    );
}
