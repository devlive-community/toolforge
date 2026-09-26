use serde_json::json;

use super::*;

fn data() -> Generated {
    Generated {
        seed: 1,
        columns: vec!["id".into(), "name".into(), "active".into(), "note".into()],
        rows: vec![
            vec![json!(1), json!("O'Brien"), json!(true), json!(null)],
            vec![
                json!(2),
                json!("a,b \"c\""),
                json!(false),
                json!("C:\\temp"),
            ],
        ],
    }
}

fn options(format: Format, dialect: Dialect) -> Options {
    Options {
        format,
        dialect,
        table: "users".into(),
    }
}

#[test]
fn renders_json_and_json_lines() {
    let json = render(&data(), &options(Format::Json, Dialect::Mysql));
    let parsed: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed[0],
        json!({ "id": 1, "name": "O'Brien", "active": true, "note": null })
    );
    // 保持字段顺序
    assert!(json.find("\"id\"").unwrap() < json.find("\"name\"").unwrap());
    let lines = render(&data(), &options(Format::Jsonl, Dialect::Mysql));
    assert_eq!(lines.lines().count(), 2);
    assert!(lines.starts_with("{\"id\":1,"));
}

#[test]
fn renders_csv_with_quoting() {
    let csv = render(&data(), &options(Format::Csv, Dialect::Mysql));
    assert_eq!(
        csv,
        "id,name,active,note\n1,O'Brien,true,\n2,\"a,b \"\"c\"\"\",false,C:\\temp\n"
    );
}

#[test]
fn renders_sql_per_dialect() {
    let mysql = render(&data(), &options(Format::Sql, Dialect::Mysql));
    assert_eq!(
        mysql,
        "INSERT INTO `users` (`id`, `name`, `active`, `note`) VALUES\n  (1, 'O''Brien', 1, NULL),\n  (2, 'a,b \"c\"', 0, 'C:\\\\temp');\n"
    );
    let pg = render(&data(), &options(Format::Sql, Dialect::Postgres));
    assert!(pg.starts_with("INSERT INTO \"users\" (\"id\""), "{pg}");
    assert!(pg.contains("TRUE") && pg.contains("'C:\\temp'"), "{pg}");
}

#[test]
fn batches_sql_statements() {
    let mut big = data();
    big.rows = (0..250)
        .map(|i| vec![json!(i), json!("x"), json!(true), json!(null)])
        .collect();
    let sql = render(&big, &options(Format::Sql, Dialect::Sqlite));
    assert_eq!(sql.matches("INSERT INTO").count(), 3);
    assert_eq!(sql.matches(";\n").count(), 3);
}
