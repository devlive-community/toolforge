use std::sync::Mutex;

use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
}

impl TaskContext for Ctx {
    fn log(&self, _level: LogLevel, code: &str, _params: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, _done: u64, _total: u64) {}
    fn stage(&self, _code: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        Ok(Box::new(File::open(path).unwrap()))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        Ok(std::fs::metadata(path).unwrap().len())
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tf-hash-manifest-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.txt"), b"abc").unwrap();
    std::fs::write(dir.join("sub").join("b.bin"), b"").unwrap();
    dir
}

#[test]
fn parses_gnu_bsd_and_escaped_lines() {
    let text = "# comment\n\
        ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  a.txt\n\
        D41D8CD98F00B204E9800998ECF8427E *sub/b.bin\n\
        SHA1 (c d.txt) = a9993e364706816aba3e25717850c26c9cd0d89d\n\
        \\0cc175b9c0f1b6a831c399e269772661  x\\\\y\\nz\n\
        garbage\n";
    let (lines, invalid) = parse(text);
    assert_eq!(invalid, vec![6]);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].name, "a.txt");
    assert_eq!(lines[1].name, "sub/b.bin");
    assert_eq!(lines[1].digest, "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(lines[2].algorithm, Some(Algorithm::Sha1));
    assert_eq!(lines[2].name, "c d.txt");
    assert_eq!(lines[3].name, "x\\y\nz");
}

#[test]
fn guesses_algorithm_from_extension_and_length() {
    assert_eq!(guess(Path::new("SHA256SUMS"), 64), Some(Algorithm::Sha256));
    assert_eq!(guess(Path::new("x.md5"), 64), Some(Algorithm::Md5));
    assert_eq!(guess(Path::new("x.sm3"), 64), Some(Algorithm::Sm3));
    assert_eq!(guess(Path::new("list.txt"), 40), Some(Algorithm::Sha1));
    assert_eq!(guess(Path::new("list.txt"), 10), None);
}

#[test]
fn relative_paths_walk_up_when_needed() {
    let base = Path::new("/data/out");
    assert_eq!(relative(base, Path::new("/data/out/a/b.txt")), "a/b.txt");
    assert_eq!(relative(base, Path::new("/data/src/c.txt")), "../src/c.txt");
}

#[test]
fn creates_manifest_and_verifies_it() {
    let dir = temp_dir("roundtrip");
    let output = dir.join("files.sha256");
    let ctx = Ctx::default();
    let report = create(
        CreateArgs {
            dir: dir.to_string_lossy().into_owned(),
            algorithm: Algorithm::Sha256,
            recursive: true,
            output: output.to_string_lossy().into_owned(),
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(report.summary.ok, 2);
    let text = std::fs::read_to_string(&output).unwrap();
    assert_eq!(
        text,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  a.txt\n\
         e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  sub/b.bin\n"
    );

    let verify_args = || VerifyArgs {
        manifest: output.to_string_lossy().into_owned(),
        algorithm: None,
    };
    let report = verify(verify_args(), &ctx).unwrap();
    assert_eq!(report.algorithm, Algorithm::Sha256);
    assert_eq!(
        (
            report.summary.ok,
            report.summary.failed,
            report.summary.missing
        ),
        (2, 0, 0)
    );

    std::fs::write(dir.join("a.txt"), b"changed").unwrap();
    std::fs::remove_file(dir.join("sub").join("b.bin")).unwrap();
    let report = verify(verify_args(), &ctx).unwrap();
    assert_eq!(
        (
            report.summary.ok,
            report.summary.failed,
            report.summary.missing
        ),
        (0, 1, 1)
    );
    assert_eq!(report.entries[0].status, Status::Failed);
    assert!(
        ctx.logs
            .lock()
            .unwrap()
            .contains(&"hash.manifest_mismatch".to_owned())
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn manifest_outside_directory_uses_parent_paths() {
    let dir = temp_dir("outside");
    let out_dir = dir.with_extension("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let output = out_dir.join("list.md5");
    let ctx = Ctx::default();
    create(
        CreateArgs {
            dir: dir.join("sub").to_string_lossy().into_owned(),
            algorithm: Algorithm::Md5,
            recursive: false,
            output: output.to_string_lossy().into_owned(),
        },
        &ctx,
    )
    .unwrap();
    let text = std::fs::read_to_string(&output).unwrap();
    assert!(
        text.ends_with("/sub/b.bin\n") && text.contains("  ../"),
        "{text}"
    );
    let report = verify(
        VerifyArgs {
            manifest: output.to_string_lossy().into_owned(),
            algorithm: None,
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(report.summary.ok, 1);
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(out_dir).unwrap();
}

#[test]
fn rejects_empty_manifests_and_crc32() {
    let dir = temp_dir("empty");
    let manifest = dir.join("empty.sha256");
    std::fs::write(&manifest, "# nothing\n").unwrap();
    let ctx = Ctx::default();
    let err = verify(
        VerifyArgs {
            manifest: manifest.to_string_lossy().into_owned(),
            algorithm: None,
        },
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, "hash.manifest_empty");
    let err = create(
        CreateArgs {
            dir: dir.to_string_lossy().into_owned(),
            algorithm: Algorithm::Crc32,
            recursive: true,
            output: dir.join("x").to_string_lossy().into_owned(),
        },
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, "hash.manifest_algorithm");
    std::fs::remove_dir_all(dir).unwrap();
}
