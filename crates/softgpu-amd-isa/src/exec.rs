//! SoftGPU ISA step / run for the Phase 10 SALU subset.
//!
//! Trap policy: fetch → decode → execute. If decode/operand resolution fails,
//! machine state (except PC for fetch diagnostics) is left unchanged for the
//! attempted instruction's effects. On successful step, PC advances by 4 unless
//! `s_endpgm` sets `halted`.

use crate::decode::decode_word;
use crate::error::{Result, TrapKind};
use crate::inst::Inst;
use crate::state::MachineState;
use crate::word::fetch_word;

/// Outcome of one SoftGPU ISA step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    Continued,
    Halted,
}

/// Fetch, decode, and execute one instruction at `state.pc`.
pub fn step(state: &mut MachineState, code: &[u8]) -> Result<StepOutcome> {
    if state.halted {
        return Err(TrapKind::Halted.to_error());
    }
    let pc = state.pc;
    let word = fetch_word(code, pc)?;
    let inst = decode_word(word, pc)?;

    // Apply effects; on operand trap, leave SGPR/SCC/halted unchanged and PC
    // unchanged so the faulting word can be inspected.
    apply(state, &inst, pc)?;

    match inst {
        Inst::SEndPgm => {
            state.halted = true;
            Ok(StepOutcome::Halted)
        }
        _ => {
            state.pc = pc.wrapping_add(4);
            Ok(StepOutcome::Continued)
        }
    }
}

fn apply(state: &mut MachineState, inst: &Inst, pc: u32) -> Result<()> {
    match *inst {
        Inst::SNop { .. } | Inst::SSleep { .. } | Inst::SWaitCnt { .. } => Ok(()),
        Inst::SEndPgm => Ok(()),
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
    }
}

/// Run until `s_endpgm`, trap, or `max_steps`.
pub fn run(state: &mut MachineState, code: &[u8], max_steps: u64) -> Result<u64> {
    let mut steps = 0u64;
    while steps < max_steps {
        match step(state, code)? {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::{Arch, WaveSize};
    use crate::word::words_to_code;

    #[test]
    fn mov_add_end() {
        // s_mov_b32 s0, 1; s_mov_b32 s1, 2; s_add_co_u32 s2, s0, s1; s_endpgm
        let code = words_to_code(&[0xbe80_0081, 0xbe81_0082, 0x8002_0100, 0xbfb0_0000]);
        let mut st = MachineState::new(Arch::Gfx1201, WaveSize::Wave32);
        let n = run(&mut st, &code, 16).unwrap();
        assert_eq!(n, 4);
        assert!(st.halted);
        assert_eq!(st.sgpr[0], 1);
        assert_eq!(st.sgpr[1], 2);
        assert_eq!(st.sgpr[2], 3);
        assert!(!st.scc);
    }

    #[test]
    fn add_sets_scc_on_overflow() {
        let mut st = MachineState::new(Arch::Gfx1201, WaveSize::Wave32);
        st.sgpr[0] = u32::MAX;
        st.sgpr[1] = 1;
        let code = words_to_code(&[0x8002_0100, 0xbfb0_0000]); // s_add_co_u32 s2, s0, s1; end
        run(&mut st, &code, 8).unwrap();
        assert_eq!(st.sgpr[2], 0);
        assert!(st.scc);
    }

    #[test]
    fn trap_leaves_sgprs() {
        let mut st = MachineState::new(Arch::Gfx1201, WaveSize::Wave32);
        st.sgpr[0] = 42;
        // Unsupported encoding
        let code = words_to_code(&[0x7e00_0000]);
        let err = step(&mut st, &code).unwrap_err();
        assert!(err.message().contains("unsupported"));
        assert_eq!(st.sgpr[0], 42);
        assert_eq!(st.pc, 0);
    }
}
