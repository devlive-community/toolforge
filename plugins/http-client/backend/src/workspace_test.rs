use super::*;
use serde_json::json;

fn request() -> Request {
    serde_json::from_value(json!({ "method": "GET", "url": "{{base}}/users" })).unwrap()
}

fn variable(key: &str, value: &str) -> Variable {
    Variable {
        key: key.into(),
        value: value.into(),
        enabled: true,
        secret: false,
    }
}

fn workspace() -> Workspace {
    serde_json::from_value(json!({
        "version": 1,
        "collections": [{ "id": "collection", "name": "API" }],
        "requests": [{
            "id": "request", "name": "Users", "collectionId": "collection",
            "request": { "method": "GET", "url": "{{base}}/users" }
        }],
        "environments": [{
            "id": "environment", "name": "Local",
            "variables": [
                { "key": "base", "value": "http://localhost:3000" },
                { "key": "token", "value": "in-memory-only", "secret": true }
            ]
        }],
        "activeEnvironmentId": "environment"
    }))
    .unwrap()
}

#[test]
fn validates_and_clears_secrets_without_changing_template_or_original() {
    let original = workspace();
    let stored = validate(original.clone()).unwrap();
    assert_eq!(stored.environments[0].variables[1].value, "");
    assert_eq!(
        original.environments[0].variables[1].value,
        "in-memory-only"
    );
    assert_eq!(stored.requests[0].request.url, "{{base}}/users");
    assert!(stored.environments[0].variables[0].enabled);
    assert!(!stored.environments[0].variables[0].secret);
}

#[test]
fn rejects_invalid_version_duplicate_ids_and_missing_references() {
    let mut state = workspace();
    state.version = 2;
    assert_eq!(validate(state).unwrap_err().code, "http.workspace_version");

    let mut state = workspace();
    state.requests[0].id = "collection".into();
    assert_eq!(
        validate(state).unwrap_err().code,
        "http.workspace_duplicate_id"
    );

    let mut state = workspace();
    state.requests[0].collection_id = Some("unknown".into());
    assert_eq!(
        validate(state).unwrap_err().code,
        "http.workspace_reference"
    );

    let mut state = workspace();
    state.active_environment_id = Some("unknown".into());
    assert_eq!(
        validate(state).unwrap_err().code,
        "http.workspace_reference"
    );

    let mut state = workspace();
    state.collections[0].name.clear();
    assert_eq!(
        validate(state).unwrap_err().code,
        "http.workspace_empty_field"
    );
}

#[test]
fn resolves_active_fields_once_without_interpreting_inserted_values() {
    let mut template = request();
    template.headers = vec![Pair {
        key: "X-{{header}}".into(),
        value: "{{token}}".into(),
        enabled: true,
    }];
    template.params = vec![
        Pair {
            key: "{{key}}".into(),
            value: "{{token}}".into(),
            enabled: true,
        },
        Pair {
            key: "skip".into(),
            value: "{{missing}}".into(),
            enabled: false,
        },
    ];
    template.body = crate::http::Body {
        kind: BodyKind::Json,
        text: r#"{"token":"{{token}}","nested":{}}"#.into(),
        form: Vec::new(),
    };
    let vars = vec![
        variable("base", "https://example.com"),
        variable("header", "Token"),
        variable("key", "access-token"),
        variable("token", "{{second}}$(printenv)"),
        variable("second", "never-expanded"),
    ];
    let resolved = resolve(template.clone(), &vars).unwrap();
    assert_eq!(resolved.url, "https://example.com/users");
    assert_eq!(resolved.headers[0].key, "X-Token");
    assert_eq!(resolved.params[0].key, "access-token");
    assert_eq!(resolved.params[0].value, "{{second}}$(printenv)");
    assert_eq!(resolved.params[1].value, "{{missing}}");
    assert_eq!(
        resolved.body.text,
        r#"{"token":"{{second}}$(printenv)","nested":{}}"#
    );
    assert_eq!(template.url, "{{base}}/users");

    template.body.kind = BodyKind::Form;
    template.body.form = vec![Pair {
        key: "{{ key }}".into(),
        value: "{{token}}".into(),
        enabled: true,
    }];
    let resolved = resolve(template, &vars).unwrap();
    assert_eq!(resolved.body.form[0].key, "access-token");
    assert_eq!(resolved.body.form[0].value, "{{second}}$(printenv)");
}

#[test]
fn rejects_missing_disabled_duplicate_and_malformed_variables() {
    assert_eq!(
        resolve(request(), &[]).unwrap_err().code,
        "http.variable_missing"
    );
    let mut disabled = variable("base", "https://example.com");
    disabled.enabled = false;
    assert_eq!(
        resolve(request(), &[disabled]).unwrap_err().code,
        "http.variable_missing"
    );
    assert_eq!(
        resolve(request(), &[variable("base", "x"), variable("base", "y")])
            .unwrap_err()
            .code,
        "http.variable_duplicate"
    );
    assert_eq!(
        resolve(request(), &[variable("1+1", "x")])
            .unwrap_err()
            .code,
        "http.variable_key"
    );
    for url in ["{{base", "{{}}", "{{ process.env.HOME() }}", "{{{base}}}"] {
        let mut template = request();
        template.url = url.into();
        assert_eq!(
            resolve(template, &[]).unwrap_err().code,
            "http.variable_syntax"
        );
    }
}

#[test]
fn bounds_workspaces_requests_and_substitution_expansion() {
    let mut state = workspace();
    state.requests = vec![state.requests[0].clone(); 201];
    assert_eq!(validate(state).unwrap_err().code, "http.limit_exceeded");

    let mut state = workspace();
    state.requests[0].request.body.text = "x".repeat(MAX_TEXT + 1);
    assert_eq!(validate(state).unwrap_err().code, "http.limit_exceeded");

    let mut state = workspace();
    for (index, key) in ["one", "two", "three"].iter().enumerate() {
        state.environments[0]
            .variables
            .push(variable(key, &"x".repeat(MAX_TEXT - index)));
    }
    assert_eq!(validate(state).unwrap_err().code, "http.limit_exceeded");

    let mut template = request();
    template.url = "https://example.com/{{base}}{{base}}".into();
    assert_eq!(
        resolve(template, &[variable("base", &"x".repeat(MAX_URL / 2))])
            .unwrap_err()
            .code,
        "http.limit_exceeded"
    );
}

#[test]
fn ids_are_distinct_across_repeated_calls() {
    let ids: HashSet<_> = (0..1000).map(|_| new_id()).collect();
    assert_eq!(ids.len(), 1000);
}
