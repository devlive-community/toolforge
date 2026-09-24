use super::*;

fn run(input: &str, codec: Codec, direction: Direction, options: Options) -> PluginResult<Output> {
    transform(Args {
        input: input.into(),
        codec,
        direction,
        options,
    })
}

fn enc(input: &str, codec: Codec) -> String {
    run(
        input,
        codec,
        Direction::Encode,
        Options {
            padding: true,
            ..Options::default()
        },
    )
    .unwrap()
    .output
}

fn dec(input: &str, codec: Codec) -> Output {
    run(input, codec, Direction::Decode, Options::default()).unwrap()
}

#[test]
fn rfc4648_vectors() {
    assert_eq!(enc("foobar", Codec::Base64), "Zm9vYmFy");
    assert_eq!(enc("fooba", Codec::Base64), "Zm9vYmE=");
    assert_eq!(enc("foobar", Codec::Base32), "MZXW6YTBOI======");
    assert_eq!(enc("foobar", Codec::Hex), "666f6f626172");
    assert_eq!(enc("<<???>>", Codec::Base64), "PDw/Pz8+Pg==");
    assert_eq!(enc("<<???>>", Codec::Base64url), "PDw_Pz8-Pg==");
}

#[test]
fn base64_options() {
    let nopad = run(
        "fooba",
        Codec::Base64,
        Direction::Encode,
        Options {
            padding: false,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(nopad.output, "Zm9vYmE");
    let long = "x".repeat(100);
    let wrapped = run(
        &long,
        Codec::Base64,
        Direction::Encode,
        Options {
            padding: true,
            wrap: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(wrapped.output.lines().next().unwrap().len(), 76);
    let upper = run(
        "\u{00ff}",
        Codec::Hex,
        Direction::Encode,
        Options {
            uppercase: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(upper.output, "C3BF");
}

#[test]
fn decoding_is_lenient_about_whitespace_padding_and_separators() {
    assert_eq!(dec("Zm9v\nYmFy", Codec::Base64).output, "foobar");
    assert_eq!(dec("Zm9vYg", Codec::Base64).output, "foob");
    assert_eq!(dec("mzxw6ytboi", Codec::Base32).output, "foobar");
    assert_eq!(dec("0x66 0x6F:6f", Codec::Hex).output, "foo");
}

#[test]
fn non_utf8_results_are_returned_as_hex() {
    let out = dec("//8=", Codec::Base64);
    assert!(!out.text);
    assert_eq!(out.output, "ff ff");
    let url = dec("%FF%41", Codec::UrlComponent);
    assert!(!url.text);
    assert_eq!(url.output, "ff 41");
}

#[test]
fn url_variants() {
    assert_eq!(
        enc("a b&c/中?", Codec::UrlComponent),
        "a%20b%26c%2F%E4%B8%AD%3F"
    );
    assert_eq!(
        enc("https://x.com/a b?q=中&r=1", Codec::Url),
        "https://x.com/a%20b?q=%E4%B8%AD&r=1"
    );
    assert_eq!(enc("a b&c", Codec::Form), "a+b%26c");
    assert_eq!(dec("%E4%B8%AD%20x", Codec::UrlComponent).output, "中 x");
    assert_eq!(dec("a+b%26c", Codec::Form).output, "a b&c");
}

#[test]
fn html_entities() {
    let text = r#"<a href="x">é</a>"#;
    assert_eq!(
        enc(text, Codec::Html),
        "&lt;a href=&quot;x&quot;&gt;é&lt;/a&gt;"
    );
    let decimal = run(
        "é",
        Codec::Html,
        Direction::Encode,
        Options {
            html_mode: HtmlMode::Decimal,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(decimal.output, "&#233;");
    let hex = run(
        "é",
        Codec::Html,
        Direction::Encode,
        Options {
            html_mode: HtmlMode::Hex,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(hex.output, "&#xE9;");
    assert_eq!(
        dec("&lt;&copy;&#233;&#xE9;&amp;", Codec::Html).output,
        "<©éé&"
    );
}

#[test]
fn invalid_input_reports_codec() {
    let err = run("@@@", Codec::Base64, Direction::Decode, Options::default()).unwrap_err();
    assert_eq!(err.code, "encode.invalid_input");
    assert_eq!(err.params["codec"], "base64");
    assert!(run("zz", Codec::Hex, Direction::Decode, Options::default()).is_err());
}

#[test]
fn reports_byte_counts() {
    let out = run(
        "中",
        Codec::Base64,
        Direction::Encode,
        Options {
            padding: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!((out.input_bytes, out.output_bytes), (3, 4));
}
