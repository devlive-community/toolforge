use sm2::dsa::signature::Signer;
use sm2::elliptic_curve::PrimeField;
use sm2::elliptic_curve::ops::Reduce;
use sm2::elliptic_curve::point::AffineCoordinates;
use sm2::{FieldBytes, ProjectivePoint, Scalar};

use super::*;

// GB/T 32918 推荐曲线示例：d、公钥、ZA、e、k、r、s
const D: &str = "3945208f7b2144b13f36e38ac6d39f95889393692860b51a42fb81ef4df7c5b8";
const PUB: &str = "0409f9df311e5421a150dd7d161e4bc5c672179fad1833fc076bb08ff356f35020\
                   ccea490ce26775a52dc6ea718cc1aa600aed05fbf35e084a6632f6072da9ad13";
const ZA: &str = "b2e14c5c79c6df5b85f4fe7ed8db7a262b9da7e07ccb0ea9f4747b8ccda8a4f3";
const E: &str = "f0b43e94ba45accaace692ed534382eb17e6ab5a19ce7b31f4486fdfc0d28640";
const K: &str = "59276e27d506861a16680f3ad9c02dccef3cc1fa3cdbe4ce6d54b80deac1bc21";
const R: &str = "f5a03b0648d2c4630eeac513e1bb81a15944da3827d5b74143ac7eaceee720b3";
const S: &str = "b1b6aa29df212fd8763182bc0d421ca1bb9038fd1f7f42d4840b69c485bbc1aa";
const MESSAGE: &str = "message digest";

// 由另一独立实现生成的 SM2 密钥对与签名（消息 "message digest"）：
// EXT_SIG_EMPTY 未指定用户标识（该实现此时使用空标识），EXT_SIG_STD 使用默认标识，
// EXT_SIG_ALICE 使用 alice@example.com
const EXT_PRIVATE: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqBHM9VAYItBG0wawIBAQQgjqXGl6Yv9RDMSHhJ
s12NuMwHKA29GAN8uYClXT4I6GShRANCAAQ0cvNuJwiHVU7UN/bwYtqdWi3OU+DK
X922k88wgDyqmOipNdnyKiaHG43VzjCi0rQrc1xvPBevGWCy80TWGD3Z
-----END PRIVATE KEY-----";
const EXT_PUBLIC: &str = "-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoEcz1UBgi0DQgAENHLzbicIh1VO1Df28GLanVotzlPg
yl/dtpPPMIA8qpjoqTXZ8iomhxuN1c4wotK0K3NcbzwXrxlgsvNE1hg92Q==
-----END PUBLIC KEY-----";
const EXT_SIG_EMPTY: &str = "3046022100d85560f53858b8bd7df9e0305c1ee0f64bcadbbc86d5aa25f85347d1\
                                dd0d20f5022100f4d4b6003113fbcbb52dec34f20c460b55abf5ee82bdd66e03e8\
                                ab9f4531ceb3";
const EXT_SIG_STD: &str = "3045022100b013a9fda0586b40e298409261d46c7fdcc01e168d8b805c2aef6869387\
                            7ce30022008a2c7dd209cacfc59710bdd6975d5171e42ca4a608f54aa1067965af4\
                            63a818";
const EXT_SIG_ALICE: &str = "3045022100c422ff01c7ac858dadf5977f4906cdda3ce0ebcc988df95880463222d8\
                              241b2e0220155a1c77a010ee65c96dd00456618b2a092676e7b65d7ec7512dbf0a12\
                              4261dc";
const P256_PUBLIC: &str = "-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAECPhKnkZEdohAHD2licLP3+2zd2pf
AEmvRQ+M3ZkemFgXvKRubr9uISMG/5YdVudZqcB1GsBXTxkkNGaUtmBCgg==
-----END PUBLIC KEY-----";
const RSA_PUBLIC: &str = "-----BEGIN PUBLIC KEY-----
MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDT4XDOeKghJKPydYiU8DOBo7RZ
KG/ycEbxjjxHz3WyZs14dt/emRu+Gt77QvgFQNbHrN3KSohWgQpmDqk2txH13Ll2
T+bohcumMeyJm+V2iGEIfNDUfL8zL3GuMfirchRuXGYMRiOqDPmkPqtfnAsNjsbz
iyxD1AG1KfWCIZ8dWQIDAQAB
-----END PUBLIC KEY-----";

fn input(key: &str, user_id: &str, message: &str) -> Input {
    Input {
        key: key.into(),
        user_id: user_id.into(),
        user_id_encoding: Encoding::Utf8,
        message: message.into(),
        message_encoding: Encoding::Utf8,
    }
}

fn verify_with(input: Input, signature: &str) -> PluginResult<VerifyOutput> {
    verify(VerifyArgs {
        input,
        signature: signature.into(),
        format: SignatureInput::Auto,
        encoding: Encoding::Hex,
    })
}

fn sign_with(input: Input, format: SignatureFormat) -> SignOutput {
    sign(SignArgs {
        input,
        format,
        encoding: Encoding::Hex,
    })
    .unwrap()
}

fn code<T: std::fmt::Debug>(result: PluginResult<T>) -> String {
    result.unwrap_err().code
}

#[test]
fn computes_standard_za_and_digest() {
    let key = parse_key(D).unwrap();
    assert_eq!(key.format, "raw-private");
    assert_eq!(public_hex(&key.public), PUB);
    let za = za(DEFAULT_USER_ID.as_bytes(), &key.public).unwrap();
    assert_eq!(hex(&za), ZA);
    assert_eq!(hex(&digest(&za, MESSAGE.as_bytes())), E);
}

#[test]
fn reproduces_standard_signature_with_fixed_k() {
    // 按 GB/T 32918.2 步骤用示例 k 复算 r、s，确认与标准一致
    let scalar = |h: &str| Scalar::reduce_bytes(FieldBytes::from_slice(&unhex(h)));
    let (d, e, k) = (scalar(D), scalar(E), scalar(K));
    let x1 = (ProjectivePoint::GENERATOR * k).to_affine().x();
    let r = e + Scalar::reduce_bytes(&x1);
    let s = (Scalar::ONE + d).invert().unwrap() * (k - r * d);
    assert_eq!(hex(&r.to_repr()), R);
    assert_eq!(hex(&s.to_repr()), S);
    assert!(bool::from(!s.is_zero()));
}

#[test]
fn verifies_standard_signature() {
    let raw = format!("{R}{S}");
    for key in [D, PUB, &PUB[2..]] {
        let out = verify_with(input(key, DEFAULT_USER_ID, MESSAGE), &raw).unwrap();
        assert!(out.valid, "{key}");
        assert_eq!(out.format, SignatureFormat::Raw);
        assert_eq!(out.details.za, ZA);
        assert_eq!(out.details.e, E);
        assert_eq!(out.details.entla, 128);
    }
}

#[test]
fn detects_mismatches() {
    let raw = format!("{R}{S}");
    let cases = [
        input(PUB, "1234567812345679", MESSAGE),
        input(PUB, DEFAULT_USER_ID, "message digesT"),
        input(EXT_PUBLIC, DEFAULT_USER_ID, MESSAGE),
    ];
    for case in cases {
        assert!(!verify_with(case, &raw).unwrap().valid);
    }
}

#[test]
fn matches_crate_hashing_for_text_ids() {
    // 自己算的 ZA 与 crate 内部 hash_z 一致：同一私钥、同一标识下确定性签名相同
    for id in [DEFAULT_USER_ID, "alice@example.com", ""] {
        let ours = sign_with(input(D, id, MESSAGE), SignatureFormat::Raw);
        let secret = parse_key(D).unwrap().secret.unwrap();
        let theirs: Signature = SigningKey::new(id, &secret)
            .unwrap()
            .sign(MESSAGE.as_bytes());
        assert_eq!(ours.signature, hex(&theirs.to_bytes()), "id={id:?}");
    }
}

#[test]
fn interoperates_with_an_independent_implementation() {
    let ok = verify_with(input(EXT_PUBLIC, DEFAULT_USER_ID, MESSAGE), EXT_SIG_STD).unwrap();
    assert!(ok.valid);
    assert_eq!(ok.format, SignatureFormat::Der);
    let alice = input(EXT_PUBLIC, "alice@example.com", MESSAGE);
    assert!(verify_with(alice, EXT_SIG_ALICE).unwrap().valid);
    // 未指定标识时对方用的是空标识：按默认标识验签不匹配，按空标识通过
    let default = input(EXT_PUBLIC, DEFAULT_USER_ID, MESSAGE);
    assert!(!verify_with(default, EXT_SIG_EMPTY).unwrap().valid);
    let empty = verify_with(input(EXT_PUBLIC, "", MESSAGE), EXT_SIG_EMPTY).unwrap();
    assert!(empty.valid);
    assert_eq!(empty.details.entla, 0);
    // 用户标识不同即验签失败
    let wrong = input(EXT_PUBLIC, DEFAULT_USER_ID, MESSAGE);
    assert!(!verify_with(wrong, EXT_SIG_ALICE).unwrap().valid);
    // 用对方生成的私钥签名，再用其公钥验签（对方验证我们签名的方向已手工核对）
    let ours = sign_with(
        input(EXT_PRIVATE, DEFAULT_USER_ID, MESSAGE),
        SignatureFormat::Der,
    );
    let check = input(EXT_PUBLIC, DEFAULT_USER_ID, MESSAGE);
    assert!(verify_with(check, &ours.signature).unwrap().valid);
}

#[test]
fn accepts_hex_user_ids_and_messages() {
    let mut hexed = input(
        PUB,
        "31323334353637383132333435363738",
        "6d65737361676520646967657374",
    );
    hexed.user_id_encoding = Encoding::Hex;
    hexed.message_encoding = Encoding::Hex;
    let out = verify_with(hexed, &format!("{R}{S}")).unwrap();
    assert!(out.valid);
    assert_eq!(out.details.user_id, hex(DEFAULT_USER_ID.as_bytes()));
    assert_eq!(out.details.message_bytes, 14);
    // 非 UTF-8 的用户标识也能参与 ZA
    let mut binary = input(D, "00ff10", MESSAGE);
    binary.user_id_encoding = Encoding::Hex;
    let signed = sign_with(binary, SignatureFormat::Der);
    assert_eq!(signed.details.entla, 24);
    let mut check = input(PUB, "00ff10", MESSAGE);
    check.user_id_encoding = Encoding::Hex;
    assert!(verify_with(check, &signed.signature).unwrap().valid);
}

#[test]
fn signing_requires_a_private_key() {
    let result = sign(SignArgs {
        input: input(PUB, DEFAULT_USER_ID, MESSAGE),
        format: SignatureFormat::Der,
        encoding: Encoding::Base64,
    });
    assert_eq!(code(result), "crypto.sm2_private_required");
}

#[test]
fn converts_between_der_and_raw() {
    let der = convert(ConvertArgs {
        signature: format!("{R}{S}"),
        from: SignatureInput::Auto,
        encoding: Encoding::Hex,
        to: SignatureFormat::Der,
        output_encoding: Encoding::Hex,
    })
    .unwrap();
    assert_eq!(der.from, SignatureFormat::Raw);
    // r、s 最高位为 1，DER 需要补 0x00
    assert_eq!(der.signature, format!("3046022100{R}022100{S}"));
    let back = convert(ConvertArgs {
        signature: der.signature,
        from: SignatureInput::Der,
        encoding: Encoding::Hex,
        to: SignatureFormat::Raw,
        output_encoding: Encoding::Base64,
    })
    .unwrap();
    assert_eq!(
        decode(&back.signature, Encoding::Base64, "s").unwrap(),
        unhex(&format!("{R}{S}"))
    );
    // 短整数（EXT_SIG_ALICE 的 s 只有 32 字节且最高位为 0）往返不变
    let (format, r, s) = parse_signature(&unhex(EXT_SIG_ALICE), SignatureInput::Auto).unwrap();
    assert_eq!(format, SignatureFormat::Der);
    assert_eq!(
        hex(&serialize_signature(&r, &s, SignatureFormat::Der)),
        EXT_SIG_ALICE
    );
}

#[test]
fn rejects_malformed_signatures() {
    let der = unhex(EXT_SIG_EMPTY);
    let parse = |bytes: &[u8], format| code(parse_signature(bytes, format));
    let mut trailing = der.clone();
    trailing.push(0);
    assert_eq!(
        parse(&trailing, SignatureInput::Der),
        "crypto.sm2_signature_trailing"
    );
    assert_eq!(
        parse(&der[..der.len() - 1], SignatureInput::Der),
        "crypto.sm2_signature_truncated"
    );
    // 长度字段与内容一致但 s 后面多出数据
    let mut extra = der.clone();
    extra[1] += 2;
    extra.extend([0x05, 0x00]);
    assert_eq!(
        parse(&extra, SignatureInput::Der),
        "crypto.sm2_signature_trailing"
    );
    // 非最短编码：r 前多一个 0x00
    let r_short = format!("3008020300{:02x}01020101", 0x7f);
    assert_eq!(
        parse(&unhex(&r_short), SignatureInput::Der),
        "crypto.sm2_signature_not_minimal"
    );
    // 负数与零
    assert_eq!(
        parse(&unhex("30060201800201ff"), SignatureInput::Der),
        "crypto.sm2_signature_range"
    );
    assert_eq!(
        parse(&unhex("3006020100020101"), SignatureInput::Der),
        "crypto.sm2_signature_range"
    );
    // r = n 越界
    let n = format!("{R}{S}").replacen(R, ORDER, 1);
    let out_of_range = parse_signature(&unhex(&n), SignatureInput::Raw).unwrap_err();
    assert_eq!(
        (
            out_of_range.code.as_str(),
            out_of_range.params["part"].as_str()
        ),
        ("crypto.sm2_signature_range", Some("r"))
    );
    // 原始格式必须正好 64 字节
    assert_eq!(
        parse(&unhex(&format!("{R}{S}"))[..63], SignatureInput::Raw),
        "crypto.sm2_signature_length"
    );
    assert_eq!(
        parse(&[1, 2, 3], SignatureInput::Auto),
        "crypto.sm2_signature_length"
    );
    // auto 下看起来像 DER 的输入报告 DER 的具体问题
    assert_eq!(
        parse(&trailing, SignatureInput::Auto),
        "crypto.sm2_signature_trailing"
    );
}

#[test]
fn verify_reports_invalid_signature_input() {
    let err = verify_with(input(PUB, DEFAULT_USER_ID, MESSAGE), "zz").unwrap_err();
    assert_eq!(err.code, "crypto.invalid_hex");
    let err = verify_with(input(PUB, DEFAULT_USER_ID, MESSAGE), "3006020100020101").unwrap_err();
    assert_eq!(err.code, "crypto.sm2_signature_range");
}

#[test]
fn parses_key_formats() {
    let pem = parse_key(EXT_PRIVATE).unwrap();
    assert_eq!((pem.format, pem.secret.is_some()), ("pkcs8-pem", true));
    let public = parse_key(EXT_PUBLIC).unwrap();
    assert_eq!(
        (public.format, public.secret.is_some()),
        ("spki-pem", false)
    );
    assert_eq!(public_hex(&public.public), public_hex(&pem.public));
    // DER：Base64 与十六进制
    let der = |text: &str| pem::decode_vec(text.as_bytes()).unwrap().1;
    let b64 = data_encoding::BASE64.encode(&der(EXT_PRIVATE));
    assert_eq!(parse_key(&b64).unwrap().format, "pkcs8-der");
    assert_eq!(
        parse_key(&hex(&der(EXT_PUBLIC))).unwrap().format,
        "spki-der"
    );
    // 原始公钥：未压缩、x ‖ y、压缩
    assert_eq!(parse_key(PUB).unwrap().format, "raw-public");
    assert_eq!(parse_key(&PUB[2..]).unwrap().format, "raw-public-xy");
    let compressed = hex(parse_key(PUB)
        .unwrap()
        .public
        .to_encoded_point(true)
        .as_bytes());
    let key = parse_key(&compressed).unwrap();
    assert_eq!(
        (key.format, public_hex(&key.public)),
        ("raw-public-compressed", PUB.to_owned())
    );
}

#[test]
fn inspects_and_round_trips_keys() {
    let info = inspect(D).unwrap();
    assert_eq!((info.kind, info.public_key.as_str()), ("private", PUB));
    assert_eq!(inspect(&info.public_key_pem).unwrap().public_key, PUB);
    let exported = export_private(D).unwrap();
    assert!(exported.starts_with("-----BEGIN PRIVATE KEY-----"));
    assert_eq!(inspect(&exported).unwrap().public_key, PUB);
    assert_eq!(code(export_private(PUB)), "crypto.sm2_private_required");
    // 生成的密钥可直接签名验签
    let pair = generate().unwrap();
    let signed = sign_with(input(&pair.private_key, "id", "hi"), SignatureFormat::Raw);
    assert!(
        verify_with(input(&pair.public_key, "id", "hi"), &signed.signature)
            .unwrap()
            .valid
    );
}

#[test]
fn rejects_unsupported_keys() {
    let err = parse_key(P256_PUBLIC).unwrap_err();
    assert_eq!(
        (err.code.as_str(), err.params["curve"].as_str()),
        ("crypto.sm2_wrong_curve", Some("P-256"))
    );
    let err = parse_key(RSA_PUBLIC).unwrap_err();
    assert_eq!(
        (err.code.as_str(), err.params["algorithm"].as_str()),
        ("crypto.sm2_not_ec", Some("RSA"))
    );
    let sec1 = EXT_PRIVATE.replace("PRIVATE KEY", "SM2 PRIVATE KEY");
    let err = parse_key(&sec1).unwrap_err();
    assert_eq!(
        (err.code.as_str(), err.params["format"].as_str()),
        ("crypto.sm2_unsupported_format", Some("SM2 PRIVATE KEY"))
    );
    let encrypted =
        "-----BEGIN ENCRYPTED PRIVATE KEY-----\nMAA=\n-----END ENCRYPTED PRIVATE KEY-----";
    assert_eq!(code(parse_key(encrypted)), "crypto.encrypted_key");
    assert_eq!(code(parse_key("  ")), "crypto.sm2_key_required");
    // 私钥标量越界：0、n、n-1
    let zero = "00".repeat(32);
    let mut n_minus_1 = order();
    n_minus_1[31] -= 1;
    for scalar in [zero, ORDER.to_owned(), hex(&n_minus_1)] {
        assert_eq!(code(parse_key(&scalar)), "crypto.sm2_key_range");
    }
    // 不在曲线上的点
    let off_curve = format!("04{}", "11".repeat(64));
    assert_eq!(code(parse_key(&off_curve)), "crypto.sm2_point_invalid");
    assert_eq!(
        code(parse_key("not a key!")),
        "crypto.sm2_unsupported_format"
    );
}

#[test]
fn rejects_overlong_user_ids() {
    let long = "a".repeat(8192);
    assert_eq!(
        code(verify_with(input(PUB, &long, MESSAGE), &format!("{R}{S}"))),
        "crypto.sm2_user_id_too_long"
    );
    let max = "a".repeat(8191);
    assert!(
        !verify_with(input(PUB, &max, MESSAGE), &format!("{R}{S}"))
            .unwrap()
            .valid
    );
}
