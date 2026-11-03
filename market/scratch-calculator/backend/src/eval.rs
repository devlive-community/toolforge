//! 逐行求值器：四则与幂、括号、百分比与取模、常见长度/质量/时间单位、
//! 变量赋值与引用、上一行结果（`prev`）。手写递归下降，无第三方依赖。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tf_plugin_api::{PluginError, PluginResult};

/// 同量纲单位换算到基准单位（米 / 克 / 秒）
const UNITS: &[(&str, f64, &str)] = &[
    ("km", 1000.0, "length"),
    ("m", 1.0, "length"),
    ("dm", 0.1, "length"),
    ("cm", 0.01, "length"),
    ("mm", 0.001, "length"),
    ("um", 1e-6, "length"),
    ("nm", 1e-9, "length"),
    ("mi", 1609.344, "length"),
    ("yd", 0.9144, "length"),
    ("ft", 0.3048, "length"),
    ("in", 0.0254, "length"),
    ("t", 1e6, "mass"),
    ("kg", 1000.0, "mass"),
    ("g", 1.0, "mass"),
    ("mg", 0.001, "mass"),
    ("lb", 453.59237, "mass"),
    ("oz", 28.349_523_125, "mass"),
    ("d", 86_400.0, "time"),
    ("h", 3600.0, "time"),
    ("min", 60.0, "time"),
    ("s", 1.0, "time"),
    ("ms", 0.001, "time"),
];

/// 上一行（最近一个产生值而非赋值的行）的引用名
pub const PREV: &str = "prev";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 多行表达式；空行与 # 开头的注释行跳过
    pub input: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub raw: String,
    /// 赋值行的变量名
    pub name: Option<String>,
    pub value: Option<f64>,
    /// 错误码（eval.*），由前端翻译
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub lines: Vec<Line>,
    /// 最后一个产生值的行（含赋值）；全部失败时缺省
    pub last: Option<f64>,
}

#[derive(Debug)]
enum Token {
    Number(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn tokenize(src: &str) -> PluginResult<Vec<Token>> {
    let mut out = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' => {
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut text = String::new();
                while let Some(&n) = chars.peek() {
                    if n.is_ascii_digit() || n == '.' || n == 'e' || n == 'E' {
                        // 1e-3 的负指数属于数字
                        if (n == 'e' || n == 'E')
                            && !text.chars().last().is_some_and(|p| p.is_ascii_digit())
                        {
                            break;
                        }
                        text.push(n);
                        chars.next();
                        if (n == 'e' || n == 'E') && matches!(chars.peek(), Some('-') | Some('+')) {
                            text.push(chars.next().unwrap());
                        }
                    } else {
                        break;
                    }
                }
                let value: f64 = text.parse().map_err(|_| {
                    PluginError::new("eval.invalid_number").with("text", text.clone())
                })?;
                out.push(Token::Number(value));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut text = String::new();
                while let Some(&n) = chars.peek() {
                    if n.is_ascii_alphanumeric() || n == '_' {
                        text.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(Token::Ident(text));
            }
            '+' | '-' | '*' | '/' | '^' | '%' | '=' => {
                out.push(Token::Op(c));
                chars.next();
            }
            '(' => {
                out.push(Token::LParen);
                chars.next();
            }
            ')' => {
                out.push(Token::RParen);
                chars.next();
            }
            ',' => {
                out.push(Token::Comma);
                chars.next();
            }
            other => {
                return Err(
                    PluginError::new("eval.unexpected_char").with("char", other.to_string())
                );
            }
        }
    }
    Ok(out)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    vars: &'a HashMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Dim {
    None,
    Length,
    Mass,
    Time,
}

impl Dim {
    fn combine(self, other: Dim) -> PluginResult<Dim> {
        match (self, other) {
            (Dim::None, d) | (d, Dim::None) => Ok(d),
            (a, b) if a == b => Ok(a),
            _ => Err(PluginError::new("eval.dimension_mismatch")),
        }
    }
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos);
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expr(&mut self) -> PluginResult<(f64, Dim)> {
        let mut left = self.term()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.peek() {
            let op = *op;
            self.pos += 1;
            let right = self.term()?;
            left = (
                if op == '+' {
                    left.0 + right.0
                } else {
                    left.0 - right.0
                },
                left.1.combine(right.1)?,
            );
        }
        Ok(left)
    }

    fn term(&mut self) -> PluginResult<(f64, Dim)> {
        let mut left = self.unary()?;
        while let Some(Token::Op(op @ ('*' | '/' | '%'))) = self.peek() {
            let op = *op;
            self.pos += 1;
            let right = self.unary()?;
            left = (
                match op {
                    '*' => left.0 * right.0,
                    '/' => {
                        if right.0 == 0.0 {
                            return Err(PluginError::new("eval.divide_by_zero"));
                        }
                        left.0 / right.0
                    }
                    _ => {
                        if right.0 == 0.0 {
                            return Err(PluginError::new("eval.divide_by_zero"));
                        }
                        left.0 % right.0
                    }
                },
                // 乘除把单位折算成数值（米/米=无量纲），保留双方非空时的第一个量纲
                match (left.1, right.1) {
                    (Dim::None, d) | (d, Dim::None) => d,
                    _ => Dim::None,
                },
            );
        }
        Ok(left)
    }

    fn unary(&mut self) -> PluginResult<(f64, Dim)> {
        if let Some(Token::Op('-')) = self.peek() {
            self.pos += 1;
            let (value, dim) = self.unary()?;
            return Ok((-value, dim));
        }
        if let Some(Token::Op('+')) = self.peek() {
            self.pos += 1;
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> PluginResult<(f64, Dim)> {
        let base = self.postfix()?;
        if let Some(Token::Op('^')) = self.peek() {
            self.pos += 1;
            // 幂右结合：2^3^2 = 2^9
            let (exponent, dim) = self.unary()?;
            if dim != Dim::None {
                return Err(PluginError::new("eval.dimension_mismatch"));
            }
            return Ok((base.0.powf(exponent), base.1));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> PluginResult<(f64, Dim)> {
        let (value, dim) = self.primary()?;
        if let Some(Token::Op('%')) = self.peek() {
            // % 后面没有操作数时是百分比后缀（50% = 0.5）；
            // 后面跟操作数则是中缀取模（10 % 3），留给 term 循环处理
            let next_is_operand = matches!(
                self.tokens.get(self.pos + 1),
                Some(Token::Number(_) | Token::Ident(_) | Token::LParen)
            );
            if !next_is_operand {
                self.pos += 1;
                return Ok((value / 100.0, Dim::None));
            }
        }
        Ok((value, dim))
    }

    fn primary(&mut self) -> PluginResult<(f64, Dim)> {
        match self.next() {
            Some(Token::Number(value)) => {
                let mut value = *value;
                let mut dim = Dim::None;
                // 数字后紧跟的标识符作为单位（1km）；未知单位报错
                if let Some(Token::Ident(name)) = self.peek()
                    && let Some((_, factor, kind)) = UNITS.iter().find(|(u, _, _)| u == name)
                {
                    value *= factor;
                    dim = match *kind {
                        "length" => Dim::Length,
                        "mass" => Dim::Mass,
                        _ => Dim::Time,
                    };
                    self.pos += 1;
                }
                Ok((value, dim))
            }
            Some(Token::Ident(name)) => {
                let name = name.clone();
                // 常量与变量直接取值；带括号视为函数调用（科学函数在扩展里提供）
                if let Some(value) = self.vars.get(&name) {
                    return Ok((*value, Dim::None));
                }
                Err(PluginError::new("eval.unknown_name").with("name", name))
            }
            Some(Token::LParen) => {
                let (value, dim) = self.expr()?;
                if !matches!(self.next(), Some(Token::RParen)) {
                    return Err(PluginError::new("eval.expected_rparen"));
                }
                Ok((value, dim))
            }
            _ => Err(PluginError::new("eval.unexpected_end")),
        }
    }
}

/// 解析单行：`name = expr` 返回 (Some(name), 值)；纯表达式返回 (None, 值)
pub fn eval_line(
    raw: &str,
    vars: &mut HashMap<String, f64>,
) -> PluginResult<(Option<String>, f64)> {
    let tokens = tokenize(raw)?;
    if tokens.is_empty() {
        return Ok((None, vars.get(PREV).copied().unwrap_or(0.0)));
    }
    // 赋值：标识符 = 表达式（= 必须是第二个 token）
    if matches!(tokens.first(), Some(Token::Ident(_)))
        && matches!(tokens.get(1), Some(Token::Op('=')))
    {
        let Token::Ident(name) = &tokens[0] else {
            unreachable!()
        };
        let name = name.clone();
        let mut parser = Parser {
            tokens: &tokens[2..],
            pos: 0,
            vars,
        };
        let (value, _) = parser.expr()?;
        if parser.pos != parser.tokens.len() {
            return Err(PluginError::new("eval.trailing_input"));
        }
        if !isfinite(value) {
            return Err(PluginError::new("eval.not_finite"));
        }
        vars.insert(name.clone(), value);
        return Ok((Some(name), value));
    }
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        vars,
    };
    let (value, _) = parser.expr()?;
    if parser.pos != parser.tokens.len() {
        return Err(PluginError::new("eval.trailing_input"));
    }
    if !isfinite(value) {
        return Err(PluginError::new("eval.not_finite"));
    }
    Ok((None, value))
}

fn isfinite(value: f64) -> bool {
    value.is_finite()
}

pub fn evaluate(input: &str) -> PluginResult<Output> {
    let mut vars: HashMap<String, f64> = HashMap::new();
    let mut lines = Vec::new();
    let mut last: Option<f64> = None;
    let mut prev: Option<f64> = None;
    for raw in input.lines() {
        let raw = raw.trim();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let mut line = Line {
            raw: raw.to_owned(),
            name: None,
            value: None,
            error: None,
        };
        // prev 指向最近一个「纯表达式」行的值
        if let Some(prev_value) = prev {
            vars.insert(PREV.to_owned(), prev_value);
        }
        match eval_line(raw, &mut vars) {
            Ok((name, value)) => {
                if name.is_none() {
                    prev = Some(value);
                }
                last = Some(value);
                line.value = Some(value);
                line.name = name;
            }
            Err(err) => line.error = Some(err.code),
        }
        lines.push(line);
    }
    if lines.is_empty() {
        return Err(PluginError::new("eval.empty_input"));
    }
    Ok(Output { lines, last })
}

#[cfg(test)]
#[path = "eval_test.rs"]
mod tests;
