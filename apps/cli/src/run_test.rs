use serde_json::json;

use super::*;
use crate::builtin;

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn hash_command(items: &[&str]) -> i32 {
    let catalog = Catalog::new(builtin::all(), None);
    plugin_command(&catalog, false, "tfp-hash", "hash", &argv(items))
}

#[test]
fn program_name_strips_directories_and_exe() {
    assert_eq!(program_name("/usr/local/bin/tfp-hash"), "tfp-hash");
    assert_eq!(
        program_name(r"C:\tools\tfp.exe"),
        if cfg!(windows) {
            "tfp"
        } else {
            r"C:\tools\tfp"
        }
    );
    assert_eq!(program_name("tfp"), "tfp");
}

#[test]
fn error_codes_map_to_exit_codes() {
    assert_eq!(exit_code("cli.unknown_option"), EXIT_USAGE);
    assert_eq!(exit_code("plugin.permission_denied"), EXIT_PERMISSION);
    assert_eq!(exit_code("market.needs_app"), EXIT_NEEDS_APP);
    assert_eq!(exit_code("task.cancelled"), EXIT_CANCELLED);
    assert_eq!(exit_code("hash.no_algorithms"), EXIT_FAILED);
}

#[test]
fn runs_functions_and_reports_usage_errors() {
    assert_eq!(hash_command(&["hash_text", "hello"]), EXIT_OK);
    assert_eq!(hash_command(&["hash_text", "hello", "--json"]), EXIT_OK);
    assert_eq!(hash_command(&["hash_text", "--help"]), EXIT_OK);
    assert_eq!(hash_command(&["--help"]), EXIT_OK);
    assert_eq!(
        hash_command(&[]),
        EXIT_USAGE,
        "functions must be named when there are several"
    );
    assert_eq!(hash_command(&["nothing"]), EXIT_USAGE);
    assert_eq!(hash_command(&["hash_text", "x", "--bogus"]), EXIT_USAGE);
    assert_eq!(
        hash_command(&["hash_text", "x", "--algorithms", "crc"]),
        EXIT_USAGE,
        "values outside the enum are rejected before calling the plugin"
    );
}

#[test]
fn task_functions_run_and_return_plugin_errors() {
    let dir = std::env::temp_dir().join(format!("tfp-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.txt");
    std::fs::write(&file, "hello").unwrap();
    let path = file.to_string_lossy().into_owned();
    assert_eq!(hash_command(&["hash_files", &path]), EXIT_OK);
    assert_eq!(
        hash_command(&[
            "manifest_verify",
            &dir.join("missing.sums").to_string_lossy()
        ]),
        EXIT_FAILED
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unknown_plugins_are_usage_errors() {
    let catalog = Catalog::new(builtin::all(), None);
    assert_eq!(
        plugin_command(&catalog, false, "tfp-nope", "nope", &[]),
        EXIT_USAGE
    );
}

#[test]
fn output_follows_the_declared_hint() {
    let loaded = Catalog::new(builtin::all(), None)
        .load("hash", "en-US")
        .unwrap();
    let messages = Messages::new("en-US").with_plugin(loaded.locale);
    let manifest = loaded.plugin.manifest().clone();
    let declaration = manifest.functions["hash_text"].declaration();
    let interface = declaration
        .exposed(tf_plugin_api::interface::Channel::Cli)
        .unwrap();
    let value = loaded
        .plugin
        .call(
            "hash_text",
            json!({ "input": "hello", "algorithms": ["md5"] }),
        )
        .unwrap();
    assert_eq!(
        output_text(&value, interface, false, &messages),
        "Algorithm  Digest\nmd5        5d41402abc4b2a76b9719d911017c592"
    );
    assert!(output_text(&value, interface, true, &messages).contains("\"results\""));
}
