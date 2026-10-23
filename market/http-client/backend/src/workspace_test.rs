use super::*;
use crate::oauth::OAuthConfig;
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

fn with_oauth() -> Workspace {
    let mut state = workspace();
    state.oauth_configs = vec![OAuthConfig {
        id: "auth".into(),
        name: "Login".into(),
        authorization_url: "{{base}}/authorize".into(),
        token_url: "{{base}}/token".into(),
        client_id: "cli".into(),
        scope: "read".into(),
        redirect_port: Some(8765),
        redirect_path: "/callback".into(),
    }];
    state.collections[0].auth = Some("auth".into());
    state.requests[0].auth = Some(AUTH_NONE.into());
    state
}

#[test]
fn stores_oauth_configs_and_bindings_without_tokens() {
    // 旧版本保存的工作区没有授权字段
    let stored = validate(workspace()).unwrap();
    assert!(stored.oauth_configs.is_empty());
    assert!(stored.requests[0].auth.is_none());

    let stored = validate(with_oauth()).unwrap();
    let saved = serde_json::to_value(&stored).unwrap();
    assert_eq!(saved["oauthConfigs"][0]["redirectPort"], 8765);
    assert_eq!(saved["collections"][0]["auth"], "auth");
    assert_eq!(saved["requests"][0]["auth"], "none");
    // 配置只有非敏感字段
    let keys: Vec<&String> = saved["oauthConfigs"][0]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert!(!keys.iter().any(|k| k.contains("token") && *k != "tokenUrl"));
    assert!(
        !keys
            .iter()
            .any(|k| k.contains("secret") || k.contains("verifier"))
    );
    // 不认识的字段（例如令牌）不会被接受
    let mut raw = serde_json::to_value(with_oauth()).unwrap();
    raw["oauthConfigs"][0]["accessToken"] = json!("leak");
    assert!(serde_json::from_value::<Workspace>(raw).is_err());
}

#[test]
fn rejects_dangling_oauth_bindings() {
    let mut state = with_oauth();
    state.requests[0].auth = Some("missing".into());
    assert_eq!(validate(state).unwrap_err().params["field"], "request.auth");
    let mut state = with_oauth();
    state.collections[0].auth = Some(AUTH_NONE.into());
    assert_eq!(
        validate(state).unwrap_err().params["field"],
        "collection.auth"
    );
    let mut state = with_oauth();
    state.oauth_configs[0].redirect_path = "/a b".into();
    assert_eq!(validate(state).unwrap_err().code, "oauth.invalid_path");
    let mut state = with_oauth();
    state.oauth_configs[0].id = "collection".into();
    state.collections[0].auth = Some("collection".into());
    assert_eq!(
        validate(state).unwrap_err().code,
        "http.workspace_duplicate_id"
    );
}

#[test]
fn flat_headers_and_data_extend_the_request() {
    let args: RequestArgs = serde_json::from_value(json!({
        "method": "POST",
        "url": "http://localhost/items",
        "header": ["X-Trace: 1", "Accept:application/json"],
        "data": "{\"name\":\"a\"}"
    }))
    .unwrap();
    let request = args.resolve().unwrap();
    let headers: Vec<_> = request
        .headers
        .iter()
        .map(|p| (p.key.as_str(), p.value.as_str()))
        .collect();
    assert_eq!(headers, [("X-Trace", "1"), ("Accept", "application/json")]);
    assert!(matches!(request.body.kind, BodyKind::Json));
    assert_eq!(request.body.text, "{\"name\":\"a\"}");

    let text: RequestArgs = serde_json::from_value(
        json!({ "method": "POST", "url": "http://localhost", "data": "a=1" }),
    )
    .unwrap();
    assert!(matches!(text.resolve().unwrap().body.kind, BodyKind::Text));

    let invalid: RequestArgs = serde_json::from_value(
        json!({ "method": "GET", "url": "http://localhost", "header": ["broken"] }),
    )
    .unwrap();
    assert_eq!(invalid.resolve().unwrap_err().code, "http.invalid_header");
}
