//! SoftGPU Phase 11 tiny gfx1201 kernel launch helpers.
//!
//! Calling convention (`softgpu-gfx1201-e2e-tiny-v1`):
//! - SoftGPU sets `s[4:5]` = kernarg base address before entry
//! - SoftGPU sets `v0` = global_id_x for each active lane
//! - SoftGPU sets EXEC for the wave
//! - Machine code produced by llvm-mc `-mcpu=gfx1201` (checked-in bytes)

use crate::arch::{Arch, WaveSize};
use crate::error::{IsaError, IsaErrorKind, Result};
use crate::exec::run;
use crate::state::MachineState;

/// llvm-mc assembled `.text` for SoftGPU `tiny_add` (`b[i]=a[i]+1` as i32).
///
/// Provenance: Homebrew LLVM 21.1.8 `llvm-mc -arch=amdgcn -mcpu=gfx1201`
/// access date 2026-09-16. See `goldens/` and Article 12.
pub const TINY_ADD_TEXT: &[u8] = &[
    0x02, 0x20, 0x00, 0xf4, 0x00, 0x00, 0x00, 0xf8, // s_load_b64 s[0:1], s[4:5], 0x0
    0x82, 0x20, 0x00, 0xf4, 0x08, 0x00, 0x00, 0xf8, // s_load_b64 s[2:3], s[4:5], 0x8
    0x00, 0x00, 0x89, 0xbf, // s_waitcnt
    0x82, 0x00, 0x02, 0x30, // v_lshlrev_b32_e32 v1, 2, v0
    0x00, 0x00, 0x05, 0xee, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, // global_load_b32 v2, v1, s[0:1]
    0x00, 0x00, 0x89, 0xbf, // s_waitcnt
    0x81, 0x04, 0x04, 0x4a, // v_add_nc_u32_e32 v2, 1, v2
    0x02, 0x80, 0x06, 0xee, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00,
    0x00, // global_store_b32 v1, v2, s[2:3]
    0x00, 0x00, 0xb0, 0xbf, // s_endpgm
];

/// SoftGPU kernarg layout for tiny_add: two device pointers.
#[derive(Debug, Clone, Copy)]
pub struct TinyAddKernarg {
    pub a_addr: u64,
    pub b_addr: u64,
}

/// Run SoftGPU tiny_add over `grid_x` workitems (1D), wave_size lanes at a time.
pub fn run_tiny_add_1d(
    mem: &mut dyn crate::mem::IsaMemory,
    kernarg_addr: u64,
    grid_x: u32,
    wave_size: WaveSize,
) -> Result<u64> {
    run_code_1d(TINY_ADD_TEXT, mem, kernarg_addr, grid_x, wave_size)
}

/// Generic 1D launch for SoftGPU e2e tiny calling convention.
pub fn run_code_1d(
    code: &[u8],
    mem: &mut dyn crate::mem::IsaMemory,
    kernarg_addr: u64,
    grid_x: u32,
    wave_size: WaveSize,
) -> Result<u64> {
    if code.is_empty() {
        return Err(IsaError::new(IsaErrorKind::Config, "empty kernel code"));
    }
    let lanes = wave_size.lanes() as u32;
    let mut total_steps = 0u64;
    let mut base = 0u32;
    while base < grid_x {
        let mut st = MachineState::new(Arch::Gfx1201, wave_size);
        st.write_sgpr_pair(4, kernarg_addr, 0)?;
        let mut exec = 0u64;
        for lane in 0..lanes {
            let gid = base + lane;
            if gid < grid_x {
                st.vgpr[lane as usize][0] = gid;
                exec |= 1u64 << lane;
            }
        }
        st.exec = exec;
        if exec == 0 {
            break;
        }
        let steps = run(&mut st, code, mem, 10_000)?;
        if !st.halted {
            return Err(IsaError::new(
                IsaErrorKind::Validation,
                "kernel wave finished without s_endpgm",
            ));
        }
        total_steps = total_steps.saturating_add(steps);
        base = base.saturating_add(lanes);
    }
    Ok(total_steps)
}

/// Host reference for SoftGPU tiny_add (i32).
pub fn tiny_add_host_ref(a: &[i32], b: &mut [i32]) {
    assert_eq!(a.len(), b.len());
    for i in 0..a.len() {
        b[i] = a[i].wrapping_add(1);
    }
}
