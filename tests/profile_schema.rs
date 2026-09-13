//! Integration tests for device profile schema validation.

use softgpu::error::ErrorCategory;
use softgpu::profile::DeviceProfile;
use std::path::PathBuf;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[test]
fn generic_profile_fixture_validates() {
    let profile = DeviceProfile::load_path(repo_path("profiles/softgpu-generic-v0.json"))
        .expect("generic profile should load");
    assert_eq!(profile.profile_id, "softgpu-generic");
    assert!(!profile.conformance_allowed);
}

#[test]
fn r9700_profile_does_not_invent_numeric_limits() {
    let profile = DeviceProfile::load_path(repo_path(
        "profiles/amd-radeon-ai-pro-r9700-gfx1201-v0.json",
    ))
    .expect("r9700 profile should load");
    assert_eq!(
        profile.identity.llvm_target.value.as_deref(),
        Some("gfx1201")
    );
    assert!(profile.resource_limits.max_workgroup_size_x.value.is_none());
    assert!(profile.resource_limits.wavefront_size.value.is_none());
    assert!(!profile.conformance_allowed);
}

#[test]
fn invalid_schema_version_fixture_fails() {
    let err = DeviceProfile::load_path(repo_path("tests/fixtures/invalid-schema-version.json"))
        .expect_err("schema 99 must fail");
    assert_eq!(err.category(), ErrorCategory::Profile);
    assert!(err.message().contains("schema_version"));
}
