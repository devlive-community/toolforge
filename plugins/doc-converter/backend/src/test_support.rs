//! 测试用的临时文件夹、任务上下文与样例文件。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use docx_rs::{Docx, Paragraph, Run, Table, TableCell, TableRow};
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
use serde_json::Value;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};

pub fn dir(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tfp-doc-converter-{name}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Default)]
pub struct Ctx(pub Mutex<Vec<String>>);

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.0.lock().unwrap().push(code.to_owned());
    }
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

/// 两个有数据的工作表和一个空工作表
pub fn workbook(path: &Path) {
    let mut book = Workbook::new();
    let date = Format::new().set_num_format("yyyy-mm-dd");
    let sheet = book.add_worksheet();
    sheet.set_name("销售").unwrap();
    sheet.write_string(0, 0, "城市").unwrap();
    sheet.write_string(0, 1, "金额").unwrap();
    sheet.write_string(0, 2, "日期").unwrap();
    sheet.write_string(1, 0, "北京").unwrap();
    sheet.write_number(1, 1, 0.1 + 0.2).unwrap();
    sheet
        .write_datetime_with_format(1, 2, ExcelDateTime::from_ymd(2025, 10, 1).unwrap(), &date)
        .unwrap();
    sheet.write_string(2, 0, "上海\n浦东").unwrap();
    sheet.write_number(2, 1, 1200.0).unwrap();
    book.add_worksheet().set_name("空表").unwrap();
    let other = book.add_worksheet();
    other.set_name("Other").unwrap();
    other.write_string(0, 0, "x").unwrap();
    book.save(path).unwrap();
}

fn cell(text: &str) -> TableCell {
    TableCell::new().add_paragraph(Paragraph::new().add_run(Run::new().add_text(text)))
}

pub fn document(path: &Path, with_table: bool) {
    let mut docx =
        Docx::new().add_paragraph(Paragraph::new().add_run(Run::new().add_text("员工名单")));
    if with_table {
        let rows = vec![
            TableRow::new(vec![cell("姓名"), cell("工号"), cell("工资")]),
            TableRow::new(vec![cell("张三"), cell("007"), cell("8500.5")]),
            TableRow::new(vec![cell("李四"), cell("110101199003077777"), cell("-12")]),
        ];
        docx = docx.add_table(Table::new(rows));
    }
    docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text("第二段")));
    docx.build()
        .pack(std::fs::File::create(path).unwrap())
        .unwrap();
}
