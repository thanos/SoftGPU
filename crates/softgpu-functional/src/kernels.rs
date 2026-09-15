//! Built-in SoftGPU Functional IR kernels (Rust builders + JSON twins).

use crate::ir::{KernargField, Op, Program, TypeId, SFIR_SCHEMA};

fn base(name: &str, provenance: &str, body: Vec<Op>, layout: Vec<KernargField>) -> Program {
    Program {
        schema: SFIR_SCHEMA.into(),
        fidelity: "functional".into(),
        note: "not_gfx1201_isa_emulation".into(),
        name: name.into(),
        source_provenance: provenance.into(),
        kernarg_layout: layout,
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
