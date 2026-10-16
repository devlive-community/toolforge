use super::*;

fn resolve_with(path: &str, root: &str, mappings: &[(&str, &str)]) -> PluginResult<Resolved> {
    resolve(&Args {
        path: path.to_owned(),
        root: root.to_owned(),
        mappings: mappings
            .iter()
            .map(|(from, to)| Mapping {
                from: (*from).to_owned(),
                to: (*to).to_owned(),
            })
            .collect(),
    })
}

#[test]
fn maps_ci_prefixes_with_the_longest_match() {
    let out = resolve_with(
        "/home/runner/work/app/app/src/lib.rs",
        "/Users/me/app",
        &[
            ("/home/runner", "/elsewhere"),
            ("/home/runner/work/app/app", "/Users/me/app"),
        ],
    )
    .unwrap();
    assert_eq!(out.path, "/Users/me/app/src/lib.rs");
    assert_eq!(out.mapping.as_deref(), Some("/home/runner/work/app/app"));
}

#[test]
fn joins_relative_paths_to_the_root() {
    assert_eq!(
        resolve_with("src/./a.rs", "/r/", &[]).unwrap().path,
        "/r/src/a.rs"
    );
    // 映射到相对目录也拼在源码根下
    assert_eq!(
        resolve_with("/ci/x/a.rs", "/r", &[("/ci", "sub")])
            .unwrap()
            .path,
        "/r/sub/x/a.rs"
    );
}

#[test]
fn handles_windows_paths() {
    let out = resolve_with(
        "D:\\a\\proj\\src\\Main.cs",
        "C:\\Users\\me\\proj",
        &[("d:/a/proj", "C:\\Users\\me\\proj")],
    )
    .unwrap();
    assert_eq!(out.path, "C:\\Users\\me\\proj\\src\\Main.cs");
    // 盘符与 Windows 路径段不区分大小写
    assert!(resolve_with("c:/users/ME/proj/x.cs", "C:\\Users\\me\\proj", &[]).is_ok());
    // UNC 路径
    assert!(resolve_with("\\\\srv\\share\\p\\a.c", "//srv/share/p", &[]).is_ok());
}

#[test]
fn rejects_paths_outside_the_root() {
    let err = |path: &str, root: &str| resolve_with(path, root, &[]).unwrap_err().code;
    assert_eq!(err("../secret.txt", "/r"), "lcov.outside_root");
    assert_eq!(err("src/../../etc/passwd", "/r"), "lcov.outside_root");
    assert_eq!(err("/etc/passwd", "/r"), "lcov.outside_root");
    assert_eq!(err("/rest/a.rs", "/r"), "lcov.outside_root");
    // POSIX 路径区分大小写
    assert_eq!(err("/R/a.rs", "/r"), "lcov.outside_root");
    assert_eq!(err("a.rs", "relative/root"), "lcov.root_not_absolute");
}
