/// 把 UTF-8 字节偏移换算为 UTF-16 偏移（JS 字符串下标 / CodeMirror 位置）。
///
/// 预先记录每个字符边界的两种偏移，查询时二分；适用于非单调的查询顺序（分组位于匹配内部）。
pub struct Utf16Offsets {
    bytes: Vec<usize>,
    units: Vec<usize>,
}

impl Utf16Offsets {
    pub fn new(text: &str) -> Self {
        let mut bytes = Vec::with_capacity(text.len() + 1);
        let mut units = Vec::with_capacity(text.len() + 1);
        let mut unit = 0;
        for (byte, ch) in text.char_indices() {
            bytes.push(byte);
            units.push(unit);
            unit += ch.len_utf16();
        }
        bytes.push(text.len());
        units.push(unit);
        Self { bytes, units }
    }

    pub fn get(&self, byte: usize) -> usize {
        match self.bytes.binary_search(&byte) {
            Ok(index) => self.units[index],
            // 正则匹配位置总在字符边界上；防御性地取前一个边界
            Err(index) => self.units[index.saturating_sub(1)],
        }
    }
}

#[cfg(test)]
#[path = "offsets_test.rs"]
mod tests;
