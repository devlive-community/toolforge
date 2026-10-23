use super::*;

#[test]
fn roundtrips_every_format() {
    let data = "ToolForge 工具箱 ".repeat(200);
    for format in [Format::Gzip, Format::Deflate, Format::Zlib, Format::Zstd] {
        let packed = compress(format, data.as_bytes()).unwrap();
        assert!(packed.len() < data.len(), "{format:?}");
        assert_eq!(
            decompress(format, &packed).unwrap(),
            data.as_bytes(),
            "{format:?}"
        );
    }
    assert_eq!(&compress(Format::Gzip, b"x").unwrap()[..2], &[0x1f, 0x8b]);
}

#[test]
fn reports_corrupt_data_with_offset() {
    let mut packed = compress(Format::Gzip, &[7u8; 5000]).unwrap();
    let len = packed.len();
    packed.truncate(len - 6);
    let err = decompress(Format::Gzip, &packed).unwrap_err();
    assert_eq!(err.code, "encode.corrupt");
    assert!(err.params["offset"].as_u64().unwrap() > 0);
    assert_eq!(
        decompress(Format::Zstd, b"not zstd").unwrap_err().code,
        "encode.corrupt"
    );
    assert_eq!(
        decompress(Format::Zlib, b"garbage!").unwrap_err().code,
        "encode.corrupt"
    );
}
