//! 快捷键速查插件后端：内置主流应用与系统的快捷键表，按应用/关键词检索。
//!
//! 条目：应用键、功能（i18n 键）、macOS 与 Windows 键位、分类。键位渲染交给前端。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct ShortcutCheatsheet {
    manifest: Manifest,
}

impl Default for ShortcutCheatsheet {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for ShortcutCheatsheet {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => to_value(list(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(ShortcutCheatsheet);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// apps.<key>
    pub app: &'static str,
    /// actions.<key>（按功能搜索即匹配此键与中英文文案）
    pub action: &'static str,
    /// 分类键 categories.<key>
    pub category: &'static str,
    pub mac: &'static str,
    pub win: &'static str,
}

macro_rules! shortcuts {
    ($($app:ident => [ $(($cat:ident, $action:literal, $mac:literal, $win:literal)),+ $(,)? ]),+ $(,)?) => {
        &[ $( $( Entry { app: stringify!($app), action: $action, category: stringify!($cat), mac: $mac, win: $win } ),+ ),+ ]
    };
}

static ENTRIES: &[Entry] = shortcuts![
    vscode => [
        (edit, "ac_comment", "Cmd+/", "Ctrl+/"),
        (edit, "ac_format", "Shift+Alt+F", "Shift+Alt+F"),
        (nav, "ac_goto_file", "Cmd+P", "Ctrl+P"),
        (nav, "ac_goto_symbol", "Cmd+Shift+O", "Ctrl+Shift+O"),
        (nav, "ac_goto_line", "Ctrl+G", "Ctrl+G"),
        (nav, "ac_definition", "F12", "F12"),
        (edit, "ac_rename", "F2", "F2"),
        (edit, "ac_delete_line", "Cmd+Shift+K", "Ctrl+Shift+K"),
        (edit, "ac_copy_line_down", "Shift+Alt+Down", "Shift+Alt+Down"),
        (edit, "ac_move_line_up", "Alt+Up", "Alt+Up"),
        (nav, "ac_split", "Cmd+\\", "Ctrl+\\"),
        (term, "ac_toggle_term", "Ctrl+`", "Ctrl+`"),
        (find, "ac_find_in_files", "Cmd+Shift+F", "Ctrl+Shift+F"),
        (find, "ac_replace", "Cmd+Alt+F", "Ctrl+H"),
        (ui, "ac_command", "Cmd+Shift+P", "Ctrl+Shift+P"),
        (ui, "ac_sidebar", "Cmd+B", "Ctrl+B"),
    ],
    chrome => [
        (nav, "ac_new_tab", "Cmd+T", "Ctrl+T"),
        (nav, "ac_reopen_tab", "Cmd+Shift+T", "Ctrl+Shift+T"),
        (nav, "ac_close_tab", "Cmd+W", "Ctrl+W"),
        (nav, "ac_next_tab", "Ctrl+Tab", "Ctrl+Tab"),
        (nav, "ac_reload", "Cmd+R", "Ctrl+R"),
        (nav, "ac_hard_reload", "Cmd+Shift+R", "Ctrl+Shift+R"),
        (find, "ac_find", "Cmd+F", "Ctrl+F"),
        (nav, "ac_devtools", "Cmd+Alt+I", "Ctrl+Shift+I"),
        (nav, "ac_history", "Cmd+Y", "Ctrl+H"),
        (ui, "ac_incognito", "Cmd+Shift+N", "Ctrl+Shift+N"),
        (nav, "ac_address", "Cmd+L", "Ctrl+L"),
    ],
    jetbrains => [
        (nav, "ac_search_everywhere", "Double Shift", "Double Shift"),
        (nav, "ac_goto_class", "Cmd+O", "Ctrl+N"),
        (edit, "ac_comment", "Cmd+/", "Ctrl+/"),
        (edit, "ac_format", "Cmd+Alt+L", "Ctrl+Alt+L"),
        (edit, "ac_rename", "Shift+F6", "Shift+F6"),
        (nav, "ac_usages", "Alt+F7", "Alt+F7"),
        (run, "ac_run", "Ctrl+R", "Shift+F10"),
        (run, "ac_debug", "Ctrl+D", "Shift+F9"),
        (term, "ac_toggle_term", "Alt+F12", "Alt+F12"),
        (ui, "ac_recent_files", "Cmd+E", "Ctrl+E"),
    ],
    office => [
        (edit, "ac_save", "Cmd+S", "Ctrl+S"),
        (edit, "ac_undo", "Cmd+Z", "Ctrl+Z"),
        (edit, "ac_redo", "Cmd+Shift+Z", "Ctrl+Y"),
        (edit, "ac_bold", "Cmd+B", "Ctrl+B"),
        (edit, "ac_italic", "Cmd+I", "Ctrl+I"),
        (edit, "ac_underline", "Cmd+U", "Ctrl+U"),
        (find, "ac_find_replace", "Cmd+Shift+H", "Ctrl+H"),
        (edit, "ac_paste_value", "Cmd+Shift+V", "Ctrl+Alt+V"),
        (nav, "ac_new_doc", "Cmd+N", "Ctrl+N"),
        (edit, "ac_hyperlink", "Cmd+K", "Ctrl+K"),
    ],
    macos => [
        (ui, "ac_switch_app", "Cmd+Tab", "Alt+Tab"),
        (ui, "ac_spotlight", "Cmd+Space", "Win+S"),
        (ui, "ac_screenshot", "Cmd+Shift+4", "Win+Shift+S"),
        (ui, "ac_lock", "Ctrl+Cmd+Q", "Win+L"),
        (ui, "ac_force_quit", "Cmd+Alt+Esc", "Ctrl+Alt+Del"),
        (ui, "ac_minimize", "Cmd+M", "Win+Down"),
        (ui, "ac_hide", "Cmd+H", "Win+D"),
        (ui, "ac_preview", "Space", "Alt+Space"),
    ],
    terminal => [
        (term, "ac_interrupt", "Ctrl+C", "Ctrl+C"),
        (term, "ac_exit", "Ctrl+D", "Ctrl+D"),
        (term, "ac_clear", "Cmd+K", "Ctrl+L"),
        (term, "ac_search_history", "Ctrl+R", "Ctrl+R"),
        (term, "ac_begin_line", "Ctrl+A", "Home"),
        (term, "ac_end_line", "Ctrl+E", "End"),
        (term, "ac_kill_word", "Alt+Backspace", "Ctrl+Backspace"),
        (term, "ac_suspend", "Ctrl+Z", "Ctrl+Z"),
    ],
];

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    /// 按应用过滤（apps.<key> 的 key）
    #[serde(default)]
    pub app: Option<String>,
    /// 按功能关键词过滤（匹配 action 键、macOS/Windows 键位文本）
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOutput {
    pub entries: Vec<Entry>,
    pub total: usize,
    pub apps: Vec<&'static str>,
}

pub fn list(args: ListArgs) -> PluginResult<ListOutput> {
    let query = args.query.trim().to_lowercase();
    let entries: Vec<Entry> = ENTRIES
        .iter()
        .filter(|e| args.app.as_ref().is_none_or(|app| e.app == app))
        .filter(|e| {
            query.is_empty()
                || e.action.contains(&query)
                || e.mac.to_lowercase().contains(&query)
                || e.win.to_lowercase().contains(&query)
        })
        .cloned()
        .collect();
    if entries.is_empty() && (!query.is_empty() || args.app.is_some()) {
        return Err(PluginError::new("shortcut.not_found").with("query", args.query.trim()));
    }
    let mut apps: Vec<&'static str> = ENTRIES.iter().map(|e| e.app).collect();
    apps.sort_unstable();
    apps.dedup();
    let total = entries.len();
    Ok(ListOutput {
        entries,
        total,
        apps,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
