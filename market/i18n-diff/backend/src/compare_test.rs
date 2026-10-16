use super::*;

fn file(name: &str, text: &str) -> FileInput {
    FileInput {
        name: name.to_owned(),
        text: text.to_owned(),
        format: None,
    }
}

#[test]
fn reports_every_issue_kind() {
    let base = file(
        "en.json",
        r#"{"a":"Hello {name}","b":"Bye","c":"Save","d":"%s of %s","ok":"OK"}"#,
    );
    let zh = file(
        "zh.json",
        r#"{"a":"你好","b":"","c":"Save","d":"%s / %s","x":"多余","ok":"OK"}"#,
    );
    let out = run(Args {
        files: vec![base, zh],
    })
    .unwrap();
    let keys: Vec<&str> = out.rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["a", "b", "c", "d", "ok", "x"]);
    let issues = |key: &str| {
        out.rows.iter().find(|r| r.key == key).unwrap().cells[1]
            .issues
            .clone()
    };
    assert_eq!(issues("a"), [Issue::Placeholders]);
    assert_eq!(issues("b"), [Issue::Empty]);
    assert_eq!(issues("c"), [Issue::Untranslated]);
    assert!(issues("d").is_empty());
    // 「OK」只有两个字母，仍按未翻译提示；单字母或纯占位符不提示
    assert_eq!(issues("ok"), [Issue::Untranslated]);
    assert_eq!(issues("x"), [Issue::Extra]);
    let a = &out.rows[0].cells[1];
    assert_eq!(a.missing_placeholders, ["{name}"]);

    let zh = &out.files[1];
    assert_eq!(
        (
            zh.keys,
            zh.missing,
            zh.extra,
            zh.empty,
            zh.untranslated,
            zh.placeholders
        ),
        (6, 0, 1, 1, 2, 1)
    );
    assert_eq!(zh.coverage, 80.0);
    assert!(zh.writable);
}

#[test]
fn compares_across_formats_and_finds_missing_and_duplicate_keys() {
    let base = file("messages.properties", "title=Title\nbody=Body\n");
    let de = file("messages_de.properties", "title=Titel\ntitle=Titel 2\n");
    let fr = file("fr.yml", "title: Titre\nbody: Corps\n");
    let out = run(Args {
        files: vec![base, de, fr],
    })
    .unwrap();
    assert_eq!(out.rows[0].cells[1].issues, [Issue::Duplicate]);
    assert_eq!(out.rows[0].cells[1].value.as_deref(), Some("Titel 2"));
    assert_eq!(out.rows[1].cells[1].issues, [Issue::Missing]);
    assert!(out.rows[1].cells[2].issues.is_empty());
    assert_eq!(out.files[1].coverage, 50.0);
    assert!(!out.files[2].writable);
}

#[test]
fn validates_input() {
    assert_eq!(
        run(Args {
            files: vec![file("a.json", "{}")]
        })
        .unwrap_err()
        .code,
        "i18n.file_count"
    );
    assert_eq!(
        run(Args {
            files: vec![file("a.json", "{}"), file("b.txt", "")]
        })
        .unwrap_err()
        .code,
        "i18n.unknown_format"
    );
}
