use super::*;
use crate::testkit::pool;

fn file(name: &str) -> FileDescriptorProto {
    pool()
        .get_file_by_name(name)
        .unwrap()
        .file_descriptor_proto()
        .clone()
}

#[test]
fn orders_files_by_dependency() {
    // 以依赖在后的顺序给出，build_pool 仍能组装
    let files = BTreeMap::from([
        ("echo.proto".to_owned(), file("echo.proto")),
        ("common/types.proto".to_owned(), file("common/types.proto")),
        (
            "google/protobuf/timestamp.proto".to_owned(),
            file("google/protobuf/timestamp.proto"),
        ),
    ]);
    let built = build_pool(files).unwrap();
    assert!(built.get_service_by_name("demo.v1.Echo").is_some());
}

#[test]
fn reports_missing_dependencies() {
    let files = BTreeMap::from([("echo.proto".to_owned(), file("echo.proto"))]);
    let err = build_pool(files).unwrap_err();
    assert_eq!(err.code, "grpc.reflection_incomplete");
    assert!(
        err.params["files"]
            .as_str()
            .unwrap()
            .contains("common/types.proto")
    );
}

#[test]
fn reflection_messages_round_trip() {
    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(Request::FileContainingSymbol("demo.v1.Echo".into())),
    };
    let decoded = ServerReflectionRequest::decode(request.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, request);
}
