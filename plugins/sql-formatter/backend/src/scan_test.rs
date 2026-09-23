use super::*;

#[test]
fn minify_collapses_whitespace_and_strips_comments() {
    let sql = "SELECT  a ,\n  b -- trailing\nFROM t /* block */ WHERE ( x = 1 ) ;";
    assert_eq!(minify(sql), "SELECT a,b FROM t WHERE (x = 1);");
}

#[test]
fn minify_preserves_string_and_identifier_contents() {
    let sql = "SELECT 'a  --  b', \"col  x\", `k  y`, [sq  l] FROM t WHERE s = 'it''s'";
    assert_eq!(
        minify(sql),
        "SELECT 'a  --  b',\"col  x\",`k  y`,[sq  l] FROM t WHERE s = 'it''s'"
    );
}

#[test]
fn minify_keeps_dollar_quoted_bodies() {
    let sql = "CREATE FUNCTION f() RETURNS int AS $body$\n  SELECT  1;\n$body$ LANGUAGE sql;";
    assert!(minify(sql).contains("$body$\n  SELECT  1;\n$body$"));
}

#[test]
fn counts_statements_outside_strings_and_comments() {
    assert_eq!(count_statements("select 1; select ';'; -- ;\n select 3"), 3);
    assert_eq!(count_statements(";;  ;"), 0);
    assert_eq!(count_statements("select 1;"), 1);
}

#[test]
fn unterminated_literals_do_not_panic() {
    assert_eq!(minify("select 'abc"), "select 'abc");
    assert_eq!(count_statements("/* open comment"), 0);
}

#[test]
fn handles_multibyte_text() {
    assert_eq!(
        minify("SELECT  '中文  内容' ,  名称 FROM 表"),
        "SELECT '中文  内容',名称 FROM 表"
    );
}
