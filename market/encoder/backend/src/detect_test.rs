use super::*;

#[test]
fn detects_encodings() {
    assert_eq!(detect("q%3Dhello%20world").unwrap().label, "url");
    assert_eq!(detect("\\u4f60\\u597d").unwrap().label, "unicode");
    assert_eq!(detect("a &lt; b &amp;&amp; c").unwrap().label, "html");
    assert_eq!(detect("&#x4f60;&#22909;").unwrap().label, "html");
    assert_eq!(
        detect("VG9vbEZvcmdlIOW3peWFt+eusQ==").unwrap(),
        Detection::new(60, "base64")
    );
    assert_eq!(
        detect("VG9vbEZvcmdlIOW3peWFt-eusQ").unwrap().label,
        "base64url"
    );
    assert!(
        detect("d41d8cd98f00b204e9800998ecf8427e").is_none(),
        "hex digests are not base64"
    );
    assert!(
        detect("AAAAAAAAAAAAAAAAAAAAAA==").is_none(),
        "binary data is not text"
    );
    assert!(detect("100% sure").is_none());
    assert!(detect("tom & jerry; friends").is_none());
    assert!(detect("plain words").is_none());
}
