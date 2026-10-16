//! 手动验证：`cargo run -p tf-wasm --example run -- <manifest.json> <plugin.wasm> <function> <json>`

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [manifest, wasm, function, json] = args.as_slice() else {
        eprintln!("usage: run <manifest.json> <plugin.wasm> <function> <json-args>");
        std::process::exit(2);
    };
    let manifest: tf_plugin_api::Manifest =
        serde_json::from_str(&std::fs::read_to_string(manifest).expect("manifest"))
            .expect("manifest json");
    let started = std::time::Instant::now();
    let plugin =
        tf_wasm::WasmPlugin::load(manifest, &std::fs::read(wasm).expect("wasm")).expect("load");
    println!("loaded in {:?}", started.elapsed());
    let started = std::time::Instant::now();
    let result = tf_plugin_api::ToolPlugin::call(
        &plugin,
        function,
        serde_json::from_str(json).expect("args"),
    );
    println!("called in {:?}", started.elapsed());
    match result {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        Err(err) => println!("error: {err:?}"),
    }
}
