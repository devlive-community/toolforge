//! 回溯风险分析：静态扫描已知的 ReDoS 模式，并用计步的回溯模拟器做动态基准。
//!
//! 本工具的引擎在不使用反向引用、环视等特性时是线性时间的，测不出回溯问题；
//! 模拟器按 JavaScript / Java / Python / PCRE 等回溯引擎的方式执行，用于评估移植后的风险。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use fancy_regex::{Assertion, Expr, LookAround, Regex};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::engine::Flags;

/// 用来近似字符集合的探针字符（不超过 128 个，集合用 u128 位图表示）
const PROBES: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 \t\n\r!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~é中я😀\u{a0}";
const STEP_BUDGET: u64 = 1_000_000;
const TIME_BUDGET: Duration = Duration::from_secs(2);
const MAX_SAMPLE_CHARS: usize = 2000;
const ATTACK_LENGTHS: [usize; 3] = [10, 20, 40];
const SIM_STACK: usize = 128 * 1024 * 1024;
/// 上限不小于这个值的量词视为「无界」
const UNBOUNDED: usize = 16;

type Set = u128;

fn probes() -> Vec<char> {
    PROBES.chars().collect()
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    NestedQuantifier,
    OverlappingAlternation,
    AdjacentQuantifiers,
}

#[derive(Debug, Serialize)]
pub struct Finding {
    pub kind: Kind,
    pub severity: Severity,
    /// 有问题的片段（由语法树还原）
    pub snippet: String,
    /// 能触发大量回溯的示例输入
    pub attack: String,
    /// 攻击串的组成：前缀、重复单元、结尾使匹配失败的字符
    #[serde(skip)]
    parts: (String, char, char),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub length: usize,
    pub steps: u64,
    pub exceeded: bool,
    pub matched: bool,
    pub elapsed_ms: f64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Growth {
    Linear,
    Polynomial,
    Exponential,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// 本工具执行该表达式是否需要回溯引擎（使用了反向引用、环视等）
    pub backtracking_engine: bool,
    pub findings: Vec<Finding>,
    /// 模拟器不支持该表达式中的某些特性时为 false
    pub simulated: bool,
    pub sample: Option<Run>,
    pub sample_truncated: bool,
    pub attacks: Vec<Run>,
    pub growth: Option<Growth>,
    pub step_budget: u64,
}

#[derive(Deserialize)]
pub struct Args {
    pub pattern: String,
    #[serde(default)]
    pub flags: Flags,
    #[serde(default)]
    pub input: String,
}

/// 字符集合与可空性的近似
struct Sets {
    probes: Vec<char>,
    cache: HashMap<(String, bool), Set>,
}

impl Sets {
    fn new() -> Self {
        Self {
            probes: probes(),
            cache: HashMap::new(),
        }
    }

    fn all(&self) -> Set {
        (1u128 << self.probes.len()) - 1
    }

    fn of_char(&self, c: char, casei: bool) -> Set {
        let mut set = 0;
        for (index, probe) in self.probes.iter().enumerate() {
            let same = if casei {
                probe.to_lowercase().eq(c.to_lowercase())
            } else {
                *probe == c
            };
            if same {
                set |= 1 << index;
            }
        }
        set
    }

    fn delegate(&mut self, inner: &str, casei: bool) -> Set {
        if let Some(set) = self.cache.get(&(inner.to_owned(), casei)) {
            return *set;
        }
        let pattern = format!("^{}(?:{inner})$", if casei { "(?i)" } else { "" });
        let set = match Regex::new(&pattern) {
            Ok(re) => self
                .probes
                .iter()
                .enumerate()
                .filter(|(_, c)| re.is_match(&c.to_string()).unwrap_or(false))
                .fold(0, |set, (index, _)| set | (1 << index)),
            Err(_) => self.all(),
        };
        self.cache.insert((inner.to_owned(), casei), set);
        set
    }

    fn pick(&self, set: Set) -> Option<char> {
        // 优先选字母数字，示例更易读
        let preferred = (0..self.probes.len())
            .find(|i| set & (1 << i) != 0 && self.probes[*i].is_ascii_alphanumeric());
        preferred
            .or_else(|| (0..self.probes.len()).find(|i| set & (1 << i) != 0))
            .map(|i| self.probes[i])
    }

    /// 可能出现在匹配结果中的字符
    fn chars(&mut self, e: &Expr) -> Set {
        match e {
            Expr::Any { newline, .. } => {
                if *newline {
                    self.all()
                } else {
                    self.all() & !self.of_char('\n', false)
                }
            }
            Expr::Literal { val, casei } => val.chars().fold(0, |s, c| s | self.of_char(c, *casei)),
            Expr::Delegate { inner, casei } => self.delegate(inner, *casei),
            Expr::Backref { .. } | Expr::BackrefWithRelativeRecursionLevel { .. } => self.all(),
            Expr::LookAround(..) | Expr::Assertion(_) | Expr::Empty => 0,
            Expr::Concat(items) | Expr::Alt(items) => {
                items.iter().fold(0, |s, i| s | self.chars(i))
            }
            Expr::Group(child) => self.chars(child),
            Expr::Repeat { child, hi, .. } => {
                if *hi == 0 {
                    0
                } else {
                    self.chars(child)
                }
            }
            Expr::AtomicGroup(child) => self.chars(child),
            _ => e.children_iter().fold(0, |s, c| s | self.chars(c)),
        }
    }

    /// 匹配结果的首字符
    fn first(&mut self, e: &Expr) -> Set {
        match e {
            Expr::Literal { val, casei } => {
                val.chars().next().map_or(0, |c| self.of_char(c, *casei))
            }
            Expr::Concat(items) => {
                let mut set = 0;
                for item in items {
                    set |= self.first(item);
                    if !nullable(item) {
                        break;
                    }
                }
                set
            }
            Expr::Alt(items) => items.iter().fold(0, |s, i| s | self.first(i)),
            Expr::Group(child) => self.first(child),
            Expr::AtomicGroup(child) => self.first(child),
            Expr::Repeat { child, hi, .. } => {
                if *hi == 0 {
                    0
                } else {
                    self.first(child)
                }
            }
            other => self.chars(other),
        }
    }
}

fn nullable(e: &Expr) -> bool {
    match e {
        Expr::Empty | Expr::Assertion(_) | Expr::LookAround(..) => true,
        Expr::Backref { .. } | Expr::BackrefWithRelativeRecursionLevel { .. } => true,
        Expr::Literal { val, .. } => val.is_empty(),
        Expr::Any { .. } | Expr::Delegate { .. } => false,
        Expr::Concat(items) => items.iter().all(nullable),
        Expr::Alt(items) => items.iter().any(nullable),
        Expr::Group(child) => nullable(child),
        Expr::AtomicGroup(child) => nullable(child),
        Expr::Repeat { child, lo, .. } => *lo == 0 || nullable(child),
        _ => true,
    }
}

/// 去掉外层分组
fn unwrap(e: &Expr) -> &Expr {
    match e {
        Expr::Group(child) => unwrap(child),
        other => other,
    }
}

fn parts(e: &Expr) -> Vec<&Expr> {
    match unwrap(e) {
        Expr::Concat(items) => items.iter().collect(),
        other => vec![other],
    }
}

fn needs_backtracking(e: &Expr) -> bool {
    match e {
        Expr::Backref { .. }
        | Expr::BackrefWithRelativeRecursionLevel { .. }
        | Expr::LookAround(..)
        | Expr::AtomicGroup(_)
        | Expr::Conditional { .. }
        | Expr::BackrefExistsCondition { .. }
        | Expr::SubroutineCall(_)
        | Expr::KeepOut
        | Expr::ContinueFromPreviousMatchEnd
        | Expr::BacktrackingControlVerb(_)
        | Expr::Absent(_) => true,
        _ => e.children_iter().any(needs_backtracking),
    }
}

fn quote(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// 由语法树还原的可读片段（不保证与原文逐字一致）
pub fn render(e: &Expr) -> String {
    match e {
        Expr::Empty => String::new(),
        Expr::Any { newline, .. } => if *newline { "(?s:.)" } else { "." }.to_owned(),
        Expr::Literal { val, .. } => quote(val),
        Expr::Delegate { inner, .. } => inner.clone(),
        Expr::Concat(items) => items.iter().map(render).collect(),
        Expr::Alt(items) => items.iter().map(render).collect::<Vec<_>>().join("|"),
        Expr::Group(child) => format!("({})", render(child)),
        Expr::AtomicGroup(child) => format!("(?>{})", render(child)),
        Expr::LookAround(child, kind) => {
            let prefix = match kind {
                LookAround::LookAhead => "?=",
                LookAround::LookAheadNeg => "?!",
                LookAround::LookBehind => "?<=",
                LookAround::LookBehindNeg => "?<!",
            };
            format!("({prefix}{})", render(child))
        }
        Expr::Repeat {
            child,
            lo,
            hi,
            greedy,
        } => {
            let inner = render(child);
            let atom = match child.as_ref() {
                Expr::Literal { val, .. } if val.chars().count() > 1 => format!("(?:{inner})"),
                Expr::Concat(_) | Expr::Alt(_) => format!("(?:{inner})"),
                _ => inner,
            };
            let quantifier = match (*lo, *hi) {
                (0, usize::MAX) => "*".to_owned(),
                (1, usize::MAX) => "+".to_owned(),
                (0, 1) => "?".to_owned(),
                (lo, usize::MAX) => format!("{{{lo},}}"),
                (lo, hi) if lo == hi => format!("{{{lo}}}"),
                (lo, hi) => format!("{{{lo},{hi}}}"),
            };
            format!("{atom}{quantifier}{}", if *greedy { "" } else { "?" })
        }
        Expr::Assertion(a) => match a {
            Assertion::StartText => "^",
            Assertion::EndText => "$",
            Assertion::WordBoundary => "\\b",
            Assertion::NotWordBoundary => "\\B",
            _ => "",
        }
        .to_owned(),
        Expr::Backref { group, .. } => format!("\\{group}"),
        _ => "…".to_owned(),
    }
}

/// 生成能被表达式匹配的最短示例
fn example(e: &Expr, sets: &mut Sets) -> String {
    match e {
        Expr::Literal { val, .. } => val.clone(),
        Expr::Any { .. } => "a".into(),
        Expr::Delegate { inner, casei } => {
            let set = sets.delegate(inner, *casei);
            sets.pick(set).map(String::from).unwrap_or_default()
        }
        Expr::Concat(items) => items.iter().map(|i| example(i, sets)).collect(),
        Expr::Alt(items) => items.first().map(|i| example(i, sets)).unwrap_or_default(),
        Expr::Group(child) => example(child, sets),
        Expr::AtomicGroup(child) => example(child, sets),
        Expr::Repeat { child, lo, .. } => example(child, sets).repeat(*lo),
        _ => String::new(),
    }
}

fn attack_text((prefix, unit, killer): &(String, char, char), length: usize) -> String {
    format!("{prefix}{}{killer}", unit.to_string().repeat(length))
}

struct Scanner<'a> {
    sets: &'a mut Sets,
    findings: Vec<Finding>,
    /// 当前节点之前必须先匹配的文本（各层 Concat 中前面的兄弟节点）
    prefix: Vec<String>,
}

impl Scanner<'_> {
    fn push(&mut self, kind: Kind, severity: Severity, node: &Expr, set: Set) {
        let unit = self.sets.pick(set).unwrap_or('a');
        // 结尾放一个不在集合中的字符，迫使匹配失败后回溯
        let killer = ['!', '#', '\n', ' ', '-', 'X', '0']
            .into_iter()
            .find(|c| self.sets.of_char(*c, false) & set == 0)
            .unwrap_or('!');
        let parts = (self.prefix.concat(), unit, killer);
        self.findings.push(Finding {
            kind,
            severity,
            snippet: render(node),
            attack: attack_text(&parts, 24),
            parts,
        });
    }

    fn scan(&mut self, e: &Expr) {
        match e {
            Expr::Repeat { child, hi, .. } if *hi >= UNBOUNDED => {
                self.check_repeat(e, child);
                self.scan(child);
            }
            Expr::Concat(items) => {
                self.check_adjacent(items);
                let depth = self.prefix.len();
                for item in items {
                    self.scan(item);
                    let text = example(item, self.sets);
                    self.prefix.push(text);
                }
                self.prefix.truncate(depth);
            }
            _ => {
                for child in e.children_iter() {
                    self.scan(child);
                }
            }
        }
    }

    fn check_repeat(&mut self, node: &Expr, child: &Expr) {
        let body = parts(child);
        // 嵌套量词：重复体中有无界量词，且其余必须出现的部分都无法充当分隔符
        for (index, part) in body.iter().enumerate() {
            let Expr::Repeat {
                child: inner, hi, ..
            } = unwrap(part)
            else {
                continue;
            };
            if *hi < UNBOUNDED {
                continue;
            }
            let set = self.sets.chars(inner);
            let separated = body.iter().enumerate().any(|(other, sibling)| {
                other != index && !nullable(sibling) && self.sets.chars(sibling) & set == 0
            });
            if !separated && set != 0 {
                self.push(Kind::NestedQuantifier, Severity::High, node, set);
                return;
            }
        }
        // 重叠分支：重复的分支之间首字符可能相同
        if let Expr::Alt(branches) = unwrap(child) {
            let firsts: Vec<Set> = branches.iter().map(|b| self.sets.first(b)).collect();
            let chars: Vec<Set> = branches.iter().map(|b| self.sets.chars(b)).collect();
            for i in 0..branches.len() {
                for j in i + 1..branches.len() {
                    let overlap = firsts[i] & firsts[j];
                    if overlap == 0 {
                        continue;
                    }
                    let contained = chars[i] & !chars[j] == 0 || chars[j] & !chars[i] == 0;
                    let severity = if contained {
                        Severity::High
                    } else {
                        Severity::Medium
                    };
                    self.push(Kind::OverlappingAlternation, severity, node, overlap);
                    return;
                }
            }
        }
    }

    fn check_adjacent(&mut self, items: &[Expr]) {
        for (index, item) in items.iter().enumerate() {
            let Expr::Repeat { child, hi, .. } = unwrap(item) else {
                continue;
            };
            if *hi < UNBOUNDED {
                continue;
            }
            let set = self.sets.chars(child);
            for next in &items[index + 1..] {
                if let Expr::Repeat {
                    child: other, hi, ..
                } = unwrap(next)
                    && *hi >= UNBOUNDED
                {
                    let overlap = set & self.sets.chars(other);
                    if overlap != 0 {
                        let pair = Expr::Concat(vec![item.clone(), next.clone()]);
                        self.push(Kind::AdjacentQuantifiers, Severity::Medium, &pair, overlap);
                        return;
                    }
                }
                if !nullable(next) {
                    break;
                }
            }
        }
    }
}

/// 计步的回溯模拟器
struct Sim {
    text: Vec<char>,
    steps: u64,
    budget: u64,
    deadline: Instant,
    aborted: bool,
    unsupported: bool,
    groups: HashMap<*const Expr, usize>,
    captures: Vec<Option<(usize, usize)>>,
    char_memo: HashMap<(*const Expr, char), bool>,
    delegates: HashMap<(String, bool), Option<Regex>>,
}

type Cont<'k> = &'k mut dyn FnMut(&mut Sim, usize) -> bool;

fn number_groups(e: &Expr, map: &mut HashMap<*const Expr, usize>) {
    if let Expr::Group(child) = e {
        let next = map.len() + 1;
        map.insert(e as *const Expr, next);
        number_groups(child, map);
        return;
    }
    for child in e.children_iter() {
        number_groups(child, map);
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Sim {
    fn tick(&mut self) -> bool {
        self.steps += 1;
        if self.steps > self.budget
            || (self.steps.is_multiple_of(4096) && Instant::now() > self.deadline)
        {
            self.aborted = true;
        }
        !self.aborted
    }

    fn char_matches(&mut self, e: &Expr, c: char) -> bool {
        let key = (e as *const Expr, c);
        if let Some(hit) = self.char_memo.get(&key) {
            return *hit;
        }
        let hit = match e {
            Expr::Any { newline, .. } => *newline || c != '\n',
            Expr::Delegate { inner, casei } => self
                .delegates
                .entry((inner.clone(), *casei))
                .or_insert_with(|| {
                    Regex::new(&format!(
                        "^{}(?:{inner})$",
                        if *casei { "(?i)" } else { "" }
                    ))
                    .ok()
                })
                .as_ref()
                .and_then(|re| re.is_match(&c.to_string()).ok())
                .unwrap_or(false),
            _ => false,
        };
        self.char_memo.insert(key, hit);
        hit
    }

    fn assertion(&self, a: &Assertion, pos: usize) -> bool {
        let before = pos.checked_sub(1).map(|i| self.text[i]);
        let after = self.text.get(pos).copied();
        match a {
            Assertion::StartText => pos == 0,
            Assertion::EndText => pos == self.text.len(),
            Assertion::StartLine { .. } | Assertion::StartLineOniguruma { .. } => {
                pos == 0 || before == Some('\n')
            }
            Assertion::EndLine { .. } => pos == self.text.len() || after == Some('\n'),
            Assertion::WordBoundary => before.is_some_and(is_word) != after.is_some_and(is_word),
            Assertion::NotWordBoundary => before.is_some_and(is_word) == after.is_some_and(is_word),
            _ => true,
        }
    }

    fn m(&mut self, e: &Expr, pos: usize, k: Cont<'_>) -> bool {
        if !self.tick() {
            return false;
        }
        match e {
            Expr::Empty => k(self, pos),
            Expr::Literal { val, casei } => {
                let mut p = pos;
                for c in val.chars() {
                    let Some(&t) = self.text.get(p) else {
                        return false;
                    };
                    let same = if *casei {
                        t.to_lowercase().eq(c.to_lowercase())
                    } else {
                        t == c
                    };
                    if !same {
                        return false;
                    }
                    p += 1;
                }
                k(self, p)
            }
            Expr::Any { .. } | Expr::Delegate { .. } => match self.text.get(pos).copied() {
                Some(c) if self.char_matches(e, c) => k(self, pos + 1),
                _ => false,
            },
            Expr::Assertion(a) => self.assertion(a, pos) && k(self, pos),
            Expr::Concat(items) => self.seq(items, 0, pos, k),
            Expr::Alt(branches) => {
                for branch in branches {
                    if self.m(branch, pos, k) {
                        return true;
                    }
                    if self.aborted {
                        return false;
                    }
                }
                false
            }
            Expr::Group(child) => {
                let index = self.groups.get(&(e as *const Expr)).copied().unwrap_or(0);
                if self.captures.len() <= index {
                    self.captures.resize(index + 1, None);
                }
                let saved = self.captures[index];
                let matched = self.m(child, pos, &mut |s: &mut Sim, end| {
                    let previous = s.captures[index];
                    s.captures[index] = Some((pos, end));
                    if k(s, end) {
                        return true;
                    }
                    s.captures[index] = previous;
                    false
                });
                if !matched {
                    self.captures[index] = saved;
                }
                matched
            }
            Expr::Repeat {
                child,
                lo,
                hi,
                greedy,
            } => self.rep(child, *lo, *hi, *greedy, 0, pos, k),
            Expr::Backref { group, casei } => {
                let Some(Some((start, end))) = self.captures.get(*group).copied() else {
                    return false;
                };
                let len = end - start;
                if pos + len > self.text.len() {
                    return false;
                }
                let same = (0..len).all(|i| {
                    let (a, b) = (self.text[start + i], self.text[pos + i]);
                    if *casei {
                        a.to_lowercase().eq(b.to_lowercase())
                    } else {
                        a == b
                    }
                });
                same && k(self, pos + len)
            }
            Expr::LookAround(child, kind) => {
                let found = match kind {
                    LookAround::LookAhead | LookAround::LookAheadNeg => {
                        self.m(child, pos, &mut |_, _| true)
                    }
                    LookAround::LookBehind | LookAround::LookBehindNeg => (0..=pos)
                        .rev()
                        .any(|start| self.m(child, start, &mut |_, end| end == pos)),
                };
                if self.aborted {
                    return false;
                }
                let positive = matches!(kind, LookAround::LookAhead | LookAround::LookBehind);
                found == positive && k(self, pos)
            }
            Expr::AtomicGroup(child) => {
                let mut end = None;
                self.m(child, pos, &mut |_, p| {
                    end = Some(p);
                    true
                });
                match end {
                    Some(p) => k(self, p),
                    None => false,
                }
            }
            _ => {
                self.unsupported = true;
                self.aborted = true;
                false
            }
        }
    }

    fn seq(&mut self, items: &[Expr], index: usize, pos: usize, k: Cont<'_>) -> bool {
        match items.get(index) {
            None => k(self, pos),
            Some(item) => self.m(item, pos, &mut |s: &mut Sim, p| {
                s.seq(items, index + 1, p, k)
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(
        &mut self,
        child: &Expr,
        lo: usize,
        hi: usize,
        greedy: bool,
        count: usize,
        pos: usize,
        k: Cont<'_>,
    ) -> bool {
        if !self.tick() {
            return false;
        }
        let more = |s: &mut Sim, k: Cont<'_>| {
            count < hi
                && s.m(child, pos, &mut |s: &mut Sim, p| {
                    // 零宽迭代不再继续，避免死循环
                    (p != pos || count < lo) && s.rep(child, lo, hi, greedy, count + 1, p, k)
                })
        };
        if greedy {
            if more(self, k) {
                return true;
            }
            !self.aborted && count >= lo && k(self, pos)
        } else {
            if count >= lo && k(self, pos) {
                return true;
            }
            !self.aborted && more(self, k)
        }
    }
}

fn simulate(expr: &Expr, text: &str) -> (Run, bool) {
    let start = Instant::now();
    let mut groups = HashMap::new();
    number_groups(expr, &mut groups);
    let mut sim = Sim {
        text: text.chars().collect(),
        steps: 0,
        budget: STEP_BUDGET,
        deadline: start + TIME_BUDGET,
        aborted: false,
        unsupported: false,
        groups,
        captures: Vec::new(),
        char_memo: HashMap::new(),
        delegates: HashMap::new(),
    };
    let mut matched = false;
    for begin in 0..=sim.text.len() {
        sim.captures.clear();
        if sim.m(expr, begin, &mut |_, _| true) {
            matched = true;
            break;
        }
        if sim.aborted {
            break;
        }
    }
    let run = Run {
        length: sim.text.len(),
        steps: sim.steps,
        exceeded: sim.aborted && !sim.unsupported,
        matched,
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    };
    (run, sim.unsupported)
}

fn classify(runs: &[Run]) -> Option<Growth> {
    let [first, second, third] = runs else {
        return None;
    };
    if first.exceeded || second.exceeded || third.exceeded {
        return Some(Growth::Exponential);
    }
    // 长度翻倍时的步数比：线性约 2，平方约 4
    let ratio = third.steps as f64 / second.steps.max(1) as f64;
    Some(if ratio > 12.0 {
        Growth::Exponential
    } else if ratio > 3.0 {
        Growth::Polynomial
    } else {
        Growth::Linear
    })
}

fn with_flags(pattern: &str, flags: Flags) -> String {
    let mut inline = String::new();
    for (on, letter) in [
        (flags.case_insensitive, 'i'),
        (flags.multi_line, 'm'),
        (flags.dot_all, 's'),
        (flags.extended, 'x'),
    ] {
        if on {
            inline.push(letter);
        }
    }
    if inline.is_empty() {
        pattern.to_owned()
    } else {
        format!("(?{inline}){pattern}")
    }
}

fn analyze_inner(args: Args) -> PluginResult<Report> {
    if args.pattern.is_empty() {
        return Err(PluginError::new("regex.empty"));
    }
    let pattern = with_flags(&args.pattern, args.flags);
    let tree = Expr::parse_tree(&pattern)
        .map_err(|e| PluginError::new("regex.invalid").with("detail", e.to_string()))?;
    let expr = tree.expr;
    let mut sets = Sets::new();
    let mut scanner = Scanner {
        sets: &mut sets,
        findings: Vec::new(),
        prefix: Vec::new(),
    };
    scanner.scan(&expr);
    let mut findings = scanner.findings;
    findings.sort_by_key(|f| std::cmp::Reverse(f.severity));

    let sample_truncated = args.input.chars().count() > MAX_SAMPLE_CHARS;
    let sample_text: String = args.input.chars().take(MAX_SAMPLE_CHARS).collect();
    let (sample, unsupported) = if sample_text.is_empty() {
        (None, false)
    } else {
        let (run, unsupported) = simulate(&expr, &sample_text);
        (Some(run), unsupported)
    };

    let mut attacks = Vec::new();
    if !unsupported && let Some(worst) = findings.first() {
        // 用最严重问题的攻击串按不同长度重放，观察步数增长
        for length in ATTACK_LENGTHS {
            let text = attack_text(&worst.parts, length);
            let (mut run, _) = simulate(&expr, &text);
            run.length = length;
            let exceeded = run.exceeded;
            attacks.push(run);
            if exceeded {
                break;
            }
        }
        while attacks.len() < ATTACK_LENGTHS.len() && attacks.last().is_some_and(|r| r.exceeded) {
            let length = ATTACK_LENGTHS[attacks.len()];
            attacks.push(Run {
                length,
                steps: STEP_BUDGET,
                exceeded: true,
                matched: false,
                elapsed_ms: 0.0,
            });
        }
    }
    let growth = classify(&attacks);

    Ok(Report {
        backtracking_engine: needs_backtracking(&expr),
        findings,
        simulated: !unsupported,
        sample,
        sample_truncated,
        attacks,
        growth,
        step_budget: STEP_BUDGET,
    })
}

/// 在大栈线程中分析，避免深度回溯递归撑爆调用线程的栈
pub fn analyze(args: Args) -> PluginResult<Report> {
    std::thread::Builder::new()
        .stack_size(SIM_STACK)
        .spawn(move || analyze_inner(args))
        .map_err(|e| PluginError::new("regex.runtime").with("detail", e.to_string()))?
        .join()
        .map_err(|_| PluginError::new("regex.runtime"))?
}

#[cfg(test)]
#[path = "redos_test.rs"]
mod tests;
