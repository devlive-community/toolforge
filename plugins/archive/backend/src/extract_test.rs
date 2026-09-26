use std::io::Write;

use super::*;
use crate::test_support::{Ctx, workspace, write};

fn zip_with(path: &Path, files: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, content) in files {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap();
}

/// 手工构造带危险名字的 tar（tar 库自身会拒绝 ..）
fn tar_with(path: &Path, entries: &[(&str, tar::EntryType, &[u8], Option<&str>)]) {
    let mut builder = tar::Builder::new(File::create(path).unwrap());
    for (name, kind, content, link) in entries {
        let mut header = tar::Header::new_gnu();
        let bytes = name.as_bytes();
        header.as_old_mut().name[..bytes.len()].copy_from_slice(bytes);
        header.set_entry_type(*kind);
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        if let Some(link) = link {
            header.set_link_name(link).unwrap();
        }
        header.set_cksum();
        builder.append(&header, *content).unwrap();
    }
    builder.finish().unwrap();
}

fn run(
    archive: &Path,
    dest: &Path,
    entries: &[String],
    conflict: Conflict,
    create_folder: bool,
) -> PluginResult<Outcome> {
    extract(
        &Request {
            archive,
            password: None,
            dest,
            entries,
            conflict,
            create_folder,
        },
        &Ctx::default(),
    )
}

#[test]
fn blocks_path_traversal() {
    let dir = workspace("slip");
    let dest = dir.join("dest");
    let zip = dir.join("evil.zip");
    zip_with(
        &zip,
        &[
            ("../evil.txt", b"x"),
            ("/abs.txt", b"y"),
            ("ok/fine.txt", b"z"),
            ("C:/win.txt", b"w"),
        ],
    );
    let outcome = run(&zip, &dest, &[], Conflict::Rename, false).unwrap();
    assert_eq!((outcome.files, outcome.unsafe_paths), (1, 3));
    assert!(!dir.join("evil.txt").exists() && !Path::new("/abs.txt").exists());
    assert!(dest.join("ok/fine.txt").exists());

    let tar = dir.join("evil.tar");
    tar_with(
        &tar,
        &[
            ("../up.txt", tar::EntryType::Regular, b"x", None),
            ("link", tar::EntryType::Symlink, b"", Some("/etc/passwd")),
            ("safe.txt", tar::EntryType::Regular, b"ok", None),
        ],
    );
    let dest = dir.join("tar-dest");
    let outcome = run(&tar, &dest, &[], Conflict::Rename, false).unwrap();
    assert_eq!(
        (outcome.files, outcome.unsafe_paths, outcome.links),
        (1, 1, 1)
    );
    assert!(!dir.join("up.txt").exists());
    assert!(
        std::fs::symlink_metadata(dest.join("link")).is_err(),
        "links are not created"
    );
}

#[cfg(unix)]
#[test]
fn does_not_write_through_existing_symlinks() {
    let dir = workspace("through");
    let outside = dir.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let dest = dir.join("dest");
    std::fs::create_dir_all(&dest).unwrap();
    std::os::unix::fs::symlink(&outside, dest.join("sub")).unwrap();
    let zip = dir.join("a.zip");
    zip_with(&zip, &[("sub/file.txt", b"x")]);
    let outcome = run(&zip, &dest, &[], Conflict::Overwrite, false).unwrap();
    assert_eq!(outcome.unsafe_paths, 1);
    assert!(!outside.join("file.txt").exists());
}

#[test]
fn handles_conflicts_selection_and_folders() {
    let dir = workspace("conflict");
    let zip = dir.join("photos.tar.gz.zip");
    zip_with(
        &zip,
        &[
            ("a.txt", b"new"),
            ("docs/b.txt", b"b"),
            ("docs/c.txt", b"c"),
            ("other/d.txt", b"d"),
        ],
    );
    let dest = dir.join("dest");
    write(&dest, "a.txt", b"old");

    let skip = run(&zip, &dest, &["a.txt".into()], Conflict::Skip, false).unwrap();
    assert_eq!((skip.files, skip.skipped), (0, 1));
    assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"old");

    run(&zip, &dest, &["a.txt".into()], Conflict::Rename, false).unwrap();
    assert_eq!(std::fs::read(dest.join("a (1).txt")).unwrap(), b"new");

    run(&zip, &dest, &["a.txt".into()], Conflict::Overwrite, false).unwrap();
    assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"new");

    let only_docs = run(&zip, &dest, &["docs".into()], Conflict::Rename, false).unwrap();
    assert_eq!(only_docs.files, 2);
    assert!(!dest.join("other").exists());

    let first = run(&zip, &dest, &[], Conflict::Rename, true).unwrap();
    let second = run(&zip, &dest, &[], Conflict::Rename, true).unwrap();
    assert!(first.dest.ends_with("photos.tar.gz"), "{}", first.dest);
    assert!(
        second.dest.ends_with("photos.tar.gz (1)"),
        "{}",
        second.dest
    );
}

#[test]
fn names_folders_after_archives() {
    assert_eq!(folder_name(Path::new("/x/photos.tar.gz")), "photos");
    assert_eq!(folder_name(Path::new("/x/Backup 2024.ZIP")), "Backup 2024");
    assert_eq!(folder_name(Path::new("/x/data.tgz")), "data");
    assert_eq!(folder_name(Path::new("/x/.zip")), ".zip");
    assert_eq!(folder_name(Path::new("/x/noext")), "noext");
}
