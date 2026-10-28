use super::*;

fn ids() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("market/a/manifest.json".to_owned(), "org.x.a".to_owned()),
        ("market/b/manifest.json".to_owned(), "org.x.b".to_owned()),
    ])
}

const LOG: &str = r#"@@commit 2026-10-01T08:00:00Z

diff --git a/market/a/manifest.json b/market/a/manifest.json
new file mode 100644
--- /dev/null
+++ b/market/a/manifest.json
@@ -0,0 +1,4 @@
+{
+  "id": "org.x.a",
+  "version": "0.1.0",
+  "minAppVersion": "0.2.0",
@@commit 2026-10-05T09:30:00Z

diff --git a/market/a/manifest.json b/market/a/manifest.json
--- a/market/a/manifest.json
+++ b/market/a/manifest.json
@@ -3 +3 @@
-  "version": "0.1.0",
+  "version": "0.1.1",
diff --git a/market/b/manifest.json b/market/b/manifest.json
--- /dev/null
+++ b/market/b/manifest.json
@@ -0,0 +1 @@
+{ "id": "org.x.b", "version": "1.0.0" }
@@commit 2026-10-09T00:00:00Z

diff --git a/market/a/manifest.json b/market/a/manifest.json
--- a/market/a/manifest.json
+++ b/market/a/manifest.json
@@ -3 +3 @@
-  "version": "0.1.1",
+  "version": "0.1.0",
"#;

#[test]
fn first_appearance_of_each_version_wins() {
    let dates = parse_log(LOG, &ids());
    let get = |id: &str, v: &str| dates.get(&(id.to_owned(), v.to_owned())).cloned();
    assert_eq!(
        get("org.x.a", "0.1.0").as_deref(),
        Some("2026-10-01T08:00:00Z")
    );
    assert_eq!(
        get("org.x.a", "0.1.1").as_deref(),
        Some("2026-10-05T09:30:00Z")
    );
    // 单行 manifest 里 version 不在行首，不误读；只认 `"version": "x"` 独占一行的写法
    assert_eq!(get("org.x.b", "1.0.0"), None);
    // minAppVersion 不会被当成 version
    assert_eq!(dates.len(), 2);
}

#[test]
fn unknown_files_and_bad_dates_are_ignored() {
    let log = "@@commit not-a-date\n+++ b/market/a/manifest.json\n+  \"version\": \"9.9.9\",\n@@commit 2026-10-01T00:00:00Z\n+++ b/market/zzz/manifest.json\n+  \"version\": \"1.0.0\",\n";
    assert!(parse_log(log, &ids()).is_empty());
}

#[test]
fn timestamps_must_be_utc_rfc3339() {
    assert!(is_utc_timestamp("2026-10-28T12:34:56Z"));
    assert!(!is_utc_timestamp("2026-10-28T12:34:56+08:00"));
    assert!(!is_utc_timestamp("2026-10-28"));
}
