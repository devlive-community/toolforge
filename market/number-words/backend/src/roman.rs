//! 罗马数字（1–3999）。

const TABLE: [(u32, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
];

pub fn to_roman(mut n: u32) -> Option<String> {
    if !(1..=3999).contains(&n) {
        return None;
    }
    let mut out = String::new();
    for (value, text) in TABLE {
        while n >= value {
            out.push_str(text);
            n -= value;
        }
    }
    Some(out)
}

/// 只接受规范写法（转换回去与输入一致）
pub fn parse(input: &str) -> Option<u32> {
    let text = input.trim().to_ascii_uppercase();
    if text.is_empty() || !text.bytes().all(|b| b"MDCLXVI".contains(&b)) {
        return None;
    }
    let value = |c: u8| match c {
        b'M' => 1000,
        b'D' => 500,
        b'C' => 100,
        b'L' => 50,
        b'X' => 10,
        b'V' => 5,
        _ => 1,
    };
    let bytes = text.as_bytes();
    let mut total = 0i64;
    for (i, &c) in bytes.iter().enumerate() {
        let v = value(c);
        if bytes.get(i + 1).is_some_and(|&next| value(next) > v) {
            total -= v;
        } else {
            total += v;
        }
    }
    let n = u32::try_from(total).ok()?;
    (to_roman(n)? == text).then_some(n)
}

#[cfg(test)]
#[path = "roman_test.rs"]
mod tests;
