use super::*;
use crate::fetch::HeaderArg;
use tf_plugin_api::PluginError;

const DOC: &str = "https://app.example.com/dashboard";
const TARGET: &str = "https://api.example.com/v1/data";

fn header(name: &str, value: &str) -> HeaderArg {
    HeaderArg {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn response(status: i64, list: Vec<HeaderArg>) -> Option<ResponseArg> {
    Some(ResponseArg {
        status: Some(status),
        headers: list,
    })
}

fn run_call(
    method: &str,
    credentials: &str,
    request: Vec<HeaderArg>,
    preflight: Option<ResponseArg>,
    actual: Option<ResponseArg>,
) -> Result<Report, PluginError> {
    run(
        DOC,
        TARGET,
        method,
        credentials,
        &request,
        preflight.as_ref(),
        actual.as_ref(),
    )
}

fn condition<'a>(stage: &'a Stage, id: &str) -> &'a Condition {
    stage
        .conditions
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("condition {id} missing"))
}

#[test]
fn same_origin_requests_skip_cors() {
    let report = run(
        DOC,
        "https://app.example.com/api",
        "GET",
        "same-origin",
        &[],
        None,
        None,
    )
    .unwrap();
    assert!(report.request.same_origin);
    assert!(!report.request.preflight_needed);
    assert_eq!(report.overall, ALLOWED);
    assert_eq!(report.outcome, "same_origin");
    assert_eq!(report.reading.status, SKIPPED);
}

#[test]
fn simple_get_with_wildcard_is_allowed_without_preflight() {
    let actual = response(
        200,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Cache-Control", "no-store"),
            header("X-Internal", "secret"),
        ],
    );
    let report = run_call("GET", "same-origin", vec![], None, actual).unwrap();
    assert!(!report.request.preflight_needed);
    assert_eq!(report.preflight.status, SKIPPED);
    assert_eq!(report.outcome, "no_preflight");
    assert_eq!(report.overall, ALLOWED);
    // safelisted 响应头可读，未暴露的自定义头不可读
    assert_eq!(report.reading.readable, vec!["cache-control"]);
}

#[test]
fn json_post_requires_preflight_and_lists_request_headers() {
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("Content-Type", "application/json")],
        None,
        None,
    )
    .unwrap();
    assert!(report.request.preflight_needed);
    assert_eq!(report.request.unsafe_header_names, vec!["content-type"]);
    let draft = &report.request.options;
    assert_eq!(draft.headers[1].name, "Access-Control-Request-Method");
    assert_eq!(draft.headers[2].name, "Access-Control-Request-Headers");
    assert_eq!(draft.headers[2].value, "content-type");
}

#[test]
fn text_plain_post_stays_simple() {
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("Content-Type", "text/plain")],
        None,
        None,
    )
    .unwrap();
    assert!(!report.request.preflight_needed);
}

#[test]
fn missing_preflight_response_is_undetermined() {
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("X-Token", "abc")],
        None,
        None,
    )
    .unwrap();
    assert_eq!(report.preflight.status, UNDETERMINED);
    assert!(
        report
            .preflight
            .reasons
            .iter()
            .any(|r| r.code == "cors.preflight_missing")
    );
    assert_eq!(report.outcome, "undetermined");
    assert_eq!(report.overall, UNDETERMINED);
}

#[test]
fn preflight_failure_means_no_actual_request() {
    let preflight = response(400, vec![header("Access-Control-Allow-Origin", "*")]);
    let actual = response(200, vec![header("Access-Control-Allow-Origin", "*")]);
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("X-Token", "abc")],
        preflight,
        actual,
    )
    .unwrap();
    assert_eq!(report.preflight.status, BLOCKED);
    assert_eq!(condition(&report.preflight, "status").status, FAIL);
    assert_eq!(
        condition(&report.preflight, "status")
            .reason
            .as_ref()
            .unwrap()
            .code,
        "cors.preflight_status"
    );
    assert_eq!(report.reading.status, SKIPPED);
    assert!(
        report
            .reading
            .reasons
            .iter()
            .any(|r| r.code == "cors.preflight_failed_no_request")
    );
    assert_eq!(report.outcome, "preflight_failed");
    assert_eq!(report.overall, BLOCKED);
}

#[test]
fn wildcard_with_credentials_fails_preflight() {
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Methods", "POST"),
            header("Access-Control-Allow-Headers", "X-Token"),
        ],
    );
    let report = run_call(
        "POST",
        "include",
        vec![header("X-Token", "abc")],
        preflight,
        None,
    )
    .unwrap();
    assert_eq!(report.preflight.status, BLOCKED);
    assert_eq!(
        condition(&report.preflight, "acao")
            .reason
            .as_ref()
            .unwrap()
            .code,
        "cors.acao_wildcard_credentials"
    );
    assert_eq!(report.outcome, "preflight_failed");
}

#[test]
fn credentials_require_true_allow_credentials() {
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "https://app.example.com"),
            header("Access-Control-Allow-Methods", "POST"),
            header("Access-Control-Allow-Headers", "X-Token"),
        ],
    );
    let report = run_call(
        "POST",
        "include",
        vec![header("X-Token", "abc")],
        preflight,
        None,
    )
    .unwrap();
    assert_eq!(
        condition(&report.preflight, "acac")
            .reason
            .as_ref()
            .unwrap()
            .code,
        "cors.acac_missing"
    );
    assert_eq!(report.outcome, "preflight_failed");
}

#[test]
fn authorization_is_never_covered_by_wildcard() {
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Methods", "POST"),
            header("Access-Control-Allow-Headers", "*"),
        ],
    );
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("Authorization", "Bearer x")],
        preflight,
        None,
    )
    .unwrap();
    assert_eq!(report.preflight.status, BLOCKED);
    assert_eq!(
        condition(&report.preflight, "acah")
            .reason
            .as_ref()
            .unwrap()
            .code,
        "cors.acah_authorization_wildcard"
    );
}

#[test]
fn authorization_must_be_listed_explicitly() {
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Methods", "POST"),
            header("Access-Control-Allow-Headers", "Authorization"),
        ],
    );
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("Authorization", "Bearer x")],
        preflight,
        None,
    )
    .unwrap();
    assert_eq!(report.preflight.status, ALLOWED);
    // POST 是 safelisted 方法：预检条件只有 status / acao / acah，ACAM 不参与
    assert_eq!(
        report
            .preflight
            .conditions
            .iter()
            .filter(|c| c.status == PASS)
            .count(),
        3
    );
}

#[test]
fn header_name_case_is_normalized_for_acah() {
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Methods", "POST"),
            header("Access-Control-Allow-Headers", "X-TOKEN"),
        ],
    );
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("x-token", "abc")],
        preflight,
        None,
    )
    .unwrap();
    assert_eq!(report.preflight.status, ALLOWED);
}

#[test]
fn acam_wildcard_only_without_credentials() {
    let base = vec![header("Access-Control-Allow-Origin", "*")];
    let preflight = ResponseArg {
        status: Some(204),
        headers: [
            base.clone(),
            vec![header("Access-Control-Allow-Methods", "*")],
        ]
        .concat(),
    };
    let report = run_call("DELETE", "same-origin", vec![], Some(preflight), None).unwrap();
    assert_eq!(condition(&report.preflight, "acam").status, PASS);

    let preflight = ResponseArg {
        status: Some(204),
        headers: [
            vec![header(
                "Access-Control-Allow-Origin",
                "https://app.example.com",
            )],
            vec![header("Access-Control-Allow-Credentials", "true")],
            vec![header("Access-Control-Allow-Methods", "*")],
        ]
        .concat(),
    };
    let report = run_call("DELETE", "include", vec![], Some(preflight), None).unwrap();
    assert_eq!(
        condition(&report.preflight, "acam")
            .reason
            .as_ref()
            .unwrap()
            .code,
        "cors.acam_mismatch"
    );
}

#[test]
fn duplicate_acao_never_counts_as_allowed() {
    // 预检里重复 ACAO
    let preflight = ResponseArg {
        status: Some(204),
        headers: vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Origin", "https://app.example.com"),
            header("Access-Control-Allow-Methods", "POST"),
            header("Access-Control-Allow-Headers", "X-Token"),
        ],
    };
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("X-Token", "abc")],
        Some(preflight),
        None,
    )
    .unwrap();
    assert_eq!(
        condition(&report.preflight, "acao")
            .reason
            .as_ref()
            .unwrap()
            .code,
        "cors.duplicate_acao"
    );
}

#[test]
fn joined_origins_in_one_value_are_invalid() {
    let actual = response(
        200,
        vec![header(
            "Access-Control-Allow-Origin",
            "https://a.example.com, https://app.example.com",
        )],
    );
    let report = run_call("GET", "same-origin", vec![], None, actual).unwrap();
    assert_eq!(report.reading.status, BLOCKED);
    assert_eq!(
        report
            .reading
            .conditions
            .iter()
            .find(|c| c.id == "acao")
            .unwrap()
            .reason
            .as_ref()
            .unwrap()
            .code,
        "cors.acao_invalid"
    );
    assert_eq!(report.outcome, "sent_but_unreadable");
    assert_eq!(report.overall, BLOCKED);
}

#[test]
fn acao_mismatch_blocks_reading_but_request_was_sent() {
    let actual = response(
        200,
        vec![header(
            "Access-Control-Allow-Origin",
            "https://other.example.org",
        )],
    );
    let report = run_call("GET", "same-origin", vec![], None, actual).unwrap();
    assert_eq!(report.reading.status, BLOCKED);
    assert_eq!(
        report
            .reading
            .conditions
            .iter()
            .find(|c| c.id == "acao")
            .unwrap()
            .reason
            .as_ref()
            .unwrap()
            .code,
        "cors.acao_mismatch"
    );
    assert_eq!(report.outcome, "sent_but_unreadable");
}

#[test]
fn missing_actual_response_is_undetermined() {
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Methods", "POST"),
            header("Access-Control-Allow-Headers", "X-Token"),
        ],
    );
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("X-Token", "abc")],
        preflight,
        None,
    )
    .unwrap();
    assert_eq!(report.preflight.status, ALLOWED);
    assert_eq!(report.reading.status, UNDETERMINED);
    assert!(
        report
            .reading
            .reasons
            .iter()
            .any(|r| r.code == "cors.actual_missing")
    );
    assert_eq!(report.overall, UNDETERMINED);
    assert_eq!(report.outcome, "undetermined");
}

#[test]
fn expose_headers_decide_readability() {
    let actual = response(
        200,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Expose-Headers", "X-Debug, Set-Cookie"),
            header("X-Debug", "1"),
            header("Set-Cookie", "sid=1"),
        ],
    );
    let report = run_call("GET", "same-origin", vec![], None, actual).unwrap();
    assert_eq!(report.overall, ALLOWED);
    assert_eq!(report.reading.readable, vec!["x-debug"]);
    assert!(
        report
            .reading
            .suppressed
            .iter()
            .any(|d| d.code == "cors.forbidden_response_name")
    );
}

#[test]
fn expose_wildcard_needs_no_credentials() {
    let wildcard = response(
        200,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Expose-Headers", "*"),
            header("X-Debug", "1"),
        ],
    );
    let report = run_call("GET", "same-origin", vec![], None, wildcard).unwrap();
    assert!(report.reading.readable.contains(&"x-debug".to_owned()));

    let with_credentials = response(
        200,
        vec![
            header("Access-Control-Allow-Origin", "https://app.example.com"),
            header("Access-Control-Allow-Credentials", "true"),
            header("Access-Control-Expose-Headers", "*"),
            header("X-Debug", "1"),
        ],
    );
    let report = run_call("GET", "include", vec![], None, with_credentials).unwrap();
    assert_eq!(report.overall, ALLOWED);
    // 凭据模式下 `*` 不是可读头通配符
    assert!(!report.reading.readable.contains(&"x-debug".to_owned()));
    assert!(report.reading.suppressed.is_empty());
}

#[test]
fn null_origin_matches_literal_null_acao() {
    let actual = response(200, vec![header("Access-Control-Allow-Origin", "null")]);
    let report = run(
        "null",
        TARGET,
        "GET",
        "same-origin",
        &[],
        None,
        actual.as_ref(),
    )
    .unwrap();
    assert_eq!(report.overall, ALLOWED);
    assert_eq!(report.request.origin, "null");
}

#[test]
fn forbidden_request_headers_stop_everything() {
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("Cookie", "sid=1"), header("X-Token", "abc")],
        None,
        None,
    )
    .unwrap();
    assert_eq!(report.outcome, "forbidden_header");
    assert_eq!(report.overall, UNDETERMINED);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "cors.forbidden_header" && d.params["name"] == "cookie")
    );
    assert_eq!(report.preflight.status, SKIPPED);
}

#[test]
fn non_http_targets_are_out_of_scope() {
    let report = run(
        DOC,
        "chrome-extension://abc/x",
        "GET",
        "same-origin",
        &[],
        None,
        None,
    )
    .unwrap();
    assert_eq!(report.outcome, "unsupported_scheme");
    assert_eq!(report.overall, UNDETERMINED);
}

#[test]
fn lowercase_method_is_not_safelisted() {
    let report = run_call("get", "same-origin", vec![], None, None).unwrap();
    assert!(report.request.preflight_needed);
    assert_eq!(report.request.unsafe_header_names, Vec::<String>::new());
    // 方法不在 safelist 内：预检的 ACAM 必须精确列出它（大小写敏感）
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Methods", "get"),
        ],
    );
    let report = run_call("get", "same-origin", vec![], preflight, None).unwrap();
    assert_eq!(report.preflight.status, ALLOWED);
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Methods", "GET"),
        ],
    );
    let report = run_call("get", "same-origin", vec![], preflight, None).unwrap();
    assert_eq!(report.preflight.status, BLOCKED);
}

#[test]
fn invalid_inputs_return_errors() {
    let err = run_call("POST", "bogus", vec![], None, None).unwrap_err();
    assert_eq!(err.code, "cors.invalid_credentials");
    let err = run_call("", "same-origin", vec![], None, None).unwrap_err();
    assert_eq!(err.code, "cors.invalid_method");
    let err = run("not a url", TARGET, "GET", "same-origin", &[], None, None).unwrap_err();
    assert_eq!(err.code, "cors.invalid_document");
    let err = run(DOC, "not a url", "GET", "same-origin", &[], None, None).unwrap_err();
    assert_eq!(err.code, "cors.invalid_target");
    let err = run_call(
        "GET",
        "same-origin",
        vec![header("bad name", "x")],
        None,
        None,
    )
    .unwrap_err();
    assert_eq!(err.code, "cors.invalid_header_name");
    let err = run_call(
        "GET",
        "same-origin",
        vec![],
        Some(ResponseArg {
            status: Some(42),
            headers: vec![],
        }),
        None,
    )
    .unwrap_err();
    assert_eq!(err.code, "cors.invalid_status");
}

#[test]
fn markdown_report_contains_all_stages() {
    let preflight = response(
        204,
        vec![
            header("Access-Control-Allow-Origin", "*"),
            header("Access-Control-Allow-Methods", "POST"),
            header("Access-Control-Allow-Headers", "X-Token"),
        ],
    );
    let actual = response(200, vec![header("Access-Control-Allow-Origin", "*")]);
    let report = run_call(
        "POST",
        "same-origin",
        vec![header("X-Token", "abc")],
        preflight,
        actual,
    )
    .unwrap();
    let text = markdown(&report);
    assert!(text.contains("# CORS Preflight Inspection"));
    assert!(text.contains("## OPTIONS draft"));
    assert!(text.contains("Access-Control-Request-Method: POST"));
    assert!(text.contains("## Preflight checks"));
    assert!(text.contains("## Response reading"));
    assert!(text.contains("- acao: pass (`*`)"));
}
