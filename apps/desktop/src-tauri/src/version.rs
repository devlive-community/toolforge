//! 应用版本比较：一键安装判断插件要求的最低应用版本时使用。

/// 数字段逐段比较；非数字段按 0 处理（一键安装判断版本也用它）
pub fn at_least(version: &str, min: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['.', '-', '+'])
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parts(version), parts(min));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    true
}

#[cfg(test)]
#[path = "version_test.rs"]
mod tests;
