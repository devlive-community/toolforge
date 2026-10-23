//! 内置插件的注册入口。
//!
//! 内置插件与后续远程安装的插件遵循同一套 `ToolPlugin` 契约；
//! 这里只负责把随应用分发的插件放进注册表，宿主其余部分不感知具体工具。

use std::sync::Arc;

use tf_core::PluginRegistry;

pub fn builtin() -> PluginRegistry {
    builtin_for(std::env::consts::OS)
}

/// 按系统注册内置插件；只支持其他系统的插件会被跳过
pub fn builtin_for(os: &str) -> PluginRegistry {
    let registry = PluginRegistry::new();
    registry.register_for(Arc::new(tfp_hash::Hash::default()), os);
    registry.register_for(Arc::new(tfp_regex::Regex::default()), os);
    registry.register_for(Arc::new(tfp_encoder::Encoder::default()), os);
    registry.register_for(Arc::new(tfp_image_converter::ImageConverter::default()), os);
    registry.register_for(Arc::new(tfp_qrcode::QrCodeTool::default()), os);
    registry.register_for(Arc::new(tfp_barcode::BarcodeTool::default()), os);
    registry.register_for(
        Arc::new(tfp_background_remover::BackgroundRemover::default()),
        os,
    );
    registry.register_for(Arc::new(tfp_dns_lookup::DnsLookup::default()), os);
    registry.register_for(Arc::new(tfp_system_monitor::SystemMonitor::default()), os);
    registry.register_for(Arc::new(tfp_image_info::ImageInfo::default()), os);
    registry.register_for(Arc::new(tfp_csv_viewer::CsvViewer::default()), os);
    registry.register_for(Arc::new(tfp_ocr::Ocr::default()), os);
    registry.register_for(Arc::new(tfp_port_manager::PortManager::default()), os);
    registry.register_for(Arc::new(tfp_proxy_inspector::ProxyInspector::default()), os);
    registry.register_for(Arc::new(tfp_certificate::CertificateViewer::default()), os);
    registry.register_for(Arc::new(tfp_local_server::LocalServer::default()), os);
    registry.register_for(
        Arc::new(tfp_image_compressor::ImageCompressor::default()),
        os,
    );
    registry.register_for(Arc::new(tfp_log_viewer::LogViewer::default()), os);
    registry.register_for(Arc::new(tfp_batch_rename::BatchRename::default()), os);
    registry.register_for(
        Arc::new(tfp_duplicate_finder::DuplicateFinder::default()),
        os,
    );
    registry.register_for(Arc::new(tfp_disk_usage::DiskUsage::default()), os);
    registry.register_for(Arc::new(tfp_archive::Archive::default()), os);
    registry.register_for(Arc::new(tfp_websocket::WebSocketClient::default()), os);
    registry.register_for(Arc::new(tfp_ssh_key::SshKey::default()), os);
    registry.register_for(Arc::new(tfp_pdf::PdfTools::default()), os);
    registry.register_for(Arc::new(tfp_text_encoding::TextEncoding::default()), os);
    registry.register_for(Arc::new(tfp_icon_generator::IconGenerator::default()), os);
    registry.register_for(Arc::new(tfp_hex_viewer::HexViewer::default()), os);
    registry.register_for(Arc::new(tfp_folder_compare::FolderCompare::default()), os);
    registry.register_for(Arc::new(tfp_json_query::JsonQuery::default()), os);
    registry.register_for(Arc::new(tfp_image_palette::ImagePalette::default()), os);
    registry.register_for(Arc::new(tfp_doc_converter::DocConverter::default()), os);
    registry.register_for(Arc::new(tfp_env_editor::EnvEditor::default()), os);
    registry.register_for(Arc::new(tfp_context_menu::ContextMenu::default()), os);
    registry.register_for(Arc::new(tfp_xlsx_viewer::XlsxViewer::default()), os);
    registry.register_for(Arc::new(tfp_svg_converter::SvgConverter::default()), os);
    registry.register_for(Arc::new(tfp_input_stats::InputStats::default()), os);
    registry
}

#[cfg(test)]
#[path = "plugins_test.rs"]
mod tests;
