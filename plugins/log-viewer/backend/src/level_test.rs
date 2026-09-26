use super::*;

#[test]
fn detects_common_formats() {
    let cases: &[(&str, u8)] = &[
        ("2026-09-26 10:00:01.123 ERROR [main] c.e.App - boom", ERROR),
        ("2026-09-26T10:00:01Z WARN  request slow", WARN),
        ("I0926 10:00:01 INFO server started", INFO),
        ("[2026-09-26 10:00:01] local.DEBUG: cache hit", DEBUG),
        (
            "time=2026-09-26 level=warning msg=\"disk almost full\"",
            WARN,
        ),
        ("{\"level\":\"error\",\"msg\":\"failed\"}", ERROR),
        ("{\"level\":30,\"time\":1,\"msg\":\"hi\"}", INFO),
        ("{\"severity\": \"Debug\", \"message\": \"x\"}", DEBUG),
        ("ERROR:root:Something went wrong", ERROR),
        (
            "[Wed Sep 26 10:00:01 2026] [error] [client 1.2.3.4] File does not exist",
            ERROR,
        ),
        ("<Warning> low memory", WARN),
        ("2026/09/26 10:00:01 [crit] 123#0: oops", ERROR),
        ("Sep 26 10:00:01 host kernel: FATAL: out of memory", ERROR),
        ("\u{1b}[31mERROR\u{1b}[0m colored", ERROR),
        (
            "\u{1b}[2m10:00\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m colored",
            INFO,
        ),
        ("TRACE entering fn", TRACE),
    ];
    for (line, level) in cases {
        assert_eq!(detect(line.as_bytes()), *level, "{line}");
    }
}

#[test]
fn ignores_level_words_in_messages() {
    for line in [
        "2026-09-26 10:00:01 user reported no error today",
        "GET /api/errors 200 12ms",
        "information retrieval done",
        "",
        "Warning signs are lowercase words here: warn",
    ] {
        assert_eq!(detect(line.as_bytes()), NONE, "{line}");
    }
}

#[test]
fn inherits_levels_for_stack_traces() {
    assert_eq!(
        classify(b"\tat com.example.App.main(App.java:10)", ERROR),
        ERROR
    );
    assert_eq!(classify(b"Caused by: java.io.IOException", ERROR), ERROR);
    assert_eq!(classify(b"    raise ValueError()", WARN), WARN);
    assert_eq!(
        classify(
            b"java.sql.SQLTransientConnectionException: timed out",
            ERROR
        ),
        ERROR
    );
    assert_eq!(
        classify(b"Traceback (most recent call last):", ERROR),
        ERROR
    );
    assert_eq!(classify(b"2026-09-26 10:00:00 plain record", ERROR), NONE);
    assert_eq!(classify(b"[main] plain record", ERROR), NONE);
    assert_eq!(classify(b"{\"msg\":\"json record\"}", ERROR), NONE);
    assert_eq!(classify(b"", ERROR), NONE);
    assert_eq!(classify(b"  INFO indented but explicit", ERROR), INFO);
}

#[test]
fn strips_ansi_sequences() {
    assert_eq!(&*strip_ansi(b"\x1b[1;31mred\x1b[0m text"), b"red text");
    assert_eq!(&*strip_ansi(b"\x1b]0;title\x07after"), b"after");
    assert_eq!(&*strip_ansi(b"\x1b]8;;http://x\x1b\\link"), b"link");
    assert_eq!(&*strip_ansi(b"plain"), b"plain");
    assert_eq!(&*strip_ansi(b"trailing \x1b["), b"trailing ");
}
