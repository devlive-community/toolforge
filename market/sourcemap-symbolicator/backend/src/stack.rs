//! 解析错误堆栈文本：支持 Chrome/V8 的 `at 函数 (url:行:列)`、Safari 的
//! 方括号形式与 Firefox 的 `函数@url:行:列`，每行一个条目。
//!
//! 能提取 `url:行:列` 的行算作帧；其余行（报错消息、空行等）原样保留，
//! 不参与还原。列缺失时保留行但不伪造精确位置。

/// 一行堆栈文本
pub struct Frame {
    /// 去除首尾空白后的原始行
    pub raw: String,
    /// 是否包含 url:行:列
    pub is_frame: bool,
    pub function: Option<String>,
    pub url: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

pub struct Stack {
    pub frames: Vec<Frame>,
    /// 超过行数上限被截断
    pub truncated: bool,
}

/// 从行尾提取 `url:行[:列]`：列在前、行在后，各由 `:` 分隔；只有一个
/// 数字段时视为行号（列缺失）
fn extract_position(text: &str) -> Option<(&str, u32, Option<u32>)> {
    let number = |part: &str| -> Option<u32> {
        if part.is_empty() || part.len() > 10 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let (before_last, last) = text.rsplit_once(':')?;
    if let Some(column) = number(last) {
        if let Some((url, line_text)) = before_last.rsplit_once(':')
            && let Some(line) = number(line_text)
        {
            return Some((url, line, Some(column)));
        }
        return Some((before_last, column, None));
    }
    None
}

/// 从 `close` 位置向前找配对的左括号（按深度计数，正确处理嵌套）
fn matching_open(bytes: &[u8], close: usize, open_ch: u8, close_ch: u8) -> Option<usize> {
    let mut depth = 0usize;
    for index in (0..close).rev() {
        let byte = bytes[index];
        if byte == close_ch {
            depth += 1;
        } else if byte == open_ch {
            if depth == 0 {
                return Some(index);
            }
            depth -= 1;
        }
    }
    None
}

fn trim_function(text: &str) -> Option<String> {
    let text = text.trim();
    let text = text.strip_prefix("at ").unwrap_or(text).trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn frame(raw: &str) -> Frame {
    let mut out = Frame {
        raw: raw.to_owned(),
        is_frame: false,
        function: None,
        url: None,
        line: None,
        column: None,
    };
    // at 函数 (url:行:列) / at (url:行:列) / at [url:行:列]。
    // eval 嵌套时取最外层括号，列取行内最后一个 url:行:列
    if let Some(close) = raw.rfind([')', ']']) {
        let (open_ch, close_ch) = if raw.as_bytes()[close] == b')' {
            (b'(', b')')
        } else {
            (b'[', b']')
        };
        let open = matching_open(raw.as_bytes(), close, open_ch, close_ch);
        if let Some(open) = open
            && let Some((mut url, line, column)) = extract_position(&raw[open + 1..close])
        {
            // eval 嵌套时 url 会带上内层括号文本，截到最内层的标识
            if url.contains(['(', ')'])
                && let Some(pos) = url.rfind([' ', '('])
            {
                url = &url[pos + 1..];
            }
            out.is_frame = true;
            out.function = trim_function(&raw[..open]);
            out.url = Some(url.to_owned());
            out.line = Some(line);
            out.column = column;
            return out;
        }
    }
    // Firefox：函数@url:行:列
    if let Some(at) = raw.rfind('@')
        && let Some((url, line, column)) = extract_position(&raw[at + 1..])
    {
        out.is_frame = true;
        out.function = trim_function(&raw[..at]);
        out.url = Some(url.to_owned());
        out.line = Some(line);
        out.column = column;
        return out;
    }
    // 整行就是 url:行:列
    if let Some((url, line, column)) = extract_position(raw) {
        out.is_frame = true;
        out.url = Some(url.to_owned());
        out.line = Some(line);
        out.column = column;
    }
    out
}

/// 逐行解析堆栈文本；超过 `max_lines` 行后停止并标记截断
pub fn parse(text: &str, max_lines: usize) -> Stack {
    let mut frames = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if frames.len() >= max_lines {
            return Stack {
                frames,
                truncated: true,
            };
        }
        frames.push(frame(trimmed));
    }
    Stack {
        frames,
        truncated: false,
    }
}

#[cfg(test)]
#[path = "stack_test.rs"]
mod tests;
