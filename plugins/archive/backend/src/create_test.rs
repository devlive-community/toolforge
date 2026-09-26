use super::*;
use crate::extract::{self, Conflict, Request};
use crate::format::{self, Kind};
use crate::test_support::{Ctx, sample, workspace};

fn args(sources: Vec<PathBuf>, output: &Path, format: Output, password: Option<&str>) -> Args {
    Args {
        sources: sources
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        output: output.to_string_lossy().into_owned(),
        format,
        level: 6,
        password: password.map(String::from),
        skip_system: true,
    }
}

fn names(path: &Path, password: Option<&str>) -> PluginResult<Vec<(String, Kind)>> {
    let mut names = Vec::new();
    let read = std::sync::Arc::new(AtomicU64::new(0));
    format::walk(path, password, false, &read, &mut |meta, _| {
        names.push((meta.path.clone(), meta.kind));
        Ok(format::Flow::Continue)
    })?;
    names.sort();
    Ok(names)
}

fn round_trip(out: Output, file: &str, password: Option<&str>) {
    let dir = workspace(file);
    let root = sample(&dir);
    let output = dir.join(file);
    let ctx = Ctx::default();
    let outcome = create(&args(vec![root.clone()], &output, out, password), &ctx).unwrap();
    assert_eq!(
        (outcome.files, outcome.skipped),
        (3, 1),
        "{file}: .DS_Store is skipped"
    );
    assert!(
        outcome.size > 0 && outcome.size < outcome.bytes,
        "{file} compresses: {outcome:?}"
    );
    assert!(!dir.join(format!(".{file}.part")).exists());

    let listed = names(&output, password).unwrap();
    let paths: Vec<&str> = listed.iter().map(|(p, _)| p.as_str()).collect();
    for expected in [
        "project/readme.md",
        "project/src/main.rs",
        "project/assets/data.bin",
        "project/empty",
    ] {
        assert!(paths.contains(&expected), "{file}: {paths:?}");
    }

    let dest = dir.join("out");
    let result = extract::extract(
        &Request {
            archive: &output,
            password,
            dest: &dest,
            entries: &[],
            conflict: Conflict::Rename,
            create_folder: false,
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(result.files, 3, "{file}");
    for name in ["readme.md", "src/main.rs", "assets/data.bin"] {
        assert_eq!(
            std::fs::read(dest.join("project").join(name)).unwrap(),
            std::fs::read(root.join(name)).unwrap(),
            "{file}: {name}"
        );
    }
    assert!(
        dest.join("project/empty").is_dir(),
        "{file}: empty folders are kept"
    );
    // 修改时间被还原（zip 精度为 2 秒）
    let original = std::fs::metadata(root.join("readme.md"))
        .unwrap()
        .modified()
        .unwrap();
    let restored = std::fs::metadata(dest.join("project/readme.md"))
        .unwrap()
        .modified()
        .unwrap();
    let diff = original
        .duration_since(restored)
        .or_else(|_| restored.duration_since(original))
        .unwrap();
    assert!(diff.as_secs() <= 2, "{file}: {diff:?}");
}

#[test]
fn round_trips_every_output_format() {
    round_trip(Output::Zip, "a.zip", None);
    round_trip(Output::TarGz, "a.tar.gz", None);
    round_trip(Output::TarXz, "a.tar.xz", None);
    round_trip(Output::SevenZ, "a.7z", None);
}

#[test]
fn encrypts_zip_and_7z() {
    round_trip(Output::Zip, "secret.zip", Some("s3cret"));
    round_trip(Output::SevenZ, "secret.7z", Some("s3cret"));

    let dir = workspace("passwords");
    let root = sample(&dir);
    let zip = dir.join("p.zip");
    create(
        &args(vec![root.clone()], &zip, Output::Zip, Some("right")),
        &Ctx::default(),
    )
    .unwrap();
    let target = dir.join("x");
    let request = |password| Request {
        archive: &zip,
        password,
        dest: &target,
        entries: &[],
        conflict: Conflict::Rename,
        create_folder: false,
    };
    assert_eq!(
        extract::extract(&request(None), &Ctx::default())
            .unwrap_err()
            .code,
        "archive.password_required"
    );
    assert_eq!(
        extract::extract(&request(Some("wrong")), &Ctx::default())
            .unwrap_err()
            .code,
        "archive.wrong_password"
    );

    // 7z 默认加密文件名：没有密码连列表都无法读取
    let seven = dir.join("p.7z");
    create(
        &args(vec![root], &seven, Output::SevenZ, Some("right")),
        &Ctx::default(),
    )
    .unwrap();
    assert_eq!(
        names(&seven, None).unwrap_err().code,
        "archive.password_required"
    );
    assert_eq!(
        names(&seven, Some("wrong")).unwrap_err().code,
        "archive.wrong_password"
    );
}

#[test]
fn validates_and_cleans_up() {
    let dir = workspace("validate");
    let root = sample(&dir);
    let out = dir.join("x.tar.gz");
    assert_eq!(
        create(&args(vec![], &out, Output::Zip, None), &Ctx::default())
            .unwrap_err()
            .code,
        "archive.no_sources"
    );
    let err = create(
        &args(vec![root.clone()], &out, Output::TarGz, Some("pw")),
        &Ctx::default(),
    )
    .unwrap_err();
    assert_eq!(err.code, "archive.password_unsupported");
    let ctx = Ctx::default();
    ctx.cancelled.store(true, Ordering::Relaxed);
    let err = create(&args(vec![root], &out, Output::Zip, None), &ctx).unwrap_err();
    assert_eq!(err.code, "task.cancelled");
    assert!(!out.exists() && !dir.join(".x.tar.gz.part").exists());
}

#[cfg(unix)]
#[test]
fn skips_symlinks_when_creating() {
    let dir = workspace("symlink");
    let root = sample(&dir);
    std::os::unix::fs::symlink("/etc/hosts", root.join("hosts")).unwrap();
    let out = dir.join("s.zip");
    let outcome = create(&args(vec![root], &out, Output::Zip, None), &Ctx::default()).unwrap();
    assert_eq!(outcome.skipped, 2);
    assert!(
        !names(&out, None)
            .unwrap()
            .iter()
            .any(|(p, _)| p.ends_with("hosts"))
    );
}

/// 与系统工具互通：`TOOLFORGE_ARCHIVE_INTEROP=<目录> cargo test -p tfp-archive interop -- --ignored --nocapture`
/// 在目录中写出本插件创建的压缩包，并读取目录 in/ 中由系统工具创建的压缩包
#[test]
#[ignore]
fn interop() {
    let Some(dir) = std::env::var_os("TOOLFORGE_ARCHIVE_INTEROP") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let root = sample(&dir);
    for (format, name, password) in [
        (Output::Zip, "ours.zip", None),
        (Output::Zip, "ours-aes.zip", Some("pw123")),
        (Output::TarGz, "ours.tar.gz", None),
        (Output::TarXz, "ours.tar.xz", None),
        (Output::SevenZ, "ours.7z", None),
        (Output::SevenZ, "ours-aes.7z", Some("pw123")),
    ] {
        let _ = std::fs::remove_file(dir.join(name));
        create(
            &args(vec![root.clone()], &dir.join(name), format, password),
            &Ctx::default(),
        )
        .unwrap();
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("in")) {
        for entry in entries.flatten() {
            let path = entry.path();
            let dest = dir.join("out").join(path.file_name().unwrap());
            let _ = std::fs::remove_dir_all(&dest);
            let result = extract::extract(
                &Request {
                    archive: &path,
                    password: Some("pw123"),
                    dest: &dest,
                    entries: &[],
                    conflict: Conflict::Rename,
                    create_folder: false,
                },
                &Ctx::default(),
            );
            println!(
                "{:?}: {:?}",
                path.file_name().unwrap(),
                result.map(|o| (o.files, o.bytes)).map_err(|e| e.code)
            );
        }
    }
}
