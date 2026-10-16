//! Check generated results against Git itself, rather than a second matcher.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct GitFixture {
    root: PathBuf,
}

impl GitFixture {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "toolforge-gitignore-{}-{unique}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("empty-config"), "").unwrap();
        let fixture = Self { root };
        let output = fixture
            .command()
            .args(["init", "--quiet"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for parts in [
            vec!["build"],
            vec!["nested", "build"],
            vec!["foo", "x"],
            vec!["logs"],
            vec!["target"],
            vec!["node_modules"],
        ] {
            let mut path = fixture.root.clone();
            for part in parts {
                path.push(part);
            }
            std::fs::create_dir_all(path).unwrap();
        }
        fixture
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command
            .current_dir(&self.root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.root.join("empty-config"))
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_COMMON_DIR")
            .args(["-c", "core.ignorecase=false", "-c", "core.excludesfile="]);
        command
    }

    fn ignored(&self, patterns: &str, paths: &[&str]) -> Vec<u8> {
        std::fs::write(self.root.join(".gitignore"), patterns).unwrap();
        let mut child = self
            .command()
            .args(["check-ignore", "--no-index", "-z", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Git must be available to verify gitignore semantics");
        let mut stdin = child.stdin.take().unwrap();
        for path in paths {
            stdin.write_all(path.as_bytes()).unwrap();
            stdin.write_all(&[0]).unwrap();
        }
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "Git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
}

impl Drop for GitFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn git_equivalence_preserves_negations_escapes_and_parent_directory_rules() {
    let fixture = GitFixture::new();
    let base = "\u{feff}# Keep comments\n*.log\n*.log\n!keep.log\n*.log\n!keep2.log\nbuild/\n!build/keep.txt\n\\#hash\n\\!bang\n leading\nodd\\ \n";
    let custom = "\u{feff}*.log\n!keep2.log\nbuild/\n!build/\nbuild/*\n!build/keep.txt\nfoo/**\n!foo/x/\nfoo/x/*\n!foo/x/y.txt\n.env*\n!.env\n";
    let paths = [
        "trace.log",
        "keep.log",
        "keep2.log",
        "notes.txt",
        "logs/keep.log",
        "build/cache.bin",
        "build/keep.txt",
        "nested/build/cache.bin",
        "nested/build/keep.txt",
        "foo/a.txt",
        "foo/x/y.txt",
        "foo/x/z.txt",
        ".env",
        ".env.local",
        "#hash",
        "!bang",
        " leading",
        "odd ",
        "plain",
    ];
    let baseline = format!(
        "{}\n{}",
        base.trim_start_matches('\u{feff}'),
        custom.trim_start_matches('\u{feff}')
    );
    let expected = fixture.ignored(&baseline, &paths);
    assert!(expected.windows(9).any(|part| part == b"trace.log"));
    for line_ending in [LineEnding::Lf, LineEnding::Crlf] {
        let result = run(Args {
            base_content: base.into(),
            custom_rules: custom.into(),
            line_ending,
            ..Args::default()
        })
        .unwrap();
        assert!(result.stats.duplicate_count > 0);
        assert_eq!(fixture.ignored(&result.output, &paths), expected);
    }
}

#[test]
fn git_equivalence_covers_bom_and_literal_carriage_return_edges() {
    let fixture = GitFixture::new();
    let paths = [
        "foo",
        "\u{feff}foo",
        "foo\r",
        "foo\rbar",
        "#hash",
        "!bang",
        "\t",
        "escaped ",
    ];
    for source in [
        "\u{feff}foo\n",
        "\u{feff}\u{feff}foo\n",
        "foo\r",
        "foo\r\n",
        "foo\r\r\n",
        "foo\rbar\n",
        "\\#hash\n\\!bang\n\t\nescaped\\ \n",
    ] {
        let expected = fixture.ignored(source, &paths);
        for line_ending in [LineEnding::Lf, LineEnding::Crlf] {
            let generated = run(Args {
                base_content: source.into(),
                line_ending,
                ..Args::default()
            })
            .unwrap();
            assert_eq!(
                fixture.ignored(&generated.output, &paths),
                expected,
                "{source:?}"
            );
        }
    }
}

#[test]
fn git_equivalence_covers_all_bundled_templates_in_selected_order() {
    let fixture = GitFixture::new();
    let paths = [
        "target/debug/tool",
        "Cargo.lock",
        "node_modules/example/a.js",
        "main.pyc",
        ".env",
        ".env.local",
        ".DS_Store",
        "Thumbs.db",
        "build/cache.bin",
        ".idea/workspace.xml",
        ".vscode/settings.json",
        "notes.swp",
        "bin/app.exe",
        "coverage/lcov.info",
        "README.md",
        "src/main.rs",
        "foo/x/y.txt",
    ];
    for reversed in [false, true] {
        let mut templates: Vec<_> = catalog::TEMPLATES.iter().collect();
        if reversed {
            templates.reverse();
        }
        let baseline = templates
            .iter()
            .map(|template| {
                let lines: Vec<_> = source_lines(template.text).collect();
                lines.join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let generated = run(Args {
            template_ids: templates
                .iter()
                .map(|template| template.id.into())
                .collect(),
            ..Args::default()
        })
        .unwrap();
        assert_eq!(
            fixture.ignored(&generated.output, &paths),
            fixture.ignored(&baseline, &paths)
        );
    }
}

#[test]
fn git_equivalence_covers_reusable_custom_templates_mixed_with_builtins() {
    use crate::library::{Library, Template};

    let fixture = GitFixture::new();
    let before = "*.log\n!keep.log\nbuild/\n";
    let after = "*.log\n!build/\nbuild/*\n!build/keep.txt\n";
    let library = Library {
        next_id: 3,
        templates: vec![
            Template {
                id: "custom-1".into(),
                name: "Before Node".into(),
                content: before.into(),
            },
            Template {
                id: "custom-2".into(),
                name: "After Node".into(),
                content: after.into(),
            },
        ],
        ..Library::default()
    };
    let generated = run(Args {
        library,
        template_ids: vec!["custom-1".into(), "node".into(), "custom-2".into()],
        ..Args::default()
    })
    .unwrap();
    let baseline = format!("{before}\n{}\n{after}", catalog::find("node").unwrap().text);
    let paths = [
        "trace.log",
        "keep.log",
        "build/cache.bin",
        "build/keep.txt",
        "node_modules/a.js",
        "src/main.js",
    ];
    assert_eq!(
        fixture.ignored(&generated.output, &paths),
        fixture.ignored(&baseline, &paths)
    );
}
