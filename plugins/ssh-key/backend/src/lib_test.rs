use super::*;

#[test]
fn saves_keys_with_safe_permissions() {
    let dir = std::env::temp_dir().join(format!("tfp-ssh-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".ssh/id_test");
    let plugin = SshKey::default();
    let generated = keys::generate(keys::Kind::Ed25519, "me", None).unwrap();
    let args = json!({ "path": path, "private": generated.private, "public": generated.public });
    plugin.call("save", args.clone()).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), generated.private);
    assert_eq!(
        std::fs::read_to_string(dir.join(".ssh/id_test.pub")).unwrap(),
        format!("{}\n", generated.public)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(
        plugin.call("save", args.clone()).unwrap_err().code,
        "ssh.file_exists"
    );
    let mut overwrite = args;
    overwrite["overwrite"] = json!(true);
    plugin.call("save", overwrite).unwrap();
}

#[test]
fn detects_keys_on_the_clipboard() {
    assert_eq!(
        detect::detect("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI me")
            .unwrap()
            .label,
        "publicKey"
    );
    assert_eq!(
        detect::detect("command=\"x\" ssh-rsa AAAAB3Nza me")
            .unwrap()
            .label,
        "publicKey"
    );
    assert_eq!(
        detect::detect("-----BEGIN OPENSSH PRIVATE KEY-----\nabc")
            .unwrap()
            .label,
        "privateKey"
    );
    assert!(detect::detect("ssh-ed25519 is an algorithm").is_none());
    assert!(detect::detect("hello").is_none());
}
