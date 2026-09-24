//! 插件资源（模型等大文件）的下载、校验与存放。
//!
//! 布局：`<root>/<plugin-id>/<resource-id>` 为资源文件，`<resource-id>.json` 为安装记录，
//! 下载中的数据写入 `<resource-id>.part`，中断或取消后可断点续传。

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tf_plugin_api::{LogLevel, PluginError, PluginResult, ResourceSpec, TaskContext, cancelled};

use crate::store::now_millis;
use crate::{AppError, AppResult};

const CHUNK: usize = 256 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// 阻塞客户端的超时作用于每次等待（发送请求、每次读取），相当于无数据停滞超时
const STALL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceStatus {
    pub id: String,
    pub size: u64,
    pub license: Option<String>,
    pub installed: bool,
    /// 已下载但未完成的字节数（可续传）
    pub partial: u64,
    pub installed_at: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    sha256: String,
    size: u64,
    url: String,
    installed_at: u64,
}

pub struct Resources {
    root: PathBuf,
}

fn invalid_id(id: &str) -> PluginError {
    PluginError::new("resource.invalid_id").with("id", id)
}

fn io_error(err: std::io::Error, path: &Path) -> PluginError {
    PluginError::new("fs.io")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", err.to_string())
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or(url)
        .to_owned()
}

fn client() -> PluginResult<reqwest::blocking::Client> {
    // 与 updater 共用 rustls 的 ring 实现，只需安装一次
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    });
    reqwest::blocking::Client::builder()
        .user_agent(concat!("ToolForge/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(STALL_TIMEOUT)
        .build()
        .map_err(|e| PluginError::new("resource.network").with("detail", e.to_string()))
}

impl Resources {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn dir(&self, plugin_id: &str) -> PluginResult<PathBuf> {
        // 插件 id 形如 org.devlive.toolforge.xxx，同样不允许路径分隔符
        if plugin_id.is_empty() || plugin_id.contains(['/', '\\']) || plugin_id.starts_with('.') {
            return Err(PluginError::new("resource.invalid_id").with("id", plugin_id));
        }
        Ok(self.root.join(plugin_id))
    }

    fn paths(&self, plugin_id: &str, id: &str) -> PluginResult<(PathBuf, PathBuf, PathBuf)> {
        if !ResourceSpec::valid_id(id) {
            return Err(invalid_id(id));
        }
        let dir = self.dir(plugin_id)?;
        Ok((
            dir.join(id),
            dir.join(format!("{id}.json")),
            dir.join(format!("{id}.part")),
        ))
    }

    fn record(&self, plugin_id: &str, id: &str) -> Option<(PathBuf, Record)> {
        let (file, meta, _) = self.paths(plugin_id, id).ok()?;
        let record: Record = serde_json::from_slice(&fs::read(meta).ok()?).ok()?;
        let size = fs::metadata(&file).ok()?.len();
        (size == record.size).then_some((file, record))
    }

    /// 已安装资源的路径
    pub fn path(&self, plugin_id: &str, id: &str) -> Option<PathBuf> {
        self.record(plugin_id, id).map(|(file, _)| file)
    }

    pub fn status(&self, plugin_id: &str, specs: &[ResourceSpec]) -> Vec<ResourceStatus> {
        specs
            .iter()
            .map(|spec| {
                let record = self
                    .record(plugin_id, &spec.id)
                    .filter(|(_, r)| r.sha256 == spec.sha256);
                let partial = self
                    .paths(plugin_id, &spec.id)
                    .ok()
                    .and_then(|(_, _, part)| fs::metadata(part).ok())
                    .map(|m| m.len())
                    .unwrap_or(0);
                ResourceStatus {
                    id: spec.id.clone(),
                    size: spec.size,
                    license: spec.license.clone(),
                    installed: record.is_some(),
                    partial,
                    installed_at: record.map(|(_, r)| r.installed_at),
                }
            })
            .collect()
    }

    pub fn remove(&self, plugin_id: &str, id: &str) -> AppResult<()> {
        let (file, meta, part) = self.paths(plugin_id, id).map_err(AppError::from)?;
        for path in [meta, file, part] {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_error(e, &path).into()),
            }
        }
        Ok(())
    }

    /// 下载并校验资源；已安装则直接返回。依次尝试各个地址，支持断点续传与取消。
    pub fn download(
        &self,
        plugin_id: &str,
        spec: &ResourceSpec,
        ctx: &dyn TaskContext,
    ) -> PluginResult<PathBuf> {
        let (file, meta, part) = self.paths(plugin_id, &spec.id)?;
        if let Some((path, record)) = self.record(plugin_id, &spec.id)
            && record.sha256 == spec.sha256
        {
            ctx.log(
                LogLevel::Info,
                "resource.already_installed",
                json!({ "id": spec.id }),
            );
            return Ok(path);
        }
        if spec.urls.is_empty() {
            return Err(PluginError::new("resource.no_source").with("id", spec.id.as_str()));
        }
        fs::create_dir_all(file.parent().unwrap_or(&self.root))
            .map_err(|e| io_error(e, &self.root))?;

        let client = client()?;
        let start = Instant::now();
        let mut last_error = None;
        for (index, url) in spec.urls.iter().enumerate() {
            ctx.log(
                LogLevel::Info,
                "resource.download_start",
                json!({ "id": spec.id, "host": host(url), "size": spec.size, "attempt": index + 1 }),
            );
            match self.fetch(&client, url, spec, &part, ctx) {
                Ok(()) => {
                    last_error = None;
                    let record = Record {
                        sha256: spec.sha256.clone(),
                        size: spec.size,
                        url: url.clone(),
                        installed_at: now_millis() as u64,
                    };
                    fs::rename(&part, &file).map_err(|e| io_error(e, &file))?;
                    let json = serde_json::to_vec_pretty(&record).unwrap_or_default();
                    fs::write(&meta, json).map_err(|e| io_error(e, &meta))?;
                    ctx.log(
                        LogLevel::Info,
                        "resource.installed",
                        json!({ "id": spec.id, "ms": start.elapsed().as_millis() as u64 }),
                    );
                    break;
                }
                Err(err) if err.code == "task.cancelled" => {
                    ctx.log(LogLevel::Warn, "resource.paused", json!({ "id": spec.id }));
                    return Err(err);
                }
                Err(err) => {
                    ctx.log(
                        LogLevel::Warn,
                        "resource.source_failed",
                        json!({ "host": host(url), "code": err.code }),
                    );
                    last_error = Some(err);
                }
            }
        }
        match last_error {
            Some(err) => Err(err),
            None => Ok(file),
        }
    }

    fn fetch(
        &self,
        client: &reqwest::blocking::Client,
        url: &str,
        spec: &ResourceSpec,
        part: &Path,
        ctx: &dyn TaskContext,
    ) -> PluginResult<()> {
        let network = |e: reqwest::Error| {
            PluginError::new("resource.network")
                .with("host", host(url))
                .with("detail", e.to_string())
        };

        // 续传：先对已下载部分计算哈希
        let mut hasher = Sha256::new();
        let mut offset = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
        if offset > spec.size {
            offset = 0;
        }
        if offset > 0 {
            let mut existing = File::open(part).map_err(|e| io_error(e, part))?;
            let mut buffer = vec![0u8; CHUNK];
            let mut remaining = offset;
            while remaining > 0 {
                let read = existing
                    .read(&mut buffer[..CHUNK.min(remaining as usize)])
                    .map_err(|e| io_error(e, part))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                remaining -= read as u64;
            }
        }

        let mut request = client.get(url);
        if offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
        }
        let mut response = request.send().map_err(network)?;
        let status = response.status();
        if !status.is_success() {
            return Err(PluginError::new("resource.http_status")
                .with("status", status.as_u16())
                .with("host", host(url)));
        }
        // 服务器不支持 Range 时从头开始
        if offset > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
            offset = 0;
            hasher = Sha256::new();
        }
        if offset > 0 {
            ctx.log(
                LogLevel::Info,
                "resource.resumed",
                json!({ "id": spec.id, "offset": offset }),
            );
        }

        let mut out = OpenOptions::new()
            .create(true)
            .write(true)
            .append(offset > 0)
            .truncate(offset == 0)
            .open(part)
            .map_err(|e| io_error(e, part))?;
        let mut done = offset;
        let mut buffer = vec![0u8; CHUNK];
        ctx.progress(done, spec.size);
        loop {
            if ctx.is_cancelled() {
                let _ = out.flush();
                return Err(cancelled());
            }
            let read = response.read(&mut buffer).map_err(|e| {
                PluginError::new("resource.network")
                    .with("host", host(url))
                    .with("detail", e.to_string())
            })?;
            if read == 0 {
                break;
            }
            out.write_all(&buffer[..read])
                .map_err(|e| io_error(e, part))?;
            hasher.update(&buffer[..read]);
            done += read as u64;
            if done > spec.size {
                break;
            }
            ctx.progress(done, spec.size);
        }
        out.flush().map_err(|e| io_error(e, part))?;
        drop(out);

        if done != spec.size {
            // 连接中途断开：保留已下载部分以便续传；超出预期大小则丢弃
            if done > spec.size {
                let _ = fs::remove_file(part);
            }
            return Err(PluginError::new("resource.size_mismatch")
                .with("expected", spec.size)
                .with("actual", done));
        }
        let actual = to_hex(&hasher.finalize());
        if actual != spec.sha256.to_ascii_lowercase() {
            let _ = fs::remove_file(part);
            return Err(PluginError::new("resource.checksum_mismatch").with("id", spec.id.as_str()));
        }
        ctx.log(
            LogLevel::Info,
            "resource.verified",
            json!({ "id": spec.id, "sha256": &actual[..16] }),
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "resources_test.rs"]
mod tests;
