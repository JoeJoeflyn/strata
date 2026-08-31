// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    ReleaseResponse, is_newer, markdown_to_pango, metadata, release_page_url, request_error_message,
};

#[test]
fn newer_patch_version_is_detected() {
    assert!(is_newer("0.2.1", "0.2.0"));
}

#[test]
fn newer_minor_version_is_detected() {
    assert!(is_newer("0.3.0", "0.2.9"));
}

#[test]
fn equal_version_is_not_newer() {
    assert!(!is_newer("0.2.0", "0.2.0"));
}

#[test]
fn older_version_is_not_newer() {
    assert!(!is_newer("0.1.9", "0.2.0"));
}

#[test]
fn missing_or_malformed_segments_fall_back_to_zero() {
    assert!(!is_newer("0.2", "0.2.0"));
    assert!(!is_newer("0.2.x", "0.2.1"));
}

#[test]
fn release_body_is_retained() {
    let response: ReleaseResponse = serde_json::from_str(
        r###"{"tag_name":"v1.2.3","html_url":"https://example.test/release","body":"## Changes\n\n- Fast"}"###,
    )
    .expect("release fixture should deserialize");
    let release = metadata(&response);
    assert_eq!(release.version, "1.2.3");
    assert_eq!(release.notes, "## Changes\n\n- Fast");
}

#[test]
fn missing_release_body_becomes_empty_notes() {
    let response: ReleaseResponse =
        serde_json::from_str(r#"{"tag_name":"v1.2.3","html_url":"https://example.test/release"}"#)
            .expect("release fixture should deserialize");
    assert!(metadata(&response).notes.is_empty());
}

#[test]
fn null_release_body_becomes_empty_notes() {
    let response: ReleaseResponse = serde_json::from_str(
        r#"{"tag_name":"v1.2.3","html_url":"https://example.test/release","body":null}"#,
    )
    .expect("release fixture should deserialize");
    assert!(metadata(&response).notes.is_empty());
}

#[test]
fn rate_limit_failures_have_a_distinct_message() {
    assert_eq!(
        request_error_message(&ureq::Error::StatusCode(429)),
        "GitHub API rate limit reached"
    );
}

#[test]
fn other_api_failures_include_the_status() {
    assert_eq!(
        request_error_message(&ureq::Error::StatusCode(500)),
        "GitHub API returned HTTP 500"
    );
}

#[test]
fn release_markdown_renders_supported_formatting() {
    let markup =
        markdown_to_pango("## Changes\n\n- **Fast** and `safe`\n- [Details](https://example.test)");
    assert!(markup.contains("<span size=\"large\"><b>Changes</b></span>"));
    assert!(markup.contains("•  <b>Fast</b> and <tt>safe</tt>"));
    assert!(markup.contains("<a href=\"https://example.test\">Details</a>"));
}

#[test]
fn release_markdown_keeps_html_inert_and_does_not_load_images() {
    let markup = markdown_to_pango(
        "<script>alert('no')</script>\n\n![tracking](https://example.test/pixel.png)",
    );
    assert!(!markup.contains("<script>"));
    assert!(markup.contains("&lt;script&gt;"));
    assert!(!markup.contains("href=\"https://example.test/pixel.png"));
    assert!(markup.contains("[Image: tracking]"));
}

#[test]
fn release_markdown_does_not_activate_non_web_links() {
    assert_eq!(
        markdown_to_pango("[Run](javascript:alert('no'))"),
        "<u>Run</u>"
    );
}

#[test]
fn empty_release_markdown_is_empty_markup() {
    assert!(markdown_to_pango("  \n").is_empty());
}

#[test]
fn current_release_fallback_uses_exact_version_tag() {
    assert_eq!(
        release_page_url("1.2.3"),
        "https://github.com/lgse/strata/releases/tag/v1.2.3"
    );
}
