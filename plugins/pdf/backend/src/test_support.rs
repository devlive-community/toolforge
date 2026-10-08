//! 测试共用：生成示例 PDF、临时目录与任务上下文。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use lopdf::{Dictionary, Document, Object, Stream};
use serde_json::Value;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};

/// 每页填充的颜色（红、绿、蓝、黄……循环），便于在缩略图与重排结果中区分页面
pub const COLORS: [(f32, f32, f32); 4] = [
    (1.0, 0.0, 0.0),
    (0.0, 0.8, 0.0),
    (0.0, 0.0, 1.0),
    (1.0, 0.85, 0.0),
];

pub fn workspace(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tfp-pdf-{label}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 用键值对构造字典
pub fn dict(entries: Vec<(&str, Object)>) -> Dictionary {
    let mut dict = Dictionary::new();
    for (key, value) in entries {
        dict.set(key, value);
    }
    dict
}

fn name(value: &str) -> Object {
    Object::Name(value.as_bytes().to_vec())
}

/// 生成示例：页面尺寸与字体资源由页面树继承，第 3 页起放在一个旋转 90 度的中间节点下
pub fn sample_doc(pages: u32, title: &str) -> Document {
    let mut doc = Document::with_version("1.5");
    let root_id = doc.new_object_id();
    let font_id = doc.add_object(dict(vec![
        ("Type", name("Font")),
        ("Subtype", name("Type1")),
        ("BaseFont", name("Helvetica")),
    ]));
    let mut fonts = Dictionary::new();
    fonts.set("F1", font_id);
    let mut resources = Dictionary::new();
    resources.set("Font", fonts);
    let middle_id = doc.new_object_id();
    let mut root_kids = Vec::new();
    let mut middle_kids = Vec::new();
    for number in 1..=pages {
        let (r, g, b) = COLORS[(number as usize - 1) % COLORS.len()];
        let content = format!(
            "{r} {g} {b} rg 0 0 300 400 re f BT /F1 24 Tf 0 0 0 rg 40 200 Td (Page {number}) Tj ET"
        );
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let parent = if number >= 3 { middle_id } else { root_id };
        let page_id = doc.add_object(dict(vec![
            ("Type", name("Page")),
            ("Parent", parent.into()),
            ("Contents", content_id.into()),
        ]));
        if number >= 3 {
            middle_kids.push(page_id.into());
        } else {
            root_kids.push(page_id.into());
        }
    }
    if !middle_kids.is_empty() {
        let count = middle_kids.len() as i64;
        doc.objects.insert(
            middle_id,
            Object::Dictionary(dict(vec![
                ("Type", name("Pages")),
                ("Parent", root_id.into()),
                ("Kids", Object::Array(middle_kids)),
                ("Count", count.into()),
                ("Rotate", 90.into()),
            ])),
        );
        root_kids.push(middle_id.into());
    }
    doc.objects.insert(
        root_id,
        Object::Dictionary(dict(vec![
            ("Type", name("Pages")),
            ("Kids", Object::Array(root_kids)),
            ("Count", (pages as i64).into()),
            (
                "MediaBox",
                Object::Array(vec![0.into(), 0.into(), 300.into(), 400.into()]),
            ),
            ("Resources", resources.into()),
        ])),
    );
    let catalog = doc.add_object(dict(vec![
        ("Type", name("Catalog")),
        ("Pages", root_id.into()),
    ]));
    let info = doc.add_object(dict(vec![
        ("Title", lopdf::text_string(title)),
        ("Author", Object::string_literal("Ada")),
    ]));
    doc.trailer.set("Root", catalog);
    doc.trailer.set("Info", info);
    doc
}

pub fn sample(dir: &Path, name: &str, pages: u32) -> String {
    let path = dir.join(name);
    sample_doc(pages, name).save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

pub fn encrypted(dir: &Path, name: &str, password: &str) -> String {
    let mut doc = sample_doc(2, "secret");
    doc.trailer.set(
        "ID",
        vec![
            Object::string_literal(b"0123456789abcdef".to_vec()),
            Object::string_literal(b"0123456789abcdef".to_vec()),
        ],
    );
    let version = lopdf::EncryptionVersion::V2 {
        document: &doc,
        owner_password: "owner",
        user_password: password,
        key_length: 128,
        permissions: lopdf::Permissions::default(),
    };
    let state = lopdf::EncryptionState::try_from(version).unwrap();
    doc.encrypt(&state).unwrap();
    let path = dir.join(name);
    doc.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

#[derive(Default)]
pub struct Ctx(pub Mutex<Vec<String>>);

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
