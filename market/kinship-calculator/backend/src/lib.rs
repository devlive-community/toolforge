//! 亲戚称呼计算器后端：关系链（父/母/兄/姐/弟/妹/夫/妻/子/女）→ 称呼与自称。
//! 内置常见直系与旁系称呼规则库（口语），支持反向查询。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct KinshipCalculator {
    manifest: Manifest,
}

impl Default for KinshipCalculator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for KinshipCalculator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "resolve" => to_value(resolve(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(KinshipCalculator);

/// 关系步：f 父 m 母 ob 兄 os 姐 yb 弟 ys 妹 h 夫 w 妻 s 子 d 女
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    F,
    M,
    Ob,
    Os,
    Yb,
    Ys,
    H,
    W,
    S,
    D,
}

fn parse_step(text: &str) -> Option<Step> {
    match text {
        "f" => Some(Step::F),
        "m" => Some(Step::M),
        "ob" => Some(Step::Ob),
        "os" => Some(Step::Os),
        "yb" => Some(Step::Yb),
        "ys" => Some(Step::Ys),
        "h" => Some(Step::H),
        "w" => Some(Step::W),
        "s" => Some(Step::S),
        "d" => Some(Step::D),
        _ => None,
    }
}

/// 称呼（口语）与自称；覆盖常见直系与旁系一~三代
fn term(chain: &[Step]) -> Option<(&'static str, &'static str)> {
    use Step::*;
    match chain {
        [F] => Some(("爸爸", "我")),
        [M] => Some(("妈妈", "我")),
        [F, F] => Some(("爷爷", "孙子/孙女")),
        [F, M] => Some(("奶奶", "孙子/孙女")),
        [M, F] => Some(("外公/姥爷", "外孙/外孙女")),
        [M, M] => Some(("外婆/姥姥", "外孙/外孙女")),
        [Ob] | [Yb] => Some(("哥哥/弟弟", "我")),
        [Os] | [Ys] => Some(("姐姐/妹妹", "我")),
        [H] => Some(("老公", "我")),
        [W] => Some(("老婆", "我")),
        [S] => Some(("儿子", "爸爸/妈妈")),
        [D] => Some(("女儿", "爸爸/妈妈")),
        [F, Ob] => Some(("伯伯/大爷", "侄子/侄女")),
        [F, Yb] => Some(("叔叔", "侄子/侄女")),
        [F, Os] => Some(("姑姑", "侄子/侄女")),
        [F, Ys] => Some(("姑姑", "侄子/侄女")),
        [M, Ob] | [M, Yb] => Some(("舅舅", "外甥/外甥女")),
        [M, Os] | [M, Ys] => Some(("姨妈", "外甥/外甥女")),
        [F, F, Ob] | [F, F, Yb] => Some(("伯公/叔公", "侄孙/侄孙女")),
        [F, F, F] => Some(("曾祖父/太爷爷", "曾孙/曾孙女")),
        [F, F, M] => Some(("曾祖母/太奶奶", "曾孙/曾孙女")),
        [M, M, F] => Some(("外曾祖父", "曾外孙/曾外孙女")),
        [F, S] | [F, D] => Some(("哥哥/姐姐（堂）", "弟弟/妹妹")),
        [F, Ob, S] | [F, Ob, D] => Some(("堂哥/堂姐", "堂弟/堂妹")),
        [F, Yb, S] | [F, Yb, D] => Some(("堂弟/堂妹", "堂哥/堂姐")),
        [F, Os, S] | [F, Os, D] | [F, Ys, S] | [F, Ys, D] => {
            Some(("表哥/表姐（姑表）", "表弟/表妹"))
        }
        [M, Ob, S] | [M, Ob, D] | [M, Yb, S] | [M, Yb, D] => {
            Some(("表哥/表姐（舅表）", "表弟/表妹"))
        }
        [M, Os, S] | [M, Os, D] | [M, Ys, S] | [M, Ys, D] => {
            Some(("表哥/表姐（姨表）", "表弟/表妹"))
        }
        [Ob, S] | [Yb, S] => Some(("侄子", "伯父/叔叔")),
        [Ob, D] | [Yb, D] => Some(("侄女", "伯父/叔叔")),
        [Os, S] | [Ys, S] => Some(("外甥", "舅舅/姨妈")),
        [Os, D] | [Ys, D] => Some(("外甥女", "舅舅/姨妈")),
        [S, S] => Some(("孙子", "爷爷/奶奶")),
        [S, D] => Some(("孙女", "爷爷/奶奶")),
        [D, S] => Some(("外孙", "外公/外婆")),
        [D, D] => Some(("外孙女", "外公/外婆")),
        [S, W] => Some(("儿媳", "公公/婆婆")),
        [D, H] => Some(("女婿", "岳父/岳母")),
        [W, F] => Some(("岳父", "女婿")),
        [W, M] => Some(("岳母", "女婿")),
        [H, F] => Some(("公公", "儿媳")),
        [H, M] => Some(("婆婆", "儿媳")),
        [W, Ob] | [W, Yb] => Some(("内兄/内弟（大舅子/小舅子）", "姐夫/妹夫")),
        [W, Os] | [W, Ys] => Some(("姨姐/姨妹（大姨子/小姨子）", "妹夫/姐夫")),
        [H, Ob] | [H, Yb] => Some(("大伯子/小叔子", "嫂子/弟媳")),
        [H, Os] | [H, Ys] => Some(("大姑子/小姑子", "弟媳/嫂子")),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveArgs {
    /// 关系步序列（f/m/ob/os/yb/ys/h/w/s/d），从「我」出发
    pub chain: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub chain: Vec<String>,
    /// 称呼（口语）
    pub term: String,
    /// 自称
    pub self_address: String,
    /// 关系链文字描述
    pub description: String,
}

const STEP_NAMES: &[(&str, &str)] = &[
    ("f", "爸爸"),
    ("m", "妈妈"),
    ("ob", "哥哥"),
    ("os", "姐姐"),
    ("yb", "弟弟"),
    ("ys", "妹妹"),
    ("h", "丈夫"),
    ("w", "妻子"),
    ("s", "儿子"),
    ("d", "女儿"),
];

pub fn resolve(args: ResolveArgs) -> PluginResult<Output> {
    let mut chain: Vec<Step> = Vec::new();
    for step in &args.chain {
        let Some(parsed) = parse_step(&step.trim().to_lowercase()) else {
            return Err(PluginError::new("kinship.bad_step").with("step", step.trim()));
        };
        chain.push(parsed);
    }
    if chain.is_empty() {
        return Err(PluginError::new("kinship.empty_chain"));
    }
    let Some((term, self_address)) = term(&chain) else {
        return Err(PluginError::new("kinship.unknown_chain"));
    };
    let description = chain
        .iter()
        .filter_map(|s| {
            STEP_NAMES
                .iter()
                .find(|(k, _)| parse_step(k) == Some(*s))
                .map(|(_, name)| *name)
        })
        .collect::<Vec<&str>>()
        .join(" 的 ");
    Ok(Output {
        chain: args.chain,
        term: term.into(),
        self_address: self_address.into(),
        description,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
