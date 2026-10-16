use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::generate::{is_rule, source_lines};
use crate::library::{self, Library};

pub const REVISION: &str = "0e5d690153ca3da8a4a1aef2d053406f408f531c";
const MAX_QUERY_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    #[default]
    All,
    Language,
    Framework,
    Editor,
    Os,
    Custom,
}

#[derive(Default, Deserialize)]
pub struct Args {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub category: Category,
    #[serde(default)]
    pub library: Library,
}

pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub keywords: &'static [&'static str],
    pub text: &'static str,
}

macro_rules! template {
    ($id:literal, $name:literal, $category:ident, [$($keyword:literal),* $(,)?]) => {
        Template {
            id: $id,
            name: $name,
            category: Category::$category,
            keywords: &[$($keyword),*],
            text: include_str!(concat!("../templates/", $id, ".gitignore")),
        }
    };
}

pub const TEMPLATES: &[Template] = &[
    template!(
        "node",
        "Node.js",
        Language,
        [
            "JavaScript",
            "TypeScript",
            "npm",
            "pnpm",
            "yarn",
            "React",
            "Vue",
            "Next.js",
            "Nuxt",
            "Svelte",
            "前端",
            "脚本"
        ]
    ),
    template!(
        "rust",
        "Rust",
        Language,
        ["Cargo", "target", "Tauri", "后端"]
    ),
    template!(
        "python",
        "Python",
        Language,
        ["pip", "uv", "venv", "Django", "Flask", "PyCharm", "脚本"]
    ),
    template!("go", "Go", Language, ["Golang", "后端"]),
    template!("java", "Java", Language, ["JVM", "Maven", "Gradle", "后端"]),
    template!(
        "kotlin",
        "Kotlin",
        Language,
        ["JVM", "Gradle", "Android", "安卓"]
    ),
    template!("c", "C", Language, ["GCC", "Clang", "native", "原生"]),
    template!(
        "cpp",
        "C++",
        Language,
        ["CPP", "GCC", "Clang", "CMake", "原生"]
    ),
    template!(
        "dotnet",
        ".NET",
        Language,
        ["Dotnet", "C#", "F#", "CSharp", "Visual Basic"]
    ),
    template!(
        "swift",
        "Swift",
        Language,
        ["Apple", "Xcode", "iOS", "macOS", "苹果"]
    ),
    template!(
        "ruby",
        "Ruby",
        Language,
        ["Gem", "Bundler", "Rails", "后端"]
    ),
    template!("dart", "Dart", Language, ["pub", "Flutter", "移动"]),
    template!(
        "angular",
        "Angular",
        Framework,
        ["TypeScript", "Node", "前端", "框架"]
    ),
    template!(
        "flutter",
        "Flutter",
        Framework,
        ["Dart", "Android", "iOS", "移动", "框架"]
    ),
    template!("unity", "Unity", Framework, ["C#", "Game", "游戏", "框架"]),
    template!(
        "jetbrains",
        "JetBrains",
        Editor,
        [
            "IntelliJ",
            "IDEA",
            "WebStorm",
            "PyCharm",
            "Rider",
            "CLion",
            "IDE",
            "编辑器"
        ]
    ),
    template!(
        "vscode",
        "Visual Studio Code",
        Editor,
        ["VSCode", "VS Code", "IDE", "编辑器"]
    ),
    template!(
        "visual-studio",
        "Visual Studio",
        Editor,
        ["Microsoft", "C++", "Dotnet", "C#", "IDE", "编辑器"]
    ),
    template!("vim", "Vim", Editor, ["Neovim", "nvim", "编辑器"]),
    template!("emacs", "Emacs", Editor, ["GNU", "编辑器"]),
    template!(
        "macos",
        "macOS",
        Os,
        ["Mac", "OS X", "Darwin", "Apple", "DS_Store", "系统", "苹果"]
    ),
    template!(
        "windows",
        "Windows",
        Os,
        ["Microsoft", "Thumbs.db", "Desktop.ini", "系统", "微软"]
    ),
    template!("linux", "Linux", Os, ["Unix", "Ubuntu", "Debian", "系统"]),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    id: String,
    name: String,
    category: Category,
    keywords: Vec<String>,
    rule_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    url: &'static str,
    revision: &'static str,
    license: &'static str,
    license_text: &'static str,
}

#[derive(Serialize)]
pub struct Output {
    templates: Vec<Metadata>,
    source: Source,
}

pub fn find(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|template| template.id == id)
}

pub fn run(args: Args) -> PluginResult<Output> {
    library::validate(&args.library)?;
    if args.query.len() > MAX_QUERY_BYTES {
        return Err(PluginError::new("gitignore.queryTooLong").with("max", MAX_QUERY_BYTES));
    }
    let query = args.query.trim().to_lowercase();
    let terms: Vec<&str> = query.split_whitespace().collect();
    let mut templates: Vec<Metadata> = TEMPLATES
        .iter()
        .filter(|template| args.category == Category::All || template.category == args.category)
        .filter(|template| {
            let haystack = format!(
                "{} {} {}",
                template.id,
                template.name,
                template.keywords.join(" ")
            )
            .to_lowercase();
            terms.iter().all(|term| haystack.contains(term))
        })
        .map(|template| Metadata {
            id: template.id.into(),
            name: template.name.into(),
            category: template.category,
            keywords: template
                .keywords
                .iter()
                .map(|keyword| (*keyword).into())
                .collect(),
            rule_count: source_lines(template.text)
                .filter(|line| is_rule(line))
                .count(),
        })
        .collect();
    if matches!(args.category, Category::All | Category::Custom) {
        templates.extend(
            args.library
                .templates
                .iter()
                .filter(|template| {
                    let haystack =
                        format!("{} {} custom 自定义", template.id, template.name).to_lowercase();
                    terms.iter().all(|term| haystack.contains(term))
                })
                .map(|template| Metadata {
                    id: template.id.clone(),
                    name: template.name.clone(),
                    category: Category::Custom,
                    keywords: vec!["custom".into(), "自定义".into()],
                    rule_count: source_lines(&template.content)
                        .filter(|line| is_rule(line))
                        .count(),
                }),
        );
    }
    Ok(Output {
        templates,
        source: Source {
            url: "https://github.com/github/gitignore",
            revision: REVISION,
            license: "CC0-1.0",
            license_text: include_str!("../templates/LICENSE"),
        },
    })
}

#[cfg(test)]
#[path = "catalog_test.rs"]
mod tests;
