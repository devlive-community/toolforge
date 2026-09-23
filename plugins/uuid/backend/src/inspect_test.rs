use super::*;

fn inspect(input: &str) -> PluginResult<Info> {
    run(Args {
        input: input.into(),
    })
}

#[test]
fn inspects_v4_with_braces_and_uppercase() {
    let info = inspect("{CFBFF0D1-9375-5685-968C-48CE8B15AE17}").unwrap();
    assert_eq!(info.kind, "uuid");
    assert_eq!(info.version, Some(5));
    assert_eq!(info.variant, Some("rfc4122"));
    assert_eq!(info.canonical, "cfbff0d1-9375-5685-968c-48ce8b15ae17");
    assert!(info.timestamp.is_none());
}

#[test]
fn extracts_v7_timestamp() {
    // 0x018f... 的 v7：前 48 位为 Unix 毫秒
    let id = Uuid::now_v7();
    let info = inspect(&id.to_string()).unwrap();
    assert_eq!(info.version, Some(7));
    let ms = info.unix_ms.unwrap();
    let now = jiff::Timestamp::now().as_millisecond();
    assert!((now - ms).abs() < 5_000);
    assert!(info.timestamp.unwrap().ends_with('Z'));
}

#[test]
fn extracts_ulid_timestamp() {
    let info = inspect("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    assert_eq!(info.kind, "ulid");
    assert_eq!(info.unix_ms, Some(1_469_922_850_259));
    assert_eq!(info.timestamp.as_deref(), Some("2016-07-30T23:54:10.259Z"));
}

#[test]
fn nil_and_invalid() {
    assert!(
        inspect("00000000-0000-0000-0000-000000000000")
            .unwrap()
            .is_nil
    );
    assert_eq!(inspect("not-an-id").unwrap_err().code, "uuid.invalid");
    assert_eq!(inspect("   ").unwrap_err().code, "uuid.empty");
}
