//! SoftGPU Phase 0 CLI.
//!
//! Commands intentionally stay small: version/info, profile validation, and
//! config probes used by negative tests. No ROCr/HSA surface is exposed yet.

use softgpu::error::{Error, ErrorCategory, Result};
use softgpu::profile::DeviceProfile;
use softgpu::{ACTIVE_PHASE, VERSION};
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
            "missing command; expected one of: help, version, info, validate-profile, check-config",
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
        "check-config" => check_config(&args[1..]),
        other => Err(
            Error::new(ErrorCategory::Config, format!("unknown command '{other}'"))
                .with_remediation("run `softgpu help` for supported commands"),
        ),
    }
}

fn check_config(args: &[String]) -> Result<()> {
    // Negative-test friendly config probe: rejects unknown keys and empty values.
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
                // Phase 0: queues are unsupported; refuse to pretend.
                if value == "true" || value == "1" {
                    return Err(Error::new(
                        ErrorCategory::Unsupported,
                        "queues are not implemented in phase 0",
                    )
                    .with_remediation("see docs/status.md; queue work begins in phase 3"));
                }
            }
            other => {
                return Err(Error::new(
                    ErrorCategory::Config,
                    format!("unknown config key '{other}'"),
                )
                .with_remediation("supported keys: log_level, profile, enable_queues"));
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
softgpu {VERSION} — Phase 0 skeleton

USAGE:
  softgpu <command> [args]

COMMANDS:
  help                         Show this help
  version                      Print version
  info                         Print phase, fidelity policy, and toolchain notes
  validate-profile <path>      Validate a device profile JSON document
  check-config KEY=VALUE...    Validate a minimal config surface (negative-test aid)

NOTES:
  SoftGPU does not implement ROCr/HSA in Phase 0.
  Unsupported behavior fails closed; never silently succeeds.
  See README.md and docs/status.md.
"
    );
}

fn print_info() {
    println!("name=softgpu");
    println!("version={VERSION}");
    println!("active_phase={ACTIVE_PHASE}");
    println!("fidelity_policy=named-levels-required");
    println!("rocr_hsa_library=not-implemented");
    println!("msrv=1.85");
    println!("nightly_features=prohibited");
    println!("conformance_claims=none");
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
