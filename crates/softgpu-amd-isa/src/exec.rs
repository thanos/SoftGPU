//! SoftGPU ISA step / run (`softgpu-gfx1201-compute-v2`).

use crate::decode::decode_at;
use crate::error::{Result, TrapKind};
use crate::inst::{Inst, SBranchCond, SCmpOp, Sop1Op, Sop2Op, SopkOp, Vop1Op, Vop2Op, VopcOp};
use crate::mem::{GlobalArena, IsaMemory};
use crate::state::{MachineState, SGPR_COUNT};

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
        _ if inst.redirects_pc() => Ok(StepOutcome::Continued),
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
        Inst::SCbranch { cond, simm16 } => {
            let taken = match cond {
                SBranchCond::Scc0 => !state.scc,
                SBranchCond::Scc1 => state.scc,
                SBranchCond::ExecZ => state.exec == 0,
                SBranchCond::ExecNz => state.exec != 0,
            };
            if taken {
                let off = (simm16 as i16 as i32).wrapping_mul(4);
                state.pc = pc.wrapping_add(4).wrapping_add_signed(off);
            } else {
                state.pc = pc.wrapping_add(4);
            }
            Ok(())
        }
        Inst::SCmp { op, ssrc0, ssrc1 } => {
            let a = state.read_scalar(ssrc0, pc)?;
            let b = state.read_scalar(ssrc1, pc)?;
            state.scc = match op {
                SCmpOp::EqI32 | SCmpOp::EqU32 => a == b,
                SCmpOp::LgU32 => a != b,
                SCmpOp::LtI32 => (a as i32) < (b as i32),
                SCmpOp::GtU32 => a > b,
                SCmpOp::GeU32 => a >= b,
                SCmpOp::LeU32 => a <= b,
            };
            Ok(())
        }
        Inst::Sopk { op, sdst, simm16 } => {
            let imm = simm16 as i16 as i32 as u32;
            match op {
                SopkOp::MovkI32 => state.write_sgpr(sdst, imm, pc),
                SopkOp::AddkCoI32 => {
                    let a = state.read_scalar(sdst, pc)?;
                    let (sum, carry) = a.overflowing_add(imm);
                    state.write_sgpr(sdst, sum, pc)?;
                    state.scc = carry;
                    Ok(())
                }
                SopkOp::MulkI32 => {
                    let a = state.read_scalar(sdst, pc)?;
                    state.write_sgpr(sdst, a.wrapping_mul(imm), pc)
                }
            }
        }
        Inst::Sop1 { op, sdst, ssrc0 } => match op {
            Sop1Op::MovB32 => {
                let v = state.read_scalar(ssrc0, pc)?;
                state.write_sgpr(sdst, v, pc)
            }
            Sop1Op::NotB32 => {
                let v = state.read_scalar(ssrc0, pc)?;
                state.write_sgpr(sdst, !v, pc)
            }
            Sop1Op::BrevB32 => {
                let v = state.read_scalar(ssrc0, pc)?;
                state.write_sgpr(sdst, v.reverse_bits(), pc)
            }
            Sop1Op::AndSaveexecB32 => {
                let src = state.read_scalar(ssrc0, pc)? as u64;
                let old = state.exec & 0xffff_ffff;
                state.write_sgpr(sdst, old as u32, pc)?;
                state.exec = (state.exec & !0xffff_ffff) | (old & src);
                Ok(())
            }
            Sop1Op::OrSaveexecB32 => {
                let src = state.read_scalar(ssrc0, pc)? as u64;
                let old = state.exec & 0xffff_ffff;
                state.write_sgpr(sdst, old as u32, pc)?;
                state.exec = (state.exec & !0xffff_ffff) | (old | src);
                Ok(())
            }
        },
        Inst::Sop2 {
            op,
            sdst,
            ssrc0,
            ssrc1,
        } => {
            let a = state.read_scalar(ssrc0, pc)?;
            let b = state.read_scalar(ssrc1, pc)?;
            match op {
                Sop2Op::AddCoU32 => {
                    let (sum, carry) = a.overflowing_add(b);
                    state.write_sgpr(sdst, sum, pc)?;
                    state.scc = carry;
                    Ok(())
                }
                Sop2Op::SubCoU32 => {
                    let (diff, borrow) = a.overflowing_sub(b);
                    state.write_sgpr(sdst, diff, pc)?;
                    state.scc = !borrow;
                    Ok(())
                }
                Sop2Op::AndB32 => {
                    let v = a & b;
                    state.write_sgpr(sdst, v, pc)?;
                    state.scc = v != 0;
                    Ok(())
                }
                Sop2Op::OrB32 => {
                    let v = a | b;
                    state.write_sgpr(sdst, v, pc)?;
                    state.scc = v != 0;
                    Ok(())
                }
                Sop2Op::XorB32 => {
                    let v = a ^ b;
                    state.write_sgpr(sdst, v, pc)?;
                    state.scc = v != 0;
                    Ok(())
                }
                Sop2Op::MinU32 => state.write_sgpr(sdst, a.min(b), pc),
                Sop2Op::MaxU32 => state.write_sgpr(sdst, a.max(b), pc),
                Sop2Op::MulI32 => state.write_sgpr(sdst, a.wrapping_mul(b), pc),
            }
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
        Inst::Vop1 {
            op, vdst, src0_enc, ..
        } => {
            for lane in 0..state.wave_size.lanes() {
                if !state.lane_active(lane) {
                    continue;
                }
                let src0 = resolve_src_enc(state, src0_enc, lane, pc)?;
                let out = match op {
                    Vop1Op::MovB32 => src0,
                    Vop1Op::NotB32 => !src0,
                    Vop1Op::BfrevB32 => src0.reverse_bits(),
                };
                state.vgpr[lane][vdst as usize] = out;
            }
            Ok(())
        }
        Inst::Vop2 {
            op,
            vdst,
            src0_enc,
            src1,
            ..
        } => {
            for lane in 0..state.wave_size.lanes() {
                if !state.lane_active(lane) {
                    continue;
                }
                let src0 = resolve_src_enc(state, src0_enc, lane, pc)?;
                let src1v = state.vgpr[lane][src1 as usize];
                let out = match op {
                    Vop2Op::CndmaskB32 => {
                        if (state.vcc >> lane) & 1 == 1 {
                            src1v
                        } else {
                            src0
                        }
                    }
                    Vop2Op::MinU32 => src0.min(src1v),
                    Vop2Op::MaxU32 => src0.max(src1v),
                    Vop2Op::LshlRevB32 => src1v << (src0 & 31),
                    Vop2Op::LshrRevB32 => src1v >> (src0 & 31),
                    Vop2Op::AshrRevI32 => ((src1v as i32) >> (src0 & 31)) as u32,
                    Vop2Op::AndB32 => src0 & src1v,
                    Vop2Op::OrB32 => src0 | src1v,
                    Vop2Op::XorB32 => src0 ^ src1v,
                    Vop2Op::AddNcU32 => src0.wrapping_add(src1v),
                    Vop2Op::SubNcU32 => src0.wrapping_sub(src1v),
                };
                state.vgpr[lane][vdst as usize] = out;
            }
            Ok(())
        }
        Inst::Vopc {
            op, src0_enc, src1, ..
        } => {
            let mut vcc = 0u64;
            for lane in 0..state.wave_size.lanes() {
                if !state.lane_active(lane) {
                    continue;
                }
                let src0 = resolve_src_enc(state, src0_enc, lane, pc)?;
                let src1v = state.vgpr[lane][src1 as usize];
                let t = match op {
                    VopcOp::EqU32 => src0 == src1v,
                    VopcOp::NeU32 => src0 != src1v,
                    VopcOp::LtU32 => src0 < src1v,
                    VopcOp::GtU32 => src0 > src1v,
                    VopcOp::LeU32 => src0 <= src1v,
                    VopcOp::GeU32 => src0 >= src1v,
                };
                if t {
                    vcc |= 1u64 << lane;
                }
            }
            state.vcc = vcc;
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

/// Resolve 9-bit AMDGPU src encoding: SGPR / inline imm / VGPR (256+n).
fn resolve_src_enc(state: &MachineState, enc: u16, lane: usize, pc: u32) -> Result<u32> {
    if enc < SGPR_COUNT as u16 {
        return Ok(state.sgpr[enc as usize]);
    }
    if (128..=192).contains(&enc) {
        return Ok((enc - 128) as u32);
    }
    if enc == 193 {
        return Ok(u32::MAX);
    }
    if (256..256 + 256).contains(&enc) {
        let vgpr = (enc - 256) as usize;
        return Ok(state.vgpr[lane][vgpr]);
    }
    Err(TrapKind::UnsupportedOperand {
        encoding: (enc & 0xff) as u8,
        role: "src_enc",
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
    fn salu_branch_loop() {
        // s0=0; s1=5; loop: s0+=1; cmp lg; cbranch_scc1 loop; end
        let code = words_to_code(&[
            0xbe80_0080,
            0xbe81_0085,
            0x8000_8100,
            0xbf07_0100,
            0xbfa2_fffd,
            0xbfb0_0000,
        ]);
        let mut st = MachineState::new(Arch::Gfx1201, WaveSize::Wave32);
        let n = run_salu(&mut st, &code, 64).unwrap();
        assert!(st.halted);
        assert_eq!(st.sgpr[0], 5);
        assert!(n > 5);
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
