#[cfg(unix)]
use std::process::Command;

use super::*;

#[cfg(unix)]
fn ssh_keygen() -> bool {
    Command::new("ssh-keygen").arg("-?").output().is_ok()
}

#[cfg(unix)]
fn temp(name: &str) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tfp-ssh-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn generates_every_key_type() {
    for (kind, algorithm, bits) in [
        (Kind::Ed25519, "ssh-ed25519", 256),
        (Kind::Ecdsa256, "ecdsa-sha2-nistp256", 256),
        (Kind::Ecdsa384, "ecdsa-sha2-nistp384", 384),
        (Kind::Ecdsa521, "ecdsa-sha2-nistp521", 521),
        (Kind::Rsa2048, "ssh-rsa", 2048),
    ] {
        let generated = generate(kind, " me@laptop ", None).unwrap();
        assert_eq!(
            (generated.info.algorithm.as_str(), generated.info.bits),
            (algorithm, bits)
        );
        assert!(
            generated
                .private
                .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----")
        );
        assert!(
            generated.public.starts_with(algorithm) && generated.public.ends_with(" me@laptop"),
            "{}",
            generated.public
        );
        let inspected = inspect(&generated.private, None);
        assert_eq!(inspected.keys[0].sha256, generated.info.sha256);
        assert!(!inspected.keys[0].encrypted);
    }
}

#[test]
fn encrypts_and_changes_passphrases() {
    let generated = generate(Kind::Ed25519, "c", Some("secret")).unwrap();
    assert!(generated.info.encrypted);
    assert_eq!(generated.info.cipher.as_deref(), Some("aes256-ctr"));
    // 不提供口令也能读取公钥与指纹
    let info = &inspect(&generated.private, None).keys[0];
    assert!(info.encrypted);
    assert_eq!(info.sha256, generated.info.sha256);
    assert_eq!(
        inspect(&generated.private, Some("wrong")).problems[0].code,
        "ssh.wrong_passphrase"
    );

    assert_eq!(
        change_passphrase(&generated.private, None, None)
            .unwrap_err()
            .code,
        "ssh.passphrase_required"
    );
    assert_eq!(
        change_passphrase(&generated.private, Some("nope"), None)
            .unwrap_err()
            .code,
        "ssh.wrong_passphrase"
    );
    let plain = change_passphrase(&generated.private, Some("secret"), None).unwrap();
    assert!(!inspect(&plain, None).keys[0].encrypted);
    let again = change_passphrase(&plain, None, Some("new")).unwrap();
    assert!(inspect(&again, Some("new")).problems.is_empty());
    assert_eq!(inspect(&again, None).keys[0].sha256, generated.info.sha256);
}

#[test]
fn inspects_authorized_keys_and_known_hosts() {
    let a = generate(Kind::Ed25519, "alice", None).unwrap().public;
    let b = generate(Kind::Ecdsa256, "", None).unwrap().public;
    let text = format!(
        "# authorized_keys\n\
         {a}\n\
         command=\"echo hi there\",no-pty {b}\n\
         \n\
         github.com,140.82.112.3 {b}\n\
         |1|JfKTdBh7rNbXkVAQCRp4OQoPfmI=|USECr3SWf1JUPsms5AqfD5QfxkM= {a}\n\
         not a key at all\n"
    );
    let result = inspect(&text, None);
    assert_eq!(result.keys.len(), 4);
    assert_eq!(
        (result.keys[0].line, result.keys[0].comment.as_str()),
        (2, "alice")
    );
    assert_eq!(
        result.keys[1].options.as_deref(),
        Some("command=\"echo hi there\",no-pty")
    );
    assert_eq!(
        result.keys[2].hosts.as_deref(),
        Some("github.com,140.82.112.3")
    );
    assert!(result.keys[3].hosts.as_deref().unwrap().starts_with("|1|"));
    assert_eq!(result.problems.len(), 1);
    assert_eq!(
        (result.problems[0].line, result.problems[0].code.as_str()),
        (7, "ssh.invalid_key")
    );
}

#[test]
fn converts_legacy_rsa_pem() {
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::pkcs8::EncodePrivateKey;
    let rsa = rsa::RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
    for pem in [
        rsa.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string(),
        rsa.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string(),
    ] {
        let result = inspect(&pem, None);
        let info = &result.keys[0];
        assert_eq!((info.label, info.bits), ("RSA", 2048));
        let converted = info.converted.as_deref().unwrap();
        assert_eq!(inspect(converted, None).keys[0].sha256, info.sha256);
    }
    let encrypted =
        "-----BEGIN ENCRYPTED PRIVATE KEY-----\nAAAA\n-----END ENCRYPTED PRIVATE KEY-----";
    assert_eq!(
        inspect(encrypted, None).problems[0].code,
        "ssh.legacy_encrypted"
    );
}

#[test]
fn draws_randomart() {
    let art = randomart("ED25519 256", &[0u8; 32], "SHA256");
    let lines: Vec<&str> = art.lines().collect();
    assert_eq!(lines.len(), 11);
    assert_eq!(lines[0], "+--[ED25519 256]--+");
    assert_eq!(lines[10], "+----[SHA256]-----+");
    assert!(lines[1..10].iter().all(|l| l.len() == 19));
    assert!(art.contains('S') && art.contains('E'));
}

/// Windows 的 ssh-keygen 会按文件 ACL 拒绝“权限过宽”的私钥，只在 Unix 上做互通测试
#[cfg(unix)]
#[test]
fn matches_ssh_keygen() {
    if !ssh_keygen() {
        return;
    }
    let dir = temp("interop");
    // ssh-keygen 生成的密钥：指纹与随机图一致
    let path = dir.join("id_ed25519");
    let status = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test@host", "-f"])
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    let private = std::fs::read_to_string(&path).unwrap();
    let info = &inspect(&private, None).keys[0];
    let listing = Command::new("ssh-keygen")
        .args(["-l", "-v", "-f"])
        .arg(&path)
        .output()
        .unwrap();
    let listing = String::from_utf8(listing.stdout).unwrap();
    assert!(listing.contains(&info.sha256), "{listing}\n{}", info.sha256);
    assert!(
        listing.contains(&info.randomart),
        "{listing}\n{}",
        info.randomart
    );
    let md5 = Command::new("ssh-keygen")
        .args(["-l", "-E", "md5", "-f"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(String::from_utf8(md5.stdout).unwrap().contains(&info.md5));

    // 本工具生成并加密的密钥：ssh-keygen 能用口令读出相同的公钥
    for kind in [Kind::Ed25519, Kind::Ecdsa384, Kind::Rsa2048] {
        let generated = generate(kind, "ours", Some("pass phrase")).unwrap();
        let ours = dir.join("ours");
        std::fs::write(&ours, &generated.private).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&ours, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let output = Command::new("ssh-keygen")
            .args(["-y", "-P", "pass phrase", "-f"])
            .arg(&ours)
            .output()
            .unwrap();
        let derived = String::from_utf8(output.stdout).unwrap();
        let expected = generated.public.rsplit_once(' ').unwrap().0;
        assert!(
            derived.starts_with(expected),
            "{kind:?}: {derived} / {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
