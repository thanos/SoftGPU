//! Phase 6 functional execution charter tests.

use softgpu_functional::kernels::{
    kernarg_one_ptr, kernarg_two_ptrs, tiny_add, tiny_copy, tiny_index,
};
use softgpu_functional::{
    load_program_str, run, FunctionalError, GlobalArena, LaunchConfig, Op, Program, TypeId,
    FUNCTIONAL_MODE_MARKER, SFIR_SCHEMA,
};

fn launch_1d(n: u32, wg: u32) -> LaunchConfig {
    LaunchConfig {
        grid: [n, 1, 1],
        workgroup: [wg, 1, 1],
    }
}

#[test]
fn tiny_add_matches_host_reference() {
    let n = 256u32;
    let mut arena = GlobalArena::new((n as usize) * 4 * 2);
    // a at 0, b at n*4
    for i in 0..n {
        arena
            .store(u64::from(i) * 4, TypeId::I32, i64::from(i as i32))
            .unwrap();
    }
    let kernarg = kernarg_two_ptrs(0, u64::from(n) * 4);
    let report = run(&tiny_add(), launch_1d(n, 64), &mut arena, &kernarg).unwrap();
    assert_eq!(report.fidelity, "functional");
    assert_eq!(report.mode, FUNCTIONAL_MODE_MARKER);
    assert_eq!(report.workitems_executed, u64::from(n));
    for i in 0..n {
        let v = arena
            .load(u64::from(n) * 4 + u64::from(i) * 4, TypeId::I32)
            .unwrap();
        assert_eq!(v, i64::from(i as i32) + 1);
    }
}

#[test]
fn tiny_copy_and_index() {
    let n = 128u32;
    let mut arena = GlobalArena::new((n as usize) * 4 * 2);
    for i in 0..n {
        arena
            .store(u64::from(i) * 4, TypeId::I32, 1000 + i64::from(i as i32))
            .unwrap();
    }
    let k = kernarg_two_ptrs(0, u64::from(n) * 4);
    run(&tiny_copy(), launch_1d(n, 32), &mut arena, &k).unwrap();
    for i in 0..n {
        let v = arena
            .load(u64::from(n) * 4 + u64::from(i) * 4, TypeId::I32)
            .unwrap();
        assert_eq!(v, 1000 + i64::from(i as i32));
    }

    let mut out = GlobalArena::new((n as usize) * 4);
    let k1 = kernarg_one_ptr(0);
    run(&tiny_index(), launch_1d(n, 16), &mut out, &k1).unwrap();
    for i in 0..n {
        assert_eq!(
            out.load(u64::from(i) * 4, TypeId::I32).unwrap(),
            i64::from(i)
        );
    }
}

#[test]
fn multi_workgroup_and_deterministic_rerun() {
    let n = 192u32; // 3 * 64
    let mut a = GlobalArena::new((n as usize) * 4 * 2);
    for i in 0..n {
        a.store(u64::from(i) * 4, TypeId::I32, i64::from(i as i32) * 2)
            .unwrap();
    }
    let k = kernarg_two_ptrs(0, u64::from(n) * 4);
    let r1 = run(&tiny_add(), launch_1d(n, 64), &mut a, &k).unwrap();
    let snap = a.as_slice().to_vec();

    let mut b = GlobalArena::from_bytes({
        let mut v = vec![0u8; (n as usize) * 4 * 2];
        for i in 0..n {
            let off = (i as usize) * 4;
            v[off..off + 4].copy_from_slice(&(i as i32 * 2).to_le_bytes());
        }
        v
    });
    let r2 = run(&tiny_add(), launch_1d(n, 64), &mut b, &k).unwrap();
    assert_eq!(r1.steps, r2.steps);
    assert_eq!(snap, b.as_slice());
}

#[test]
fn bounds_failure_on_oob_store() {
    let mut arena = GlobalArena::new(4); // only one i32
    let k = kernarg_two_ptrs(0, 0);
    // grid 2 will write index 1 → OOB
    let err = run(&tiny_add(), launch_1d(2, 1), &mut arena, &k).unwrap_err();
    assert!(matches!(err, FunctionalError::Bounds { .. }));
}

#[test]
fn unsupported_schema_and_missing_ret() {
    let bad = r#"{
      "schema": "nope",
      "fidelity": "functional",
      "note": "not_gfx1201_isa_emulation",
      "name": "x",
      "source_provenance": "t",
      "body": [{"op": "ret"}]
    }"#;
    assert!(matches!(
        load_program_str(bad),
        Err(FunctionalError::Validation { .. })
    ));

    let mut p = tiny_add();
    p.body.pop();
    assert!(matches!(
        p.validate(),
        Err(FunctionalError::Validation { .. })
    ));
}

#[test]
fn launch_validation_rejects_bad_dims() {
    let err = LaunchConfig {
        grid: [10, 1, 1],
        workgroup: [3, 1, 1],
    }
    .validate()
    .unwrap_err();
    assert!(matches!(err, FunctionalError::Validation { .. }));
}

#[test]
fn json_round_trip_tiny_add() {
    let p = tiny_add();
    let json = serde_json::to_string_pretty(&p).unwrap();
    let loaded = load_program_str(&json).unwrap();
    assert_eq!(loaded.schema, SFIR_SCHEMA);
    assert_eq!(loaded.name, "tiny_add");
    assert!(matches!(loaded.body.last(), Some(Op::Ret)));
}

#[test]
fn host_reference_differential_for_tiny_add() {
    // Documented host reference matching tiny_add.ref.c
    let n = 64usize;
    let a: Vec<i32> = (0..n as i32).collect();
    let mut b = vec![0i32; n];
    for i in 0..n {
        b[i] = a[i] + 1;
    }
    let mut arena = GlobalArena::new(n * 4 * 2);
    for (i, v) in a.iter().enumerate() {
        arena
            .store((i * 4) as u64, TypeId::I32, i64::from(*v))
            .unwrap();
    }
    let k = kernarg_two_ptrs(0, (n * 4) as u64);
    run(&tiny_add(), launch_1d(n as u32, 32), &mut arena, &k).unwrap();
    for (i, expected) in b.iter().enumerate() {
        let got = arena.load(((n + i) * 4) as u64, TypeId::I32).unwrap();
        assert_eq!(got, i64::from(*expected));
    }
}

#[test]
fn program_is_program() {
    let _: Program = tiny_add();
}
