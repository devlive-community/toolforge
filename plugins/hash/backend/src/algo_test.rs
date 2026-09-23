use super::*;

fn digest(algorithm: Algorithm, input: &str) -> String {
    let mut hasher = MultiHasher::new(&[algorithm]);
    hasher.update(input.as_bytes());
    hasher.finish(false).remove(0).1
}

#[test]
fn matches_known_vectors_for_abc() {
    let cases = [
        (Algorithm::Md5, "900150983cd24fb0d6963f7d28e17f72"),
        (Algorithm::Sha1, "a9993e364706816aba3e25717850c26c9cd0d89d"),
        (
            Algorithm::Sha224,
            "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7",
        ),
        (
            Algorithm::Sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            Algorithm::Sha3_256,
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532",
        ),
        (
            Algorithm::Sm3,
            "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0",
        ),
        (Algorithm::Crc32, "352441c2"),
    ];
    for (algorithm, expected) in cases {
        assert_eq!(digest(algorithm, "abc"), expected, "{algorithm:?}");
    }
}

#[test]
fn incremental_updates_equal_single_update() {
    let mut split = MultiHasher::new(&[Algorithm::Sha512]);
    split.update(b"hello ");
    split.update(b"world");
    assert_eq!(
        split.finish(false)[0].1,
        digest(Algorithm::Sha512, "hello world")
    );
}

#[test]
fn uppercase_encoding() {
    let mut hasher = MultiHasher::new(&[Algorithm::Md5]);
    hasher.update(b"abc");
    assert_eq!(hasher.finish(true)[0].1, "900150983CD24FB0D6963F7D28E17F72");
}

#[test]
fn algorithm_names_are_snake_case() {
    assert_eq!(
        serde_json::to_value(Algorithm::Sha3_256).unwrap(),
        "sha3_256"
    );
}
