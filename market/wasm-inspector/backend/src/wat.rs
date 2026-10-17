//! 把模块打印为 WAT 文本（只读），并按函数切分，便于按需展开单个函数。
//!
//! 打印有字节预算：超出后停止并标记截断；打印过程中定期检查取消。

use std::io;
use std::ops::Range;

use wasmprinter::Print;

use crate::model::Problem;

/// 大纲（函数体折叠后的模块文本）的上限
pub const OUTLINE_LIMIT: usize = 4 * 1024 * 1024;
/// 每写入这么多次检查一次取消
const CANCEL_EVERY: usize = 4096;

struct Sink<'a> {
    out: String,
    limit: usize,
    writes: usize,
    truncated: bool,
    cancelled: bool,
    check: &'a dyn Fn() -> bool,
}

impl Print for Sink<'_> {
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        if self.out.len() + s.len() > self.limit {
            self.truncated = true;
            return Err(io::Error::other("limit"));
        }
        self.writes += 1;
        if self.writes.is_multiple_of(CANCEL_EVERY) && (self.check)() {
            self.cancelled = true;
            return Err(io::Error::other("cancelled"));
        }
        self.out.push_str(s);
        Ok(())
    }
}

#[derive(Debug)]
pub struct Wat {
    pub text: String,
    pub truncated: bool,
    /// 每个模块内定义的函数在 text 中的字节范围（按定义顺序）
    pub functions: Vec<Range<usize>>,
}

pub enum Printed {
    Done(Wat),
    Cancelled,
    Failed(Problem),
}

/// 打印 WAT，文本超过 `limit` 字节时截断
pub fn print(bytes: &[u8], limit: usize, cancelled: &dyn Fn() -> bool) -> Printed {
    let mut sink = Sink {
        out: String::new(),
        limit,
        writes: 0,
        truncated: false,
        cancelled: false,
        check: cancelled,
    };
    let result = wasmprinter::Config::new().print(bytes, &mut sink);
    if sink.cancelled {
        return Printed::Cancelled;
    }
    if let Err(err) = result
        && !sink.truncated
    {
        return Printed::Failed(Problem {
            code: "wasm.print".into(),
            message: format!("{err:#}"),
            offset: None,
        });
    }
    let functions = function_ranges(&sink.out, sink.truncated);
    Printed::Done(Wat {
        text: sink.out,
        truncated: sink.truncated,
        functions,
    })
}

/// 模块的顶层条目缩进两个空格；函数从 `  (func` 开始，到下一个顶层条目或模块结尾为止。
/// 截断时最后一个函数可能不完整，不计入。
fn function_ranges(text: &str, truncated: bool) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut open: Option<usize> = None;
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        let top = line.starts_with("  (") || line.starts_with(')');
        if top {
            if let Some(start) = open.take() {
                ranges.push(start..pos);
            }
            if line.starts_with("  (func") {
                open = Some(pos);
            }
        }
        pos += line.len();
    }
    if let Some(start) = open
        && !truncated
    {
        ranges.push(start..pos);
    }
    ranges
}

impl Wat {
    pub fn function(&self, ordinal: usize) -> Option<&str> {
        self.functions.get(ordinal).map(|r| &self.text[r.clone()])
    }

    /// 函数体折叠成一行注释后的模块文本，用于浏览模块结构
    pub fn outline(&self) -> (String, bool) {
        let mut out = String::new();
        let mut last = 0;
        for range in &self.functions {
            out.push_str(&self.text[last..range.start]);
            let body = &self.text[range.clone()];
            let mut lines = body.lines();
            let head = lines.next().unwrap_or_default();
            let rest = lines.count();
            if rest <= 1 {
                out.push_str(body);
            } else {
                out.push_str(head);
                out.push_str(&format!("\n    ;; … {} lines\n  )\n", rest - 1));
            }
            last = range.end;
            if out.len() > OUTLINE_LIMIT {
                return (out, true);
            }
        }
        out.push_str(&self.text[last..]);
        let over = out.len() > OUTLINE_LIMIT;
        if over {
            let mut cut = OUTLINE_LIMIT;
            while !out.is_char_boundary(cut) {
                cut -= 1;
            }
            out.truncate(cut);
        }
        (out, over || self.truncated)
    }
}

#[cfg(test)]
#[path = "wat_test.rs"]
mod tests;
