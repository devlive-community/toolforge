use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tf-schema-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn args(&self, name: &str) -> Args {
        Args {
            path: self.0.join(name).to_string_lossy().into_owned(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reads_utf8_and_removes_only_a_leading_bom_without_reformatting() {
    let fixture = Fixture::new();
    let input = "\u{feff}{\n  \"中文\": \"\u{feff}\"\n}\n";
    std::fs::write(fixture.0.join("input.json"), input).unwrap();
    assert_eq!(read(fixture.args("input.json")).unwrap(), &input[3..]);
}

#[test]
fn rejects_missing_directory_non_utf8_and_oversize_inputs() {
    let fixture = Fixture::new();
    assert_eq!(
        read(fixture.args("missing.json")).unwrap_err().code,
        "schema.file_read"
    );
    assert!(
        read(Args {
            path: fixture.0.to_string_lossy().into_owned()
        })
        .is_err()
    );
    std::fs::write(fixture.0.join("binary.json"), [0xff, 0xfe, 0x00]).unwrap();
    assert_eq!(
        read(fixture.args("binary.json")).unwrap_err().code,
        "schema.file_encoding"
    );
    std::fs::write(fixture.0.join("large.json"), vec![b' '; MAX_BYTES + 1]).unwrap();
    assert_eq!(
        read(fixture.args("large.json")).unwrap_err().code,
        "schema.file_too_large"
    );
    std::fs::write(fixture.0.join("empty.json"), []).unwrap();
    assert_eq!(read(fixture.args("empty.json")).unwrap(), "");
}
