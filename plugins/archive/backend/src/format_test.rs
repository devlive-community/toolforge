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

/// 按 Windows 中文系统的方式手写一个 ZIP：文件名为 GBK、不设 UTF-8 标记，内容不压缩
fn gbk_zip(name: &str, body: &[u8]) -> Vec<u8> {
    let name = encoding_rs::GB18030.encode(name).0.into_owned();
    let mut crc = flate2::Crc::new();
    crc.update(body);
    let crc = crc.sum();
    let mut out = Vec::new();
    let u16le = |v: u16| v.to_le_bytes();
    let u32le = |v: u32| v.to_le_bytes();
    // 本地文件头：版本 20、标志 0（无 UTF-8 标记）、不压缩
    out.extend(u32le(0x0403_4b50));
    out.extend(u16le(20));
    out.extend(u16le(0));
    out.extend(u16le(0));
    out.extend(u16le(0));
    out.extend(u16le(0x21));
    out.extend(u32le(crc));
    out.extend(u32le(body.len() as u32));
    out.extend(u32le(body.len() as u32));
    out.extend(u16le(name.len() as u16));
    out.extend(u16le(0));
    out.extend(&name);
    out.extend(body);
    let central = out.len() as u32;
    out.extend(u32le(0x0201_4b50));
    out.extend(u16le(20));
    out.extend(u16le(20));
    out.extend(u16le(0));
    out.extend(u16le(0));
    out.extend(u16le(0));
    out.extend(u16le(0x21));
    out.extend(u32le(crc));
    out.extend(u32le(body.len() as u32));
    out.extend(u32le(body.len() as u32));
    out.extend(u16le(name.len() as u16));
    out.extend(u16le(0));
    out.extend(u16le(0));
    out.extend(u16le(0));
    out.extend(u16le(0));
    out.extend(u32le(0));
    out.extend(u32le(0));
    out.extend(&name);
    let size = out.len() as u32 - central;
    out.extend(u32le(0x0605_4b50));
    out.extend(u16le(0));
    out.extend(u16le(0));
    out.extend(u16le(1));
    out.extend(u16le(1));
    out.extend(u32le(size));
    out.extend(u32le(central));
    out.extend(u16le(0));
    out
}

#[test]
fn reads_gbk_names_and_contents_from_windows_zips() {
    let dir = workspace("gbk");
    let body = encoding_rs::GB18030
        .encode("数据库地址=本地\n用户=管理员\n")
        .0
        .into_owned();
    let path = write(&dir, "中文.zip", &gbk_zip("资料/说明.txt", &body));
    assert_eq!(
        listed(&path),
        [("资料/说明.txt".to_owned(), body.len() as u64)]
    );
    let mut text = None;
    let read = Arc::new(AtomicU64::new(0));
    walk(&path, None, true, &read, &mut |_, reader| {
        let mut bytes = Vec::new();
        reader.unwrap().read_to_end(&mut bytes).unwrap();
        text = crate::text::content(&bytes, false);
        Ok(Flow::Stop)
    })
    .unwrap();
    assert_eq!(text.as_deref(), Some("数据库地址=本地\n用户=管理员\n"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reads_gbk_names_from_tars() {
    let dir = workspace("gbk-tar");
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(2);
    header.set_mode(0o644);
    let name = encoding_rs::GB18030.encode("文档.txt").0.into_owned();
    header.as_old_mut().name[..name.len()].copy_from_slice(&name);
    header.set_cksum();
    builder.append(&header, &b"ok"[..]).unwrap();
    let path = write(&dir, "a.tar", &builder.into_inner().unwrap());
    assert_eq!(listed(&path), [("文档.txt".to_owned(), 2)]);
    std::fs::remove_dir_all(dir).unwrap();
}
