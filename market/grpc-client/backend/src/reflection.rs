//! gRPC Server Reflection：向服务器查询服务列表与描述符，组装成描述符池。
//!
//! 优先使用 grpc.reflection.v1，服务器返回 UNIMPLEMENTED 时退回 v1alpha；两者都没有时提示未启用反射。
//! 每次查询是一次独立调用（一条请求、读完响应）；依赖文件逐个按文件名补齐。

use std::collections::{BTreeMap, HashSet};

use prost::Message;
use prost_reflect::DescriptorPool;
use prost_types::FileDescriptorProto;
use protox::file::{FileResolver, GoogleFileResolver};
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};
use tf_plugin_guest::net::StreamEvent;

use crate::grpc::{self, CallOptions, Decoder, MetadataEntry, Target};

const V1: &str = "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo";
const V1ALPHA: &str = "/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo";
/// 最多补齐的描述符文件数
const MAX_FILES: usize = 500;

#[derive(Clone, PartialEq, Message)]
pub struct ServerReflectionRequest {
    #[prost(string, tag = "1")]
    pub host: String,
    #[prost(oneof = "Request", tags = "3, 4, 7")]
    pub message_request: Option<Request>,
}

#[derive(Clone, PartialEq, prost::Oneof)]
pub enum Request {
    #[prost(string, tag = "3")]
    FileByFilename(String),
    #[prost(string, tag = "4")]
    FileContainingSymbol(String),
    #[prost(string, tag = "7")]
    ListServices(String),
}

#[derive(Clone, PartialEq, Message)]
pub struct ServerReflectionResponse {
    #[prost(string, tag = "1")]
    pub valid_host: String,
    #[prost(oneof = "Response", tags = "4, 6, 7")]
    pub message_response: Option<Response>,
}

#[derive(Clone, PartialEq, prost::Oneof)]
pub enum Response {
    #[prost(message, tag = "4")]
    FileDescriptor(FileDescriptorResponse),
    #[prost(message, tag = "6")]
    ListServices(ListServiceResponse),
    #[prost(message, tag = "7")]
    Error(ErrorResponse),
}

#[derive(Clone, PartialEq, Message)]
pub struct FileDescriptorResponse {
    #[prost(bytes = "vec", repeated, tag = "1")]
    pub file_descriptor_proto: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ListServiceResponse {
    #[prost(message, repeated, tag = "1")]
    pub service: Vec<ServiceResponse>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ServiceResponse {
    #[prost(string, tag = "1")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ErrorResponse {
    #[prost(int32, tag = "1")]
    pub error_code: i32,
    #[prost(string, tag = "2")]
    pub error_message: String,
}

struct Session<'a> {
    target: &'a Target,
    metadata: &'a [MetadataEntry],
    deadline_ms: u64,
    path: &'static str,
    ctx: &'a dyn TaskContext,
}

enum Outcome {
    Ok(ServerReflectionResponse),
    Status(grpc::Status),
}

impl Session<'_> {
    /// 发送一条反射请求并读完响应
    fn ask(&self, request: Request) -> PluginResult<Outcome> {
        if self.ctx.is_cancelled() {
            return Err(cancelled());
        }
        let body = ServerReflectionRequest {
            host: String::new(),
            message_request: Some(request),
        }
        .encode_to_vec();
        let (mut stream, _) = grpc::open(
            &CallOptions {
                path: self.path.to_owned(),
                target: self.target,
                metadata: self.metadata,
                deadline_ms: Some(self.deadline_ms),
                body,
            },
            Some(self.ctx),
        )?;
        if let Some(status) = grpc::status_from(&stream.head().headers) {
            return Ok(Outcome::Status(status));
        }
        if stream.head().status != 200 {
            let code = grpc::http_status_code(stream.head().status);
            return Ok(Outcome::Status(grpc::Status::local(
                code,
                format!("HTTP {}", stream.head().status),
            )));
        }
        let mut decoder = Decoder::default();
        let mut first = None;
        loop {
            if self.ctx.is_cancelled() {
                return Err(cancelled());
            }
            match stream.read(std::time::Duration::from_millis(200)) {
                Ok(StreamEvent::Data(data)) => {
                    decoder.push(&data);
                    while let Some(frame) = decoder.next()? {
                        if first.is_none() {
                            first =
                                Some(ServerReflectionResponse::decode(frame.as_slice()).map_err(
                                    |e| {
                                        PluginError::new("grpc.reflection_failed")
                                            .with("detail", e.to_string())
                                    },
                                )?);
                        }
                    }
                }
                Ok(StreamEvent::Pending) => {}
                Ok(StreamEvent::End(trailers)) => {
                    let status = grpc::status_from(&trailers)
                        .unwrap_or_else(|| grpc::Status::local(2, "missing grpc-status"));
                    if status.code != 0 {
                        return Ok(Outcome::Status(status));
                    }
                    return first.map(Outcome::Ok).ok_or_else(|| {
                        PluginError::new("grpc.reflection_failed").with("detail", "empty response")
                    });
                }
                Err(err) if err.code == "net.timeout" => {
                    return Err(PluginError::new("grpc.deadline"));
                }
                Err(err) => return Err(err),
            }
        }
    }
}

fn failed(status: &grpc::Status) -> PluginError {
    PluginError::new("grpc.reflection_failed")
        .with("status", status.name)
        .with("detail", status.message.as_str())
}

/// 依赖在前的顺序加入描述符池
fn build_pool(files: BTreeMap<String, FileDescriptorProto>) -> PluginResult<DescriptorPool> {
    let mut ordered = Vec::new();
    let mut placed: HashSet<String> = HashSet::new();
    let mut pending: Vec<FileDescriptorProto> = files.into_values().collect();
    while !pending.is_empty() {
        let before = pending.len();
        pending.retain(|file| {
            let ready = file.dependency.iter().all(|d| placed.contains(d));
            if ready {
                placed.insert(file.name().to_owned());
                ordered.push(file.clone());
            }
            !ready
        });
        if pending.len() == before {
            let missing: Vec<&str> = pending
                .iter()
                .flat_map(|f| f.dependency.iter())
                .filter(|d| !placed.contains(*d))
                .map(String::as_str)
                .collect();
            return Err(
                PluginError::new("grpc.reflection_incomplete").with("files", missing.join(", "))
            );
        }
    }
    let mut pool = DescriptorPool::new();
    pool.add_file_descriptor_protos(ordered)
        .map_err(|e| PluginError::new("grpc.descriptor_invalid").with("detail", e.to_string()))?;
    Ok(pool)
}

fn add_files(
    response: ServerReflectionResponse,
    files: &mut BTreeMap<String, FileDescriptorProto>,
) -> PluginResult<()> {
    match response.message_response {
        Some(Response::FileDescriptor(set)) => {
            for raw in set.file_descriptor_proto {
                let file = FileDescriptorProto::decode(raw.as_slice()).map_err(|e| {
                    PluginError::new("grpc.descriptor_invalid").with("detail", e.to_string())
                })?;
                files.entry(file.name().to_owned()).or_insert(file);
            }
            Ok(())
        }
        Some(Response::Error(err)) => Err(PluginError::new("grpc.reflection_failed")
            .with("status", grpc::status_name(err.error_code.max(0) as u32))
            .with("detail", err.error_message)),
        _ => Err(PluginError::new("grpc.reflection_failed").with("detail", "unexpected response")),
    }
}

/// 通过服务器反射得到描述符池
pub fn reflect(
    target: &Target,
    metadata: &[MetadataEntry],
    deadline_ms: u64,
    ctx: &dyn TaskContext,
) -> PluginResult<DescriptorPool> {
    let mut session = Session {
        target,
        metadata,
        deadline_ms,
        path: V1,
        ctx,
    };
    ctx.stage("grpc.reflecting");
    let mut listed = session.ask(Request::ListServices(String::new()))?;
    if matches!(&listed, Outcome::Status(s) if s.code == 12) {
        session.path = V1ALPHA;
        listed = session.ask(Request::ListServices(String::new()))?;
    }
    let services = match listed {
        Outcome::Status(status) if status.code == 12 => {
            return Err(PluginError::new("grpc.reflection_unavailable"));
        }
        Outcome::Status(status) => return Err(failed(&status)),
        Outcome::Ok(response) => match response.message_response {
            Some(Response::ListServices(list)) => list.service,
            Some(Response::Error(err)) => {
                return Err(
                    PluginError::new("grpc.reflection_failed").with("detail", err.error_message)
                );
            }
            _ => Vec::new(),
        },
    };

    let mut files = BTreeMap::new();
    for service in services
        .iter()
        .filter(|s| !s.name.starts_with("grpc.reflection."))
    {
        match session.ask(Request::FileContainingSymbol(service.name.clone()))? {
            Outcome::Ok(response) => add_files(response, &mut files)?,
            Outcome::Status(status) => return Err(failed(&status)),
        }
        ctx.progress(files.len() as u64, 0);
    }
    // 补齐依赖；服务器没有的标准类型用内置定义
    let google = GoogleFileResolver::new();
    loop {
        let missing: Vec<String> = files
            .values()
            .flat_map(|f| f.dependency.iter())
            .filter(|d| !files.contains_key(*d))
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if missing.is_empty() {
            break;
        }
        if files.len() + missing.len() > MAX_FILES {
            return Err(PluginError::new("grpc.too_many_files").with("limit", MAX_FILES));
        }
        for name in missing {
            let served = match session.ask(Request::FileByFilename(name.clone()))? {
                Outcome::Ok(response) => {
                    add_files(response, &mut files).is_ok() && files.contains_key(&name)
                }
                Outcome::Status(_) => false,
            };
            match served {
                true => {}
                false => {
                    let file = google.open_file(&name).map_err(|_| {
                        PluginError::new("grpc.reflection_incomplete").with("files", name.as_str())
                    })?;
                    files.insert(name, file.file_descriptor_proto().clone());
                }
            }
        }
        ctx.progress(files.len() as u64, 0);
    }
    build_pool(files)
}

#[cfg(test)]
#[path = "reflection_test.rs"]
mod tests;
