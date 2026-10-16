use super::*;
use serde_json::json;

fn mutate(
    library: Library,
    operation: Operation,
    id: Option<&str>,
    name: Option<&str>,
    content: Option<&str>,
) -> PluginResult<Output> {
    run(Args {
        operation,
        library,
        id: id.map(str::to_owned),
        name: name.map(str::to_owned),
        content: content.map(str::to_owned),
    })
}

fn save(library: Library, id: Option<&str>, name: &str, content: &str) -> Output {
    mutate(library, Operation::Save, id, Some(name), Some(content)).unwrap()
}

fn template(number: u64) -> Template {
    Template {
        id: format!("custom-{number}"),
        name: format!("Template {number}"),
        content: format!("custom-{number}/\n"),
    }
}

fn one_template() -> Library {
    Library {
        next_id: 2,
        templates: vec![template(1)],
        ..Library::default()
    }
}

#[test]
fn create_update_delete_and_storage_roundtrip_preserve_stable_ids_and_order() {
    let first = save(
        Library::default(),
        None,
        "My Rust project",
        "# kept\ntarget/\n!target/keep\n",
    );
    assert_eq!(first.template_id.as_deref(), Some("custom-1"));
    let second = save(first.library, None, "团队规则", "private/\n");
    assert_eq!(second.template_id.as_deref(), Some("custom-2"));
    let serialized = serde_json::to_string(&second.library).unwrap();
    let loaded: Library = serde_json::from_str(&serialized).unwrap();
    assert_eq!(loaded, second.library);
    let validated = mutate(loaded, Operation::Validate, None, None, None).unwrap();
    assert_eq!(validated.library, second.library);
    assert!(validated.template_id.is_none());
    let updated = save(
        validated.library,
        Some("custom-1"),
        "MY RUST PROJECT",
        "target/\ncustom/\n",
    );
    assert_eq!(updated.library.templates[0].id, "custom-1");
    assert_eq!(updated.library.templates[1].id, "custom-2");
    assert_eq!(updated.library.next_id, 3);
    assert_eq!(updated.library.templates[0].content, "target/\ncustom/\n");
    let deleted = mutate(
        updated.library,
        Operation::Delete,
        Some("custom-1"),
        None,
        None,
    )
    .unwrap();
    assert_eq!(deleted.library.next_id, 3);
    assert_eq!(deleted.library.templates[0].id, "custom-2");
    let recreated = save(deleted.library, None, "New rules", "new/\n");
    assert_eq!(recreated.template_id.as_deref(), Some("custom-3"));
    assert_eq!(recreated.library.templates[0].id, "custom-2");
    assert_eq!(recreated.library.templates[1].id, "custom-3");
    assert_eq!(recreated.library.next_id, 4);
}

#[test]
fn rejects_corrupt_persisted_schema_version_identity_and_counter_without_repair() {
    for value in [
        json!({}),
        json!({"version":1,"nextId":1}),
        json!({"version":1,"nextId":1,"templates":[],"extra":true}),
        json!({"version":1,"nextId":2,"templates":[{"id":"custom-1","name":"Name","content":"a","extra":true}]}),
    ] {
        assert!(serde_json::from_value::<Library>(value).is_err());
    }
    let mut invalids = vec![
        Library {
            version: 2,
            ..Library::default()
        },
        Library {
            next_id: 0,
            ..Library::default()
        },
        Library {
            next_id: MAX_NEXT_ID + 1,
            ..Library::default()
        },
        Library {
            next_id: 1,
            templates: vec![template(1)],
            ..Library::default()
        },
        Library {
            next_id: 2,
            templates: vec![template(1), template(1)],
            ..Library::default()
        },
    ];
    for id in [
        "node",
        "custom-0",
        "custom-01",
        "custom-+1",
        "custom-1\n",
        "custom-9007199254740991",
    ] {
        let mut library = one_template();
        library.templates[0].id = id.into();
        invalids.push(library);
    }
    for library in invalids {
        let original = library.clone();
        assert!(validate(&library).is_err(), "{library:?}");
        assert_eq!(library, original);
        assert!(
            mutate(
                library.clone(),
                Operation::Save,
                None,
                Some("Extra"),
                Some("extra/")
            )
            .is_err()
        );
        assert!(mutate(library, Operation::Delete, Some("custom-1"), None, None).is_err());
    }
}

#[test]
fn rejects_name_header_injection_duplicate_names_and_empty_rules() {
    for name in [
        "",
        "   ",
        "name\nsecret/",
        "name\rsecret/",
        "name\t",
        "name\0",
        "name\u{2028}secret/",
        "name\u{2029}secret/",
    ] {
        let error = mutate(
            Library::default(),
            Operation::Save,
            None,
            Some(name),
            Some("a/"),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "gitignore.libraryInvalidName", "{name:?}");
    }
    let long_name = "中".repeat(MAX_NAME_CHARACTERS + 1);
    assert_eq!(
        mutate(
            Library::default(),
            Operation::Save,
            None,
            Some(&long_name),
            Some("a/")
        )
        .err()
        .unwrap()
        .code,
        "gitignore.libraryInvalidName"
    );
    let max_name = "中".repeat(MAX_NAME_CHARACTERS);
    assert_eq!(
        save(Library::default(), None, &max_name, "a/")
            .library
            .templates[0]
            .name,
        max_name
    );
    let first = save(Library::default(), None, "My Rules", "a/");
    let error = mutate(
        first.library,
        Operation::Save,
        None,
        Some(" my RULES "),
        Some("b/"),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "gitignore.libraryDuplicateName");
    for content in ["", "\u{feff}", "# only comments\n \n!  \n"] {
        let error = mutate(
            Library::default(),
            Operation::Save,
            None,
            Some("Rules"),
            Some(content),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "gitignore.libraryEmptyContent");
    }
    let error = mutate(
        Library::default(),
        Operation::Save,
        None,
        Some("Rules"),
        Some("a\0b"),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "gitignore.libraryInvalidContent");
}

#[test]
fn enforces_count_content_total_and_serialized_sqlite_limits() {
    let large = "x".repeat(MAX_CONTENT_BYTES + 1);
    assert_eq!(
        mutate(
            Library::default(),
            Operation::Save,
            None,
            Some("Rules"),
            Some(&large)
        )
        .err()
        .unwrap()
        .code,
        "gitignore.libraryContentTooLarge"
    );
    let mut full = Library {
        next_id: 65,
        templates: (1..=64).map(template).collect(),
        ..Library::default()
    };
    validate(&full).unwrap();
    assert_eq!(
        mutate(
            full.clone(),
            Operation::Save,
            None,
            Some("Extra"),
            Some("x/")
        )
        .err()
        .unwrap()
        .code,
        "gitignore.libraryTooMany"
    );
    full.templates[0].content = "\u{1}".repeat(MAX_CONTENT_BYTES);
    for item in full.templates.iter_mut().take(4).skip(1) {
        item.content = "\u{1}".repeat(MAX_CONTENT_BYTES);
    }
    full.templates.truncate(4);
    validate(&full).unwrap();
    assert!(serde_json::to_string(&full).unwrap().len() < 2 * 1024 * 1024);
    full.templates.push(template(5));
    assert_eq!(
        validate(&full).unwrap_err().code,
        "gitignore.libraryTooLarge"
    );
}

#[test]
fn unknown_deletions_updates_missing_fields_and_exhaustion_are_errors() {
    for operation in [Operation::Save, Operation::Delete] {
        let error = mutate(
            one_template(),
            operation,
            Some("custom-9"),
            Some("Other"),
            Some("a/"),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "gitignore.libraryUnknownTemplate");
        assert_eq!(error.params["id"], "custom-9");
    }
    assert_eq!(
        mutate(Library::default(), Operation::Save, None, None, Some("a/"))
            .err()
            .unwrap()
            .code,
        "gitignore.libraryInvalid"
    );
    assert_eq!(
        mutate(Library::default(), Operation::Delete, None, None, None)
            .err()
            .unwrap()
            .code,
        "gitignore.libraryInvalid"
    );
    let exhausted = Library {
        next_id: MAX_NEXT_ID,
        ..Library::default()
    };
    validate(&exhausted).unwrap();
    assert_eq!(
        mutate(exhausted, Operation::Save, None, Some("Other"), Some("a/"))
            .err()
            .unwrap()
            .code,
        "gitignore.libraryIdExhausted"
    );
}
