//! SoftGPU CLI.

use softgpu::error::{Error, ErrorCategory, Result};
use softgpu::profile::DeviceProfile;
use softgpu::{ACTIVE_PHASE, VERSION};
use softgpu_amd_code_object::inspect_path;
use softgpu_functional::kernels::{kernarg_two_ptrs, tiny_add};
use softgpu_functional::{load_program_path, run, GlobalArena, LaunchConfig, TypeId};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run_cli(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err.format_diagnostic());
            ExitCode::from(exit_status(&err))
        }
    }
}

fn exit_status(err: &Error) -> u8 {
    match err.category() {
        ErrorCategory::Config => 2,
        ErrorCategory::Profile | ErrorCategory::Validation => 3,
        ErrorCategory::Unsupported => 4,
        ErrorCategory::Io => 5,
        ErrorCategory::Internal => 1,
    }
}

fn run_cli(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        return Err(Error::new(
            ErrorCategory::Config,
            "missing command; expected one of: help, version, info, validate-profile, check-config, inspect-code-object, run-functional",
        )
        .with_remediation("run `softgpu help`"));
    }

    match args[0].as_str() {
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "version" | "--version" => {
            println!("softgpu {VERSION}");
            Ok(())
        }
        "info" => {
            print_info();
            Ok(())
        }
        "validate-profile" => {
            let path = args.get(1).ok_or_else(|| {
                Error::new(
                    ErrorCategory::Config,
                    "validate-profile requires a path argument",
                )
                .with_remediation("usage: softgpu validate-profile <path-to-profile.json>")
            })?;
            let profile = DeviceProfile::load_path(path)?;
            println!(
                "ok: profile_id={} revision={} max_fidelity={} conformance_allowed={}",
                profile.profile_id,
                profile.profile_revision,
                profile.max_fidelity,
                profile.conformance_allowed
            );
            Ok(())
        }
        "inspect-code-object" => {
            let path = args.get(1).ok_or_else(|| {
                Error::new(
                    ErrorCategory::Config,
                    "inspect-code-object requires a path argument",
                )
                .with_remediation("usage: softgpu inspect-code-object <path-to-elf>")
            })?;
            let info = inspect_path(path).map_err(|e| {
                let cat = match &e {
                    softgpu_amd_code_object::CodeObjectError::Io(_) => ErrorCategory::Io,
                    softgpu_amd_code_object::CodeObjectError::UnsupportedMetadataVersion {
                        ..
                    }
                    | softgpu_amd_code_object::CodeObjectError::UnsupportedTarget { .. }
                    | softgpu_amd_code_object::CodeObjectError::UnsupportedClass { .. }
                    | softgpu_amd_code_object::CodeObjectError::UnsupportedEndian { .. }
                    | softgpu_amd_code_object::CodeObjectError::UnsupportedElfType { .. }
                    | softgpu_amd_code_object::CodeObjectError::UnsupportedNote { .. } => {
                        ErrorCategory::Unsupported
                    }
                    _ => ErrorCategory::Validation,
                };
                Error::new(cat, e.to_string())
                    .with_remediation("see docs/code-object.md; SoftGPU parses metadata only")
            })?;
            let json = serde_json::to_string_pretty(&info).map_err(|e| {
                Error::new(ErrorCategory::Internal, format!("json encode failed: {e}"))
            })?;
            println!("{json}");
            Ok(())
        }
        "run-functional" => run_functional(&args[1..]),
        "check-config" => check_config(&args[1..]),
        other => Err(
            Error::new(ErrorCategory::Config, format!("unknown command '{other}'"))
                .with_remediation("run `softgpu help` for supported commands"),
        ),
    }
}

fn run_functional(args: &[String]) -> Result<()> {
    // Usage:
    //   softgpu run-functional --builtin tiny_add [--n 256] [--wg 64]
    //   softgpu run-functional <program.sfir.json> --n 256 --wg 64
    let mut builtin: Option<String> = None;
    let mut path: Option<PathBuf> = None;
    let mut n: u32 = 256;
    let mut wg: u32 = 64;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--builtin" => {
                i += 1;
                builtin = Some(
                    args.get(i)
                        .ok_or_else(|| {
                            Error::new(ErrorCategory::Config, "--builtin requires a name")
                        })?
                        .clone(),
                );
            }
            "--n" => {
                i += 1;
                n = args
                    .get(i)
                    .ok_or_else(|| Error::new(ErrorCategory::Config, "--n requires a value"))?
                    .parse()
                    .map_err(|_| Error::new(ErrorCategory::Validation, "invalid --n"))?;
            }
            "--wg" => {
                i += 1;
                wg = args
                    .get(i)
                    .ok_or_else(|| Error::new(ErrorCategory::Config, "--wg requires a value"))?
                    .parse()
                    .map_err(|_| Error::new(ErrorCategory::Validation, "invalid --wg"))?;
            }
            other if !other.starts_with('-') && path.is_none() && builtin.is_none() => {
                path = Some(PathBuf::from(other));
            }
            other => {
                return Err(Error::new(
                    ErrorCategory::Config,
                    format!("unknown run-functional arg '{other}'"),
                )
                .with_remediation(
                    "usage: softgpu run-functional --builtin tiny_add [--n N] [--wg WG]",
                ));
            }
        }
        i += 1;
    }

    let program = if let Some(name) = builtin {
        match name.as_str() {
            "tiny_add" => tiny_add(),
            other => {
                return Err(Error::new(
                    ErrorCategory::Config,
                    format!("unknown builtin '{other}'"),
                )
                .with_remediation("builtins: tiny_add"));
            }
        }
    } else if let Some(p) = path {
        load_program_path(&p).map_err(|e| {
            let cat = match &e {
                softgpu_functional::FunctionalError::Io(_) => ErrorCategory::Io,
                softgpu_functional::FunctionalError::Unsupported { .. } => {
                    ErrorCategory::Unsupported
                }
                _ => ErrorCategory::Validation,
            };
            Error::new(cat, e.to_string()).with_remediation("see docs/functional-path.md")
        })?
    } else {
        return Err(Error::new(
            ErrorCategory::Config,
            "run-functional requires --builtin <name> or a .sfir.json path",
        )
        .with_remediation("example: softgpu run-functional --builtin tiny_add --n 256 --wg 64"));
    };

    let mut arena = GlobalArena::new((n as usize) * 4 * 2);
    for i in 0..n {
        arena
            .store(u64::from(i) * 4, TypeId::I32, i64::from(i as i32))
            .map_err(|e| Error::new(ErrorCategory::Internal, e.to_string()))?;
    }
    let kernarg = kernarg_two_ptrs(0, u64::from(n) * 4);
    let launch = LaunchConfig {
        grid: [n, 1, 1],
        workgroup: [wg, 1, 1],
    };
    let report = run(&program, launch, &mut arena, &kernarg).map_err(|e| {
        let cat = match &e {
            softgpu_functional::FunctionalError::Unsupported { .. } => ErrorCategory::Unsupported,
            softgpu_functional::FunctionalError::Bounds { .. }
            | softgpu_functional::FunctionalError::Validation { .. } => ErrorCategory::Validation,
            _ => ErrorCategory::Internal,
        };
        Error::new(cat, e.to_string()).with_remediation("see docs/functional-path.md")
    })?;

    // Spot-check first and last element against host reference for tiny_add-shaped runs.
    let first = arena
        .load(u64::from(n) * 4, TypeId::I32)
        .map_err(|e| Error::new(ErrorCategory::Internal, e.to_string()))?;
    let last = arena
        .load(u64::from(n) * 4 + u64::from(n - 1) * 4, TypeId::I32)
        .map_err(|e| Error::new(ErrorCategory::Internal, e.to_string()))?;
    if program.name == "tiny_add" && (first != 1 || last != i64::from(n as i32)) {
        return Err(Error::new(
            ErrorCategory::Internal,
            format!("tiny_add reference mismatch first={first} last={last}"),
        ));
    }

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| Error::new(ErrorCategory::Internal, format!("json encode failed: {e}")))?;
    println!("{json}");
    Ok(())
}

fn check_config(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(Error::new(
            ErrorCategory::Config,
            "check-config requires KEY=VALUE arguments",
        )
        .with_remediation("example: softgpu check-config log_level=info"));
    }

    let mut seen_log_level = false;
    for raw in args {
        let (key, value) = raw.split_once('=').ok_or_else(|| {
            Error::new(
                ErrorCategory::Config,
                format!("malformed config entry '{raw}'; expected KEY=VALUE"),
            )
        })?;
        if key.is_empty() || value.is_empty() {
            return Err(Error::new(
                ErrorCategory::Config,
                format!("empty key or value in '{raw}'"),
            ));
        }
        match key {
            "log_level" => {
                match value {
                    "off" | "error" | "warn" | "info" | "debug" | "trace" => {}
                    _ => {
                        return Err(Error::new(
                            ErrorCategory::Validation,
                            format!("invalid log_level '{value}'"),
                        )
                        .with_remediation("allowed values: off, error, warn, info, debug, trace"));
                    }
                }
                seen_log_level = true;
            }
            "profile" => {
                let path = PathBuf::from(value);
                if !path.exists() {
                    return Err(Error::new(
                        ErrorCategory::Io,
                        format!("profile path does not exist: {value}"),
                    ));
                }
                DeviceProfile::load_path(&path)?;
            }
            "enable_queues" => {
                if !(value == "true" || value == "1" || value == "false" || value == "0") {
                    return Err(Error::new(
                        ErrorCategory::Validation,
                        format!("invalid enable_queues '{value}'"),
                    )
                    .with_remediation("allowed values: true, false, 1, 0"));
                }
            }
            "enable_execution" => {
                if value == "true" || value == "1" {
                    return Err(Error::new(
                        ErrorCategory::Unsupported,
                        "HIP/HSA AQL kernel execution is not enabled; use SoftGPU Functional IR",
                    )
                    .with_remediation(
                        "see docs/functional-path.md; `softgpu run-functional` runs SFIR only (not gfx1201 ISA)",
                    ));
                }
            }
            other => {
                return Err(Error::new(
                    ErrorCategory::Config,
                    format!("unknown config key '{other}'"),
                )
                .with_remediation(
                    "supported keys: log_level, profile, enable_queues, enable_execution",
                ));
            }
        }
    }

    if !seen_log_level {
        return Err(Error::new(
            ErrorCategory::Config,
            "check-config requires log_level=...",
        ));
    }

    println!("ok: config accepted (phase={ACTIVE_PHASE})");
    Ok(())
}

fn print_help() {
    println!(
        "\
softgpu {VERSION} — Phase 7 (SoftGPU Functional IR: waves/group/barriers)

USAGE:
  softgpu <command> [args]

COMMANDS:
  help                         Show this help
  version                      Print version
  info                         Print phase, fidelity policy, and HSA adapter notes
  validate-profile <path>      Validate a device profile JSON document
  check-config KEY=VALUE...    Validate a minimal config surface (negative-test aid)
  inspect-code-object <path>   Parse AMDGPU ELF metadata (no ISA execution)
  run-functional ...           Execute SoftGPU Functional IR (CPU; not gfx1201 ISA)

RUN-FUNCTIONAL:
  softgpu run-functional --builtin tiny_add [--n 256] [--wg 64]
  softgpu run-functional path/to/program.sfir.json [--n N] [--wg WG]

NOTES:
  Functional mode is SoftGPU-owned SFIR on the CPU. It is never gfx1201 ISA
  emulation. See docs/functional-path.md and Article 7.
"
    );
}

fn print_info() {
    println!("name=softgpu");
    println!("version={VERSION}");
    println!("active_phase={ACTIVE_PHASE}");
    println!("fidelity_policy=named-levels-required");
    println!("rocr_hsa_library=softgpu-hsa (memory/signals/queues/AQL + fail-closed stubs)");
    println!("agent_discovery=one-virtual-gpu");
    println!("feature=KERNEL_DISPATCH (queue+AQL intercept; SFIR functional separate)");
    println!("aql=diagnostic_complete_no_execution");
    println!("code_object=amdgpu_metadata_gfx1201_subset");
    println!("functional=softgpu-sfir-v1_cpu_waves_group_barriers_not_gfx1201_isa");
    println!("memory=path-c-regions-and-amd-pools");
    println!("msrv=1.85");
    println!("nightly_features=prohibited");
    println!("conformance_claims=none");
    println!("hip_rocm_load_proof=ci-rocm-integration (ROCm 7.14.0 pinned)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_command_is_config_error() {
        let err = run_cli(vec![]).unwrap_err();
        assert_eq!(err.category(), ErrorCategory::Config);
    }

    #[test]
    fn unknown_command_is_config_error() {
        let err = run_cli(vec!["launch-kernel".into()]).unwrap_err();
        assert_eq!(err.category(), ErrorCategory::Config);
        assert!(err.message().contains("unknown command"));
    }
}
