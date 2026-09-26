//! 内置插件的注册入口。
//!
//! 内置插件与后续远程安装的插件遵循同一套 `ToolPlugin` 契约；
//! 这里只负责把随应用分发的插件放进注册表，宿主其余部分不感知具体工具。

use std::sync::Arc;

use tf_core::PluginRegistry;

pub fn builtin() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(tfp_json_formatter::JsonFormatter::default()));
    registry.register(Arc::new(tfp_hash::Hash::default()));
    registry.register(Arc::new(tfp_uuid::UuidTool::default()));
    registry.register(Arc::new(tfp_timestamp::Timestamp::default()));
    registry.register(Arc::new(tfp_regex::Regex::default()));
    registry.register(Arc::new(tfp_xml_formatter::XmlFormatter::default()));
    registry.register(Arc::new(tfp_sql_formatter::SqlFormatter::default()));
    registry.register(Arc::new(tfp_encoder::Encoder::default()));
    registry.register(Arc::new(tfp_jwt::Jwt::default()));
    registry.register(Arc::new(tfp_text_tools::TextTools::default()));
    registry.register(Arc::new(tfp_text_diff::TextDiff::default()));
    registry.register(Arc::new(tfp_format_converter::FormatConverter::default()));
    registry.register(Arc::new(tfp_image_converter::ImageConverter::default()));
    registry.register(Arc::new(tfp_base_converter::BaseConverter::default()));
    registry.register(Arc::new(tfp_color::ColorTool::default()));
    registry.register(Arc::new(tfp_password::Password::default()));
    registry.register(Arc::new(tfp_qrcode::QrCodeTool::default()));
    registry.register(Arc::new(
        tfp_background_remover::BackgroundRemover::default(),
    ));
    registry.register(Arc::new(tfp_ip_calculator::IpCalculator::default()));
    registry.register(Arc::new(tfp_http_client::HttpClient::default()));
    registry.register(Arc::new(tfp_dns_lookup::DnsLookup::default()));
    registry.register(Arc::new(tfp_system_monitor::SystemMonitor::default()));
    registry.register(Arc::new(tfp_unit_converter::UnitConverter::default()));
    registry.register(Arc::new(tfp_cron::CronTool::default()));
    registry.register(Arc::new(tfp_json_to_code::JsonToCode::default()));
    registry.register(Arc::new(tfp_image_info::ImageInfo::default()));
    registry.register(Arc::new(tfp_markdown_editor::MarkdownEditor::default()));
    registry.register(Arc::new(tfp_curl_converter::CurlConverter::default()));
    registry.register(Arc::new(tfp_csv_viewer::CsvViewer::default()));
    registry.register(Arc::new(tfp_ocr::Ocr::default()));
    registry.register(Arc::new(tfp_port_manager::PortManager::default()));
    registry.register(Arc::new(tfp_certificate::CertificateViewer::default()));
    registry.register(Arc::new(tfp_mock_data::MockData::default()));
    registry
}

#[cfg(test)]
#[path = "plugins_test.rs"]
mod tests;
