//! Built-in SoftGPU Functional IR kernels (Rust builders + JSON twins).

use crate::ir::{KernargField, Op, Program, TypeId, SFIR_SCHEMA};

fn base(name: &str, provenance: &str, body: Vec<Op>, layout: Vec<KernargField>) -> Program {
    base_group(name, provenance, body, layout, 0)
}

fn base_group(
    name: &str,
    provenance: &str,
    body: Vec<Op>,
    layout: Vec<KernargField>,
    group_bytes: u32,
) -> Program {
    Program {
        schema: SFIR_SCHEMA.into(),
        fidelity: "functional".into(),
        note: "not_gfx1201_isa_emulation".into(),
        name: name.into(),
        source_provenance: provenance.into(),
        kernarg_layout: layout,
        group_bytes,
        body,
    }
}

/// `b[i] = a[i] + 1` for i = global_id_x. Pointers in kernarg at 0 and 8.
pub fn tiny_add() -> Program {
    base(
        "tiny_add",
        "fixtures/functional/tiny_add.ref.c (hand-translated to softgpu-sfir-v1)",
        vec![
            Op::GlobalId {
                dst: "i".into(),
                dim: 0,
            },
            Op::Const {
                dst: "four".into(),
                ty: TypeId::U64,
                value: 4,
            },
            Op::Mul {
                dst: "off".into(),
                lhs: "i".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::KernargLoad {
                dst: "pa".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr_a".into(),
                lhs: "pa".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::LoadGlobal {
                dst: "va".into(),
                addr: "addr_a".into(),
                ty: TypeId::I32,
            },
            Op::Const {
                dst: "one".into(),
                ty: TypeId::I32,
                value: 1,
            },
            Op::Add {
                dst: "out".into(),
                lhs: "va".into(),
                rhs: "one".into(),
                ty: TypeId::I32,
            },
            Op::KernargLoad {
                dst: "pb".into(),
                offset: 8,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr_b".into(),
                lhs: "pb".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr_b".into(),
                src: "out".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![
            KernargField {
                name: "a".into(),
                offset: 0,
                size: 8,
                kind: "global_ptr".into(),
            },
            KernargField {
                name: "b".into(),
                offset: 8,
                size: 8,
                kind: "global_ptr".into(),
            },
        ],
    )
}

/// `b[i] = a[i]` copy.
pub fn tiny_copy() -> Program {
    base(
        "tiny_copy",
        "SoftGPU-authored softgpu-sfir-v1 (scalar copy)",
        vec![
            Op::GlobalId {
                dst: "i".into(),
                dim: 0,
            },
            Op::Const {
                dst: "four".into(),
                ty: TypeId::U64,
                value: 4,
            },
            Op::Mul {
                dst: "off".into(),
                lhs: "i".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::KernargLoad {
                dst: "pa".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr_a".into(),
                lhs: "pa".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::LoadGlobal {
                dst: "va".into(),
                addr: "addr_a".into(),
                ty: TypeId::I32,
            },
            Op::KernargLoad {
                dst: "pb".into(),
                offset: 8,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr_b".into(),
                lhs: "pb".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr_b".into(),
                src: "va".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![
            KernargField {
                name: "a".into(),
                offset: 0,
                size: 8,
                kind: "global_ptr".into(),
            },
            KernargField {
                name: "b".into(),
                offset: 8,
                size: 8,
                kind: "global_ptr".into(),
            },
        ],
    )
}

/// `out[i] = global_id_x` (index write).
pub fn tiny_index() -> Program {
    base(
        "tiny_index",
        "SoftGPU-authored softgpu-sfir-v1 (index write)",
        vec![
            Op::GlobalId {
                dst: "i".into(),
                dim: 0,
            },
            Op::Const {
                dst: "four".into(),
                ty: TypeId::U64,
                value: 4,
            },
            Op::Mul {
                dst: "off".into(),
                lhs: "i".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "p".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr".into(),
                src: "i".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "out".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
    )
}

/// Pack two u64 pointers into a 16-byte kernarg.
pub fn kernarg_two_ptrs(a: u64, b: u64) -> [u8; 16] {
    let mut k = [0u8; 16];
    k[0..8].copy_from_slice(&a.to_le_bytes());
    k[8..16].copy_from_slice(&b.to_le_bytes());
    k
}

pub fn kernarg_one_ptr(p: u64) -> [u8; 8] {
    p.to_le_bytes()
}

/// SoftGPU Phase 7: write `(wave_id * 16) + lane_id` to `out[global_id]`.
pub fn wave_lane_ids() -> Program {
    base(
        "wave_lane_ids",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 7 wave/lane indexing)",
        vec![
            Op::GlobalId {
                dst: "i".into(),
                dim: 0,
            },
            Op::WaveId { dst: "w".into() },
            Op::LaneId { dst: "l".into() },
            Op::Const {
                dst: "thousand".into(),
                ty: TypeId::I32,
                value: 1000,
            },
            Op::Mul {
                dst: "wh".into(),
                lhs: "w".into(),
                rhs: "thousand".into(),
                ty: TypeId::I32,
            },
            Op::Add {
                dst: "v".into(),
                lhs: "wh".into(),
                rhs: "l".into(),
                ty: TypeId::I32,
            },
            Op::Const {
                dst: "four".into(),
                ty: TypeId::U64,
                value: 4,
            },
            Op::Mul {
                dst: "off".into(),
                lhs: "i".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "p".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr".into(),
                src: "v".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "out".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
    )
}

/// SoftGPU Phase 7: even lanes add 1 to `buf[i]`; odd lanes leave value unchanged.
pub fn predicated_inc() -> Program {
    base(
        "predicated_inc",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 7 divergence)",
        vec![
            Op::GlobalId {
                dst: "i".into(),
                dim: 0,
            },
            Op::LaneId { dst: "l".into() },
            Op::Const {
                dst: "one".into(),
                ty: TypeId::I32,
                value: 1,
            },
            Op::Const {
                dst: "zero".into(),
                ty: TypeId::I32,
                value: 0,
            },
            Op::And {
                dst: "odd".into(),
                lhs: "l".into(),
                rhs: "one".into(),
                ty: TypeId::I32,
            },
            Op::CmpEq {
                dst: "even".into(),
                lhs: "odd".into(),
                rhs: "zero".into(),
                ty: TypeId::I32,
            },
            Op::Const {
                dst: "four".into(),
                ty: TypeId::U64,
                value: 4,
            },
            Op::Mul {
                dst: "off".into(),
                lhs: "i".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "p".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::If {
                cond: "even".into(),
                then_body: vec![
                    Op::LoadGlobal {
                        dst: "v".into(),
                        addr: "addr".into(),
                        ty: TypeId::I32,
                    },
                    Op::Add {
                        dst: "out".into(),
                        lhs: "v".into(),
                        rhs: "one".into(),
                        ty: TypeId::I32,
                    },
                    Op::StoreGlobal {
                        addr: "addr".into(),
                        src: "out".into(),
                        ty: TypeId::I32,
                    },
                ],
                else_body: vec![],
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "buf".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
    )
}

/// SoftGPU Phase 7: `group[lid]=lid; barrier; out[gid]=group[(lid+1)%wg]`.
pub fn group_exchange(workgroup_x: u32) -> Program {
    let wg = i64::from(workgroup_x);
    base_group(
        "group_exchange",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 7 group memory + barrier)",
        vec![
            Op::LocalId {
                dst: "lid".into(),
                dim: 0,
            },
            Op::GlobalId {
                dst: "gid".into(),
                dim: 0,
            },
            Op::Const {
                dst: "four".into(),
                ty: TypeId::U64,
                value: 4,
            },
            Op::Mul {
                dst: "goff".into(),
                lhs: "lid".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::StoreGroup {
                addr: "goff".into(),
                src: "lid".into(),
                ty: TypeId::I32,
            },
            Op::Barrier,
            Op::Const {
                dst: "one".into(),
                ty: TypeId::I32,
                value: 1,
            },
            Op::Const {
                dst: "wg".into(),
                ty: TypeId::I32,
                value: wg,
            },
            Op::Add {
                dst: "np1".into(),
                lhs: "lid".into(),
                rhs: "one".into(),
                ty: TypeId::I32,
            },
            Op::CmpEq {
                dst: "wrap".into(),
                lhs: "np1".into(),
                rhs: "wg".into(),
                ty: TypeId::I32,
            },
            Op::If {
                cond: "wrap".into(),
                then_body: vec![Op::Const {
                    dst: "nbr".into(),
                    ty: TypeId::I32,
                    value: 0,
                }],
                else_body: vec![Op::Add {
                    dst: "nbr".into(),
                    lhs: "lid".into(),
                    rhs: "one".into(),
                    ty: TypeId::I32,
                }],
            },
            Op::Mul {
                dst: "noff".into(),
                lhs: "nbr".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::LoadGroup {
                dst: "val".into(),
                addr: "noff".into(),
                ty: TypeId::I32,
            },
            Op::Mul {
                dst: "ooff".into(),
                lhs: "gid".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "p".into(),
                rhs: "ooff".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr".into(),
                src: "val".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "out".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
        workgroup_x * 4,
    )
}

/// SoftGPU Phase 7: each lane `atomic_add(counter, 1)`; write previous value to `out[i]`.
pub fn atomic_inc_reduce() -> Program {
    use crate::ir::{AddrSpace, AtomicOrder, AtomicScope};
    base(
        "atomic_inc_reduce",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 7 atomics)",
        vec![
            Op::GlobalId {
                dst: "i".into(),
                dim: 0,
            },
            Op::Const {
                dst: "one".into(),
                ty: TypeId::I32,
                value: 1,
            },
            Op::Const {
                dst: "z".into(),
                ty: TypeId::U64,
                value: 0,
            },
            Op::KernargLoad {
                dst: "pc".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "caddr".into(),
                lhs: "pc".into(),
                rhs: "z".into(),
                ty: TypeId::U64,
            },
            Op::AtomicAdd {
                dst: "old".into(),
                addr: "caddr".into(),
                src: "one".into(),
                space: AddrSpace::Global,
                scope: AtomicScope::Device,
                order: AtomicOrder::Relaxed,
            },
            Op::Const {
                dst: "four".into(),
                ty: TypeId::U64,
                value: 4,
            },
            Op::Mul {
                dst: "off".into(),
                lhs: "i".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::KernargLoad {
                dst: "po".into(),
                offset: 8,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "po".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr".into(),
                src: "old".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![
            KernargField {
                name: "counter".into(),
                offset: 0,
                size: 8,
                kind: "global_ptr".into(),
            },
            KernargField {
                name: "out".into(),
                offset: 8,
                size: 8,
                kind: "global_ptr".into(),
            },
        ],
    )
}

/// SoftGPU Phase 7: busy loop until step budget trips (watchdog).
pub fn infinite_loop_watchdog() -> Program {
    base(
        "infinite_loop_watchdog",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 7 step-budget watchdog)",
        vec![
            Op::Const {
                dst: "one".into(),
                ty: TypeId::I32,
                value: 1,
            },
            Op::Const {
                dst: "tmp".into(),
                ty: TypeId::I32,
                value: 0,
            },
            Op::While {
                cond: "one".into(),
                body: vec![Op::Add {
                    dst: "tmp".into(),
                    lhs: "tmp".into(),
                    rhs: "one".into(),
                    ty: TypeId::I32,
                }],
            },
            Op::Ret,
        ],
        vec![],
    )
}

/// SoftGPU Phase 8 (negative): every workitem stores to global offset 0 (race).
pub fn race_all_store_global_zero() -> Program {
    base(
        "race_all_store_global_zero",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 8 intentional global race)",
        vec![
            Op::Const {
                dst: "z".into(),
                ty: TypeId::U64,
                value: 0,
            },
            Op::Const {
                dst: "one".into(),
                ty: TypeId::I32,
                value: 1,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "p".into(),
                rhs: "z".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr".into(),
                src: "one".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "buf".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
    )
}

/// SoftGPU Phase 8 (negative): store at `base + (n * 4)` — one past last element.
pub fn oob_store_past_end(n: u32) -> Program {
    let past = i64::from(n) * 4;
    base(
        "oob_store_past_end",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 8 intentional OOB store)",
        vec![
            Op::Const {
                dst: "off".into(),
                ty: TypeId::U64,
                value: past,
            },
            Op::Const {
                dst: "one".into(),
                ty: TypeId::I32,
                value: 1,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "p".into(),
                rhs: "off".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr".into(),
                src: "one".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "buf".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
    )
}

/// SoftGPU Phase 8 (negative): group exchange without barrier (MissingBarrier).
pub fn group_exchange_missing_barrier(workgroup_x: u32) -> Program {
    let wg = i64::from(workgroup_x);
    base_group(
        "group_exchange_missing_barrier",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 8 intentional missing barrier)",
        vec![
            Op::LocalId {
                dst: "lid".into(),
                dim: 0,
            },
            Op::GlobalId {
                dst: "gid".into(),
                dim: 0,
            },
            Op::Const {
                dst: "four".into(),
                ty: TypeId::U64,
                value: 4,
            },
            Op::Mul {
                dst: "goff".into(),
                lhs: "lid".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::StoreGroup {
                addr: "goff".into(),
                src: "lid".into(),
                ty: TypeId::I32,
            },
            // Intentionally no Barrier.
            Op::Const {
                dst: "one".into(),
                ty: TypeId::I32,
                value: 1,
            },
            Op::Const {
                dst: "wg".into(),
                ty: TypeId::I32,
                value: wg,
            },
            Op::Add {
                dst: "np1".into(),
                lhs: "lid".into(),
                rhs: "one".into(),
                ty: TypeId::I32,
            },
            Op::CmpEq {
                dst: "wrap".into(),
                lhs: "np1".into(),
                rhs: "wg".into(),
                ty: TypeId::I32,
            },
            Op::If {
                cond: "wrap".into(),
                then_body: vec![Op::Const {
                    dst: "nid".into(),
                    ty: TypeId::I32,
                    value: 0,
                }],
                else_body: vec![Op::Add {
                    dst: "nid".into(),
                    lhs: "lid".into(),
                    rhs: "one".into(),
                    ty: TypeId::I32,
                }],
            },
            Op::Mul {
                dst: "noff".into(),
                lhs: "nid".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::LoadGroup {
                dst: "val".into(),
                addr: "noff".into(),
                ty: TypeId::I32,
            },
            Op::Mul {
                dst: "ooff".into(),
                lhs: "gid".into(),
                rhs: "four".into(),
                ty: TypeId::U64,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "p".into(),
                rhs: "ooff".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr".into(),
                src: "val".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "out".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
        workgroup_x * 4,
    )
}

/// SoftGPU Phase 8 (negative): read group memory before any store (uninit).
pub fn uninit_group_read() -> Program {
    base_group(
        "uninit_group_read",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 8 intentional uninit group read)",
        vec![
            Op::Const {
                dst: "z".into(),
                ty: TypeId::U64,
                value: 0,
            },
            Op::LoadGroup {
                dst: "v".into(),
                addr: "z".into(),
                ty: TypeId::I32,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "p".into(),
                src: "v".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "out".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
        4,
    )
}

/// SoftGPU Phase 8 (negative): every workgroup stores to the same global cell.
pub fn cross_workgroup_store_zero() -> Program {
    base(
        "cross_workgroup_store_zero",
        "SoftGPU-authored softgpu-sfir-v1 (Phase 8 intentional cross-WG race)",
        vec![
            Op::Const {
                dst: "z".into(),
                ty: TypeId::U64,
                value: 0,
            },
            Op::WorkgroupId {
                dst: "w".into(),
                dim: 0,
            },
            Op::KernargLoad {
                dst: "p".into(),
                offset: 0,
                ty: TypeId::U64,
            },
            Op::Add {
                dst: "addr".into(),
                lhs: "p".into(),
                rhs: "z".into(),
                ty: TypeId::U64,
            },
            Op::StoreGlobal {
                addr: "addr".into(),
                src: "w".into(),
                ty: TypeId::I32,
            },
            Op::Ret,
        ],
        vec![KernargField {
            name: "buf".into(),
            offset: 0,
            size: 8,
            kind: "global_ptr".into(),
        }],
    )
}

/// SoftGPU Phase 8 (positive): private per-lane global store (no race).
pub fn sanitize_clean_index() -> Program {
    tiny_index()
}
