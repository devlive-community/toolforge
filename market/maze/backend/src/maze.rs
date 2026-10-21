use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::time::Instant;
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};

pub const ALGORITHM: &str = "dfs-splitmix64-v1";
pub const MAX_STEPS: usize = 50_000;
pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("maze.{code}"))
}
pub fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}
impl Side {
    pub fn bit(self) -> u8 {
        match self {
            Self::Top => 1,
            Self::Right => 2,
            Self::Bottom => 4,
            Self::Left => 8,
        }
    }
    pub fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
        }
    }
}
pub const SIDES: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Port {
    pub side: Side,
    pub index: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Spec {
    pub algorithm: String,
    pub rows: usize,
    pub cols: usize,
    pub entrance: Port,
    pub exit: Port,
    pub seed: String,
}
impl Spec {
    pub fn cell(&self, port: &Port) -> PluginResult<usize> {
        let bound = match port.side {
            Side::Top | Side::Bottom => self.cols,
            _ => self.rows,
        };
        if port.index == 0 || port.index > bound {
            return Err(error("port"));
        }
        let k = port.index - 1;
        Ok(match port.side {
            Side::Top => k,
            Side::Bottom => (self.rows - 1) * self.cols + k,
            Side::Left => k * self.cols,
            Side::Right => k * self.cols + self.cols - 1,
        })
    }
    pub fn validate(&self) -> PluginResult<()> {
        if self.algorithm != ALGORITHM {
            return Err(error("version"));
        }
        if !(2..=40).contains(&self.rows) || !(2..=40).contains(&self.cols) {
            return Err(error("size"));
        }
        if self.seed.is_empty() || self.seed.len() > 80 || self.seed.chars().any(char::is_control) {
            return Err(error("seed"));
        }
        if self.cell(&self.entrance)? == self.cell(&self.exit)? {
            return Err(error("same_port"));
        }
        Ok(())
    }
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(ALGORITHM.as_bytes());
        hash.update([0]);
        hash.update((self.rows as u16).to_le_bytes());
        hash.update((self.cols as u16).to_le_bytes());
        for p in [&self.entrance, &self.exit] {
            hash.update([p.side.bit(), p.index as u8]);
        }
        hash.update(self.seed.as_bytes());
        hash.finalize().into()
    }
    pub fn id(&self) -> String {
        format!(
            "M1-{}",
            self.digest()[..12]
                .iter()
                .map(|x| format!("{x:02X}"))
                .collect::<String>()
        )
    }
    pub fn code(&self) -> String {
        format!(
            "M1.{}.{}.{}.{}.{}.{}.{}",
            self.rows,
            self.cols,
            self.entrance.side.bit(),
            self.entrance.index,
            self.exit.side.bit(),
            self.exit.index,
            self.seed
                .as_bytes()
                .iter()
                .map(|x| format!("{x:02x}"))
                .collect::<String>()
        )
    }
    pub fn from_code(code: &str) -> PluginResult<Self> {
        if code.len() > 256 {
            return Err(error("code"));
        }
        let parts = code.trim().split('.').collect::<Vec<_>>();
        if parts.len() != 8 || parts[0] != "M1" {
            return Err(error("code"));
        }
        let number = |s: &str| s.parse::<usize>().map_err(|_| error("code"));
        let side = |s: &str| match s {
            "1" => Ok(Side::Top),
            "2" => Ok(Side::Right),
            "4" => Ok(Side::Bottom),
            "8" => Ok(Side::Left),
            _ => Err(error("code")),
        };
        let hex = parts[7].as_bytes();
        if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
            return Err(error("code"));
        }
        let mut bytes = Vec::new();
        for pair in hex.as_chunks::<2>().0 {
            bytes.push(
                u8::from_str_radix(std::str::from_utf8(pair).map_err(|_| error("code"))?, 16)
                    .map_err(|_| error("code"))?,
            );
        }
        let spec = Self {
            algorithm: ALGORITHM.into(),
            rows: number(parts[1])?,
            cols: number(parts[2])?,
            entrance: Port {
                side: side(parts[3])?,
                index: number(parts[4])?,
            },
            exit: Port {
                side: side(parts[5])?,
                index: number(parts[6])?,
            },
            seed: String::from_utf8(bytes).map_err(|_| error("code"))?,
        };
        spec.validate()?;
        Ok(spec)
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn pick(&mut self, bound: usize) -> usize {
        let b = bound as u64;
        let threshold = b.wrapping_neg() % b;
        loop {
            let n = self.next();
            if n >= threshold {
                return (n % b) as usize;
            }
        }
    }
}
pub struct Maze {
    pub spec: Spec,
    pub passages: Vec<u8>,
    pub start: usize,
    pub exit: usize,
}
impl Maze {
    pub fn adjacent(&self, cell: usize, side: Side) -> Option<usize> {
        let row = cell / self.spec.cols;
        let col = cell % self.spec.cols;
        match side {
            Side::Top if row > 0 => Some(cell - self.spec.cols),
            Side::Right if col + 1 < self.spec.cols => Some(cell + 1),
            Side::Bottom if row + 1 < self.spec.rows => Some(cell + self.spec.cols),
            Side::Left if col > 0 => Some(cell - 1),
            _ => None,
        }
    }
    pub fn legal(&self, from: usize, to: usize) -> bool {
        from < self.passages.len()
            && to < self.passages.len()
            && SIDES
                .iter()
                .any(|&s| self.passages[from] & s.bit() != 0 && self.adjacent(from, s) == Some(to))
    }
    pub fn route(&self, from: usize, ctx: &dyn TaskContext) -> PluginResult<Vec<usize>> {
        if from >= self.passages.len() {
            return Err(error("state"));
        }
        let mut parents = vec![usize::MAX; self.passages.len()];
        parents[from] = from;
        let mut queue = VecDeque::from([from]);
        while let Some(at) = queue.pop_front() {
            check(ctx)?;
            if at == self.exit {
                break;
            }
            for side in SIDES {
                if self.passages[at] & side.bit() != 0
                    && let Some(next) = self.adjacent(at, side)
                    && parents[next] == usize::MAX
                {
                    parents[next] = at;
                    queue.push_back(next);
                }
            }
        }
        if parents[self.exit] == usize::MAX {
            return Err(error("disconnected"));
        }
        let mut path = vec![self.exit];
        while *path.last().unwrap() != from {
            path.push(parents[*path.last().unwrap()]);
        }
        path.reverse();
        Ok(path)
    }
    /// Independent graph checks, used for every generated puzzle.
    pub fn verify(&self, ctx: &dyn TaskContext) -> PluginResult<()> {
        let n = self.passages.len();
        let mut seen = vec![false; n];
        seen[self.start] = true;
        let mut queue = VecDeque::from([self.start]);
        let mut edges = 0;
        while let Some(at) = queue.pop_front() {
            check(ctx)?;
            for side in SIDES {
                if self.passages[at] & side.bit() == 0 {
                    continue;
                }
                match self.adjacent(at, side) {
                    Some(next) => {
                        if self.passages[next] & side.opposite().bit() == 0 {
                            return Err(error("disconnected"));
                        }
                        edges += 1;
                        if !seen[next] {
                            seen[next] = true;
                            queue.push_back(next);
                        }
                    }
                    None => {
                        let valid = (at == self.start && side == self.spec.entrance.side)
                            || (at == self.exit && side == self.spec.exit.side);
                        if !valid {
                            return Err(error("disconnected"));
                        }
                    }
                }
            }
        }
        if seen.iter().any(|v| !*v) || edges != 2 * (n - 1) {
            return Err(error("disconnected"));
        }
        Ok(())
    }
}

pub fn generate(spec: Spec, ctx: &dyn TaskContext) -> PluginResult<Maze> {
    spec.validate()?;
    let started = Instant::now();
    let start = spec.cell(&spec.entrance)?;
    let exit = spec.cell(&spec.exit)?;
    let mut rng = Rng(u64::from_le_bytes(spec.digest()[..8].try_into().unwrap()));
    let mut maze = Maze {
        passages: vec![0; spec.rows * spec.cols],
        spec,
        start,
        exit,
    };
    let mut visited = vec![false; maze.passages.len()];
    visited[start] = true;
    let mut stack = vec![start];
    while let Some(&at) = stack.last() {
        check(ctx)?;
        if started.elapsed().as_secs_f64() > 2.0 {
            return Err(error("time"));
        }
        let options: Vec<_> = SIDES
            .into_iter()
            .filter_map(|s| {
                maze.adjacent(at, s)
                    .filter(|&n| !visited[n])
                    .map(|n| (s, n))
            })
            .collect();
        if options.is_empty() {
            stack.pop();
            continue;
        }
        let (side, next) = options[rng.pick(options.len())];
        maze.passages[at] |= side.bit();
        maze.passages[next] |= side.opposite().bit();
        visited[next] = true;
        stack.push(next);
    }
    maze.passages[start] |= maze.spec.entrance.side.bit();
    maze.passages[exit] |= maze.spec.exit.side.bit();
    maze.verify(ctx)?;
    Ok(maze)
}

#[cfg(test)]
#[path = "maze_test.rs"]
mod tests;
