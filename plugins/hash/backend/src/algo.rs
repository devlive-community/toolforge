use digest::Digest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_256,
    Sha3_512,
    Sm3,
    Crc32,
}

/// 统一的增量哈希接口，便于一次读取文件同时计算多种摘要
pub trait Hasher: Send {
    fn update(&mut self, data: &[u8]);
    fn finish(self: Box<Self>) -> Vec<u8>;
}

struct DigestHasher<D>(D);

impl<D: Digest + Send> Hasher for DigestHasher<D> {
    fn update(&mut self, data: &[u8]) {
        Digest::update(&mut self.0, data);
    }

    fn finish(self: Box<Self>) -> Vec<u8> {
        self.0.finalize().to_vec()
    }
}

struct Crc32(crc32fast::Hasher);

impl Hasher for Crc32 {
    fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    fn finish(self: Box<Self>) -> Vec<u8> {
        self.0.finalize().to_be_bytes().to_vec()
    }
}

impl Algorithm {
    pub fn hasher(self) -> Box<dyn Hasher> {
        match self {
            Algorithm::Md5 => Box::new(DigestHasher(md5::Md5::new())),
            Algorithm::Sha1 => Box::new(DigestHasher(sha1::Sha1::new())),
            Algorithm::Sha224 => Box::new(DigestHasher(sha2::Sha224::new())),
            Algorithm::Sha256 => Box::new(DigestHasher(sha2::Sha256::new())),
            Algorithm::Sha384 => Box::new(DigestHasher(sha2::Sha384::new())),
            Algorithm::Sha512 => Box::new(DigestHasher(sha2::Sha512::new())),
            Algorithm::Sha3_256 => Box::new(DigestHasher(sha3::Sha3_256::new())),
            Algorithm::Sha3_512 => Box::new(DigestHasher(sha3::Sha3_512::new())),
            Algorithm::Sm3 => Box::new(DigestHasher(sm3::Sm3::new())),
            Algorithm::Crc32 => Box::new(Crc32(crc32fast::Hasher::new())),
        }
    }
}

pub fn encode(bytes: &[u8], uppercase: bool) -> String {
    if uppercase {
        hex::encode_upper(bytes)
    } else {
        hex::encode(bytes)
    }
}

/// 一次计算多种摘要
pub struct MultiHasher(Vec<(Algorithm, Box<dyn Hasher>)>);

impl MultiHasher {
    pub fn new(algorithms: &[Algorithm]) -> Self {
        Self(algorithms.iter().map(|a| (*a, a.hasher())).collect())
    }

    pub fn update(&mut self, data: &[u8]) {
        for (_, hasher) in &mut self.0 {
            hasher.update(data);
        }
    }

    pub fn finish(self, uppercase: bool) -> Vec<(Algorithm, String)> {
        self.0
            .into_iter()
            .map(|(algorithm, hasher)| (algorithm, encode(&hasher.finish(), uppercase)))
            .collect()
    }
}

#[cfg(test)]
#[path = "algo_test.rs"]
mod tests;
