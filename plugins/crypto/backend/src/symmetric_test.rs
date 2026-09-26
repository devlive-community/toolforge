use super::*;

const NIST_KEY: &str = "2b7e151628aed2a6abf7158809cf4f3c";
const NIST_PLAIN: &str = "6bc1bee22e409f96e93d7e117393172a";

fn args(algorithm: Algorithm, mode: Mode, key: &str, iv: &str, input: &str) -> Args {
    Args {
        algorithm,
        mode,
        direction: Direction::Encrypt,
        input: input.into(),
        input_encoding: Encoding::Hex,
        output_encoding: Encoding::Hex,
        key: key.into(),
        key_encoding: KeyEncoding::Hex,
        key_bits: (key.len() * 4) as u16,
        iv: iv.into(),
        embed_iv: false,
        salt: String::new(),
        iterations: 1000,
        aad: String::new(),
        padding: Padding::None,
    }
}

fn reverse(mut a: Args, output: &str) -> Args {
    a.direction = Direction::Decrypt;
    a.input = output.into();
    a
}

fn check(a: Args, expected: &str) {
    let out = run(Args {
        ..a.clone_for_test()
    })
    .unwrap();
    assert_eq!(out.output, expected);
    let back = run(reverse(a.clone_for_test(), &out.output)).unwrap();
    assert_eq!(back.output, a.input);
}

impl Args {
    fn clone_for_test(&self) -> Args {
        Args {
            input: self.input.clone(),
            key: self.key.clone(),
            iv: self.iv.clone(),
            salt: self.salt.clone(),
            aad: self.aad.clone(),
            ..*self
        }
    }
}

#[test]
fn matches_nist_aes_vectors() {
    // NIST SP 800-38A F.1.1 / F.2.1 / F.5.1
    check(
        args(Algorithm::Aes, Mode::Ecb, NIST_KEY, "", NIST_PLAIN),
        "3ad77bb40d7a3660a89ecaf32466ef97",
    );
    check(
        args(
            Algorithm::Aes,
            Mode::Cbc,
            NIST_KEY,
            "000102030405060708090a0b0c0d0e0f",
            NIST_PLAIN,
        ),
        "7649abac8119b246cee98e9b12e9197d",
    );
    check(
        args(
            Algorithm::Aes,
            Mode::Ctr,
            NIST_KEY,
            "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff",
            NIST_PLAIN,
        ),
        "874d6191b620e3261bef6864990db6ce",
    );
    // GCM 规范测试用例 2：全零密钥、nonce 与明文
    check(
        args(
            Algorithm::Aes,
            Mode::Gcm,
            &"00".repeat(16),
            &"00".repeat(12),
            &"00".repeat(16),
        ),
        "0388dace60b6a392f328c2b971b2fe78ab6e47d42cec13bdf53a67b21257bddf",
    );
}

#[test]
fn matches_the_sm4_standard_example() {
    // GB/T 32907-2016 附录 A.1
    let key = "0123456789abcdeffedcba9876543210";
    check(
        args(Algorithm::Sm4, Mode::Ecb, key, "", key),
        "681edf34d206965e86b3e94f536e4246",
    );
}

#[test]
fn round_trips_text_with_passwords_and_embedded_ivs() {
    for (algorithm, mode) in [
        (Algorithm::Aes, Mode::Gcm),
        (Algorithm::Aes, Mode::Cbc),
        (Algorithm::Sm4, Mode::Gcm),
        (Algorithm::Sm4, Mode::Cbc),
        (Algorithm::Chacha20, Mode::Gcm),
    ] {
        let mut a = args(algorithm, mode, "correct horse", "", "你好，ToolForge");
        a.key_encoding = KeyEncoding::Password;
        a.key_bits = 256;
        a.input_encoding = Encoding::Utf8;
        a.output_encoding = Encoding::Base64;
        a.embed_iv = true;
        a.padding = Padding::Pkcs7;
        a.aad = "header".into();
        let out = run(a.clone_for_test()).unwrap();
        let salt = out.salt.clone().unwrap();
        assert_eq!(
            out.derived_key.as_ref().unwrap().len(),
            if algorithm == Algorithm::Sm4 { 32 } else { 64 }
        );

        let mut back = reverse(a, &out.output);
        back.input_encoding = Encoding::Base64;
        back.output_encoding = Encoding::Utf8;
        back.salt = salt;
        assert_eq!(
            run(back.clone_for_test()).unwrap().output,
            "你好，ToolForge",
            "{algorithm:?} {mode:?}"
        );
        // 认证数据不同则解密失败
        back.aad = "other".into();
        if mode == Mode::Gcm {
            assert_eq!(run(back).unwrap_err().code, "crypto.decrypt_failed");
        }
    }
}

#[test]
fn random_ivs_differ_each_time() {
    let mut a = args(Algorithm::Aes, Mode::Gcm, &"11".repeat(32), "", "00ff");
    a.key_bits = 256;
    let first = run(a.clone_for_test()).unwrap();
    let second = run(a).unwrap();
    assert_ne!(first.iv, second.iv);
    assert_ne!(first.output, second.output);
}

#[test]
fn reports_errors() {
    let code = |a: Args| run(a).unwrap_err().code;
    assert_eq!(
        code(args(Algorithm::Aes, Mode::Cbc, "00", "", "00")),
        "crypto.invalid_key_bits"
    );
    let mut short = args(Algorithm::Aes, Mode::Cbc, &"00".repeat(15), "", "00");
    short.key_bits = 128;
    assert_eq!(code(short), "crypto.invalid_key_length");
    assert_eq!(
        code(args(
            Algorithm::Aes,
            Mode::Cbc,
            NIST_KEY,
            "0011",
            NIST_PLAIN
        )),
        "crypto.invalid_iv_length"
    );
    assert_eq!(
        code(args(Algorithm::Aes, Mode::Ecb, NIST_KEY, "", "00")),
        "crypto.not_aligned"
    );
    assert_eq!(
        code(reverse(
            args(Algorithm::Aes, Mode::Cbc, NIST_KEY, "", NIST_PLAIN),
            NIST_PLAIN
        )),
        "crypto.iv_required"
    );
    let mut wrong = reverse(
        args(Algorithm::Aes, Mode::Ecb, NIST_KEY, "", ""),
        "3ad77bb40d7a3660a89ecaf32466ef97",
    );
    wrong.padding = Padding::Pkcs7;
    assert_eq!(code(wrong), "crypto.decrypt_failed");
    let mut password = reverse(args(Algorithm::Aes, Mode::Gcm, "pw", "", "00"), "00");
    password.key_encoding = KeyEncoding::Password;
    password.key_bits = 128;
    assert_eq!(code(password), "crypto.salt_required");
    let mut binary = args(Algorithm::Aes, Mode::Ecb, NIST_KEY, "", NIST_PLAIN);
    binary.output_encoding = Encoding::Utf8;
    assert_eq!(code(binary), "crypto.not_utf8");
    assert_eq!(
        code(args(Algorithm::Aes, Mode::Ecb, NIST_KEY, "", "")),
        "crypto.empty_input"
    );
}
