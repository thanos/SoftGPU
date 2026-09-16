//! SoftGPU ISA step / run (Phase 10 SALU + Phase 11 e2e tiny with memory).

use crate::decode::decode_at;
use crate::error::{Result, TrapKind};
use crate::inst::Inst;
use crate::mem::{GlobalArena, IsaMemory};
use crate::state::MachineState;

/// Outcome of one SoftGPU ISA step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    Continued,
    Halted,
}

/// Fetch, decode, and execute one instruction at `state.pc`.
pub fn step(state: &mut MachineState, code: &[u8], mem: &mut dyn IsaMemory) -> Result<StepOutcome> {
    if state.halted {
        return Err(TrapKind::Halted.to_error());
    }
    let pc = state.pc;
    let inst = decode_at(code, pc)?;
    let size = inst.size_bytes();
    apply(state, &inst, pc, mem)?;

    match inst {
        Inst::SEndPgm => {
            state.halted = true;
            Ok(StepOutcome::Halted)
        }
        _ => {
            state.pc = pc.wrapping_add(size);
            Ok(StepOutcome::Continued)
        }
    }
}

/// SALU-only convenience (empty arena) for Phase 10 tests / CLI `run-isa`.
pub fn step_salu(state: &mut MachineState, code: &[u8]) -> Result<StepOutcome> {
    let mut mem = GlobalArena::new(0, 0);
    step(state, code, &mut mem)
}

fn apply(state: &mut MachineState, inst: &Inst, pc: u32, mem: &mut dyn IsaMemory) -> Result<()> {
    match *inst {
        Inst::SNop { .. } | Inst::SSleep { .. } | Inst::SWaitCnt { .. } | Inst::SEndPgm => Ok(()),
        Inst::SMovB32 { sdst, ssrc0 } => {
            let v = state.read_scalar(ssrc0, pc)?;
            state.write_sgpr(sdst, v, pc)
        }
        Inst::SAddCoU32 { sdst, ssrc0, ssrc1 } => {
            let a = state.read_scalar(ssrc0, pc)?;
            let b = state.read_scalar(ssrc1, pc)?;
            let (sum, carry) = a.overflowing_add(b);
            state.write_sgpr(sdst, sum, pc)?;
            state.scc = carry;
            Ok(())
        }
        Inst::SLoadB64 {
            sdst,
            sbase,
            offset,
            ..
        } => {
            let base = state.read_sgpr_pair(sbase, pc)?;
            let addr = base.wrapping_add(offset as u64);
            let val = mem.load_u64(addr)?;
            state.write_sgpr_pair(sdst, val, pc)
        }
        Inst::VLshlRevB32E32 {
            vdst,
            src0_enc,
            src1,
            ..
        } => {
            let shift = resolve_vop2_src0(src0_enc, pc)? & 31;
            for lane in 0..state.wave_size.lanes() {
                if !state.lane_active(lane) {
                    continue;
                }
                let src = state.vgpr[lane][src1 as usize];
                state.vgpr[lane][vdst as usize] = src << shift;
            }
            Ok(())
        }
        Inst::VAddNcU32E32 {
            vdst,
            src0_enc,
            src1,
            ..
        } => {
            let src0 = resolve_vop2_src0(src0_enc, pc)?;
            for lane in 0..state.wave_size.lanes() {
                if !state.lane_active(lane) {
                    continue;
                }
                let src1v = state.vgpr[lane][src1 as usize];
                state.vgpr[lane][vdst as usize] = src0.wrapping_add(src1v);
            }
            Ok(())
        }
        Inst::GlobalLoadB32 {
            vdst, vaddr, saddr, ..
        } => {
            let base = state.read_sgpr_pair(saddr, pc)?;
            for lane in 0..state.wave_size.lanes() {
                if !state.lane_active(lane) {
                    continue;
                }
                let off = state.vgpr[lane][vaddr as usize] as u64;
                let val = mem.load_u32(base.wrapping_add(off))?;
                state.vgpr[lane][vdst as usize] = val;
            }
            Ok(())
        }
        Inst::GlobalStoreB32 {
            vaddr,
            vdata,
            saddr,
            ..
        } => {
            let base = state.read_sgpr_pair(saddr, pc)?;
            for lane in 0..state.wave_size.lanes() {
                if !state.lane_active(lane) {
                    continue;
                }
                let off = state.vgpr[lane][vaddr as usize] as u64;
                let val = state.vgpr[lane][vdata as usize];
                mem.store_u32(base.wrapping_add(off), val)?;
            }
            Ok(())
        }
    }
}

fn resolve_vop2_src0(enc: u16, pc: u32) -> Result<u32> {
    if (128..=192).contains(&enc) {
        return Ok((enc - 128) as u32);
    }
    if enc == 193 {
        return Ok(u32::MAX);
    }
    Err(TrapKind::UnsupportedOperand {
        encoding: enc as u8,
        role: "vop2_src0",
        pc,
    }
    .to_error())
}

/// Run until `s_endpgm`, trap, or `max_steps`.
pub fn run(
    state: &mut MachineState,
    code: &[u8],
    mem: &mut dyn IsaMemory,
    max_steps: u64,
) -> Result<u64> {
    let mut steps = 0u64;
    while steps < max_steps {
        match step(state, code, mem)? {
            StepOutcome::Continued => steps += 1,
            StepOutcome::Halted => {
                steps += 1;
                return Ok(steps);
            }
        }
    }
    Err(crate::error::IsaError::new(
        crate::error::IsaErrorKind::Validation,
        format!("exceeded max_steps={max_steps} without s_endpgm"),
    )
    .with_remediation("increase max_steps or ensure code ends with s_endpgm"))
}

/// Phase 10-compatible run without memory ops.
pub fn run_salu(state: &mut MachineState, code: &[u8], max_steps: u64) -> Result<u64> {
    let mut mem = GlobalArena::new(0, 0);
    run(state, code, &mut mem, max_steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::{Arch, WaveSize};
    use crate::word::words_to_code;

    #[test]
    fn mov_add_end() {
        let code = words_to_code(&[0xbe80_0081, 0xbe81_0082, 0x8002_0100, 0xbfb0_0000]);
        let mut st = MachineState::new(Arch::Gfx1201, WaveSize::Wave32);
        let n = run_salu(&mut st, &code, 16).unwrap();
        assert_eq!(n, 4);
        assert!(st.halted);
        assert_eq!(st.sgpr[0], 1);
        assert_eq!(st.sgpr[1], 2);
        assert_eq!(st.sgpr[2], 3);
        assert!(!st.scc);
    }

    #[test]
    fn trap_leaves_sgprs() {
        let mut st = MachineState::new(Arch::Gfx1201, WaveSize::Wave32);
        st.sgpr[0] = 42;
        let code = words_to_code(&[0x1234_5678]);
        let mut mem = GlobalArena::new(0, 64);
        let err = step(&mut st, &code, &mut mem).unwrap_err();
        assert!(err.message().contains("unsupported"));
        assert_eq!(st.sgpr[0], 42);
        assert_eq!(st.pc, 0);
    }
}
