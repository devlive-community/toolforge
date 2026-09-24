use super::*;

#[test]
fn manifest_declares_models_matching_the_code() {
    let tool = BackgroundRemover::default();
    let manifest = tool.manifest();
    assert!(manifest.functions["remove"].task);
    let ids: Vec<_> = manifest.resources.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            model::Model::U2netp.resource_id(),
            model::Model::Isnet.resource_id()
        ]
    );
    for resource in &manifest.resources {
        assert_eq!(resource.sha256.len(), 64);
        assert!(resource.urls.iter().all(|u| u.starts_with("https://")));
    }
}
