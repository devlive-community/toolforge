use std::io::Read;
use std::sync::Mutex;

use super::*;
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
    progress: Mutex<Vec<(u64, u64)>>,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, done: u64, total: u64) {
        self.progress.lock().unwrap().push((done, total));
    }
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|_| PluginError::new("fs.not_found"))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|_| PluginError::new("fs.not_found"))
    }
}

fn temp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("tfp-xlsx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().into_owned()
}

/// 两个工作表：Data（name / age / date）与 Notes
fn data_workbook(path: &str) {
    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet().set_name("Data").unwrap();
    sheet.write(0, 0, "name").unwrap();
    sheet.write(0, 1, "age").unwrap();
    sheet.write(0, 2, "date").unwrap();
    let date_fmt = Format::new().set_num_format("yyyy-mm-dd");
    sheet.write(1, 0, "Ada").unwrap();
    sheet.write(1, 1, 36).unwrap();
    sheet
        .write_with_format(
            1,
            2,
            ExcelDateTime::from_ymd(2023, 7, 1).unwrap(),
            &date_fmt,
        )
        .unwrap();
    sheet.write(2, 0, "Bob").unwrap();
    sheet.write(2, 1, 7).unwrap();
    sheet
        .write_with_format(
            2,
            2,
            ExcelDateTime::from_ymd(2024, 2, 29).unwrap(),
            &date_fmt,
        )
        .unwrap();
    let notes = wb.add_worksheet().set_name("Notes").unwrap();
    notes.write(0, 0, "note").unwrap();
    notes.write(1, 0, "hello").unwrap();
    wb.save(path).unwrap();
}

#[test]
fn opens_multi_sheet_workbook_and_pages() {
    let tool = XlsxViewer::default();
    let ctx = Ctx::default();
    let path = temp("book.xlsx");
    data_workbook(&path);
    let info = tool
        .run_task("open", json!({ "path": path }), &ctx)
        .unwrap();
    let sheets = info["sheets"].as_array().unwrap();
    assert_eq!(sheets.len(), 2);
    assert_eq!(info["sheets"][0]["name"], "Data");
    assert_eq!(info["sheets"][0]["rows"], 2);
    assert_eq!(info["sheets"][0]["columns"][0]["name"], "name");
    assert_eq!(info["sheets"][0]["columns"][1]["kind"], "integer");
    // 日期列按数据行推断，即使表头是文本
    assert_eq!(info["sheets"][0]["columns"][2]["kind"], "date");
    assert_eq!(info["sheets"][1]["name"], "Notes");
    assert_eq!(info["name"], "book.xlsx");
    let (done, total) = *ctx.progress.lock().unwrap().last().unwrap();
    assert_eq!((done, total), (2, 2));
    let id = info["id"].as_u64().unwrap();

    let page = tool
        .call(
            "rows",
            json!({ "id": id, "sheet": 0, "sort": [{ "column": 1, "desc": true }] }),
        )
        .unwrap();
    assert_eq!(page["total"], 2);
    assert_eq!(page["rows"][0]["cells"][0], "Ada");
    assert_eq!(page["rows"][0]["cells"][2], "2023-07-01");

    let stats = tool
        .call("stats", json!({ "id": id, "sheet": 0, "column": 1 }))
        .unwrap();
    assert_eq!(stats["sum"], 43.0);

    let out = temp("out.csv");
    let result = tool
        .run_task(
            "export",
            json!({ "id": id, "sheet": 0, "path": out, "format": "csv" }),
            &ctx,
        )
        .unwrap();
    assert_eq!(result["rows"], 2);
    let csv = std::fs::read_to_string(result["path"].as_str().unwrap()).unwrap();
    assert_eq!(csv, "name,age,date\nAda,36,2023-07-01\nBob,7,2024-02-29\n");
}

#[test]
fn can_treat_first_row_as_data() {
    let tool = XlsxViewer::default();
    let ctx = Ctx::default();
    let path = temp("header.xlsx");
    data_workbook(&path);
    let info = tool
        .run_task("open", json!({ "path": path, "header": false }), &ctx)
        .unwrap();
    assert_eq!(info["sheets"][0]["rows"], 3);
    // 没有表头，列名为空
    assert!(info["sheets"][0]["columns"][0]["name"].is_null());
    let id = info["id"].as_u64().unwrap();
    let page = tool.call("rows", json!({ "id": id, "sheet": 0 })).unwrap();
    assert_eq!(page["rows"][0]["cells"], json!(["name", "age", "date"]));
}

#[test]
fn skips_empty_sheets_and_rejects_blank_workbook() {
    let tool = XlsxViewer::default();
    let ctx = Ctx::default();
    let blank_first = temp("blank-first.xlsx");
    let mut wb = Workbook::new();
    wb.add_worksheet().set_name("Empty").unwrap();
    let sheet = wb.add_worksheet().set_name("Data").unwrap();
    sheet.write(0, 0, "x").unwrap();
    wb.save(&blank_first).unwrap();
    let info = tool
        .run_task("open", json!({ "path": blank_first }), &ctx)
        .unwrap();
    let sheets = info["sheets"].as_array().unwrap();
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0]["name"], "Data");
    // 只有一行表头，数据为 0 行
    assert_eq!(sheets[0]["rows"], 0);

    let all_blank = temp("all-blank.xlsx");
    let mut wb = Workbook::new();
    wb.add_worksheet().set_name("Empty").unwrap();
    wb.save(&all_blank).unwrap();
    let err = tool
        .run_task("open", json!({ "path": all_blank }), &ctx)
        .unwrap_err();
    assert_eq!(err.code, "xlsx.empty_workbook");

    let not_xlsx = temp("broken.xlsx");
    std::fs::write(&not_xlsx, b"not a zip").unwrap();
    let err = tool
        .run_task("open", json!({ "path": not_xlsx }), &ctx)
        .unwrap_err();
    assert_eq!(err.code, "xlsx.read_failed");
}

#[test]
fn rejects_invalid_sheet_and_expires_old_workbooks() {
    let tool = XlsxViewer::default();
    let ctx = Ctx::default();
    let path = temp("expire-a.xlsx");
    data_workbook(&path);
    let first = tool
        .run_task("open", json!({ "path": path.clone() }), &ctx)
        .unwrap();
    let err = tool
        .call("rows", json!({ "id": first["id"], "sheet": 9 }))
        .unwrap_err();
    assert_eq!(err.code, "xlsx.invalid_sheet");

    let second = temp("expire-b.xlsx");
    data_workbook(&second);
    tool.run_task("open", json!({ "path": second }), &ctx)
        .unwrap();
    let third = temp("expire-c.xlsx");
    data_workbook(&third);
    tool.run_task("open", json!({ "path": third }), &ctx)
        .unwrap();
    // 最早打开的工作簿已被释放
    let err = tool
        .call("rows", json!({ "id": first["id"], "sheet": 0 }))
        .unwrap_err();
    assert_eq!(err.code, "xlsx.table_expired");
}
