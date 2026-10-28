use super::*;
use crate::ops::tests::with_item;
use crate::ops::{Op, Request, apply};

fn lend(l: &Ledger, item: &str, op: &str) -> Ledger {
    apply(Request {
        ledger: l.clone(),
        op: Op::Lend {
            item_id: item.into(),
            borrower: "Kim".into(),
            qty: 1,
            date: "2026-10-20".into(),
            due: None,
            note: String::new(),
        },
        op_id: op.into(),
        revision: l.revision,
        today: "2026-10-28".into(),
        preview: false,
    })
    .unwrap()
    .ledger
    .unwrap()
}

fn tiny_jpeg() -> String {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 4));
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut out)
        .encode_image(&image)
        .unwrap();
    format!(
        "data:image/jpeg;base64,{}",
        data_encoding::BASE64.encode(&out)
    )
}

fn empty() -> Ledger {
    Ledger {
        schema: SCHEMA,
        ..Ledger::default()
    }
}

#[test]
fn backup_round_trips_items_loans_and_photos() {
    let (mut l, item) = with_item(3);
    l = lend(&l, &item, "a");
    l.items[0].has_photo = true;
    let photos = BTreeMap::from([(item.clone(), tiny_jpeg())]);
    let text = backup_json(&l, &photos, "2026-10-28").unwrap();
    let out = restore(&empty(), &text, Mode::Replace, 0, false).unwrap();
    let restored = out.ledger.unwrap();
    assert_eq!(restored.items, l.items);
    assert_eq!(restored.loans, l.loans, "借出单与藏品的关系保留");
    assert_eq!(out.photos.len(), 1);
}

#[test]
fn merge_never_overwrites_existing_records() {
    let (l, item) = with_item(3);
    let text = backup_json(&l, &BTreeMap::new(), "2026-10-28").unwrap();
    // 当前台账已修改同一件藏品
    let current = lend(&l, &item, "a");
    let out = restore(&current, &text, Mode::Merge, current.revision, false).unwrap();
    assert_eq!(out.report.items_skipped, 1);
    assert_eq!(out.report.items_added, 0);
    let merged = out.ledger.unwrap();
    assert_eq!(merged.loans.len(), 1, "现有借出单不受影响");
    assert_eq!(merged.revision, current.revision + 1);
}

#[test]
fn merge_adds_new_items_with_their_loans() {
    let (other, item) = with_item(2);
    let other = lend(&other, &item, "a");
    let text = backup_json(&other, &BTreeMap::new(), "2026-10-28").unwrap();
    let mut current = empty();
    current.revision = 7;
    let preview = restore(&current, &text, Mode::Merge, 7, true).unwrap();
    assert!(preview.ledger.is_none(), "预览不产生新台账");
    assert_eq!(preview.report.items_added, 1);
    assert_eq!(preview.report.loans_added, 1);
}

#[test]
fn broken_or_oversized_backups_are_rejected() {
    let current = empty();
    assert_eq!(
        restore(&current, "{}", Mode::Merge, 0, true)
            .unwrap_err()
            .code,
        "ledger.bad_backup"
    );
    let big = " ".repeat(MAX_BACKUP + 1);
    assert_eq!(
        restore(&current, &big, Mode::Merge, 0, true)
            .unwrap_err()
            .code,
        "ledger.backup_too_large"
    );
    // 守恒被破坏的备份
    let (mut l, item) = with_item(1);
    l.loans.push(Loan {
        id: "l-x".into(),
        item_id: item,
        borrower: "Bo".into(),
        qty: 9,
        date: "2026-10-01".into(),
        ..Loan::default()
    });
    let text = serde_json::to_string(&Backup {
        format: FORMAT.into(),
        version: SCHEMA,
        exported_at: "2026-10-28".into(),
        items: l.items,
        loans: l.loans,
        photos: BTreeMap::new(),
    })
    .unwrap();
    assert_eq!(
        restore(&current, &text, Mode::Merge, 0, true)
            .unwrap_err()
            .code,
        "ledger.conservation"
    );
}

#[test]
fn stale_restore_is_rejected() {
    let text = backup_json(&empty(), &BTreeMap::new(), "2026-10-28").unwrap();
    assert_eq!(
        restore(&empty(), &text, Mode::Merge, 3, true)
            .unwrap_err()
            .code,
        "ledger.stale"
    );
}

#[test]
fn csv_guards_formulas_and_reports_counts() {
    let (mut l, item) = with_item(3);
    l.items[0].notes = "=HYPERLINK(\"x\")".into();
    l = lend(&l, &item, "a");
    let headers: Vec<String> = (0..12).map(|i| format!("h{i}")).collect();
    let csv = list_csv(&l, &headers).unwrap();
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.contains("\"'=HYPERLINK(\"\"x\"\")\""));
    // 保有 3、在手 2、借出 1
    assert!(csv.contains(",3,2,1,0,"));
}

#[test]
fn photos_must_be_small_jpegs() {
    assert!(check_photo(&tiny_jpeg()).is_ok());
    assert_eq!(
        check_photo("data:image/png;base64,AAAA").unwrap_err().code,
        "ledger.bad_photo"
    );
    let huge = format!("data:image/jpeg;base64,{}", "A".repeat(MAX_PHOTO));
    assert_eq!(
        check_photo(&huge).unwrap_err().code,
        "ledger.photo_too_large"
    );
}
