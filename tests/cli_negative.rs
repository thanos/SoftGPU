//! Negative CLI / config tests for Phase 0.

use std::process::Command;

fn softgpu_bin() -> Command {
    // Prefer the binary Cargo built for integration tests when available.
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_softgpu"));
    cmd.env_remove("SOFTGPU_FORCE_FAIL");
    cmd
}

#[test]
fn unknown_config_key_fails() {
    let output = softgpu_bin()
        .args(["check-config", "log_level=info", "not_a_real_key=1"])
        .output()
        .expect("run softgpu");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[config]"), "stderr was: {stderr}");
    assert!(stderr.contains("unknown config key"));
}

#[test]
fn empty_log_level_value_fails() {
    let output = softgpu_bin()
        .args(["check-config", "log_level="])
        .output()
        .expect("run softgpu");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[config]"), "stderr was: {stderr}");
}

#[test]
fn enable_queues_true_is_accepted() {
    let output = softgpu_bin()
        .args(["check-config", "log_level=info", "enable_queues=true"])
        .output()
        .expect("run softgpu");
    assert!(
        output.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn enable_execution_true_is_unsupported() {
    let output = softgpu_bin()
        .args(["check-config", "log_level=info", "enable_execution=true"])
        .output()
        .expect("run softgpu");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[unsupported]"),
        "stderr was: {stderr}"
    );
}

#[test]
fn validate_profile_missing_path_fails() {
    let output = softgpu_bin()
        .args(["validate-profile"])
        .output()
        .expect("run softgpu");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[config]"), "stderr was: {stderr}");
}
