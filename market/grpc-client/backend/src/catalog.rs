//! 服务定义：从 .proto 源文件（含导入目录）、FileDescriptorSet 或服务器反射得到描述符池，
//! 整理成「服务 → 方法」目录。
//!
//! .proto 由 protox 在插件内编译；导入按用户给出的目录顺序经宿主读取文件解析，
//! google/protobuf 的标准类型内置。缺少导入时指出缺的是哪个文件、由哪个文件导入。

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::rc::Rc;

use prost_reflect::{DescriptorPool, MethodDescriptor};
use protox::file::{ChainFileResolver, File, FileResolver, GoogleFileResolver};
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult, TaskContext};

/// 一次加载最多读取的源文件数
const MAX_FILES: usize = 500;
/// 单个源文件 / 描述符文件的大小上限
const MAX_FILE: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Method {
    pub name: String,
    /// `/包名.服务/方法`，即 gRPC 请求路径
    pub path: String,
    pub input: String,
    pub output: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    /// 首版只支持一元与服务端流
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub name: String,
    pub methods: Vec<Method>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub services: Vec<Service>,
    pub files: usize,
    pub messages: usize,
}

pub fn method_path(method: &MethodDescriptor) -> String {
    format!("/{}/{}", method.parent_service().full_name(), method.name())
}

/// 整理描述符池中的全部服务；反射服务本身不列出
pub fn catalog(pool: &DescriptorPool) -> Catalog {
    let mut services: Vec<Service> = pool
        .services()
        .filter(|s| !s.full_name().starts_with("grpc.reflection."))
        .map(|service| Service {
            name: service.full_name().to_owned(),
            methods: service
                .methods()
                .map(|m| Method {
                    name: m.name().to_owned(),
                    path: method_path(&m),
                    input: m.input().full_name().to_owned(),
                    output: m.output().full_name().to_owned(),
                    client_streaming: m.is_client_streaming(),
                    server_streaming: m.is_server_streaming(),
                    supported: !m.is_client_streaming(),
                })
                .collect(),
        })
        .collect();
    services.sort_by(|a, b| a.name.cmp(&b.name));
    Catalog {
        services,
        files: pool.files().count(),
        messages: pool.all_messages().count(),
    }
}

/// 按 `/包名.服务/方法` 或 `包名.服务/方法` 查找方法
pub fn find_method(pool: &DescriptorPool, path: &str) -> PluginResult<MethodDescriptor> {
    let trimmed = path.trim().trim_start_matches('/');
    let (service, method) = trimmed
        .rsplit_once('/')
        .ok_or_else(|| PluginError::new("grpc.method_not_found").with("method", path))?;
    pool.get_service_by_name(service)
        .and_then(|s| s.methods().find(|m| m.name() == method))
        .ok_or_else(|| PluginError::new("grpc.method_not_found").with("method", path))
}

fn read_text(path: &str, ctx: &dyn TaskContext) -> PluginResult<Option<String>> {
    let Ok(size) = ctx.file_size(path) else {
        return Ok(None);
    };
    if size > MAX_FILE {
        return Err(PluginError::new("grpc.file_too_large")
            .with("path", path)
            .with("mb", MAX_FILE >> 20));
    }
    let mut text = String::new();
    ctx.open_file(path)?
        .read_to_string(&mut text)
        .map_err(|e| {
            PluginError::new("grpc.read_failed")
                .with("path", path)
                .with("detail", e.to_string())
        })?;
    Ok(Some(text))
}

/// 已读入内存的源文件；找不到的名字记下来，由外层读取后重新编译
struct Sources {
    files: HashMap<String, String>,
    misses: Rc<RefCell<Vec<String>>>,
}

impl FileResolver for Sources {
    fn open_file(&self, name: &str) -> Result<File, protox::Error> {
        match self.files.get(name) {
            Some(source) => File::from_source(name, source),
            None => {
                self.misses.borrow_mut().push(name.to_owned());
                Err(protox::Error::file_not_found(name))
            }
        }
    }
}

fn compile_error(err: &protox::Error) -> PluginError {
    let mut out = PluginError::new("grpc.proto_invalid").with("detail", err.to_string());
    if let Some(file) = err.file() {
        out = out.with("file", file);
    }
    out
}

fn join(dir: &str, name: &str) -> String {
    Path::new(dir).join(name).to_string_lossy().into_owned()
}

/// 编译 .proto：主文件所在目录自动作为第一个导入目录
pub fn load_proto(
    path: &str,
    includes: &[String],
    ctx: &dyn TaskContext,
) -> PluginResult<DescriptorPool> {
    let main = Path::new(path);
    let name = main
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| PluginError::new("grpc.read_failed").with("path", path))?;
    let mut dirs: Vec<String> = Vec::new();
    if let Some(parent) = main.parent().filter(|p| !p.as_os_str().is_empty()) {
        dirs.push(parent.to_string_lossy().into_owned());
    }
    dirs.extend(includes.iter().filter(|d| !d.trim().is_empty()).cloned());

    let mut files = HashMap::new();
    let source = read_text(path, ctx)?
        .ok_or_else(|| PluginError::new("grpc.read_failed").with("path", path))?;
    files.insert(name.clone(), source);
    loop {
        if ctx.is_cancelled() {
            return Err(tf_plugin_api::cancelled());
        }
        let misses = Rc::new(RefCell::new(Vec::new()));
        let mut resolver = ChainFileResolver::new();
        resolver.add(Sources {
            files: files.clone(),
            misses: misses.clone(),
        });
        resolver.add(GoogleFileResolver::new());
        let mut compiler = protox::Compiler::with_file_resolver(resolver);
        compiler.include_imports(true);
        let err = match compiler.open_file(&name) {
            Ok(_) => return Ok(compiler.descriptor_pool()),
            Err(err) => err,
        };
        let missing: Vec<String> = misses.borrow().clone();
        // google/protobuf 下的文件由内置解析器提供，未命中也会经过这里记一次
        let missing: Vec<String> = missing
            .into_iter()
            .filter(|m| !files.contains_key(m))
            .collect();
        if missing.is_empty() || !err.is_file_not_found() {
            return Err(compile_error(&err));
        }
        let mut progressed = false;
        for import in missing {
            if import.starts_with("google/protobuf/") {
                continue;
            }
            let found = dirs
                .iter()
                .map(|dir| join(dir, &import))
                .find_map(|candidate| read_text(&candidate, ctx).transpose());
            match found {
                Some(text) => {
                    files.insert(import, text?);
                    progressed = true;
                }
                None => {
                    let mut out = PluginError::new("grpc.import_missing")
                        .with("import", import.as_str())
                        .with("searched", dirs.join(", "));
                    if let Some(from) = err.file() {
                        out = out.with("from", from);
                    }
                    return Err(out);
                }
            }
        }
        if !progressed {
            return Err(compile_error(&err));
        }
        if files.len() > MAX_FILES {
            return Err(PluginError::new("grpc.too_many_files").with("limit", MAX_FILES));
        }
        ctx.progress(files.len() as u64, 0);
    }
}

/// 读取 FileDescriptorSet（protoc --descriptor_set_out --include_imports 的输出）
pub fn load_descriptor(path: &str, ctx: &dyn TaskContext) -> PluginResult<DescriptorPool> {
    let size = ctx.file_size(path)?;
    if size > MAX_FILE {
        return Err(PluginError::new("grpc.file_too_large")
            .with("path", path)
            .with("mb", MAX_FILE >> 20));
    }
    let mut bytes = Vec::new();
    ctx.open_file(path)?.read_to_end(&mut bytes).map_err(|e| {
        PluginError::new("grpc.read_failed")
            .with("path", path)
            .with("detail", e.to_string())
    })?;
    DescriptorPool::decode(bytes.as_slice())
        .map_err(|e| PluginError::new("grpc.descriptor_invalid").with("detail", e.to_string()))
}

#[cfg(test)]
#[path = "catalog_test.rs"]
mod tests;
