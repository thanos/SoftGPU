//! Positive CLI smoke paths (coverage + regression for Phase 5/6 commands).

use softgpu_amd_code_object::fixture::fixture_tiny_add_gfx1201;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn softgpu_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_softgpu"));
    cmd.env_remove("SOFTGPU_FORCE_FAIL");
    cmd
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn assert_ok(output: &std::process::Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn help_version_info() {
    for args in [["help"], ["version"], ["info"], ["--help"], ["--version"]] {
        let output = softgpu_bin().args(args).output().expect("run softgpu");
        assert_ok(&output, &format!("softgpu {}", args.join(" ")));
    }
    let info = softgpu_bin().arg("info").output().expect("info");
    let stdout = String::from_utf8_lossy(&info.stdout);
    assert!(stdout.contains("active_phase=phase-6"), "stdout={stdout}");
    assert!(
        stdout.contains("functional=softgpu-sfir-v1_cpu_not_gfx1201_isa"),
        "stdout={stdout}"
    );
}

#[test]
fn validate_profile_ok() {
    let path = repo_root().join("profiles/softgpu-generic-v0.json");
    let output = softgpu_bin()
        .args(["validate-profile", path.to_str().unwrap()])
        .output()
        .expect("run");
    assert_ok(&output, "validate-profile");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ok: profile_id="), "stdout={stdout}");
}

#[test]
fn run_functional_builtin_and_json() {
    let output = softgpu_bin()
        .args([
            "run-functional",
            "--builtin",
            "tiny_add",
            "--n",
            "32",
            "--wg",
            "8",
        ])
        .output()
        .expect("run");
    assert_ok(&output, "run-functional builtin");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"fidelity\": \"functional\""),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("not_gfx1201_isa_emulation"),
        "stdout={stdout}"
    );

    let json = repo_root().join("fixtures/functional/tiny_add.sfir.json");
    let output = softgpu_bin()
        .args([
            "run-functional",
            json.to_str().unwrap(),
            "--n",
            "16",
            "--wg",
            "8",
        ])
        .output()
        .expect("run");
    assert_ok(&output, "run-functional json");
}

#[test]
fn inspect_code_object_fixture() {
    let dir = std::env::temp_dir().join(format!("softgpu-cli-cov-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let elf = dir.join("tiny_add.gfx1201.so");
    fs::write(&elf, fixture_tiny_add_gfx1201()).unwrap();

    let output = softgpu_bin()
        .args(["inspect-code-object", elf.to_str().unwrap()])
        .output()
        .expect("run");
    assert_ok(&output, "inspect-code-object");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tiny_add"), "stdout={stdout}");
    assert!(stdout.contains("gfx1201"), "stdout={stdout}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn inspect_and_run_functional_missing_args_fail() {
    let output = softgpu_bin()
        .args(["inspect-code-object"])
        .output()
        .expect("run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[config]"), "stderr={stderr}");

    let output = softgpu_bin()
        .args(["run-functional"])
        .output()
        .expect("run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[config]"), "stderr={stderr}");

    let output = softgpu_bin()
        .args(["run-functional", "--builtin", "nope"])
        .output()
        .expect("run");
    assert!(!output.status.success());
}

#[test]
fn unknown_command_fails() {
    let output = softgpu_bin()
        .args(["definitely-not-a-command"])
        .output()
        .expect("run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[config]"), "stderr={stderr}");
}
