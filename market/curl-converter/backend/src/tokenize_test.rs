use super::*;

fn t(input: &str) -> Vec<String> {
    tokenize(input).unwrap()
}

#[test]
fn splits_bash_quoting() {
    assert_eq!(
        t("curl 'https://a.b/?x=1&y=2' \\\n  -H \"X-A: \\\"q\\\" \\$HOME\" -d 'a b' # trailing"),
        vec![
            "curl",
            "https://a.b/?x=1&y=2",
            "-H",
            "X-A: \"q\" $HOME",
            "-d",
            "a b"
        ]
    );
    assert_eq!(t("curl a\\ b ''"), vec!["curl", "a b", ""]);
    assert_eq!(
        t("curl -d $'{\"a\":\"\\u00e9\\n\\x41\\'\"}'"),
        vec!["curl", "-d", "{\"a\":\"é\nA'\"}"]
    );
    assert_eq!(t("curl x | jq . > out"), vec!["curl", "x"]);
    assert_eq!(t("curl x\r\n"), vec!["curl", "x"]);
}

#[test]
fn splits_windows_cmd() {
    let input = "curl ^\"https://a.b/?q=1^&r=2^\" ^\r\n  -H ^\"accept: */*^\" ^\n  --data-raw ^\"^{^\\^\"k^\\^\":1^}^\"";
    assert_eq!(
        t(input),
        vec![
            "curl",
            "https://a.b/?q=1&r=2",
            "-H",
            "accept: */*",
            "--data-raw",
            "{\"k\":1}"
        ]
    );
}

#[test]
fn reports_unterminated_quotes() {
    for input in ["curl 'x", "curl \"x", "curl $'x", "curl ^\"x"] {
        assert_eq!(
            tokenize(input).unwrap_err().code,
            "curl.unterminated_quote",
            "{input}"
        );
    }
}
