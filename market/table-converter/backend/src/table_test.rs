use super::*;

fn t(headers: &[&str], rows: &[&[&str]]) -> Table {
    Table {
        headers: headers.iter().map(|s| s.to_string()).collect(),
        rows: rows
            .iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect(),
        aligns: Vec::new(),
    }
}

#[test]
fn normalizes_and_moves_headers() {
    let table = t(&[], &[&["a", "b"], &["1"], &["", " "], &["2", "3", "4"]]).normalize();
    assert_eq!(
        table.rows,
        [vec!["a", "b", ""], vec!["1", "", ""], vec!["2", "3", "4"]]
    );
    let with = table.clone().with_header(true);
    assert_eq!(with.headers, ["a", "b", ""]);
    assert_eq!(with.with_header(false).rows.len(), 3);
}

#[test]
fn transposes_including_headers() {
    let table = t(&["name", "age"], &[&["Ann", "31"], &["Bob", "17"]]).transpose();
    assert_eq!(table.headers, ["name", "Ann", "Bob"]);
    assert_eq!(table.rows, [vec!["age", "31", "17"]]);
}
