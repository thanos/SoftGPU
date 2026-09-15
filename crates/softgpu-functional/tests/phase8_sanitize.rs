//! Phase 8 SoftGPU sanitizer charter tests.

use softgpu_functional::kernels::{
    atomic_inc_reduce, cross_workgroup_store_zero, group_exchange, group_exchange_missing_barrier,
    kernarg_one_ptr, kernarg_two_ptrs, oob_store_past_end, race_all_store_global_zero, tiny_add,
    tiny_index, uninit_group_read,
};
use softgpu_functional::{
    run_with_config_sanitized, run_with_sanitizer, AddrSpace, ExecConfig, FindingKind,
    FunctionalError, GlobalArena, LaunchConfig, ReplayBundle, SanitizeMode, Sanitizer,
    SchedulePolicy, TypeId, MAX_SHADOW_BYTES, SANITIZER_REPLAY_SCHEMA,
};

fn launch_1d(n: u32, wg: u32) -> LaunchConfig {
    LaunchConfig {
        grid: [n, 1, 1],
        workgroup: [wg, 1, 1],
    }
}

fn cfg_sanitize(launch: LaunchConfig, mode: SanitizeMode) -> ExecConfig {
    let mut cfg = ExecConfig::from_launch(launch);
    cfg.sanitize = mode;
    cfg
}

#[test]
fn positive_tiny_add_and_group_exchange_clean() {
    let n = 64u32;
    let mut arena = GlobalArena::new((n as usize) * 8);
    arena.fill_i32_ramp(0);
    let k = kernarg_two_ptrs(0, (n as u64) * 4);
    let cfg = cfg_sanitize(launch_1d(n, 32), SanitizeMode::Collect);
    let (_r, san) = run_with_config_sanitized(&tiny_add(), cfg, &mut arena, &k).unwrap();
    assert!(san.ok(), "{:?}", san.findings);

    let wg = 16u32;
    let prog = group_exchange(wg);
    let mut a = GlobalArena::new((wg as usize) * 4);
    let k = kernarg_one_ptr(0);
    let mut cfg = cfg_sanitize(launch_1d(wg, wg), SanitizeMode::Collect);
    cfg.schedule = SchedulePolicy::WaveBarrier;
    cfg.wave_size = 8;
    let (_r, san) = run_with_config_sanitized(&prog, cfg, &mut a, &k).unwrap();
    assert!(san.ok(), "{:?}", san.findings);
}

#[test]
fn positive_atomics_do_not_race_under_softgpu_subset() {
    let n = 32u32;
    let mut arena = GlobalArena::new(4 + (n as usize) * 4);
    arena.store(0, TypeId::I32, 0).unwrap();
    let k = kernarg_two_ptrs(0, 4);
    let mut cfg = cfg_sanitize(launch_1d(n, n), SanitizeMode::Collect);
    cfg.schedule = SchedulePolicy::WaveBarrier;
    let (_r, san) = run_with_config_sanitized(&atomic_inc_reduce(), cfg, &mut arena, &k).unwrap();
    assert!(san.ok(), "{:?}", san.findings);
}

#[test]
fn negative_oob_fail_fast() {
    let n = 8u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = cfg_sanitize(launch_1d(1, 1), SanitizeMode::FailFast);
    let err = run_with_config_sanitized(&oob_store_past_end(n), cfg, &mut arena, &k).unwrap_err();
    match err {
        FunctionalError::Sanitize(f) => assert_eq!(f.kind, FindingKind::OutOfBounds),
        other => panic!("expected Sanitize OOB, got {other}"),
    }
}

#[test]
fn negative_use_after_free() {
    let n = 4u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = cfg_sanitize(launch_1d(n, n), SanitizeMode::FailFast);
    let mut san = Sanitizer::new(SanitizeMode::FailFast, arena.len(), 0).unwrap();
    san.mark_freed(AddrSpace::Global, 0, 4);
    let err = run_with_sanitizer(&tiny_index(), cfg, &mut arena, &k, san).unwrap_err();
    match err {
        FunctionalError::Sanitize(f) => assert_eq!(f.kind, FindingKind::UseAfterFree),
        other => panic!("expected UAF, got {other}"),
    }
}

#[test]
fn negative_uninitialized_group_read() {
    let mut arena = GlobalArena::new(4);
    let k = kernarg_one_ptr(0);
    let mut cfg = cfg_sanitize(launch_1d(1, 1), SanitizeMode::FailFast);
    cfg.schedule = SchedulePolicy::WaveBarrier;
    let err = run_with_config_sanitized(&uninit_group_read(), cfg, &mut arena, &k).unwrap_err();
    match err {
        FunctionalError::Sanitize(f) => assert_eq!(f.kind, FindingKind::UninitializedRead),
        other => panic!("expected uninit, got {other}"),
    }
}

#[test]
fn negative_global_race() {
    let mut arena = GlobalArena::new(4);
    let k = kernarg_one_ptr(0);
    let mut cfg = cfg_sanitize(launch_1d(8, 8), SanitizeMode::FailFast);
    cfg.schedule = SchedulePolicy::LexWorkitem;
    let err =
        run_with_config_sanitized(&race_all_store_global_zero(), cfg, &mut arena, &k).unwrap_err();
    match err {
        FunctionalError::Sanitize(f) => {
            assert_eq!(f.kind, FindingKind::Race);
            assert!(f.other.is_some());
        }
        other => panic!("expected Race, got {other}"),
    }
}

#[test]
fn negative_missing_barrier() {
    let wg = 8u32;
    let mut arena = GlobalArena::new((wg as usize) * 4);
    let k = kernarg_one_ptr(0);
    let mut cfg = cfg_sanitize(launch_1d(wg, wg), SanitizeMode::FailFast);
    cfg.schedule = SchedulePolicy::WaveBarrier;
    cfg.wave_size = wg; // one SoftGPU wave so stores precede loads
    let err = run_with_config_sanitized(&group_exchange_missing_barrier(wg), cfg, &mut arena, &k)
        .unwrap_err();
    match err {
        FunctionalError::Sanitize(f) => assert_eq!(f.kind, FindingKind::MissingBarrier),
        other => panic!("expected MissingBarrier, got {other}"),
    }
}

#[test]
fn negative_cross_workgroup_race() {
    let mut arena = GlobalArena::new(4);
    let k = kernarg_one_ptr(0);
    // grid=2, workgroup=1 → two workgroups, each one lane
    let mut cfg = cfg_sanitize(
        LaunchConfig {
            grid: [2, 1, 1],
            workgroup: [1, 1, 1],
        },
        SanitizeMode::FailFast,
    );
    cfg.schedule = SchedulePolicy::LexWorkitem;
    let err =
        run_with_config_sanitized(&cross_workgroup_store_zero(), cfg, &mut arena, &k).unwrap_err();
    match err {
        FunctionalError::Sanitize(f) => assert_eq!(f.kind, FindingKind::CrossWorkgroupRace),
        other => panic!("expected CrossWorkgroupRace, got {other}"),
    }
}

#[test]
fn collect_mode_records_race_without_stopping_on_soft_finding() {
    let mut arena = GlobalArena::new(4);
    let k = kernarg_one_ptr(0);
    let mut cfg = cfg_sanitize(launch_1d(4, 4), SanitizeMode::Collect);
    cfg.schedule = SchedulePolicy::LexWorkitem;
    let (_r, san) =
        run_with_config_sanitized(&race_all_store_global_zero(), cfg, &mut arena, &k).unwrap();
    assert!(!san.ok());
    assert!(san.findings.iter().any(|f| f.kind == FindingKind::Race));
}

#[test]
fn shadow_limit_rejected() {
    let err = Sanitizer::new(SanitizeMode::Collect, MAX_SHADOW_BYTES, 1).unwrap_err();
    match err {
        FunctionalError::Sanitize(f) => assert_eq!(f.kind, FindingKind::ShadowLimit),
        other => panic!("expected ShadowLimit, got {other}"),
    }
}

#[test]
fn replay_bundle_is_deterministic() {
    let n = 8u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = cfg_sanitize(launch_1d(1, 1), SanitizeMode::FailFast);
    let prog = oob_store_past_end(n);
    let err = run_with_config_sanitized(&prog, cfg, &mut arena, &k).unwrap_err();
    let FunctionalError::Sanitize(f) = err else {
        panic!("expected sanitize");
    };
    let b1 = ReplayBundle::from_finding(&prog, &cfg, f.clone());
    let b2 = ReplayBundle::from_finding(&prog, &cfg, f);
    assert_eq!(b1.schema, SANITIZER_REPLAY_SCHEMA);
    assert_eq!(b1, b2);
    let j1 = serde_json::to_string(&b1).unwrap();
    let j2 = serde_json::to_string(&b2).unwrap();
    assert_eq!(j1, j2);
}
