use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tf_plugin_api::LogLevel;
#[derive(Default)]
struct Context {
    cancelled: AtomicBool,
    cancel_on: Option<&'static str>,
}
impl TaskContext for Context {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn stage(&self, code: &str) {
        if self.cancel_on == Some(code) {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    fn progress(&self, _: u64, _: u64) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        Err(error("path"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(error("path"))
    }
}
fn base() -> Scenario {
    examples().remove(0)
}
fn run(s: &Scenario) -> CaseReport {
    engine::analyze(s, &Context::default()).unwrap()
}
fn response(control: &str) -> Scenario {
    let mut s = base();
    s.response_headers =
        format!("Date: Thu, 22 Oct 2026 10:00:00 GMT\nAge: 20\nCache-Control: {control}");
    s
}
fn has(r: &CaseReport, note: &str) -> bool {
    r.diagnostics.iter().any(|n| n.code == note)
}
#[test]
fn private_and_shared_lifetimes_follow_smaxage_precedence_and_age_formula() {
    let a = run(&base());
    let b = run(&examples()[1]);
    assert_eq!(
        (
            a.timing.apparent_age,
            a.timing.response_delay,
            a.timing.corrected_age_value,
            a.timing.corrected_initial_age,
            a.timing.resident_time,
            a.timing.current_age
        ),
        (Some(4), Some(2), Some(22), Some(22), Some(60), Some(82))
    );
    assert_eq!(
        (a.timing.lifetime, a.timing.remaining),
        (Some(60), Some(-22))
    );
    assert_eq!(
        (b.timing.lifetime, b.timing.remaining),
        (Some(600), Some(518))
    );
    assert_eq!(a.summary.reuse, "validate");
    assert_eq!(b.summary.reuse, "candidate");
}
#[test]
fn equal_lifetime_is_stale_and_apparent_age_can_dominate_age_header() {
    let r = run(&response("max-age=82"));
    assert_eq!(r.timing.freshness, "stale");
    assert_eq!(r.timing.remaining, Some(0));
    let mut s = response("max-age=1000");
    s.request_time = "2026-10-22T10:01:38Z".into();
    s.response_time = "2026-10-22T10:01:40Z".into();
    s.now = "2026-10-22T10:01:46Z".into();
    let r = run(&s);
    assert_eq!(r.timing.corrected_initial_age, Some(100));
    assert_eq!(r.timing.current_age, Some(106));
}
#[test]
fn no_cache_requires_validation_but_no_store_prohibits_storage() {
    let r = run(&response("max-age=1000, no-cache"));
    assert_eq!(r.summary.storage, "allowed");
    assert_eq!(r.timing.freshness, "fresh");
    assert_eq!(r.summary.reuse, "validate");
    let r = run(&response("public, max-age=1000, no-store"));
    assert_eq!(r.summary.storage, "blocked");
    assert_eq!(r.summary.reuse, "blocked");
    assert!(has(&r, "store_conflict"));
}
#[test]
fn authorization_shared_exceptions_are_exact_and_private_still_wins() {
    for cc in [
        "max-age=600",
        "max-age=600, public",
        "max-age=600, must-revalidate",
        "s-maxage=600",
        "max-age=600, proxy-revalidate",
        "public, private",
    ] {
        let mut s = response(cc);
        s.role = Role::Shared;
        s.request_headers = "Authorization: Bearer test-secret".into();
        let r = run(&s);
        let allowed = cc.contains("public") && !cc.contains("private")
            || cc.contains("must-revalidate")
            || cc.contains("s-maxage");
        assert_eq!(
            r.summary.storage,
            if allowed { "allowed" } else { "blocked" },
            "{cc}"
        );
    }
    let mut s = response("max-age=600");
    s.request_headers = "Authorization: Bearer abc".into();
    assert_eq!(run(&s).summary.storage, "allowed");
}
#[test]
fn new_authorized_request_does_not_gain_shared_access_to_an_unqualified_response() {
    let mut s = response("max-age=600");
    s.role = Role::Shared;
    s.new_request_headers = Some("Authorization: Bearer abc".into());
    let r = run(&s);
    assert_eq!(r.summary.storage, "allowed");
    assert_eq!(r.summary.reuse, "blocked");
    assert!(has(&r, "presented_authorization"));
}
#[test]
fn original_and_presented_no_store_have_different_effects_on_existing_entry() {
    let mut s = response("max-age=600");
    s.request_headers = "Cache-Control: no-store".into();
    assert_eq!(run(&s).summary.storage, "blocked");
    s.request_headers.clear();
    s.new_request_headers = Some("Cache-Control: no-store".into());
    let r = run(&s);
    assert_eq!(r.summary.storage, "allowed");
    assert_eq!(r.summary.reuse, "candidate");
    assert!(has(&r, "presented_no_store"));
}
#[test]
fn request_no_cache_max_age_min_fresh_and_only_if_cached_are_explained() {
    for (headers, reuse) in [
        ("Cache-Control: no-cache", "validate"),
        ("Cache-Control: max-age=81", "validate"),
        ("Cache-Control: max-age=82", "candidate"),
        ("Cache-Control: min-fresh=518", "candidate"),
        ("Cache-Control: min-fresh=519", "validate"),
    ] {
        let mut s = response("max-age=600");
        s.new_request_headers = Some(headers.into());
        assert_eq!(run(&s).summary.reuse, reuse, "{headers}");
    }
    let mut s = response("max-age=0");
    s.new_request_headers = Some("Cache-Control: only-if-cached".into());
    let r = run(&s);
    assert!(r.only_if_cached_unsatisfied);
    assert!(has(&r, "only_if_cached_unsatisfied"));
}
#[test]
fn vary_wildcard_missing_fields_and_case_sensitive_values_do_not_match() {
    let mut s = base();
    s.role = Role::Shared;
    s.new_request_headers = Some("accept-encoding: gzip".into());
    assert_eq!(run(&s).summary.reuse, "candidate");
    for headers in ["Accept-Encoding: br", "", "Accept-Encoding: GZIP"] {
        s.new_request_headers = Some(headers.into());
        assert_eq!(run(&s).summary.reuse, "variant");
    }
    s.response_headers.push_str("\nVary: *");
    assert_eq!(run(&s).summary.reuse, "variant");
}
#[test]
fn vary_absent_matches_absent_but_not_present_empty_and_multiple_vary_lines_combine() {
    let mut s = response("max-age=600");
    s.response_headers.push_str("\nVary: X-Mode\nVary: X-Other");
    s.request_headers.clear();
    s.new_request_headers = Some(String::new());
    let r = run(&s);
    assert_eq!(r.vary.len(), 2);
    assert_eq!(r.summary.reuse, "candidate");
    s.new_request_headers = Some("X-Mode:".into());
    assert_eq!(run(&s).summary.reuse, "variant");
}
#[test]
fn invalid_vary_and_malformed_headers_return_diagnostics_without_false_reuse() {
    let mut s = response("max-age=600");
    s.response_headers.push_str("\nVary: not a token");
    assert_eq!(run(&s).summary.reuse, "unknown");
    let mut s = response("max-age=600");
    s.request_headers = "Authorization Bearer test-secret\n folded".into();
    let r = run(&s);
    assert_eq!(r.summary.storage, "unknown");
    assert!(has(&r, "header_syntax"));
    assert!(!serde_json::to_string(&r).unwrap().contains("test-secret"));
}
#[test]
fn duplicate_and_invalid_directives_never_silently_take_the_last_value() {
    for cc in [
        "max-age=10, max-age=600",
        "max-age=-1",
        "max-age=1.5",
        "max-age",
        "max-age=1e3",
    ] {
        let r = run(&response(cc));
        assert_eq!(r.timing.lifetime, Some(0));
        assert_ne!(r.summary.reuse, "candidate");
        assert!(has(&r, "freshness_invalid"));
    }
    let r = run(&response("no-store=oops, max-age=600"));
    assert_eq!(r.summary.storage, "unknown");
}
#[test]
fn quoted_numbers_are_accepted_and_quoted_commas_stay_inside_one_directive() {
    let r = run(&response("max-age=\"600\""));
    assert_eq!(r.timing.lifetime, Some(600));
    assert_eq!(r.summary.reuse, "candidate");
    let r = run(&response("max-age=600, x-note=\"a,b\\\"c\""));
    assert_eq!(
        r.directives
            .iter()
            .filter(|d| d.context == "response")
            .count(),
        2
    );
    assert!(has(&r, "directive_boundary"));
    assert_eq!(r.summary.reuse, "unknown");
    assert!(has(
        &run(&response("max-age=600, broken=\"unterminated")),
        "control_syntax"
    ));
}
#[test]
fn qualified_controls_and_stale_extensions_are_explicit_boundaries() {
    for cc in [
        "max-age=600, no-cache=\"set-cookie\"",
        "max-age=600, stale-while-revalidate=30",
        "max-age=600, immutable",
    ] {
        assert_eq!(run(&response(cc)).summary.reuse, "unknown");
    }
    let mut s = response("max-age=600, private=\"set-cookie\"");
    s.role = Role::Shared;
    assert_eq!(run(&s).summary.storage, "unknown");
    s.response_headers.push_str(", no-store");
    assert_eq!(run(&s).summary.storage, "blocked");
}
#[test]
fn must_understand_overrides_response_no_store_only_with_understood_status() {
    let r = run(&response("max-age=600, must-understand, no-store"));
    assert_eq!(r.summary.storage, "allowed");
    assert!(has(&r, "must_understand_override"));
    let mut s = response("max-age=600, must-understand, no-store");
    s.request_headers = "Cache-Control: no-store".into();
    assert_eq!(run(&s).summary.storage, "blocked");
    s.status = "206".into();
    assert_eq!(run(&s).summary.storage, "unknown");
}
#[test]
fn must_revalidate_only_requires_validation_when_stale_and_smaxage_is_shared_only() {
    assert_eq!(
        run(&response("max-age=600, must-revalidate")).summary.reuse,
        "candidate"
    );
    let r = run(&response("max-age=60, must-revalidate"));
    assert_eq!(r.summary.reuse, "validate");
    assert!(
        r.conditions
            .iter()
            .any(|c| c.note.code == "stale_must_validate")
    );
    let r = run(&response("max-age=600, s-maxage=0"));
    assert_eq!(r.summary.reuse, "candidate");
    assert_eq!(r.timing.lifetime, Some(600));
}
#[test]
fn expires_precedence_and_invalid_expiration_are_not_heuristic_guesses() {
    let mut s = response("");
    s.response_headers
        .push_str("\nExpires: Thu, 22 Oct 2026 10:02:00 GMT");
    assert_eq!(run(&s).timing.lifetime, Some(120));
    s.response_headers.push_str("\nCache-Control: max-age=600");
    assert_eq!(run(&s).timing.lifetime, Some(600));
    let mut s = response("");
    s.response_headers.push_str("\nExpires: 0");
    assert_eq!(run(&s).timing.lifetime, Some(0));
    let r = run(&response("public"));
    assert_eq!(r.summary.storage, "allowed");
    assert_eq!(r.timing.lifetime, None);
    assert_eq!(r.summary.reuse, "unknown");
}
#[test]
fn missing_dates_and_times_keep_partial_formula_terms_without_exact_remaining() {
    let mut s = response("max-age=600");
    s.request_time.clear();
    let r = run(&s);
    assert_eq!(r.timing.response_delay, None);
    assert_eq!(r.timing.apparent_age, Some(4));
    assert_eq!(r.timing.remaining, None);
    assert_eq!(r.summary.reuse, "unknown");
    let mut s = response("max-age=600");
    s.response_headers = "Cache-Control: max-age=600".into();
    let r = run(&s);
    assert_eq!(r.timing.date, None);
    assert_eq!(r.timing.current_age, None);
    s.response_headers
        .push_str("\nExpires: Thu, 22 Oct 2026 10:02:00 GMT");
    s.response_headers = s
        .response_headers
        .replace("Cache-Control: max-age=600\n", "");
    assert_eq!(run(&s).timing.lifetime, Some(116));
}
#[test]
fn age_first_member_and_invalid_ignore_follow_documented_policies() {
    let mut s = response("max-age=600");
    s.response_headers = s
        .response_headers
        .replace("Age: 20", "Age: 20, 900\nAge: 800");
    let r = run(&s);
    assert_eq!(r.timing.age_value, 20);
    assert_eq!(r.timing.current_age, Some(82));
    assert!(has(&r, "age_first"));
    s.response_headers = s
        .response_headers
        .replace("Age: 20, 900\nAge: 800", "Age: -1");
    let r = run(&s);
    assert_eq!(r.timing.age_value, 0);
    assert!(has(&r, "age_invalid"));
    assert_eq!(r.timing.current_age, Some(64));
}
#[test]
fn invalid_time_order_future_date_duplicate_date_and_large_deltas_are_visible() {
    let mut s = base();
    s.now = "2026-10-22T09:00:00Z".into();
    let r = run(&s);
    assert_eq!(r.timing.current_age, None);
    assert!(has(&r, "time_order"));
    let mut s = base();
    s.response_headers = s.response_headers.replace("10:00:00 GMT", "10:02:00 GMT");
    let r = run(&s);
    assert_eq!(r.timing.apparent_age, Some(0));
    assert!(has(&r, "date_future"));
    s.response_headers
        .push_str("\nDate: Thu, 22 Oct 2026 10:00:00 GMT");
    assert_eq!(run(&s).timing.current_age, None);
    let r = run(&response("max-age=999999999999999999999999"));
    assert!(has(&r, "delta_clamped"));
    assert_ne!(r.summary.reuse, "candidate");
    let mut s = base();
    s.now = "2026-10-22T10:01:04.5Z".into();
    assert!(engine::analyze(&s, &Context::default()).is_err());
}
#[test]
fn validators_preserve_weak_etags_and_exclude_bad_or_duplicated_fields() {
    let mut s = base();
    s.response_headers = s.response_headers.replace("\"asset-v1\"", "W/\"asset-v1\"");
    let r = run(&s);
    assert!(
        r.conditional
            .starts_with("If-None-Match: W/\"asset-v1\"\nIf-Modified-Since:")
    );
    s.response_headers.push_str("\nETag: \"another\"");
    assert!(!run(&s).conditional.contains("If-None-Match"));
    let mut s = response("max-age=600");
    s.response_headers
        .push_str("\nETag: not-a-tag\nLast-Modified: nonsense");
    let r = run(&s);
    assert!(r.conditional.is_empty());
    assert!(has(&r, "validator_invalid"));
}
#[test]
fn scope_excludes_ranges_non_get_non_200_and_precondition_execution() {
    for (method, status) in [
        ("HEAD", "200"),
        ("POST", "200"),
        ("GET", "304"),
        ("GET", "206"),
    ] {
        let mut s = base();
        s.method = method.into();
        s.status = status.into();
        assert_eq!(run(&s).summary.reuse, "unknown");
    }
    let mut s = response("max-age=600");
    s.new_request_headers = Some("Range: bytes=0-10".into());
    assert_eq!(run(&s).summary.storage, "unknown");
    s.new_request_headers = Some("If-None-Match: \"v1\"".into());
    assert_eq!(run(&s).summary.reuse, "unknown");
}
#[test]
fn input_budgets_reject_instead_of_silently_cutting_fields_or_cases() {
    let mut s = base();
    s.request_headers = "x".repeat(HEADER_BYTES + 1);
    assert!(engine::analyze(&s, &Context::default()).is_err());
    s.request_headers = (0..129).map(|_| "invalid").collect::<Vec<_>>().join("\n");
    assert!(engine::analyze(&s, &Context::default()).is_err());
    let plugin = HttpCache::default();
    assert!(
        plugin
            .run_task(
                "inspect",
                json!({"cases":vec![base();3]}),
                &Context::default()
            )
            .is_err()
    );
}
#[test]
fn redaction_covers_all_contexts_and_vary_comparisons_without_changing_decisions() {
    let mut s = response("public, max-age=600");
    s.request_headers =
        "Authorization: Bearer test-secret\nCookie: session=test-cookie\nX-Api-Key: test-key"
            .into();
    s.new_request_headers = Some(s.request_headers.clone());
    s.response_headers
        .push_str("\nSet-Cookie: key=test-set-cookie\nVary: Authorization, Cookie, X-Api-Key");
    let r = run(&s);
    assert_eq!(r.summary.reuse, "candidate");
    let serialized = serde_json::to_string(&r).unwrap();
    for secret in ["test-secret", "test-cookie", "test-key", "test-set-cookie"] {
        assert!(!serialized.contains(secret));
    }
    assert!(serialized.contains("[redacted]"));
}
#[test]
fn atomic_cache_pages_export_and_cancellation_use_exact_selected_handles() {
    let plugin = HttpCache::default();
    let ctx = Context::default();
    let out = plugin
        .run_task("inspect", json!({"cases":examples()}), &ctx)
        .unwrap();
    let handle = out["handle"].as_str().unwrap();
    assert_eq!(out["summaries"][1]["reuse"], "candidate");
    let view = plugin
        .run_task(
            "query",
            json!({"handle":handle,"case":0,"section":"directives","limit":1,"offset":1}),
            &ctx,
        )
        .unwrap();
    assert_eq!(view["page"]["offset"], 1);
    assert_eq!(view["page"]["items"].as_array().unwrap().len(), 1);
    assert!(view["timing"].get("correctedInitialAge").is_some());
    let cancelled = Context {
        cancel_on: Some("age"),
        ..Context::default()
    };
    assert!(
        plugin
            .run_task("inspect", json!({"cases":examples()}), &cancelled)
            .is_err()
    );
    assert!(plugin.reports(handle).is_ok());
    let bad = Scenario {
        request_headers: "x".repeat(HEADER_BYTES + 1),
        ..base()
    };
    assert!(
        plugin
            .run_task("inspect", json!({"cases":[base(),bad]}), &ctx)
            .is_err()
    );
    assert!(plugin.reports(handle).is_ok());
    for format in ["json", "markdown"] {
        let out = plugin
            .run_task(
                "export",
                json!({"handle":handle,"format":format,"language":"en-US","path":null}),
                &ctx,
            )
            .unwrap();
        assert!(!out["content"].as_str().unwrap().is_empty());
    }
    plugin
        .run_task("inspect", json!({"cases":[base()]}), &ctx)
        .unwrap();
    assert!(plugin.reports(handle).is_err());
}

#[test]
fn delta_saturation_is_scoped_to_the_field_that_lost_precision() {
    let s = response("max-age=600, s-maxage=999999999999999999999");
    assert_eq!(run(&s).summary.reuse, "candidate");
    let mut s = response("s-maxage=600, max-age=999999999999999999999");
    s.role = Role::Shared;
    assert_eq!(run(&s).summary.reuse, "candidate");

    let mut s = response("max-age=600");
    s.new_request_headers = Some("Cache-Control: max-age=999999999999999999999".into());
    let r = run(&s);
    assert_eq!(r.timing.lifetime, Some(600));
    assert_eq!(r.summary.reuse, "unknown");
    let mut s = response("max-age=600");
    s.response_headers = s
        .response_headers
        .replace("Age: 20", "Age: 999999999999999999999");
    let r = run(&s);
    assert_eq!(r.timing.current_age, None);
    assert_eq!(r.summary.freshness, "unknown");
    assert_eq!(r.timing.remaining, None);
}
#[test]
fn actual_exports_are_masked_localized_and_escape_markdown_metadata() {
    let plugin = HttpCache::default();
    let ctx = Context::default();
    let mut s = response("public, max-age=600");
    s.name = "<script> | test".into();
    s.request_headers = "Authorization: Bearer export-secret\nCookie: x=export-cookie".into();
    s.response_headers
        .push_str("\nSet-Cookie: x=export-cookie\nVary: Authorization");
    let result = plugin
        .run_task("inspect", json!({"cases":[s]}), &ctx)
        .unwrap();
    let dir = std::env::temp_dir().join(format!(
        "toolforge-http-cache-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (format, ext) in [("json", "json"), ("markdown", "md")] {
        let path = dir.join(format!("report.{ext}"));
        plugin
            .run_task(
                "export",
                json!({"handle":result["handle"],"format":format,"language":"zh-CN","path":path}),
                &ctx,
            )
            .unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.contains("export-secret"));
        assert!(!content.contains("export-cookie"));
        assert!(content.contains("[redacted]"));
        assert!(content.contains("允许存储"));
        if format == "markdown" {
            assert!(content.contains("&lt;script&gt;"));
            assert!(!content.contains("Some("));
            assert!(content.contains("Vary"));
        } else {
            let value: Value = serde_json::from_str(&content).unwrap();
            assert_eq!(value["redacted"], true);
            assert_eq!(value["cases"][0]["timing"]["currentAge"], 82);
        }
    }
    let path = dir.join("cancel.json");
    let ctx = Context {
        cancel_on: Some("export"),
        ..Context::default()
    };
    assert!(
        plugin
            .run_task(
                "export",
                json!({"handle":result["handle"],"format":"json","language":"en-US","path":path}),
                &ctx
            )
            .is_err()
    );
    assert!(!path.exists());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn large_copy_and_directive_budgets_are_explicit_and_pagination_clamps_safely() {
    let mut s = response("max-age=600");
    s.request_headers = format!("X-Filler: {}", "a".repeat(32000));
    let plugin = HttpCache::default();
    let ctx = Context::default();
    let result = plugin
        .run_task("inspect", json!({"cases":[s]}), &ctx)
        .unwrap();
    let err = plugin
        .run_task(
            "export",
            json!({"handle":result["handle"],"format":"json","language":"en-US","path":null}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(err.code, "http-cache.copy_limit");
    let view=plugin.run_task("query",json!({"handle":result["handle"],"case":0,"section":"conditions","offset":usize::MAX,"limit":2}),&ctx).unwrap();
    assert!(view["page"]["offset"].as_u64().unwrap() < view["page"]["total"].as_u64().unwrap());
    let s = response(&vec!["max-age=1"; 257].join(","));
    assert!(engine::analyze(&s, &ctx).is_err());
    let mut s = response("max-age=600");
    s.now.clear();
    s.new_request_headers = Some("Cache-Control: only-if-cached".into());
    let r = run(&s);
    assert_eq!(r.summary.reuse, "unknown");
    assert!(!r.only_if_cached_unsatisfied);
}

#[test]
fn http_dates_are_case_insensitive_but_two_digit_years_are_not_guessed() {
    assert_eq!(
        timing::http_date("thu, 22 oct 2026 10:00:00 gmt"),
        timing::http_date("Thu, 22 Oct 2026 10:00:00 GMT")
    );
    assert_eq!(
        timing::http_date("Thu Oct 22 10:00:00 2026"),
        timing::http_date("Thu, 22 Oct 2026 10:00:00 GMT")
    );
    let mut s = response("max-age=600");
    s.response_headers = s.response_headers.replace(
        "Thu, 22 Oct 2026 10:00:00 GMT",
        "Sunday, 06-Nov-94 08:49:37 GMT",
    );
    let r = run(&s);
    assert!(has(&r, "legacy_date"));
    assert_eq!(r.timing.current_age, None);
    assert_eq!(r.summary.reuse, "unknown");
    let mut s = response("");
    s.response_headers
        .push_str("\nExpires: Sunday, 06-Nov-94 08:49:37 GMT");
    assert_eq!(run(&s).timing.lifetime, None);
}
