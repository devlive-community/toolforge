use super::*;

fn minify_generic(sql: &str) -> String {
    minify(sql, DialectKind::Generic)
}

fn count_statements_generic(sql: &str) -> usize {
    count_statements(sql, DialectKind::Generic)
}

#[test]
fn minify_collapses_whitespace_and_strips_comments() {
    let sql = "SELECT  a ,\n  b -- trailing\nFROM t /* block */ WHERE ( x = 1 ) ;";
    assert_eq!(
        minify(sql, DialectKind::Generic),
        "SELECT a,b FROM t WHERE (x = 1);"
    );
}

#[test]
fn minify_preserves_string_and_identifier_contents() {
    let sql = "SELECT 'a  --  b', \"col  x\", `k  y`, [sq  l] FROM t WHERE s = 'it''s'";
    assert_eq!(
        minify(sql, DialectKind::Generic),
        "SELECT 'a  --  b',\"col  x\",`k  y`,[sq  l] FROM t WHERE s = 'it''s'"
    );
}

#[test]
fn minify_keeps_dollar_quoted_bodies() {
    let sql = "CREATE FUNCTION f() RETURNS int AS $body$\n  SELECT  1;\n$body$ LANGUAGE sql;";
    assert!(minify(sql, DialectKind::Generic).contains("$body$\n  SELECT  1;\n$body$"));
}

#[test]
fn counts_statements_outside_strings_and_comments() {
    assert_eq!(
        count_statements_generic("select 1; select ';'; -- ;\n select 3"),
        3
    );
    assert_eq!(count_statements_generic(";;  ;"), 0);
    assert_eq!(count_statements_generic("select 1;"), 1);
}

#[test]
fn unterminated_literals_do_not_panic() {
    assert_eq!(minify_generic("select 'abc"), "select 'abc");
    assert_eq!(count_statements_generic("/* open comment"), 0);
}

#[test]
fn handles_multibyte_text() {
    assert_eq!(
        minify_generic("SELECT  '中文  内容' ,  名称 FROM 表"),
        "SELECT '中文  内容',名称 FROM 表"
    );
}

#[test]
fn minify_follows_dialect_comments_and_strings() {
    assert_eq!(
        minify("SELECT a # note\n, 'it\\'s' FROM t", DialectKind::Mysql),
        "SELECT a,'it\\'s' FROM t"
    );
    assert_eq!(
        minify(
            "SELECT '''a  \n  b''' ,  r'x  y' # c\n",
            DialectKind::Bigquery
        ),
        "SELECT '''a  \n  b''',r'x  y'"
    );
    assert_eq!(
        minify("SELECT  q'[a ; b]'  FROM dual", DialectKind::Oracle),
        "SELECT q'[a ; b]' FROM dual"
    );
    // PostgreSQL 中 # 不是注释、[ ] 是数组
    assert_eq!(
        minify("SELECT a  #  b, arr[ 1 ]", DialectKind::Postgresql),
        "SELECT a # b,arr[ 1 ]"
    );
}

#[test]
fn counts_statements_per_dialect() {
    assert_eq!(
        count_statements("select 1 # ;\n; select ';'", DialectKind::Mysql),
        2
    );
    assert_eq!(
        count_statements("select q'[;]' from dual", DialectKind::Oracle),
        1
    );
    assert_eq!(
        count_statements("select ''';''';", DialectKind::Bigquery),
        1
    );
}

#[test]
fn tokens_cover_the_whole_input() {
    let sql = "SELECT r'a', q'{b}', [c], `d`, \"e\", $$f$$ -- g\n/* h */ 1;";
    for dialect in [
        DialectKind::Generic,
        DialectKind::Postgresql,
        DialectKind::Sqlserver,
        DialectKind::Mysql,
        DialectKind::Sqlite,
        DialectKind::Oracle,
        DialectKind::Bigquery,
    ] {
        let joined: String = tokenize(sql, dialect).iter().map(|t| t.text).collect();
        assert_eq!(joined, sql, "{dialect:?}");
    }
}
