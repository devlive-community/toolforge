use super::*;
use crate::test_support::{encrypted, sample, workspace};

fn src(path: &str, password: Option<&str>) -> Source {
    Source {
        path: path.into(),
        password: password.map(String::from),
    }
}

#[test]
fn reads_pages_with_inherited_attributes() {
    let dir = workspace("info");
    let path = sample(&dir, "a.pdf", 4);
    let info = info(&src(&path, None)).unwrap();
    assert_eq!(info.pages.len(), 4);
    assert_eq!(
        (
            info.pages[0].width,
            info.pages[0].height,
            info.pages[0].rotate
        ),
        (300.0, 400.0, 0)
    );
    assert_eq!(
        info.pages[2].rotate, 90,
        "rotation inherited from the intermediate node"
    );
    assert_eq!(info.metadata.title, "a.pdf");
    assert_eq!(info.metadata.author, "Ada");
    assert!(!info.encrypted);
    assert_eq!(info.version, "1.5");
}

#[test]
fn handles_passwords_and_bad_files() {
    let dir = workspace("password");
    let path = encrypted(&dir, "e.pdf", "user");
    assert_eq!(
        info(&src(&path, None)).unwrap_err().code,
        "pdf.password_required"
    );
    assert_eq!(
        info(&src(&path, Some("nope"))).unwrap_err().code,
        "pdf.wrong_password"
    );
    let opened = info(&src(&path, Some("user"))).unwrap();
    assert!(opened.encrypted);
    assert_eq!(opened.pages.len(), 2);
    assert_eq!(opened.metadata.title, "secret");

    let broken = dir.join("broken.pdf");
    std::fs::write(&broken, b"%PDF-1.4 not really").unwrap();
    assert_eq!(
        info(&src(&broken.to_string_lossy(), None))
            .unwrap_err()
            .code,
        "pdf.invalid"
    );
    assert_eq!(
        info(&src("/definitely/missing.pdf", None))
            .unwrap_err()
            .code,
        "fs.not_found"
    );
}
