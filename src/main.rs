//! SoftGPU CLI.

use softgpu::error::{Error, ErrorCategory, Result};
use softgpu::profile::DeviceProfile;
use softgpu::{ACTIVE_PHASE, VERSION};
use softgpu_amd_code_object::inspect_path;
use softgpu_amd_isa::{
    clamp64_host_ref, disasm_word, parse_hex_word, run_clamp64_1d, run_salu, run_select_gt50_1d,
    run_tiny_add_1d, select_gt50_host_ref, tiny_add_host_ref, words_to_code, Arch,
    GlobalArena as IsaGlobalArena, IsaMemory, MachineState, WaveSize, SUBSET_NAME, TARGET_ARCH,
};
use softgpu_functional::kernels::{kernarg_two_ptrs, tiny_add};
use softgpu_functional::{
    load_program_path, run_debug, run_with_config_sanitized, Breakpoint, ExecConfig, GlobalArena,
    LaunchConfig, SanitizeMode, TypeId, DEFAULT_TRACE_EVENT_BUDGET,
};
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
            "missing command; expected one of: help, version, info, validate-profile, check-config, inspect-code-object, run-functional, debug-functional, decode-isa, run-isa, run-kernel",
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
        "debug-functional" => debug_functional(&args[1..]),
        "decode-isa" => decode_isa(&args[1..]),
        "run-isa" => run_isa(&args[1..]),
        "run-kernel" => run_kernel(&args[1..]),
        "check-config" => check_config(&args[1..]),
        other => Err(
            Error::new(ErrorCategory::Config, format!("unknown command '{other}'"))
                .with_remediation("run `softgpu help` for supported commands"),
        ),
    }
}

fn run_functional(args: &[String]) -> Result<()> {
    // Usage:
    //   softgpu run-functional --builtin tiny_add [--n 256] [--wg 64] [--sanitize off|collect|fail_fast]
    //   softgpu run-functional <program.sfir.json> --n 256 --wg 64
    let mut builtin: Option<String> = None;
    let mut path: Option<PathBuf> = None;
    let mut n: u32 = 256;
    let mut wg: u32 = 64;
    let mut sanitize = SanitizeMode::Off;
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
            "--sanitize" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| {
                    Error::new(ErrorCategory::Config, "--sanitize requires a value")
                })?;
                sanitize = match v.as_str() {
                    "off" => SanitizeMode::Off,
                    "collect" => SanitizeMode::Collect,
                    "fail_fast" => SanitizeMode::FailFast,
                    other => {
                        return Err(Error::new(
                            ErrorCategory::Validation,
                            format!("invalid --sanitize '{other}'"),
                        )
                        .with_remediation("expected off|collect|fail_fast"));
                    }
                };
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
                    "usage: softgpu run-functional --builtin tiny_add [--n N] [--wg WG] [--sanitize MODE]",
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
    let mut cfg = ExecConfig::from_launch(launch);
    cfg.group_bytes = program.group_bytes;
    if program.has_barrier() {
        cfg.schedule = softgpu_functional::SchedulePolicy::WaveBarrier;
    }
    cfg.sanitize = sanitize;
    let (report, san) =
        run_with_config_sanitized(&program, cfg, &mut arena, &kernarg).map_err(|e| {
            let cat = match &e {
                softgpu_functional::FunctionalError::Unsupported { .. } => {
                    ErrorCategory::Unsupported
                }
                softgpu_functional::FunctionalError::Bounds { .. }
                | softgpu_functional::FunctionalError::Validation { .. }
                | softgpu_functional::FunctionalError::Sanitize(_) => ErrorCategory::Validation,
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
    if sanitize != SanitizeMode::Off {
        let sjson = serde_json::to_string_pretty(&san)
            .map_err(|e| Error::new(ErrorCategory::Internal, format!("json encode failed: {e}")))?;
        println!("{sjson}");
        if !san.ok() {
            return Err(Error::new(
                ErrorCategory::Validation,
                format!("sanitizer reported {} finding(s)", san.findings.len()),
            )
            .with_remediation("see docs/articles/09-building-gpu-sanitizers.md"));
        }
    }
    Ok(())
}

fn debug_functional(args: &[String]) -> Result<()> {
    // softgpu debug-functional --builtin tiny_add [--n N] [--wg WG] [--break-step N] [--break-mem]
    let mut builtin: Option<String> = None;
    let mut n: u32 = 64;
    let mut wg: u32 = 32;
    let mut break_step: Option<u64> = None;
    let mut break_mem = false;
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
            "--break-step" => {
                i += 1;
                break_step = Some(
                    args.get(i)
                        .ok_or_else(|| {
                            Error::new(ErrorCategory::Config, "--break-step requires a value")
                        })?
                        .parse()
                        .map_err(|_| {
                            Error::new(ErrorCategory::Validation, "invalid --break-step")
                        })?,
                );
            }
            "--break-mem" => {
                break_mem = true;
            }
            other => {
                return Err(Error::new(
                    ErrorCategory::Config,
                    format!("unknown debug-functional arg '{other}'"),
                )
                .with_remediation(
                    "usage: softgpu debug-functional --builtin tiny_add [--break-step N] [--break-mem]",
                ));
            }
        }
        i += 1;
    }

    let program = match builtin.as_deref() {
        Some("tiny_add") => tiny_add(),
        Some(other) => {
            return Err(
                Error::new(ErrorCategory::Config, format!("unknown builtin '{other}'"))
                    .with_remediation("builtins: tiny_add"),
            );
        }
        None => {
            return Err(Error::new(
                ErrorCategory::Config,
                "debug-functional requires --builtin <name>",
            )
            .with_remediation(
                "example: softgpu debug-functional --builtin tiny_add --break-step 5",
            ));
        }
    };

    let mut bps = Vec::new();
    if let Some(step) = break_step {
        bps.push(Breakpoint::AfterStep { step });
    }
    if break_mem {
        bps.push(Breakpoint::OnMemoryAccess);
    }
    if bps.is_empty() {
        bps.push(Breakpoint::AfterStep { step: 1 });
    }

    let mut arena = GlobalArena::new((n as usize) * 4 * 2);
    for i in 0..n {
        arena
            .store(u64::from(i) * 4, TypeId::I32, i64::from(i as i32))
            .map_err(|e| Error::new(ErrorCategory::Internal, e.to_string()))?;
    }
    let kernarg = kernarg_two_ptrs(0, u64::from(n) * 4);
    let mut cfg = ExecConfig::from_launch(LaunchConfig {
        grid: [n, 1, 1],
        workgroup: [wg, 1, 1],
    });
    cfg.group_bytes = program.group_bytes;

    let (report, trace) = run_debug(
        &program,
        cfg,
        &mut arena,
        &kernarg,
        bps,
        false,
        DEFAULT_TRACE_EVENT_BUDGET,
    )
    .map_err(|e| {
        Error::new(ErrorCategory::Validation, e.to_string())
            .with_remediation("see docs/articles/10-debugging-softgpu-lanes.md")
    })?;

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| Error::new(ErrorCategory::Internal, format!("json encode failed: {e}")))?;
    println!("{json}");
    print!(
        "{}",
        trace
            .to_jsonl()
            .map_err(|e| Error::new(ErrorCategory::Internal, e.to_string()))?
    );
    Ok(())
}

fn map_isa_err(e: softgpu_amd_isa::IsaError) -> Error {
    let cat = match e.kind() {
        softgpu_amd_isa::IsaErrorKind::Config => ErrorCategory::Config,
        softgpu_amd_isa::IsaErrorKind::Validation => ErrorCategory::Validation,
        softgpu_amd_isa::IsaErrorKind::Unsupported => ErrorCategory::Unsupported,
        softgpu_amd_isa::IsaErrorKind::Trap => ErrorCategory::Unsupported,
    };
    Error::new(cat, e.message().to_string()).with_remediation(
        "see docs/articles/11-decoding-amdgpu-isa.md; SoftGPU Phase 10 subset only",
    )
}

fn decode_isa(args: &[String]) -> Result<()> {
    // softgpu decode-isa 0xbe800081 [more words...]
    if args.is_empty() {
        return Err(Error::new(
            ErrorCategory::Config,
            "decode-isa requires one or more hex instruction words",
        )
        .with_remediation("usage: softgpu decode-isa 0xbf800000 0xbe800081"));
    }
    for (i, raw) in args.iter().enumerate() {
        let word = parse_hex_word(raw).map_err(map_isa_err)?;
        let text = disasm_word(word, (i as u32) * 4).map_err(map_isa_err)?;
        println!(
            "{i:4}: word=0x{word:08x} arch={TARGET_ARCH} subset={SUBSET_NAME} fidelity=architectural_isa :: {text}"
        );
    }
    Ok(())
}

fn run_isa(args: &[String]) -> Result<()> {
    // softgpu run-isa --words 0xbe800081,0xbe810082,0x80020100,0xbfb00000
    let mut words_arg: Option<&str> = None;
    let mut wave: u32 = 32;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--words" => {
                i += 1;
                words_arg = args.get(i).map(|s| s.as_str());
            }
            "--wave" => {
                i += 1;
                wave = args
                    .get(i)
                    .ok_or_else(|| Error::new(ErrorCategory::Config, "--wave requires 32 or 64"))?
                    .parse()
                    .map_err(|_| Error::new(ErrorCategory::Config, "invalid --wave value"))?;
            }
            other => {
                return Err(Error::new(
                    ErrorCategory::Config,
                    format!("unknown run-isa argument '{other}'"),
                )
                .with_remediation(
                    "usage: softgpu run-isa --words 0xbe800081,0xbfb00000 [--wave 32]",
                ));
            }
        }
        i += 1;
    }
    let words_raw = words_arg.ok_or_else(|| {
        Error::new(ErrorCategory::Config, "run-isa requires --words").with_remediation(
            "usage: softgpu run-isa --words 0xbe800081,0xbe810082,0x80020100,0xbfb00000",
        )
    })?;
    let mut words = Vec::new();
    for part in words_raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        words.push(parse_hex_word(part).map_err(map_isa_err)?);
    }
    if words.is_empty() {
        return Err(Error::new(
            ErrorCategory::Config,
            "run-isa --words list is empty",
        ));
    }
    let wave_size = WaveSize::parse(wave).map_err(map_isa_err)?;
    let code = words_to_code(&words);
    let mut st = MachineState::new(Arch::Gfx1201, wave_size);
    let steps = run_salu(&mut st, &code, 10_000).map_err(map_isa_err)?;
    println!(
        "{{\"fidelity\":\"architectural_isa\",\"arch\":\"{TARGET_ARCH}\",\"subset\":\"{SUBSET_NAME}\",\"steps\":{steps},\"halted\":{},\"scc\":{},\"sgpr0\":{},\"sgpr1\":{},\"sgpr2\":{},\"note\":\"Phase 10 SALU subset only; not HIP AQL kernel success\"}}",
        st.halted,
        st.scc,
        st.sgpr[0],
        st.sgpr[1],
        st.sgpr[2]
    );
    Ok(())
}

fn run_kernel(args: &[String]) -> Result<()> {
    // softgpu run-kernel --builtin tiny_add|clamp64|select_gt50 [--n 64]
    let mut builtin = None;
    let mut n: u32 = 64;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--builtin" => {
                i += 1;
                builtin = args.get(i).cloned();
            }
            "--n" => {
                i += 1;
                n = args
                    .get(i)
                    .ok_or_else(|| Error::new(ErrorCategory::Config, "--n requires a value"))?
                    .parse()
                    .map_err(|_| Error::new(ErrorCategory::Config, "invalid --n"))?;
            }
            other => {
                return Err(Error::new(
                    ErrorCategory::Config,
                    format!("unknown run-kernel argument '{other}'"),
                )
                .with_remediation(
                    "usage: softgpu run-kernel --builtin tiny_add|clamp64|select_gt50 [--n 64]",
                ));
            }
        }
        i += 1;
    }
    let builtin = builtin.ok_or_else(|| {
        Error::new(
            ErrorCategory::Config,
            "run-kernel requires --builtin tiny_add|clamp64|select_gt50",
        )
    })?;
    if n == 0 || n > 4096 {
        return Err(Error::new(
            ErrorCategory::Config,
            "run-kernel --n must be in 1..=4096",
        ));
    }

    let mut mem = IsaGlobalArena::new(0, 0x10000);
    let kernarg = 0x1000u64;
    let a_addr = 0x2000u64;
    let b_addr = 0x3000u64;
    mem.store_u64(kernarg, a_addr).map_err(map_isa_err)?;
    mem.store_u64(kernarg + 8, b_addr).map_err(map_isa_err)?;

    let (steps, ok) = match builtin.as_str() {
        "tiny_add" => {
            let mut host_a = vec![0i32; n as usize];
            let mut host_b = vec![0i32; n as usize];
            for (i, v) in host_a.iter_mut().enumerate() {
                *v = i as i32;
                mem.store_u32(a_addr + (i as u64) * 4, *v as u32)
                    .map_err(map_isa_err)?;
            }
            tiny_add_host_ref(&host_a, &mut host_b);
            let steps =
                run_tiny_add_1d(&mut mem, kernarg, n, WaveSize::Wave32).map_err(map_isa_err)?;
            let mut ok = true;
            for (i, expected) in host_b.iter().enumerate() {
                let got = mem.load_u32(b_addr + (i as u64) * 4).map_err(map_isa_err)? as i32;
                if got != *expected {
                    ok = false;
                    break;
                }
            }
            (steps, ok)
        }
        "clamp64" => {
            let mut host_a = vec![0u32; n as usize];
            let mut host_b = vec![0u32; n as usize];
            for (i, v) in host_a.iter_mut().enumerate() {
                *v = (i as u32) * 7;
                mem.store_u32(a_addr + (i as u64) * 4, *v)
                    .map_err(map_isa_err)?;
            }
            clamp64_host_ref(&host_a, &mut host_b);
            let steps =
                run_clamp64_1d(&mut mem, kernarg, n, WaveSize::Wave32).map_err(map_isa_err)?;
            let mut ok = true;
            for (i, expected) in host_b.iter().enumerate() {
                let got = mem.load_u32(b_addr + (i as u64) * 4).map_err(map_isa_err)?;
                if got != *expected {
                    ok = false;
                    break;
                }
            }
            (steps, ok)
        }
        "select_gt50" => {
            let mut host_a = vec![0u32; n as usize];
            let mut host_b = vec![0u32; n as usize];
            for (i, v) in host_a.iter_mut().enumerate() {
                *v = (i as u32) * 3;
                mem.store_u32(a_addr + (i as u64) * 4, *v)
                    .map_err(map_isa_err)?;
            }
            select_gt50_host_ref(&host_a, &mut host_b);
            let steps =
                run_select_gt50_1d(&mut mem, kernarg, n, WaveSize::Wave32).map_err(map_isa_err)?;
            let mut ok = true;
            for (i, expected) in host_b.iter().enumerate() {
                let got = mem.load_u32(b_addr + (i as u64) * 4).map_err(map_isa_err)?;
                if got != *expected {
                    ok = false;
                    break;
                }
            }
            (steps, ok)
        }
        other => {
            return Err(Error::new(
                ErrorCategory::Unsupported,
                format!("unsupported kernel builtin '{other}'"),
            )
            .with_remediation("builtins: tiny_add, clamp64, select_gt50"));
        }
    };

    println!(
        "{{\"fidelity\":\"architectural_isa\",\"arch\":\"{TARGET_ARCH}\",\"subset\":\"{SUBSET_NAME}\",\"kernel\":\"{builtin}\",\"n\":{n},\"steps\":{steps},\"host_diff_ok\":{ok},\"contract\":\"softgpu_kernel_success\",\"note\":\"llvm-mc gfx1201 text; SoftGPU compute-v2 calling convention\"}}"
    );
    if !ok {
        return Err(Error::new(
            ErrorCategory::Validation,
            format!("ISA {builtin} diverged from host reference"),
        ));
    }
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
softgpu {VERSION} — phase-hip-load (HSA executable + compute-v2 ISA)

USAGE:
  softgpu <command> [args]

COMMANDS:
  help                         Show this help
  version                      Print version
  info                         Print phase, fidelity policy, and HSA adapter notes
  validate-profile <path>      Validate a device profile JSON document
  check-config KEY=VALUE...    Validate a minimal config surface (negative-test aid)
  inspect-code-object <path>   Parse AMDGPU ELF metadata
  run-functional ...           SoftGPU Functional IR (CPU)
  debug-functional ...         SoftGPU SFIR debugger
  decode-isa <word>...         Disassemble SoftGPU gfx1201 subset words
  run-isa --words w0,w1,...    Step SALU words until s_endpgm
  run-kernel --builtin NAME [--n N]
                               Run SoftGPU llvm-mc kernel (tiny_add|clamp64|select_gt50)

NOTES:
  SoftGPU claims Architectural ISA for softgpu-gfx1201-compute-v2 and
  softgpu_kernel_success for SoftGPU-registered or SoftGPU-loaded agent images.
  See docs/HIP-gap-analysis.md and docs/isa-path.md.
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
    println!("feature=KERNEL_DISPATCH (queue+AQL; registered SoftGPU ISA kernels may execute)");
    println!("aql=diagnostic_or_softgpu_kernel_success");
    println!("code_object=amdgpu_metadata_gfx1201_subset");
    println!("functional=softgpu-sfir-v1_cpu_waves_sanitize_debug");
    println!("isa=softgpu-gfx1201-compute-v2_architectural_subset");
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
