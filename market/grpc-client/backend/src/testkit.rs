//! 测试工具：内存中的示例 proto，以及本地 gRPC 测试服务（h2c）。

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::channel::Channel;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, Value};
use protox::file::{ChainFileResolver, File, FileResolver, GoogleFileResolver};

use crate::grpc::frame;
use crate::reflection::{
    ErrorResponse, FileDescriptorResponse, ListServiceResponse, Request, Response,
    ServerReflectionRequest, ServerReflectionResponse, ServiceResponse,
};

pub const ECHO_PROTO: &str = r#"
syntax = "proto3";
package demo.v1;
import "google/protobuf/timestamp.proto";
import "common/types.proto";

enum Color { COLOR_UNSPECIFIED = 0; RED = 1; }

message Item {
  int64 id = 1;
  string name = 2;
  bytes data = 3;
  Color color = 4;
  repeated string tags = 5;
  map<string, int32> counts = 6;
  google.protobuf.Timestamp at = 7;
  common.Money price = 8;
  oneof choice { string a = 9; int32 b = 10; }
  uint64 big = 11;
}

message EchoRequest { string text = 1; int64 id = 2; int32 count = 3; int32 delay_ms = 4; }
message EchoReply { string text = 1; int64 id = 2; int32 index = 3; }

service Echo {
  rpc Unary(EchoRequest) returns (EchoReply);
  rpc Fail(EchoRequest) returns (EchoReply);
  rpc Deny(EchoRequest) returns (EchoReply);
  rpc Slow(EchoRequest) returns (EchoReply);
  rpc Watch(EchoRequest) returns (stream EchoReply);
  rpc Upload(stream EchoRequest) returns (EchoReply);
}
"#;

pub const TYPES_PROTO: &str = r#"
syntax = "proto3";
package common;
message Money { string currency = 1; int64 units = 2; }
"#;

struct Memory(HashMap<String, String>);

impl FileResolver for Memory {
    fn open_file(&self, name: &str) -> Result<File, protox::Error> {
        match self.0.get(name) {
            Some(source) => File::from_source(name, source),
            None => Err(protox::Error::file_not_found(name)),
        }
    }
}

/// 示例定义的描述符池（主文件名为 echo.proto）
pub fn pool() -> DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let mut resolver = ChainFileResolver::new();
        resolver.add(Memory(HashMap::from([
            ("echo.proto".to_owned(), ECHO_PROTO.to_owned()),
            ("common/types.proto".to_owned(), TYPES_PROTO.to_owned()),
        ])));
        resolver.add(GoogleFileResolver::new());
        let mut compiler = protox::Compiler::with_file_resolver(resolver);
        compiler.include_imports(true);
        compiler.open_file("echo.proto").unwrap();
        compiler.descriptor_pool()
    })
    .clone()
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
    })
}

/// 测试服务的反射能力
#[derive(Clone, Copy, PartialEq)]
pub enum Reflection {
    V1,
    V1Alpha,
    None,
}

type Body = Channel<Bytes, Infallible>;

fn grpc_response(extra: &[(&str, &str)]) -> http::response::Builder {
    let mut builder = http::Response::builder()
        .status(200)
        .header("content-type", "application/grpc")
        .header("x-server", "test");
    for (k, v) in extra {
        builder = builder.header(*k, *v);
    }
    builder
}

fn trailers(status: u32, message: &str) -> http::HeaderMap {
    let mut map = http::HeaderMap::new();
    map.insert("grpc-status", status.to_string().parse().unwrap());
    if !message.is_empty() {
        map.insert("grpc-message", message.parse().unwrap());
    }
    map.insert("x-trailer", "done".parse().unwrap());
    map
}

fn unframe(body: &[u8]) -> Vec<u8> {
    body.get(5..).unwrap_or_default().to_vec()
}

fn reply(text: &str, id: i64, index: i32) -> Vec<u8> {
    let desc = pool().get_message_by_name("demo.v1.EchoReply").unwrap();
    let mut msg = DynamicMessage::new(desc);
    msg.set_field_by_name("text", Value::String(text.to_owned()));
    msg.set_field_by_name("id", Value::I64(id));
    msg.set_field_by_name("index", Value::I32(index));
    frame(&msg.encode_to_vec())
}

fn reflect_reply(request: &[u8]) -> Vec<u8> {
    let request = ServerReflectionRequest::decode(unframe(request).as_slice()).unwrap();
    let pool = pool();
    let file = |name: &str| {
        pool.get_file_by_name(name)
            .map(|f| f.file_descriptor_proto().encode_to_vec())
    };
    let response = match request.message_request {
        Some(Request::ListServices(_)) => Response::ListServices(ListServiceResponse {
            service: ["demo.v1.Echo", "grpc.reflection.v1.ServerReflection"]
                .iter()
                .map(|n| ServiceResponse {
                    name: (*n).to_owned(),
                })
                .collect(),
        }),
        Some(Request::FileContainingSymbol(symbol)) if symbol == "demo.v1.Echo" => {
            Response::FileDescriptor(FileDescriptorResponse {
                file_descriptor_proto: vec![file("echo.proto").unwrap()],
            })
        }
        // 标准类型故意不提供，客户端应改用内置定义
        Some(Request::FileByFilename(name)) if name == "common/types.proto" => {
            Response::FileDescriptor(FileDescriptorResponse {
                file_descriptor_proto: vec![file(&name).unwrap()],
            })
        }
        _ => Response::Error(ErrorResponse {
            error_code: 5,
            error_message: "not found".into(),
        }),
    };
    frame(
        &ServerReflectionResponse {
            valid_host: String::new(),
            message_response: Some(response),
        }
        .encode_to_vec(),
    )
}

async fn handle(
    request: http::Request<Incoming>,
    reflection: Reflection,
) -> Result<http::Response<Body>, Infallible> {
    let path = request.uri().path().to_owned();
    let user = request
        .headers()
        .get("x-user")
        .map(|v| v.to_str().unwrap_or_default().to_owned())
        .unwrap_or_default();
    let body = request.into_body().collect().await.unwrap().to_bytes();
    let (mut tx, out) = Channel::new(16);
    let reflect_v1 = path == "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo";
    let reflect_alpha = path == "/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo";
    if (reflect_v1 && reflection != Reflection::V1)
        || (reflect_alpha && reflection != Reflection::V1Alpha)
    {
        // 未实现的服务：只有 trailers 的响应
        return Ok(
            grpc_response(&[("grpc-status", "12"), ("grpc-message", "unknown service")])
                .body(out)
                .unwrap(),
        );
    }
    if path == "/demo.v1.Echo/Deny" {
        return Ok(
            grpc_response(&[("grpc-status", "7"), ("grpc-message", "no%20access")])
                .body(out)
                .unwrap(),
        );
    }
    let echo = DynamicMessage::decode(
        pool().get_message_by_name("demo.v1.EchoRequest").unwrap(),
        unframe(&body).as_slice(),
    )
    .ok();
    let field = |name: &str| {
        echo.as_ref()
            .and_then(|m| m.get_field_by_name(name).map(|v| v.into_owned()))
    };
    let text = field("text")
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    let id = field("id").and_then(|v| v.as_i64()).unwrap_or_default();
    let count = field("count").and_then(|v| v.as_i32()).unwrap_or_default();
    let delay = field("delay_ms")
        .and_then(|v| v.as_i32())
        .unwrap_or_default();
    tokio::spawn(async move {
        match path.as_str() {
            "/demo.v1.Echo/Unary" => {
                let _ = tx
                    .send_data(Bytes::from(reply(&format!("{text}|{user}"), id, 0)))
                    .await;
                let _ = tx.send_trailers(trailers(0, "")).await;
            }
            "/demo.v1.Echo/Fail" => {
                let _ = tx
                    .send_trailers(trailers(3, "bad%20input%20%E4%BD%A0"))
                    .await;
            }
            "/demo.v1.Echo/Slow" => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let _ = tx.send_data(Bytes::from(reply("late", id, 0))).await;
                let _ = tx.send_trailers(trailers(0, "")).await;
            }
            "/demo.v1.Echo/Watch" => {
                for index in 0..count {
                    tokio::time::sleep(Duration::from_millis(delay as u64)).await;
                    if tx
                        .send_data(Bytes::from(reply(&text, id, index)))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                let _ = tx.send_trailers(trailers(0, "")).await;
            }
            _ if reflect_v1 || reflect_alpha => {
                let _ = tx.send_data(Bytes::from(reflect_reply(&body))).await;
                let _ = tx.send_trailers(trailers(0, "")).await;
            }
            _ => {
                let _ = tx.send_trailers(trailers(12, "unknown method")).await;
            }
        }
    });
    Ok(grpc_response(&[]).body(out).unwrap())
}

/// 启动 h2c 测试服务，返回 `127.0.0.1:端口`
pub fn serve(reflection: Reflection) -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    runtime().spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                    .serve_connection(
                        TokioIo::new(tcp),
                        service_fn(move |req| handle(req, reflection)),
                    )
                    .await;
            });
        }
    });
    addr
}
