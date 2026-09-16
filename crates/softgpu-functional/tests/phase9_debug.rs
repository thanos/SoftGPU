//! Phase 9 SoftGPU debugger and exploration charter tests.

use softgpu_functional::kernels::{kernarg_one_ptr, race_all_store_global_zero, tiny_index};
use softgpu_functional::{
    explore_and_minimize_race, inspect_sanitizer_finding, parse_trace_jsonl, run_debug,
    run_with_config_sanitized, Breakpoint, ExecConfig, FindingKind, FunctionalError, GlobalArena,
    LaunchConfig, SanitizeMode, SchedulePolicy, TraceEvent, DEBUG_TRACE_SCHEMA,
    DEFAULT_TRACE_EVENT_BUDGET,
};

fn launch_1d(n: u32, wg: u32) -> LaunchConfig {
    LaunchConfig {
        grid: [n, 1, 1],
        workgroup: [wg, 1, 1],
    }
}

#[test]
fn breakpoint_after_step_is_accurate() {
    let n = 4u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = ExecConfig::from_launch(launch_1d(n, n));
    let (report, trace) = run_debug(
        &tiny_index(),
        cfg,
        &mut arena,
        &k,
        vec![Breakpoint::AfterStep { step: 3 }],
        false,
        DEFAULT_TRACE_EVENT_BUDGET,
    )
    .unwrap();
    let stop = report.stop.expect("expected breakpoint stop");
    assert_eq!(stop.reason, "after_step");
    assert_eq!(stop.snapshot.step, 3);
    assert!(stop.snapshot.source_mapping.is_none());
    assert!(trace
        .events()
        .iter()
        .any(|e| matches!(e, TraceEvent::Break { .. })));
}

#[test]
fn memory_breakpoint_and_register_snapshot() {
    let n = 2u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = ExecConfig::from_launch(launch_1d(n, n));
    let (report, _) = run_debug(
        &tiny_index(),
        cfg,
        &mut arena,
        &k,
        vec![Breakpoint::OnMemoryAccess],
        false,
        DEFAULT_TRACE_EVENT_BUDGET,
    )
    .unwrap();
    let stop = report.stop.expect("memory bp");
    assert_eq!(stop.reason, "memory_access");
    assert!(
        stop.snapshot.registers.contains_key("i") || stop.snapshot.op.contains("store"),
        "regs={:?}",
        stop.snapshot.registers
    );
}

#[test]
fn trace_round_trip_across_serialize_boundary() {
    let n = 2u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = ExecConfig::from_launch(launch_1d(n, n));
    let (_report, trace) = run_debug(
        &tiny_index(),
        cfg,
        &mut arena,
        &k,
        vec![Breakpoint::AfterStep { step: 1 }],
        false,
        DEFAULT_TRACE_EVENT_BUDGET,
    )
    .unwrap();
    let jsonl = trace.to_jsonl().unwrap();
    let parsed = parse_trace_jsonl(&jsonl).unwrap();
    assert_eq!(parsed, trace.events());
    match &parsed[0] {
        TraceEvent::Header { schema, .. } => assert_eq!(schema, DEBUG_TRACE_SCHEMA),
        other => panic!("expected header, got {other:?}"),
    }
}

#[test]
fn hostile_trace_rejected_safely() {
    assert!(matches!(
        parse_trace_jsonl("{not json\n"),
        Err(FunctionalError::Parse(_))
    ));
    assert!(matches!(
        parse_trace_jsonl("{\"step\":1}\n"),
        Err(FunctionalError::Parse(_)) | Err(FunctionalError::Validation { .. })
    ));
    let huge = "x".repeat(softgpu_functional::MAX_TRACE_BYTES + 1);
    assert!(matches!(
        parse_trace_jsonl(&huge),
        Err(FunctionalError::Validation { .. })
    ));
}

#[test]
fn trace_budget_exhaustion() {
    let n = 8u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = ExecConfig::from_launch(launch_1d(n, n));
    let err = run_debug(&tiny_index(), cfg, &mut arena, &k, vec![], false, 2).unwrap_err();
    match err {
        FunctionalError::Validation { detail } => assert!(detail.contains("budget")),
        other => panic!("expected budget validation, got {other}"),
    }
}

#[test]
fn inspect_sanitizer_race_finding() {
    let mut arena = GlobalArena::new(4);
    let k = kernarg_one_ptr(0);
    let mut cfg = ExecConfig::from_launch(launch_1d(4, 4));
    cfg.sanitize = SanitizeMode::FailFast;
    cfg.schedule = SchedulePolicy::LexWorkitem;
    let err =
        run_with_config_sanitized(&race_all_store_global_zero(), cfg, &mut arena, &k).unwrap_err();
    let FunctionalError::Sanitize(finding) = err else {
        panic!("expected sanitize race");
    };
    assert_eq!(finding.kind, FindingKind::Race);

    let mut arena2 = GlobalArena::new(4);
    let (report, _trace, bundle) = inspect_sanitizer_finding(
        &race_all_store_global_zero(),
        cfg,
        &mut arena2,
        &k,
        &finding,
    )
    .unwrap();
    assert!(report.stop.is_some());
    assert_eq!(bundle.finding.kind, FindingKind::Race);
    assert!(report
        .stop
        .as_ref()
        .unwrap()
        .snapshot
        .source_mapping
        .is_none());
}

#[test]
fn seeded_explore_finds_and_minimizes_race() {
    let mut base = ExecConfig::from_launch(launch_1d(8, 8));
    base.schedule = SchedulePolicy::LexWorkitem;
    let k = kernarg_one_ptr(0);
    let report =
        explore_and_minimize_race(&race_all_store_global_zero(), base, &k, 4, 0xC0FFEE, 16)
            .unwrap();
    assert!(
        report.trials.iter().any(|t| t.found),
        "trials={:?}",
        report.trials
    );
    let mini = report.minimized.expect("minimized");
    assert!(mini.found);
    assert!(mini.workgroup_x <= 8);
    assert!(mini.step.unwrap_or(0) >= 1);
}

#[test]
fn single_step_breaks_immediately() {
    let n = 2u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = ExecConfig::from_launch(launch_1d(n, n));
    let (report, trace) = run_debug(
        &tiny_index(),
        cfg,
        &mut arena,
        &k,
        vec![],
        true,
        DEFAULT_TRACE_EVENT_BUDGET,
    )
    .unwrap();
    let stop = report.stop.expect("single step");
    assert_eq!(stop.reason, "single_step");
    assert_eq!(stop.snapshot.step, 1);
    assert!(!trace.exhausted());
}

#[test]
fn run_debug_completes_without_breakpoints() {
    let n = 4u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = ExecConfig::from_launch(launch_1d(n, n));
    let (report, trace) = run_debug(
        &tiny_index(),
        cfg,
        &mut arena,
        &k,
        vec![],
        false,
        DEFAULT_TRACE_EVENT_BUDGET,
    )
    .unwrap();
    assert!(report.stop.is_none());
    assert!(report.run.is_some());
    assert!(trace
        .events()
        .iter()
        .any(|e| matches!(e, TraceEvent::Done { .. })));
}

#[test]
fn parse_trace_edge_cases() {
    assert!(parse_trace_jsonl("").unwrap().is_empty());
    assert!(parse_trace_jsonl("\n\n").unwrap().is_empty());

    let bad_schema = format!(
        "{}\n",
        serde_json::json!({
            "header": {
                "schema": "nope",
                "program_name": "x",
                "source_provenance": "p",
                "grid": [1,1,1],
                "workgroup": [1,1,1],
                "wave_size": 1,
                "schedule": "lex_workitem",
                "schedule_seed": 0,
                "note": "n"
            }
        })
    );
    // Internally tagged enum uses {"header":{...}} — wrong schema inside
    let header = serde_json::json!({
        "header": {
            "schema": "wrong-schema",
            "program_name": "x",
            "source_provenance": "p",
            "grid": [1,1,1],
            "workgroup": [1,1,1],
            "wave_size": 1,
            "schedule": "lex_workitem",
            "schedule_seed": 0,
            "note": "n"
        }
    });
    let line = format!("{header}\n");
    assert!(matches!(
        parse_trace_jsonl(&line),
        Err(FunctionalError::Validation { .. })
    ));
    let _ = bad_schema;

    let non_header_first = format!(
        "{}\n",
        serde_json::json!({"done": {"steps": 1, "workitems": 1}})
    );
    assert!(matches!(
        parse_trace_jsonl(&non_header_first),
        Err(FunctionalError::Validation { .. })
    ));

    let long_line = format!(
        "{}\n",
        "x".repeat(softgpu_functional::debug::MAX_TRACE_LINE_BYTES + 1)
    );
    assert!(matches!(
        parse_trace_jsonl(&long_line),
        Err(FunctionalError::Validation { .. })
    ));
}

#[test]
fn explore_race_free_kernel_has_no_hit() {
    let mut base = ExecConfig::from_launch(launch_1d(4, 4));
    base.schedule = SchedulePolicy::LexWorkitem;
    let k = kernarg_one_ptr(0);
    let report = explore_and_minimize_race(&tiny_index(), base, &k, 16, 1, 4).unwrap();
    assert!(report.trials.iter().all(|t| !t.found));
    assert!(report.minimized.is_none());
}

#[test]
fn debug_traces_barrier_on_group_exchange() {
    use softgpu_functional::kernels::group_exchange;
    let wg = 4u32;
    let mut arena = GlobalArena::new((wg as usize) * 4);
    let k = kernarg_one_ptr(0);
    let mut cfg = ExecConfig::from_launch(launch_1d(wg, wg));
    cfg.schedule = SchedulePolicy::WaveBarrier;
    cfg.wave_size = 2;
    // Break after many steps so barrier events are recorded first.
    let (report, trace) = run_debug(
        &group_exchange(wg),
        cfg,
        &mut arena,
        &k,
        vec![Breakpoint::AfterStep { step: 500 }],
        false,
        DEFAULT_TRACE_EVENT_BUDGET,
    )
    .unwrap();
    assert!(report.run.is_some() || report.stop.is_some());
    assert!(trace
        .events()
        .iter()
        .any(|e| matches!(e, TraceEvent::Barrier { .. })));
}

#[test]
fn noop_observer_via_run_with_observer() {
    use softgpu_functional::{run_with_observer, NoopObserver};
    let n = 4u32;
    let mut arena = GlobalArena::new((n as usize) * 4);
    let k = kernarg_one_ptr(0);
    let cfg = ExecConfig::from_launch(launch_1d(n, n));
    let mut obs = NoopObserver;
    let (report, _) = run_with_observer(&tiny_index(), cfg, &mut arena, &k, &mut obs).unwrap();
    assert_eq!(report.workitems_executed, u64::from(n));
}
