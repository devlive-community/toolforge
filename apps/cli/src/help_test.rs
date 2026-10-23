use super::*;
use crate::builtin;
use crate::catalog::Catalog;

fn hash() -> (Manifest, Messages) {
    let loaded = Catalog::new(builtin::all(), None)
        .load("hash", "en-US")
        .unwrap();
    let messages = Messages::new("en-US").with_plugin(loaded.locale);
    (loaded.plugin.manifest().clone(), messages)
}

#[test]
fn plugin_help_lists_cli_functions() {
    let (manifest, messages) = hash();
    let text = plugin("tfp-hash", &manifest, &messages);
    assert!(text.starts_with("tfp-hash — "));
    assert!(text.contains("  hash_text "));
    assert!(text.contains("Compute digests of a text"));
    assert!(text.contains("tfp-hash <function> --help"));
}

#[test]
fn function_help_describes_arguments_and_options() {
    let (manifest, messages) = hash();
    let declaration = manifest.functions["hash_files"].declaration();
    let interface = declaration
        .exposed(tf_plugin_api::interface::Channel::Cli)
        .unwrap();
    let text = function("tfp-hash", "hash_files", interface, &messages);
    assert!(
        text.contains("tfp-hash hash_files <paths>... [options]"),
        "{text}"
    );
    assert!(text.contains("--algorithms <algorithms>..."), "{text}");
    assert!(text.contains("default: sha256"), "{text}");
    assert!(text.contains("--uppercase"), "{text}");
    assert!(text.contains("tfp-hash hash_files ./a.iso"), "{text}");

    let declaration = manifest.functions["hash_text"].declaration();
    let interface = declaration
        .exposed(tf_plugin_api::interface::Channel::Cli)
        .unwrap();
    let text = function("tfp-hash", "hash_text", interface, &messages);
    assert!(
        text.contains("[<input>]"),
        "stdin input is optional: {text}"
    );
    assert!(text.contains("reads standard input when omitted"), "{text}");
}

#[test]
fn global_help_and_list_use_plugin_names() {
    let catalog = Catalog::new(builtin::all(), None);
    let zh = Messages::new("zh-CN");
    let plugins = catalog.manifests("zh-CN");
    let listed = list(&plugins, &zh);
    assert!(listed.contains("  hash  哈希计算"), "{listed}");
    let global_text = global(&plugins, &zh);
    assert!(
        global_text.contains("tfp <插件> <函数> [选项]"),
        "{global_text}"
    );
    assert!(global_text.contains("通用选项："), "{global_text}");
    assert_eq!(list(&[], &zh), zh.text("noPlugins"));
}
