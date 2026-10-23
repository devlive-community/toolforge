use super::*;

#[test]
fn base58_matches_bitcoin_vectors() {
    assert_eq!(base58_encode(b"Hello World!"), "2NEpo7TZRRrLZSi2U");
    assert_eq!(base58_encode(&[0, 0, 0x28, 0x7f, 0xb4, 0xcd]), "11233QC4");
    assert_eq!(base58_encode(b""), "");
    assert_eq!(base58_decode("2NEpo7TZRRrLZSi2U").unwrap(), b"Hello World!");
    assert_eq!(
        base58_decode("11233QC4").unwrap(),
        [0, 0, 0x28, 0x7f, 0xb4, 0xcd]
    );
    assert_eq!(
        base58_decode("0OIl").unwrap_err().code,
        "encode.invalid_input"
    );
}

#[test]
fn base85_roundtrips_with_partial_groups() {
    for data in [
        &b""[..],
        b"a",
        b"ab",
        b"abc",
        b"abcd",
        b"Hello, World!",
        &[0, 0, 0, 0, 255],
    ] {
        let text = base85_encode(data);
        assert_eq!(base85_decode(&text).unwrap(), data, "{text}");
    }
    // git 二进制补丁使用相同字母表：全 0 的 4 字节为 00000
    assert_eq!(base85_encode(&[0; 4]), "00000");
}

#[test]
fn ascii85_matches_adobe_examples() {
    assert_eq!(ascii85_encode(b"Man "), "<~9jqo^~>");
    assert_eq!(ascii85_encode(b"sure."), "<~F*2M7/c~>");
    assert_eq!(ascii85_encode(&[0; 4]), "<~z~>");
    assert_eq!(ascii85_decode("<~9jqo^F*2M7/c~>").unwrap(), b"Man sure.");
    assert_eq!(ascii85_decode("<~z~>").unwrap(), [0; 4]);
    assert_eq!(ascii85_decode("9jq o^").unwrap(), b"Man ");
    assert!(ascii85_decode("<~v~>").is_err());
}

#[test]
fn morse_roundtrips_and_handles_utf8_option() {
    let code = morse_encode("SOS Hello, World!", false).unwrap();
    assert_eq!(
        code,
        "... --- ... / .... . .-.. .-.. --- --..-- / .-- --- .-. .-.. -.. -.-.--"
    );
    assert_eq!(morse_decode(&code).unwrap(), "SOS HELLO, WORLD!");
    assert_eq!(morse_decode("··· −−− ···").unwrap(), "SOS");
    let err = morse_encode("中", false).unwrap_err();
    assert_eq!(err.code, "encode.morse_unsupported");
    // 中 = E4 B8 AD
    let hex = morse_encode("中", true).unwrap();
    assert_eq!(morse_decode(&hex).unwrap(), "E4B8AD");
    assert!(morse_decode("...---...").is_err());
}
