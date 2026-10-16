fn fmt(dialect: &str, sql: &str, case: &str) -> String {
    let args = serde_json::json!({ "input": sql, "dialect": dialect, "keywordCase": case });
    crate::format::format(serde_json::from_value(args).unwrap())
        .unwrap()
        .output
}

#[test]
fn mysql_keeps_backticks_hash_comments_and_upsert_clause() {
    assert_eq!(
        fmt(
            "mysql",
            "select `id`, # note\n name from `db`.`users` where name = 'it\\'s' limit 10, 20",
            "upper"
        ),
        "SELECT\n  `id`,  # note\n  name\nFROM\n  `db`.`users`\nWHERE\n  name = 'it\\'s'\nLIMIT\n  10, 20"
    );
    assert_eq!(
        fmt(
            "mysql",
            "insert into t (a, b) values (1, 2) on duplicate key update a = values(a), b = b + 1",
            "upper"
        ),
        "INSERT INTO\n  t (a, b)\nVALUES\n  (1, 2)\nON DUPLICATE KEY UPDATE\n  a = VALUES(a),\n  b = b + 1"
    );
}

#[test]
fn sqlite_brackets_keywords_and_upsert() {
    assert_eq!(
        fmt(
            "sqlite",
            "select [id], [my col] from t where x glob 'a*' and y = ?1 and z = :name",
            "upper"
        ),
        "SELECT\n  [id],\n  [my col]\nFROM\n  t\nWHERE\n  x GLOB 'a*'\n  AND y = ?1\n  AND z = :name"
    );
    assert_eq!(
        fmt(
            "sqlite",
            "insert into t(id, x) values (1, 'a') on conflict(id) do update set x = excluded.x",
            "upper"
        ),
        "INSERT INTO\n  t(id, x)\nVALUES\n  (1, 'a')\nON CONFLICT (id)\nDO UPDATE SET\n  x = excluded.x"
    );
}

#[test]
fn oracle_q_quotes_and_hierarchical_clauses() {
    assert_eq!(
        fmt(
            "oracle",
            "select q'[it's]' as s, :bind from emp connect by prior id = parent_id start with id = 1",
            "upper"
        ),
        "SELECT\n  q'[it's]' AS s,\n  :bind\nFROM\n  emp\nCONNECT BY\n  PRIOR id = parent_id\nSTART WITH\n  id = 1"
    );
    // PL/SQL 块的 / 终止行保留
    assert!(fmt("oracle", "begin\n  commit;\nend;\n/", "upper").ends_with("END;\n/"));
}

#[test]
fn bigquery_strings_types_and_qualify() {
    assert_eq!(
        fmt(
            "bigquery",
            "select '''a\nb''' as s, r'\\d+' as r, array<int64>[1,2] as arr from `p.d.t` qualify rn = 1",
            "upper"
        ),
        "SELECT\n  '''a\nb''' AS s,\n  r'\\d+' AS r,\n  ARRAY<INT64>[1, 2] AS arr\nFROM\n  `p.d.t`\nQUALIFY\n  rn = 1"
    );
    assert_eq!(
        fmt(
            "bigquery",
            "select cast(x as struct<a int64>) from t, unnest(arr) as e # note",
            "lower"
        ),
        "select\n  cast(x as struct<a int64>)\nfrom\n  t,\n  unnest(arr) as e # note"
    );
}

#[test]
fn preserve_case_keeps_phrases_as_written() {
    assert_eq!(
        fmt(
            "mysql",
            "insert into t values (1) On Duplicate  Key Update a = 1",
            "preserve"
        ),
        "insert into\n  t\nvalues\n  (1)\nOn Duplicate Key Update\n  a = 1"
    );
}

#[test]
fn placeholders_never_collide_with_input() {
    let sql = "select '__tf0ph' as a, r'x' as b";
    assert_eq!(
        fmt("bigquery", sql, "upper"),
        "SELECT\n  '__tf0ph' AS a,\n  r'x' AS b"
    );
    // 关键字只在单词边界处理，不改动字符串与注释
    assert_eq!(
        fmt("bigquery", "select 'struct' # struct", "upper"),
        "SELECT\n  'struct' # struct"
    );
}

#[test]
fn generic_output_is_unchanged() {
    assert_eq!(
        fmt("generic", "select a from t where b = 1", "upper"),
        "SELECT\n  a\nFROM\n  t\nWHERE\n  b = 1"
    );
}
