use serde_json::json;

use super::*;

fn field(name: &str, kind: Kind) -> Field {
    Field {
        name: name.into(),
        kind,
        min: None,
        max: None,
        decimals: None,
        from: None,
        to: None,
        options: None,
        null_percent: 0,
    }
}

fn schema(locale: Locale, fields: Vec<Field>, rows: usize) -> Schema {
    Schema {
        locale,
        rows,
        fields,
        seed: Some(42),
    }
}

fn today() -> Date {
    Date::new(2026, 9, 25).unwrap()
}

#[test]
fn checksums_follow_the_national_standard() {
    // GB 11643-1999 附录中的示例号码
    assert_eq!(id_checksum("11010519491231002"), 'X');
    assert_eq!(id_checksum("44052418800101001"), '4');
}

#[test]
fn same_seed_gives_same_rows_and_preview_is_a_prefix() {
    let s = schema(
        Locale::ZhCn,
        vec![
            field("id", Kind::Id),
            field("name", Kind::Name),
            field("email", Kind::Email),
        ],
        50,
    );
    let full = generate(&s, None, today()).unwrap();
    let again = generate(&s, None, today()).unwrap();
    let preview = generate(&s, Some(10), today()).unwrap();
    assert_eq!(full.rows, again.rows);
    assert_eq!(preview.rows[..], full.rows[..10]);
    assert_eq!(full.rows[0][0], json!(1));
    assert_eq!(full.rows[49][0], json!(50));
    assert_eq!(full.seed, 42);
}

#[test]
fn chinese_rows_are_consistent() {
    let fields = vec![
        field("name", Kind::Name),
        field("gender", Kind::Gender),
        field("birthday", Kind::Birthday),
        field("age", Kind::Age),
        field("id_card", Kind::IdCard),
        field("phone", Kind::Phone),
        field("province", Kind::Province),
        field("address", Kind::Address),
    ];
    let generated = generate(&schema(Locale::ZhCn, fields, 200), None, today()).unwrap();
    for row in &generated.rows {
        let id = row[4].as_str().unwrap();
        assert_eq!(id.len(), 18, "{id}");
        assert_eq!(id.chars().last().unwrap(), id_checksum(&id[..17]));
        // 出生日期与生日字段一致，第 17 位奇数为男性
        assert_eq!(&id[6..14], row[2].as_str().unwrap().replace('-', ""));
        let male = id.as_bytes()[16].is_ascii_digit() && (id.as_bytes()[16] - b'0') % 2 == 1;
        assert_eq!(row[1] == json!("男"), male);
        let age = row[3].as_i64().unwrap();
        assert!((18..=60).contains(&age), "{age}");
        let phone = row[5].as_str().unwrap();
        assert!(phone.len() == 11 && phone.starts_with('1'), "{phone}");
        assert!(
            row[7]
                .as_str()
                .unwrap()
                .starts_with(row[6].as_str().unwrap())
        );
    }
}

#[test]
fn emails_follow_names() {
    let fields = vec![field("name", Kind::Name), field("email", Kind::Email)];
    let generated = generate(&schema(Locale::EnUs, fields, 20), None, today()).unwrap();
    for row in &generated.rows {
        let name = row[0].as_str().unwrap().to_lowercase().replace(' ', ".");
        assert!(row[1].as_str().unwrap().starts_with(&name), "{row:?}");
    }
}

#[test]
fn respects_ranges_enums_and_nulls() {
    let mut int = field("n", Kind::Integer);
    int.min = Some(5.0);
    int.max = Some(7.0);
    let mut price = field("price", Kind::Float);
    price.min = Some(1.0);
    price.max = Some(2.0);
    price.decimals = Some(1);
    let mut status = field("status", Kind::Enum);
    status.options = Some("paid, pending ,refunded".into());
    let mut date = field("date", Kind::Date);
    date.from = Some("2024-01-01".into());
    date.to = Some("2024-01-31".into());
    let mut note = field("note", Kind::Sentence);
    note.null_percent = 100;
    let generated = generate(
        &schema(Locale::EnUs, vec![int, price, status, date, note], 300),
        None,
        today(),
    )
    .unwrap();
    for row in &generated.rows {
        assert!((5..=7).contains(&row[0].as_i64().unwrap()));
        let p = row[1].as_f64().unwrap();
        assert!((1.0..=2.0).contains(&p) && (p * 10.0).fract() == 0.0, "{p}");
        assert!(["paid", "pending", "refunded"].contains(&row[2].as_str().unwrap()));
        assert!(row[3].as_str().unwrap().starts_with("2024-01-"));
        assert!(row[4].is_null());
    }
}

#[test]
fn validates_schemas() {
    let code = |s: Schema| generate(&s, None, today()).unwrap_err().code;
    assert_eq!(
        code(schema(Locale::ZhCn, vec![field("a", Kind::Id)], 0)),
        "mock.invalid_rows"
    );
    assert_eq!(code(schema(Locale::ZhCn, vec![], 1)), "mock.no_fields");
    assert_eq!(
        code(schema(
            Locale::ZhCn,
            vec![field("a", Kind::Id), field("a", Kind::Name)],
            1
        )),
        "mock.duplicate_name"
    );
    assert_eq!(
        code(schema(Locale::ZhCn, vec![field(" ", Kind::Id)], 1)),
        "mock.empty_name"
    );
    assert_eq!(
        code(schema(Locale::ZhCn, vec![field("s", Kind::Enum)], 1)),
        "mock.no_options"
    );
    let mut bad = field("d", Kind::Date);
    bad.from = Some("2024-13-01".into());
    assert_eq!(
        code(schema(Locale::ZhCn, vec![bad], 1)),
        "mock.invalid_date"
    );
    let mut range = field("n", Kind::Integer);
    range.min = Some(9.0);
    range.max = Some(1.0);
    assert_eq!(
        code(schema(Locale::ZhCn, vec![range], 1)),
        "mock.invalid_range"
    );
}
