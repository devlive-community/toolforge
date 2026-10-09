use std::io::Read;
use std::sync::Mutex;

use super::*;

#[derive(Default)]
struct Ctx(Mutex<Vec<String>>);

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.0.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

fn dir(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tfp-icon-generator-{name}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn logo(dir: &Path) -> String {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#1e90ff"/></svg>"##;
    let path = dir.join("my-app.svg");
    std::fs::write(&path, svg).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn inspects_and_renders_previews() {
    let dir = dir("render");
    let path = logo(&dir);
    let tool = IconGenerator::default();
    let info = tool.call("inspect", json!({ "path": path })).unwrap();
    assert_eq!(info["svg"], true);
    assert_eq!(info["square"], true);
    assert_eq!(info["transparent"], true);
    assert_eq!(info["small"], false);
    assert_eq!(info["name"], "my-app");
    let previews = tool
        .call("render", json!({ "path": path, "style": { "padding": 0.1, "background": "#ffffff", "radius": 0.2 } }))
        .unwrap();
    for key in ["rounded", "opaque", "circle", "mac"] {
        assert!(
            previews[key]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
    }
    assert_eq!(
        tool.call(
            "render",
            json!({ "path": path, "style": { "radius": 0.9 } })
        )
        .unwrap_err()
        .code,
        "icon.invalid_style"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn generates_every_preset() {
    let dir = dir("generate");
    let path = logo(&dir);
    let out_dir = dir.join("out");
    std::fs::create_dir_all(out_dir.join("my-app-icons")).unwrap();
    let ctx = Ctx::default();
    let result = IconGenerator::default()
        .run_task(
            "generate",
            json!({
                "path": path,
                "presets": ["windows", "web", "ios", "android", "macos", "web"],
                "outputDir": out_dir.to_string_lossy(),
                "name": "My App",
                "style": { "padding": 0.08, "radius": 0.18 },
            }),
            &ctx,
        )
        .unwrap();
    // 已有同名文件夹时另起一个
    let root = out_dir.join("my-app-icons (1)");
    assert_eq!(result["dir"], root.to_string_lossy().as_ref());
    let expected = [
        "web/favicon.ico",
        "web/favicon-16x16.png",
        "web/favicon-32x32.png",
        "web/apple-touch-icon.png",
        "web/android-chrome-192x192.png",
        "web/android-chrome-512x512.png",
        "web/site.webmanifest",
        "web/snippet.html",
        "ios/AppIcon.appiconset/Contents.json",
        "ios/AppIcon.appiconset/Icon-167.png",
        "ios/AppIcon.appiconset/Icon-1024.png",
        "android/mipmap-xxxhdpi/ic_launcher.png",
        "android/mipmap-mdpi/ic_launcher_round.png",
        "android/play-store-512.png",
        "macos/AppIcon.icns",
        "macos/AppIcon-1024.png",
        "windows/app.ico",
    ];
    for file in expected {
        let path = file.split('/').fold(root.clone(), |p, part| p.join(part));
        assert!(path.is_file(), "{file}");
    }
    // iOS：18 个条目共用 13 个不同尺寸
    let contents: Value = serde_json::from_slice(
        &std::fs::read(
            root.join("ios")
                .join("AppIcon.appiconset")
                .join("Contents.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(contents["images"].as_array().unwrap().len(), 18);
    let pngs = std::fs::read_dir(root.join("ios").join("AppIcon.appiconset"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "png")
        })
        .count();
    assert_eq!(pngs, 13);
    // iOS 图标不透明，尺寸正确
    let icon = image::open(
        root.join("ios")
            .join("AppIcon.appiconset")
            .join("Icon-167.png"),
    )
    .unwrap()
    .into_rgba8();
    assert_eq!(icon.dimensions(), (167, 167));
    assert!(icon.pixels().all(|p| p[3] == 255));
    let mipmap = image::open(
        root.join("android")
            .join("mipmap-hdpi")
            .join("ic_launcher.png"),
    )
    .unwrap();
    assert_eq!(mipmap.width(), 72);
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(root.join("web").join("site.webmanifest")).unwrap())
            .unwrap();
    assert_eq!(manifest["name"], "My App");
    assert_eq!(result["files"], 36);
    assert!(
        result["snippet"]
            .as_str()
            .unwrap()
            .contains("apple-touch-icon")
    );
    let logs = ctx.0.lock().unwrap();
    assert_eq!(logs.iter().filter(|c| *c == "icon.preset_done").count(), 5);
    assert_eq!(logs.last().unwrap(), "icon.done");
    drop(logs);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejects_bad_arguments() {
    let dir = dir("errors");
    let path = logo(&dir);
    let ctx = Ctx::default();
    let err = |args: Value| {
        IconGenerator::default()
            .run_task("generate", args, &ctx)
            .unwrap_err()
            .code
    };
    assert_eq!(
        err(json!({ "path": path, "presets": [], "outputDir": dir.to_string_lossy() })),
        "icon.no_presets"
    );
    assert_eq!(
        err(json!({ "path": path, "presets": ["web"], "outputDir": "/definitely/not/here" })),
        "icon.output_dir_missing"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn lists_and_renders_templates_with_or_without_an_image() {
    let tool = IconGenerator::default();
    let list = tool.call("templates", json!({})).unwrap();
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), templates::TEMPLATES.len());
    assert_eq!(list[0]["id"], "folder");
    assert_eq!(list[0]["group"], "folder");
    assert!(
        list[0]["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    let dir = dir("templates");
    let path = logo(&dir);
    let one = tool
        .call(
            "renderTemplate",
            json!({ "path": path, "template": "cdDisc", "size": 512 }),
        )
        .unwrap();
    assert!(one.as_str().unwrap().len() > 1000);
    let err = tool
        .call("renderTemplate", json!({ "template": "nope" }))
        .unwrap_err();
    assert_eq!(err.code, "icon.unknown_template");
}

#[test]
fn exports_a_template_in_several_formats() {
    let dir = dir("export");
    let path = logo(&dir);
    let tool = IconGenerator::default();
    let ctx = Ctx::default();
    let args = json!({ "path": path, "template": "folder", "formats": ["png", "icns", "ico"], "outputDir": dir });
    let out = tool.run_task("exportTemplate", args.clone(), &ctx).unwrap();
    let files: Vec<String> = serde_json::from_value(out["files"].clone()).unwrap();
    assert_eq!(files.len(), 3);
    assert!(files[0].ends_with("my-app-folder.png"));
    let png = image::open(&files[0]).unwrap();
    assert_eq!((png.width(), png.height()), (1024, 1024));
    let icns = std::fs::read(&files[1]).unwrap();
    assert_eq!(&icns[..4], b"icns");
    // 再导出一次不覆盖
    let again = tool.run_task("exportTemplate", args, &ctx).unwrap();
    assert!(
        again["files"][0]
            .as_str()
            .unwrap()
            .ends_with("my-app-folder (1).png")
    );
    let none = tool.run_task(
        "exportTemplate",
        json!({ "template": "folder", "formats": [], "outputDir": dir }),
        &ctx,
    );
    assert_eq!(none.unwrap_err().code, "icon.no_formats");
}

#[cfg(target_os = "macos")]
#[test]
fn applies_and_clears_icons_on_folders() {
    let dir = dir("apply");
    let target = dir.join("Photos");
    std::fs::create_dir_all(&target).unwrap();
    let tool = IconGenerator::default();
    let ctx = Ctx::default();
    let missing = dir.join("missing").to_string_lossy().into_owned();
    let out = tool
        .run_task(
            "applyIcon",
            json!({ "template": "folderDark", "targets": [target, missing] }),
            &ctx,
        )
        .unwrap();
    assert_eq!(
        (out["applied"].as_u64(), out["total"].as_u64()),
        (Some(1), Some(2))
    );
    assert!(target.join("Icon\r").exists());
    assert!(ctx.0.lock().unwrap().contains(&"fs.not_found".to_owned()));
    tool.call("clearIcon", json!({ "targets": [target] }))
        .unwrap();
    assert!(!target.join("Icon\r").exists());
}
