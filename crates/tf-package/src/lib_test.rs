use super::*;

fn key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn keys() -> Vec<(&'static str, String)> {
    vec![("test", BASE64.encode(key().verifying_key().as_bytes()))]
}

fn files() -> BTreeMap<String, Vec<u8>> {
    let manifest = r#"{"id":"org.example.demo","version":"1.0.0","name":"Demo","description":"d","category":"other","functions":{"run":{}}}"#;
    BTreeMap::from([
        ("manifest.json".to_owned(), manifest.as_bytes().to_vec()),
        ("plugin.wasm".to_owned(), b"\0asm".to_vec()),
        (
            "ui/index.js".to_owned(),
            b"export default () => null".to_vec(),
        ),
        ("locales/zh-CN.json".to_owned(), b"{}".to_vec()),
    ])
}

fn open_test(bytes: &[u8]) -> PackageResult<Package> {
    let owned = keys();
    let keys: Vec<(&str, &str)> = owned.iter().map(|(id, k)| (*id, k.as_str())).collect();
    open_with_keys(bytes, &keys)
}

#[test]
fn signed_packages_verify_and_roundtrip() {
    let bytes = build(&files(), Some(("test", &key()))).unwrap();
    let package = open_test(&bytes).unwrap();
    assert_eq!(package.manifest.id, "org.example.demo");
    assert_eq!(
        package.trust,
        Trust::Official {
            key_id: "test".into()
        }
    );
    assert_eq!(package.files["ui/index.js"], b"export default () => null");
    assert_eq!(package.digest, digest(&files()));
    // 打包结果确定（固定时间戳）
    assert_eq!(bytes, build(&files(), Some(("test", &key()))).unwrap());
}

#[test]
fn unsigned_and_unknown_keys_are_reported() {
    let unsigned = build(&files(), None).unwrap();
    assert_eq!(open_test(&unsigned).unwrap().trust, Trust::Unsigned);
    let other = SigningKey::from_bytes(&[9u8; 32]);
    let foreign = build(&files(), Some(("someone", &other))).unwrap();
    assert_eq!(
        open_test(&foreign).unwrap().trust,
        Trust::UnknownKey {
            key_id: "someone".into()
        }
    );
}

#[test]
fn tampered_content_fails_signature() {
    let signed = build(&files(), Some(("test", &key()))).unwrap();
    let mut package_files = open_test(&signed).unwrap().files;
    package_files.insert("ui/index.js".into(), b"alert(1)".to_vec());
    // 保留原签名，只替换内容
    let mut out = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(&mut out);
    for (path, data) in &package_files {
        writer
            .start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
    assert_eq!(
        open_test(&out.into_inner()).unwrap_err().code,
        "package.bad_signature"
    );
}

#[test]
fn rejects_bad_paths_missing_files_and_ids() {
    let mut bad = files();
    bad.insert("../evil".into(), vec![]);
    assert_eq!(
        open_test(&build(&bad, None).unwrap()).unwrap_err().code,
        "package.bad_path"
    );
    let mut missing = files();
    missing.remove("plugin.wasm");
    assert_eq!(
        open_test(&build(&missing, None).unwrap()).unwrap_err().code,
        "package.missing_file"
    );
    let mut id = files();
    id.insert("manifest.json".into(), br#"{"id":"Bad ID","version":"1","name":"n","description":"d","category":"other","functions":{}}"#.to_vec());
    assert_eq!(
        open_test(&build(&id, None).unwrap()).unwrap_err().code,
        "package.bad_id"
    );
    assert_eq!(open_test(b"not a zip").unwrap_err().code, "package.invalid");
    assert!(valid_id("org.devlive.toolforge.uscc"));
    assert!(!valid_id("a..b") && !valid_id(".a") && !valid_id("x"));
}

#[test]
fn official_keys_decode() {
    for (_, key) in OFFICIAL_KEYS {
        let bytes: [u8; 32] = BASE64.decode(key.as_bytes()).unwrap().try_into().unwrap();
        assert!(VerifyingKey::from_bytes(&bytes).is_ok());
    }
}
