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

/// 内置常量（不可被赋值覆盖）
const CONSTANTS: &[(&str, f64)] = &[
    ("pi", std::f64::consts::PI),
    ("tau", std::f64::consts::TAU),
    ("e", std::f64::consts::E),
    ("phi", std::f64::consts::GOLDEN_RATIO),
];

/// 期望的参数个数；None 表示可变参数（min/max 至少 1 个）
const FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "sqrt", "cbrt", "ln", "log10", "log2", "exp",
    "abs", "round", "floor", "ceil", "fact", "deg", "rad", "min", "max",
];

fn is_known_name(name: &str) -> bool {
    CONSTANTS.iter().any(|(n, _)| *n == name) || FUNCTIONS.contains(&name) || name == PREV
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 多行表达式；空行与 # 开头的注释行跳过
    pub input: String,
    /// 三角函数的角度模式：deg（默认）或 rad
    #[serde(default)]
    pub angle_mode: Option<String>,
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
            '+' | '-' | '*' | '/' | '^' | '%' | '=' | '!' => {
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
    /// 三角函数的角度模式（true = deg）
    angle_deg: bool,
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
        let (mut value, mut dim) = self.primary()?;
        // 阶乘后缀：5!（非负整数，上限 170 防溢出）
        while let Some(Token::Op('!')) = self.peek() {
            if value < 0.0 || value.fract() != 0.0 || value > 170.0 {
                return Err(PluginError::new("eval.bad_factorial"));
            }
            self.pos += 1;
            let mut acc = 1.0;
            for n in 2..=(value as u64) {
                acc *= n as f64;
            }
            value = acc;
            dim = Dim::None;
        }
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

    /// 调用科学函数；Ident 已被消费，pos 指向参数列表
    fn call_function(&mut self, name: &str) -> PluginResult<(f64, Dim)> {
        if !matches!(self.next(), Some(Token::LParen)) {
            return Err(PluginError::new("eval.expected_lparen"));
        }
        let mut args = Vec::new();
        if matches!(self.peek(), Some(Token::RParen)) {
            self.pos += 1;
        } else {
            loop {
                let (value, _) = self.expr()?;
                args.push(value);
                match self.next() {
                    Some(Token::Comma) => continue,
                    Some(Token::RParen) => break,
                    _ => return Err(PluginError::new("eval.expected_rparen")),
                }
            }
        }
        let to_rad = |x: f64| if self.angle_deg { x.to_radians() } else { x };
        let from_rad = |x: f64| if self.angle_deg { x.to_degrees() } else { x };
        let unary = |x: f64| -> PluginResult<f64> {
            let out = match name {
                "sin" => to_rad(x).sin(),
                "cos" => to_rad(x).cos(),
                "tan" => to_rad(x).tan(),
                "asin" => from_rad(x.asin()),
                "acos" => from_rad(x.acos()),
                "atan" => from_rad(x.atan()),
                "sqrt" => x.sqrt(),
                "cbrt" => x.cbrt(),
                "ln" => x.ln(),
                "log10" => x.log10(),
                "log2" => x.log2(),
                "exp" => x.exp(),
                "abs" => x.abs(),
                "round" => x.round(),
                "floor" => x.floor(),
                "ceil" => x.ceil(),
                "deg" => x.to_degrees(),
                "rad" => x.to_radians(),
                "fact" => {
                    if x < 0.0 || x.fract() != 0.0 || x > 170.0 {
                        return Err(PluginError::new("eval.bad_factorial"));
                    }
                    (2..=(x as u64)).fold(1.0, |acc, n| acc * n as f64)
                }
                _ => return Err(PluginError::new("eval.unknown_function")),
            };
            if !out.is_finite() {
                return Err(PluginError::new("eval.not_finite"));
            }
            Ok(out)
        };
        let value = match name {
            "min" | "max" => {
                let pick = args
                    .iter()
                    .cloned()
                    .reduce(|a, b| if name == "min" { a.min(b) } else { a.max(b) });
                pick.ok_or_else(|| PluginError::new("eval.missing_args"))?
            }
            _ => {
                if args.len() != 1 {
                    return Err(PluginError::new("eval.bad_args"));
                }
                unary(args[0])?
            }
        };
        Ok((value, Dim::None))
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
                if let Some(value) = self.vars.get(&name) {
                    return Ok((*value, Dim::None));
                }
                if let Some((_, value)) = CONSTANTS.iter().find(|(n, _)| *n == name) {
                    return Ok((*value, Dim::None));
                }
                if FUNCTIONS.contains(&name.as_str()) {
                    return self.call_function(&name);
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
    angle_deg: bool,
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
            angle_deg,
        };
        let (value, _) = parser.expr()?;
        if parser.pos != parser.tokens.len() {
            return Err(PluginError::new("eval.trailing_input"));
        }
        if !isfinite(value) {
            return Err(PluginError::new("eval.not_finite"));
        }
        if is_known_name(&name) {
            return Err(PluginError::new("eval.reserved_name").with("name", name));
        }
        vars.insert(name.clone(), value);
        return Ok((Some(name), value));
    }
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        vars,
        angle_deg,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub count: u64,
    pub sum: f64,
    pub mean: f64,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    pub variance: f64,
    pub sample_variance: f64,
    pub stddev: f64,
    pub sample_stddev: f64,
}

/// 多行数值列表的统计摘要：每行先用同一求值器算出数值（支持表达式/单位），
/// 无一行成功时返回错误
pub fn statistics(input: &str, angle_mode: Option<&str>) -> PluginResult<Stats> {
    let output = evaluate(input, angle_mode)?;
    let mut values: Vec<f64> = output.lines.iter().filter_map(|line| line.value).collect();
    if values.is_empty() {
        return Err(PluginError::new("stat.no_numbers"));
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let count = values.len() as f64;
    let sum: f64 = values.iter().sum();
    let mean = sum / count;
    let median = if values.len() % 2 == 1 {
        values[values.len() / 2]
    } else {
        (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2.0
    };
    let ss: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
    let variance = ss / count;
    let sample_variance = if values.len() > 1 {
        ss / (count - 1.0)
    } else {
        variance
    };
    Ok(Stats {
        count: values.len() as u64,
        sum,
        mean,
        median,
        min: values[0],
        max: values[values.len() - 1],
        variance,
        sample_variance,
        stddev: variance.sqrt(),
        sample_stddev: sample_variance.sqrt(),
    })
}

pub fn evaluate(input: &str, angle_mode: Option<&str>) -> PluginResult<Output> {
    let angle_deg = !matches!(angle_mode, Some("rad"));
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
        match eval_line(raw, &mut vars, angle_deg) {
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
