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
    assert!(stdout.contains("active_phase=phase-11"), "stdout={stdout}");
    assert!(
        stdout.contains("isa=softgpu-gfx1201-e2e-tiny-v1_architectural_subset"),
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

#[test]
fn debug_functional_break_step_and_mem() {
    let output = softgpu_bin()
        .args([
            "debug-functional",
            "--builtin",
            "tiny_add",
            "--n",
            "8",
            "--wg",
            "8",
            "--break-step",
            "4",
        ])
        .output()
        .expect("run");
    assert_ok(&output, "debug-functional break-step");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("softgpu_functional_debug_v1"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("after_step"), "stdout={stdout}");
    assert!(stdout.contains("softgpu-debug-trace-v1"), "stdout={stdout}");

    let output = softgpu_bin()
        .args([
            "debug-functional",
            "--builtin",
            "tiny_add",
            "--n",
            "4",
            "--wg",
            "4",
            "--break-mem",
        ])
        .output()
        .expect("run");
    assert_ok(&output, "debug-functional break-mem");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("memory_access"), "stdout={stdout}");
}

#[test]
fn debug_functional_rejects_bad_args() {
    let output = softgpu_bin()
        .args(["debug-functional"])
        .output()
        .expect("run");
    assert!(!output.status.success());

    let output = softgpu_bin()
        .args(["debug-functional", "--builtin", "nope"])
        .output()
        .expect("run");
    assert!(!output.status.success());

    let output = softgpu_bin()
        .args([
            "debug-functional",
            "--builtin",
            "tiny_add",
            "--break-step",
            "x",
        ])
        .output()
        .expect("run");
    assert!(!output.status.success());
}

#[test]
fn run_functional_sanitize_collect_ok() {
    let output = softgpu_bin()
        .args([
            "run-functional",
            "--builtin",
            "tiny_add",
            "--n",
            "16",
            "--wg",
            "8",
            "--sanitize",
            "collect",
        ])
        .output()
        .expect("run");
    assert_ok(&output, "run-functional sanitize collect");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("softgpu_functional_sanitizer_v1"),
        "stdout={stdout}"
    );
}

#[test]
fn check_config_accepts_and_rejects() {
    let output = softgpu_bin()
        .args(["check-config", "log_level=info", "enable_queues=true"])
        .output()
        .expect("run");
    assert_ok(&output, "check-config ok");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("phase=phase-11"), "stdout={stdout}");

    let output = softgpu_bin()
        .args(["check-config", "log_level=info", "enable_execution=true"])
        .output()
        .expect("run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[unsupported]"), "stderr={stderr}");
}

#[test]
fn decode_and_run_isa_salu_subset() {
    let output = softgpu_bin()
        .args(["decode-isa", "0xbe800081", "0xbfb00000"])
        .output()
        .expect("run");
    assert_ok(&output, "decode-isa");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("s_mov_b32 s0, 1"), "stdout={stdout}");
    assert!(stdout.contains("s_endpgm"), "stdout={stdout}");
    assert!(
        stdout.contains("fidelity=architectural_isa"),
        "stdout={stdout}"
    );

    let output = softgpu_bin()
        .args([
            "run-isa",
            "--words",
            "0xbe800081,0xbe810082,0x80020100,0xbfb00000",
        ])
        .output()
        .expect("run");
    assert_ok(&output, "run-isa");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"halted\":true"), "stdout={stdout}");
    assert!(stdout.contains("\"sgpr2\":3"), "stdout={stdout}");
    assert!(stdout.contains("architectural_isa"), "stdout={stdout}");
}

#[test]
fn run_kernel_tiny_add() {
    let output = softgpu_bin()
        .args(["run-kernel", "--builtin", "tiny_add", "--n", "32"])
        .output()
        .expect("run");
    assert_ok(&output, "run-kernel");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"host_diff_ok\":true"), "stdout={stdout}");
    assert!(stdout.contains("softgpu_kernel_success"), "stdout={stdout}");
}
