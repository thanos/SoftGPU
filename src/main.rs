//! SoftGPU CLI.

use softgpu::error::{Error, ErrorCategory, Result};
use softgpu::profile::DeviceProfile;
use softgpu::{ACTIVE_PHASE, VERSION};
use softgpu_amd_code_object::inspect_path;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
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

fn run(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        return Err(Error::new(
            ErrorCategory::Config,
            "missing command; expected one of: help, version, info, validate-profile, check-config, inspect-code-object",
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
                    softgpu_amd_code_object::CodeObjectError::NotElf
                    | softgpu_amd_code_object::CodeObjectError::Truncated { .. }
                    | softgpu_amd_code_object::CodeObjectError::BadHeader { .. }
                    | softgpu_amd_code_object::CodeObjectError::SectionFault { .. }
                    | softgpu_amd_code_object::CodeObjectError::MsgPack { .. }
                    | softgpu_amd_code_object::CodeObjectError::Metadata { .. }
                    | softgpu_amd_code_object::CodeObjectError::NoteNotFound
                    | softgpu_amd_code_object::CodeObjectError::TooLarge { .. }
                    | softgpu_amd_code_object::CodeObjectError::LimitExceeded { .. } => {
                        ErrorCategory::Validation
                    }
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
        "check-config" => check_config(&args[1..]),
        other => Err(
            Error::new(ErrorCategory::Config, format!("unknown command '{other}'"))
                .with_remediation("run `softgpu help` for supported commands"),
        ),
    }
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
                        "kernel execution is not implemented in phase 5",
                    )
                    .with_remediation("see docs/status.md; Phase 5 is code-object metadata only"));
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
softgpu {VERSION} — Phase 5 (AMD code-object metadata)

USAGE:
  softgpu <command> [args]

COMMANDS:
  help                         Show this help
  version                      Print version
  info                         Print phase, fidelity policy, and HSA adapter notes
  validate-profile <path>      Validate a device profile JSON document
  check-config KEY=VALUE...    Validate a minimal config surface (negative-test aid)
  inspect-code-object <path>   Parse AMDGPU ELF metadata (no ISA execution)

NOTES:
  SoftGPU parses NT_AMDGPU_METADATA for gfx1201 targets. This is metadata
  inspection only — not kernel execution or gfx1201 ISA emulation.
  See README.md, docs/status.md, and docs/code-object.md.
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
    println!("feature=KERNEL_DISPATCH (queue+AQL intercept; no kernel execution)");
    println!("aql=diagnostic_complete_no_execution");
    println!("code_object=amdgpu_metadata_gfx1201_subset");
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
        let err = run(vec![]).unwrap_err();
        assert_eq!(err.category(), ErrorCategory::Config);
    }

    #[test]
    fn unknown_command_is_config_error() {
        let err = run(vec!["launch-kernel".into()]).unwrap_err();
        assert_eq!(err.category(), ErrorCategory::Config);
        assert!(err.message().contains("unknown command"));
    }
}
