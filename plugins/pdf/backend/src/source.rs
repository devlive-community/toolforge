//! 读取 PDF：解密、页面尺寸与旋转（考虑从页面树继承的属性）、文档信息。

use std::path::Path;

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 页面可以从上级页面树节点继承的属性
pub const INHERITABLE: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

#[derive(Debug, Clone, Deserialize)]
pub struct Source {
    pub path: String,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub keywords: String,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub producer: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    /// 页码，从 1 开始
    pub number: u32,
    /// 未旋转时的宽高（点）
    pub width: f32,
    pub height: f32,
    pub rotate: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub version: String,
    pub encrypted: bool,
    pub pages: Vec<PageInfo>,
    pub metadata: Metadata,
}

fn map_error(err: lopdf::Error, password: Option<&str>) -> PluginError {
    match err {
        lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword => {
            if password.is_some_and(|p| !p.is_empty()) {
                PluginError::new("pdf.wrong_password")
            } else {
                PluginError::new("pdf.password_required")
            }
        }
        lopdf::Error::IO(e) if e.kind() == std::io::ErrorKind::NotFound => {
            PluginError::new("fs.not_found")
        }
        other => PluginError::new("pdf.invalid").with("detail", other.to_string()),
    }
}

/// 加载并在需要时解密；只有所有者密码（可直接打开）的文件不需要提供密码
pub fn load(source: &Source) -> PluginResult<Document> {
    let password = source.password.as_deref().filter(|p| !p.is_empty());
    let path = Path::new(&source.path);
    if !path.is_file() {
        return Err(PluginError::new("fs.not_found").with("path", source.path.as_str()));
    }
    let mut doc = Document::load_with_password(path, password.unwrap_or(""))
        .map_err(|e| map_error(e, password).with("path", source.path.as_str()))?;
    if doc.is_encrypted() {
        doc.decrypt(password.unwrap_or(""))
            .map_err(|e| map_error(e, password).with("path", source.path.as_str()))?;
    }
    if doc.get_pages().is_empty() {
        return Err(PluginError::new("pdf.no_pages").with("path", source.path.as_str()));
    }
    Ok(doc)
}

/// 沿页面树向上查找属性
pub fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = doc.get_dictionary(page).ok()?;
    for _ in 0..32 {
        if let Ok(value) = current.get(key) {
            return Some(value.clone());
        }
        let parent = current.get(b"Parent").ok()?.as_reference().ok()?;
        current = doc.get_dictionary(parent).ok()?;
    }
    None
}

fn number(object: &Object) -> Option<f32> {
    match object {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

/// 页面尺寸：优先 CropBox，其次 MediaBox，缺省 A4
pub fn page_size(doc: &Document, page: ObjectId) -> (f32, f32) {
    let resolve = |object: Object| match object {
        Object::Reference(id) => doc.get_object(id).ok().cloned(),
        other => Some(other),
    };
    for key in [b"CropBox".as_slice(), b"MediaBox"] {
        if let Some(Object::Array(values)) = inherited(doc, page, key).and_then(resolve)
            && values.len() == 4
        {
            let numbers: Vec<f32> = values
                .iter()
                .filter_map(|v| resolve(v.clone()).as_ref().and_then(number))
                .collect();
            if numbers.len() == 4 {
                return (
                    (numbers[2] - numbers[0]).abs(),
                    (numbers[3] - numbers[1]).abs(),
                );
            }
        }
    }
    (595.0, 842.0)
}

pub fn page_rotation(doc: &Document, page: ObjectId) -> i64 {
    inherited(doc, page, b"Rotate")
        .and_then(|r| r.as_i64().ok())
        .map_or(0, |r| r.rem_euclid(360))
}

fn info_dictionary(doc: &Document) -> Option<&Dictionary> {
    match doc.trailer.get(b"Info").ok()? {
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        Object::Dictionary(dict) => Some(dict),
        _ => None,
    }
}

pub fn metadata(doc: &Document) -> Metadata {
    let Some(info) = info_dictionary(doc) else {
        return Metadata::default();
    };
    let text = |key: &[u8]| {
        info.get(key)
            .ok()
            .map(|value| match value {
                Object::Reference(id) => doc.get_object(*id).ok().cloned().unwrap_or(Object::Null),
                other => other.clone(),
            })
            .and_then(|value| lopdf::decode_text_string(&value).ok())
            .unwrap_or_default()
    };
    Metadata {
        title: text(b"Title"),
        author: text(b"Author"),
        subject: text(b"Subject"),
        keywords: text(b"Keywords"),
        creator: text(b"Creator"),
        producer: text(b"Producer"),
    }
}

pub fn info(source: &Source) -> PluginResult<Info> {
    let doc = load(source)?;
    let encrypted = Document::load(&source.path)
        .map(|d| d.is_encrypted())
        .unwrap_or(true);
    let pages = doc
        .get_pages()
        .into_iter()
        .map(|(number, id)| {
            let (width, height) = page_size(&doc, id);
            PageInfo {
                number,
                width,
                height,
                rotate: page_rotation(&doc, id),
            }
        })
        .collect();
    let path = Path::new(&source.path);
    Ok(Info {
        path: source.path.clone(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        version: doc.version.clone(),
        encrypted,
        metadata: metadata(&doc),
        pages,
    })
}

#[cfg(test)]
#[path = "source_test.rs"]
mod tests;
