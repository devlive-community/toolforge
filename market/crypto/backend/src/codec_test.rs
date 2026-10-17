use super::*;

#[test]
fn decodes_every_encoding() {
    assert_eq!(
        decode("héllo", Encoding::Utf8, "x").unwrap(),
        "héllo".as_bytes()
    );
    assert_eq!(
        decode("0x DE:AD be ef", Encoding::Hex, "x").unwrap(),
        vec![0xde, 0xad, 0xbe, 0xef]
    );
    assert_eq!(decode("aGk=", Encoding::Base64, "x").unwrap(), b"hi");
    assert_eq!(decode("aGk", Encoding::Base64, "x").unwrap(), b"hi");
    assert_eq!(
        decode("-_8", Encoding::Base64, "x").unwrap(),
        vec![0xfb, 0xff]
    );
    assert_eq!(
        decode("zz", Encoding::Hex, "key").unwrap_err().params["field"],
        "key"
    );
    assert_eq!(
        decode("***", Encoding::Base64, "x").unwrap_err().code,
        "crypto.invalid_base64"
    );
}

#[test]
fn encodes_and_rejects_binary_as_text() {
    assert_eq!(encode(b"hi", Encoding::Hex).unwrap(), "6869");
    assert_eq!(encode(b"hi", Encoding::Base64).unwrap(), "aGk=");
    assert_eq!(
        encode(&[0xff, 0xfe], Encoding::Utf8).unwrap_err().code,
        "crypto.not_utf8"
    );
}
