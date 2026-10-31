//! Asset URIs must remain local and cannot traverse the installed resource tree.

use atlas_media::{AudioKind, audio, illustration};

#[test]
fn canonical_resources_should_use_the_same_installed_namespace_on_every_platform() {
    assert_eq!(
        illustration("resilient", false).ok().as_deref(),
        Some("asset:/atlas/img/resilient.webp")
    );
}

#[test]
fn missing_optional_example_audio_should_not_request_an_unavailable_file() {
    assert_eq!(
        audio("resilient", AudioKind::ExampleTwo, 15).ok(),
        Some(None)
    );
}

#[test]
fn encoded_or_literal_path_traversal_should_be_rejected() {
    for id in [
        "../private",
        "..\\private",
        "%2e%2e%2fprivate",
        "https:remote",
        "",
        "data\0name",
    ] {
        assert!(
            illustration(id, false).is_err(),
            "accepted invalid asset ID {id:?}"
        );
    }
}
