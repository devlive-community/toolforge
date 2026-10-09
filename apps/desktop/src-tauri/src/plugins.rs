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
    let mut registry = PluginRegistry::new();
    registry.register_for(Arc::new(tfp_json_formatter::JsonFormatter::default()), os);
    registry.register_for(Arc::new(tfp_hash::Hash::default()), os);
    registry.register_for(Arc::new(tfp_uuid::UuidTool::default()), os);
    registry.register_for(Arc::new(tfp_timestamp::Timestamp::default()), os);
    registry.register_for(Arc::new(tfp_regex::Regex::default()), os);
    registry.register_for(Arc::new(tfp_xml_formatter::XmlFormatter::default()), os);
    registry.register_for(Arc::new(tfp_sql_formatter::SqlFormatter::default()), os);
    registry.register_for(Arc::new(tfp_encoder::Encoder::default()), os);
    registry.register_for(Arc::new(tfp_jwt::Jwt::default()), os);
    registry.register_for(Arc::new(tfp_text_tools::TextTools::default()), os);
    registry.register_for(Arc::new(tfp_text_diff::TextDiff::default()), os);
    registry.register_for(
        Arc::new(tfp_format_converter::FormatConverter::default()),
        os,
    );
    registry.register_for(Arc::new(tfp_image_converter::ImageConverter::default()), os);
    registry.register_for(Arc::new(tfp_base_converter::BaseConverter::default()), os);
    registry.register_for(Arc::new(tfp_color::ColorTool::default()), os);
    registry.register_for(Arc::new(tfp_password::Password::default()), os);
    registry.register_for(Arc::new(tfp_qrcode::QrCodeTool::default()), os);
    registry.register_for(
        Arc::new(tfp_background_remover::BackgroundRemover::default()),
        os,
    );
    registry.register_for(Arc::new(tfp_ip_calculator::IpCalculator::default()), os);
    registry.register_for(Arc::new(tfp_http_client::HttpClient::default()), os);
    registry.register_for(Arc::new(tfp_dns_lookup::DnsLookup::default()), os);
    registry.register_for(Arc::new(tfp_system_monitor::SystemMonitor::default()), os);
    registry.register_for(Arc::new(tfp_unit_converter::UnitConverter::default()), os);
    registry.register_for(Arc::new(tfp_cron::CronTool::default()), os);
    registry.register_for(Arc::new(tfp_json_to_code::JsonToCode::default()), os);
    registry.register_for(Arc::new(tfp_image_info::ImageInfo::default()), os);
    registry.register_for(Arc::new(tfp_markdown_editor::MarkdownEditor::default()), os);
    registry.register_for(Arc::new(tfp_curl_converter::CurlConverter::default()), os);
    registry.register_for(Arc::new(tfp_csv_viewer::CsvViewer::default()), os);
    registry.register_for(Arc::new(tfp_ocr::Ocr::default()), os);
    registry.register_for(Arc::new(tfp_port_manager::PortManager::default()), os);
    registry.register_for(Arc::new(tfp_certificate::CertificateViewer::default()), os);
    registry.register_for(Arc::new(tfp_mock_data::MockData::default()), os);
    registry.register_for(Arc::new(tfp_crypto::Crypto::default()), os);
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
    registry.register_for(Arc::new(tfp_unicode::UnicodeInspector::default()), os);
    registry.register_for(Arc::new(tfp_pdf::PdfTools::default()), os);
    registry.register_for(Arc::new(tfp_text_encoding::TextEncoding::default()), os);
    registry.register_for(Arc::new(tfp_icon_generator::IconGenerator::default()), os);
    registry.register_for(Arc::new(tfp_hex_viewer::HexViewer::default()), os);
    registry.register_for(Arc::new(tfp_folder_compare::FolderCompare::default()), os);
    registry.register_for(Arc::new(tfp_date_calculator::DateCalculator::default()), os);
    registry.register_for(Arc::new(tfp_number_words::NumberWords::default()), os);
    registry.register_for(Arc::new(tfp_url_parser::UrlParser::default()), os);
    registry.register_for(Arc::new(tfp_json_query::JsonQuery::default()), os);
    registry.register_for(Arc::new(tfp_image_palette::ImagePalette::default()), os);
    registry.register_for(Arc::new(tfp_table_converter::TableConverter::default()), os);
    registry.register_for(Arc::new(tfp_doc_converter::DocConverter::default()), os);
    registry.register_for(Arc::new(tfp_context_menu::ContextMenu::default()), os);
    registry
}

#[cfg(test)]
#[path = "plugins_test.rs"]
mod tests;
