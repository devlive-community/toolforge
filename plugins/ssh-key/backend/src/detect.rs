//! 剪贴板识别：OpenSSH 公钥行与私钥块。

use tf_plugin_api::Detection;

pub fn detect(text: &str) -> Option<Detection> {
    let trimmed = text.trim();
    if trimmed.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----") {
        return Some(Detection::new(95, "privateKey"));
    }
    let first = trimmed.lines().next()?.trim();
    let algorithm = [
        "ssh-ed25519 AAAA",
        "ssh-rsa AAAA",
        "ecdsa-sha2-nistp256 AAAA",
        "ecdsa-sha2-nistp384 AAAA",
        "ecdsa-sha2-nistp521 AAAA",
        "sk-ssh-ed25519@openssh.com AAAA",
    ]
    .iter()
    .any(|prefix| first.contains(prefix));
    algorithm.then(|| Detection::new(90, "publicKey"))
}
