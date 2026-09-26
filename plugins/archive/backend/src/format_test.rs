use std::io::Write;

use super::*;
use crate::test_support::{workspace, write};

fn tar_bytes() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(5);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "dir/hello.txt", &b"hello"[..])
        .unwrap();
    builder.into_inner().unwrap()
}

fn listed(path: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let read = Arc::new(AtomicU64::new(0));
    walk(path, None, false, &read, &mut |meta, _| {
        out.push((meta.path.clone(), meta.size));
        Ok(Flow::Continue)
    })
    .unwrap();
    out
}

#[test]
fn detects_formats_by_content() {
    let dir = workspace("detect");
    let tar = tar_bytes();
    let gz = {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(&tar).unwrap();
        e.finish().unwrap()
    };
    let bz = {
        let mut e = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
        e.write_all(&tar).unwrap();
        e.finish().unwrap()
    };
    let xz = {
        let mut e =
            lzma_rust2::XzWriter::new(Vec::new(), lzma_rust2::XzOptions::with_preset(3)).unwrap();
        e.write_all(&tar).unwrap();
        e.finish().unwrap()
    };
    let zst =
        ruzstd::encoding::compress_to_vec(&tar[..], ruzstd::encoding::CompressionLevel::Fastest);
    // 扩展名故意写错，按内容识别
    for (name, bytes, expected) in [
        ("a.bin", tar.clone(), Format::Tar),
        ("b.bin", gz, Format::TarGz),
        ("c.bin", bz, Format::TarBz2),
        ("d.bin", xz, Format::TarXz),
        ("e.bin", zst, Format::TarZst),
    ] {
        let path = write(&dir, name, &bytes);
        assert_eq!(detect(&path).unwrap(), expected, "{name}");
        assert_eq!(listed(&path), [("dir/hello.txt".to_owned(), 5)], "{name}");
    }
    let text = write(&dir, "notes.txt", b"plain text");
    assert_eq!(detect(&text).unwrap_err().code, "archive.unsupported");
    assert_eq!(
        detect(&dir.join("missing.zip")).unwrap_err().code,
        "fs.not_found"
    );
}

#[test]
fn lists_single_compressed_files() {
    let dir = workspace("single");
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(&vec![b'a'; 10_000]).unwrap();
    let path = write(&dir, "server.log.gz", &e.finish().unwrap());
    assert_eq!(detect(&path).unwrap(), Format::Gz);
    assert_eq!(listed(&path), [("server.log".to_owned(), 10_000)]);
}

#[test]
fn normalizes_paths() {
    assert_eq!(normalize("a/b/c.txt").as_deref(), Some("a/b/c.txt"));
    assert_eq!(normalize("./a//b/").as_deref(), Some("a/b"));
    assert_eq!(normalize("a\\b.txt").as_deref(), Some("a/b.txt"));
    assert_eq!(normalize("../x"), None);
    assert_eq!(normalize("a/../../x"), None);
    assert_eq!(normalize("/etc/passwd"), None);
    assert_eq!(normalize("C:/Windows/x"), None);
    assert_eq!(normalize(""), None);
    assert_eq!(normalize("./"), None);
}

#[test]
fn reports_corrupt_archives() {
    let dir = workspace("corrupt");
    let path = write(&dir, "bad.zip", b"PK\x03\x04 this is not really a zip");
    let read = Arc::new(AtomicU64::new(0));
    let err = walk(&path, None, false, &read, &mut |_, _| Ok(Flow::Continue)).unwrap_err();
    assert_eq!(err.code, "archive.corrupt");
}
