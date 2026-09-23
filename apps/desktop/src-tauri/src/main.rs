// 在 Windows release 构建中隐藏控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    toolforge_lib::run()
}
