//! Phase 7 GPU execution semantics charter tests.

use softgpu_functional::kernels::{
    atomic_inc_reduce, group_exchange, infinite_loop_watchdog, kernarg_one_ptr, kernarg_two_ptrs,
    predicated_inc, wave_lane_ids,
};
use softgpu_functional::{
    run, run_with_budget, run_with_config, ExecConfig, FunctionalError, GlobalArena, LaunchConfig,
    SchedulePolicy, TypeId, DEFAULT_WAVE_SIZE, FUNCTIONAL_MODE_MARKER, MAX_GROUP_BYTES,
};
use std::collections::BTreeSet;

fn launch_1d(n: u32, wg: u32) -> LaunchConfig {
    LaunchConfig {
        grid: [n, 1, 1],
        workgroup: [wg, 1, 1],
    }
}

#[test]
fn wave_lane_indexing_and_partial_wave() {
    let n = 40u32; // 32 + 8 → partial second wave
    let wg = 40u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let mut cfg = ExecConfig::from_launch(launch_1d(n, wg));
    cfg.schedule = SchedulePolicy::WaveBarrier;
    cfg.wave_size = DEFAULT_WAVE_SIZE;
    let report = run_with_config(&wave_lane_ids(), cfg, &mut arena, &k).unwrap();
    assert_eq!(report.mode, FUNCTIONAL_MODE_MARKER);
    assert_eq!(report.wave_size, 32);
    for i in 0..n {
        let v = arena.load(u64::from(i) * 4, TypeId::I32).unwrap() as u32;
        let wave = i / 32;
        let lane = i % 32;
        assert_eq!(v, wave * 1000 + lane, "i={i}");
    }
}

#[test]
fn predicated_divergence_even_lanes_only() {
    let n = 32u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    for i in 0..n {
        arena.store(u64::from(i) * 4, TypeId::I32, 10).unwrap();
    }
    let k = kernarg_one_ptr(0);
    let mut cfg = ExecConfig::from_launch(launch_1d(n, n));
    cfg.schedule = SchedulePolicy::WaveBarrier;
    run_with_config(&predicated_inc(), cfg, &mut arena, &k).unwrap();
    for i in 0..n {
        let v = arena.load(u64::from(i) * 4, TypeId::I32).unwrap();
        let expect = if i % 2 == 0 { 11 } else { 10 };
        assert_eq!(v, expect, "i={i}");
    }
}

#[test]
fn group_memory_exchange_barrier_and_schedule_metamorphism() {
    let wg = 16u32;
    let n = wg;
    let prog = group_exchange(wg);
    let mut a = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let mut cfg = ExecConfig::from_launch(launch_1d(n, wg));
    cfg.schedule = SchedulePolicy::WaveBarrier;
    cfg.wave_size = 8; // multiple waves in one WG
    let r1 = run_with_config(&prog, cfg, &mut a, &k).unwrap();
    assert_eq!(r1.barrier_generations, 1);
    for i in 0..n {
        let expect = ((i + 1) % wg) as i64;
        assert_eq!(
            a.load(u64::from(i) * 4, TypeId::I32).unwrap(),
            expect,
            "i={i}"
        );
    }

    // Metamorphism: same race-free kernel, different wave_size → same memory.
    let mut b = GlobalArena::new((n as usize) * 4);
    let mut cfg2 = cfg;
    cfg2.wave_size = 16;
    run_with_config(&prog, cfg2, &mut b, &k).unwrap();
    assert_eq!(a.as_slice(), b.as_slice());
}

#[test]
fn atomic_reduction_permutation_is_complete() {
    let n = 64u32;
    let mut arena = GlobalArena::new(4 + (n as usize) * 4);
    arena.store(0, TypeId::I32, 0).unwrap();
    let k = kernarg_two_ptrs(0, 4);
    let mut cfg = ExecConfig::from_launch(launch_1d(n, 32));
    cfg.schedule = SchedulePolicy::WaveBarrier;
    let report = run_with_config(&atomic_inc_reduce(), cfg, &mut arena, &k).unwrap();
    assert_eq!(report.atomics_executed, u64::from(n));
    assert_eq!(arena.load(0, TypeId::I32).unwrap(), i64::from(n));
    let mut seen = BTreeSet::new();
    for i in 0..n {
        let old = arena.load(4 + u64::from(i) * 4, TypeId::I32).unwrap() as u32;
        assert!(old < n);
        assert!(seen.insert(old), "duplicate old={old}");
    }
    assert_eq!(seen.len(), n as usize);
}

#[test]
fn infinite_loop_watchdog_trips_step_budget() {
    let err = run_with_budget(
        &infinite_loop_watchdog(),
        launch_1d(1, 1),
        &mut GlobalArena::new(0),
        &[],
        64,
    )
    .unwrap_err();
    assert!(matches!(err, FunctionalError::StepBudgetExceeded { .. }));
}

#[test]
fn resource_limit_rejects_oversize_group_before_exec() {
    let mut prog = group_exchange(8);
    prog.group_bytes = MAX_GROUP_BYTES + 1;
    let err = run(
        &prog,
        launch_1d(8, 8),
        &mut GlobalArena::new(32),
        &kernarg_one_ptr(0),
    )
    .unwrap_err();
    assert!(matches!(err, FunctionalError::Validation { .. }));
}

#[test]
fn barrier_program_rejects_lex_schedule() {
    let prog = group_exchange(8);
    let mut cfg = ExecConfig::from_launch(launch_1d(8, 8));
    cfg.schedule = SchedulePolicy::LexWorkitem;
    let err =
        run_with_config(&prog, cfg, &mut GlobalArena::new(32), &kernarg_one_ptr(0)).unwrap_err();
    assert!(matches!(err, FunctionalError::Validation { .. }));
}
