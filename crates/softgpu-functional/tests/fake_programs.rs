//! Fake SoftGPU Functional IR programs and synthetic arenas.
//!
//! These are SoftGPU-owned fakes (not a mock framework): they exercise real
//! interpreter ops, fail-closed validation, and host arena bounds that the
//! charter kernels do not hit.

use softgpu_functional::memory::kernarg_load;
use softgpu_functional::{
    load_program_path, load_program_str, run, run_with_budget, FunctionalError, GlobalArena,
    LaunchConfig, Op, Program, TypeId, SFIR_SCHEMA,
};
use std::fs;
use std::path::PathBuf;

fn base_program(name: &str, body: Vec<Op>) -> Program {
    Program {
        schema: SFIR_SCHEMA.into(),
        fidelity: "functional".into(),
        note: "not_gfx1201_isa_emulation".into(),
        name: name.into(),
        source_provenance: "softgpu-test-fake".into(),
        kernarg_layout: vec![],
        body,
    }
}

fn launch_1x1() -> LaunchConfig {
    LaunchConfig {
        grid: [1, 1, 1],
        workgroup: [1, 1, 1],
    }
}

#[test]
fn fake_sfir_ids_sub_mul_typed_stores() {
    // Charter kernels only use global_id + i32 add. This fake locks the other
    // disclosed SFIR ops SoftGPU already claims to support.
    let prog = base_program(
        "fake_ids_arith",
        vec![
            Op::LocalId {
                dst: "lx".into(),
                dim: 0,
            },
            Op::WorkgroupId {
                dst: "wx".into(),
                dim: 0,
            },
            Op::GlobalId {
                dst: "gy".into(),
                dim: 1,
            },
            Op::Const {
                dst: "c2".into(),
                ty: TypeId::U32,
                value: 2,
            },
            Op::Const {
                dst: "c3".into(),
                ty: TypeId::U64,
                value: 3,
            },
            Op::Sub {
                dst: "s".into(),
                lhs: "c3".into(),
                rhs: "c2".into(),
                ty: TypeId::U64,
            },
            Op::Mul {
                dst: "m".into(),
                lhs: "s".into(),
                rhs: "c2".into(),
                ty: TypeId::U32,
            },
            Op::Const {
                dst: "base".into(),
                ty: TypeId::U64,
                value: 0,
            },
            Op::StoreGlobal {
                addr: "base".into(),
                src: "m".into(),
                ty: TypeId::U32,
            },
            Op::Const {
                dst: "base8".into(),
                ty: TypeId::U64,
                value: 8,
            },
            Op::StoreGlobal {
                addr: "base8".into(),
                src: "s".into(),
                ty: TypeId::U64,
            },
            Op::Ret,
        ],
    );
    let mut arena = GlobalArena::new(16);
    let report = run(&prog, launch_1x1(), &mut arena, &[]).unwrap();
    assert_eq!(report.workitems_executed, 1);
    assert_eq!(arena.load(0, TypeId::U32).unwrap(), 2);
    assert_eq!(arena.load(8, TypeId::U64).unwrap(), 1);
}

#[test]
fn fake_sfir_undefined_reg_and_step_budget_fail_closed() {
    let bad = base_program(
        "fake_undef",
        vec![
            Op::Add {
                dst: "x".into(),
                lhs: "missing".into(),
                rhs: "missing".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
    );
    let mut arena = GlobalArena::new(4);
    assert!(matches!(
        run(&bad, launch_1x1(), &mut arena, &[]),
        Err(FunctionalError::UndefinedReg { .. })
    ));

    let tiny = base_program(
        "fake_budget",
        vec![
            Op::Const {
                dst: "z".into(),
                ty: TypeId::I32,
                value: 0,
            },
            Op::Ret,
        ],
    );
    assert!(matches!(
        run_with_budget(&tiny, launch_1x1(), &mut arena, &[], 0),
        Err(FunctionalError::StepBudgetExceeded { .. })
    ));
}

#[test]
fn arena_typed_load_store_and_bounds_fail_closed() {
    let mut arena = GlobalArena::new(16);
    arena.fill_i32_ramp(10);
    assert_eq!(arena.read_i32_slice().unwrap()[0], 10);

    arena.store(0, TypeId::U32, 0xffff_ffffu32 as i64).unwrap();
    assert_eq!(arena.load(0, TypeId::U32).unwrap() as u32, 0xffff_ffff);
    arena.store(0, TypeId::U64, -1).unwrap();
    assert_eq!(arena.load(0, TypeId::U64).unwrap(), -1);

    assert!(matches!(
        arena.load(100, TypeId::I32),
        Err(FunctionalError::Bounds { .. })
    ));
    assert!(matches!(
        arena.store(14, TypeId::I32, 1),
        Err(FunctionalError::Bounds { .. })
    ));
    assert!(matches!(
        GlobalArena::from_bytes(vec![1, 2, 3]).read_i32_slice(),
        Err(FunctionalError::Validation { .. })
    ));

    let kern = 7u64.to_le_bytes();
    assert_eq!(kernarg_load(&kern, 0, TypeId::U64).unwrap(), 7);
    assert!(matches!(
        kernarg_load(&kern, 4, TypeId::U64),
        Err(FunctionalError::Bounds { .. })
    ));
}

#[test]
fn validate_rejects_bad_honesty_fields_and_dims() {
    let mut p = base_program("x", vec![Op::Ret]);
    p.fidelity = "abi".into();
    assert!(p.validate().is_err());
    p.fidelity = "functional".into();
    p.note = "wrong".into();
    assert!(p.validate().is_err());
    p.note = "not_gfx1201_isa_emulation".into();
    p.body = vec![
        Op::LocalId {
            dst: "g".into(),
            dim: 9,
        },
        Op::Ret,
    ];
    assert!(p.validate().is_err());
}

#[test]
fn missing_path_and_bad_json_fail_closed() {
    let missing = PathBuf::from("/tmp/softgpu-definitely-missing-sfir-9f3a2.json");
    assert!(matches!(
        load_program_path(&missing),
        Err(FunctionalError::Io(_))
    ));

    let dir = std::env::temp_dir().join(format!("softgpu-sfir-fake-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad.json");
    fs::write(&path, "{not-json").unwrap();
    assert!(matches!(
        load_program_path(&path),
        Err(FunctionalError::Parse(_))
    ));
    let _ = fs::remove_dir_all(&dir);

    assert!(matches!(
        load_program_str("[]"),
        Err(FunctionalError::Parse(_))
    ));
}

#[test]
fn launch_rejects_zero_and_oversize_dims() {
    assert!(LaunchConfig {
        grid: [0, 1, 1],
        workgroup: [1, 1, 1],
    }
    .validate()
    .is_err());
    assert!(LaunchConfig {
        grid: [2048, 1, 1],
        workgroup: [2048, 1, 1],
    }
    .validate()
    .is_err());
    let ok = LaunchConfig {
        grid: [8, 4, 2],
        workgroup: [2, 2, 1],
    };
    ok.validate().unwrap();
    assert_eq!(ok.num_workgroups(), [4, 2, 2]);
}
