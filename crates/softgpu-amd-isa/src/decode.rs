//! Sourced gfx1201 decoder (`softgpu-gfx1201-compute-v2`).
//!
//! Multi-word layouts confirmed against llvm-mc goldens
//! (`goldens/llvm-mc-gfx1201.json`) and AMDGPUUsage.

use crate::error::{Result, TrapKind};
use crate::inst::{
    Inst, SBranchCond, SCmpOp, ScalarEnc, Sop1Op, Sop2Op, SopkOp, Vop1Op, Vop2Op, VopcOp,
};

/// SOPP encoding key: bits 31:23 == 0x17F.
const SOPP_ENC_HI: u32 = 0x17F;
/// SOPC encoding key: bits 31:23 == 0x17E.
const SOPC_ENC_HI: u32 = 0x17E;
/// SOP1 encoding key: bits 31:23 == 0x17D.
const SOP1_ENC_HI: u32 = 0x17D;
/// SOP2 encoding key: bits 31:30 == 0b10.
const SOP2_ENC_TOP: u32 = 0b10;
/// SOPK encoding key: bits 31:28 == 0xB.
const SOPK_ENC_TOP: u32 = 0xB;
/// VOP1 encoding: bits 31:25 == 0x3F → high byte 0x7E.
const VOP1_ENC: u32 = 0x3F;
/// VOPC encoding: bits 31:25 == 0x3E → high byte 0x7C.
const VOPC_ENC: u32 = 0x3E;

const SOPP_OP_NOP: u32 = 0;
const SOPP_OP_SLEEP: u32 = 3;
const SOPP_OP_WAITCNT: u32 = 9;
const SOPP_OP_CBRANCH_SCC0: u32 = 0x21;
const SOPP_OP_CBRANCH_SCC1: u32 = 0x22;
const SOPP_OP_CBRANCH_EXECZ: u32 = 0x25;
const SOPP_OP_CBRANCH_EXECNZ: u32 = 0x26;
const SOPP_OP_ENDPGM: u32 = 0x30;

fn bits(word: u32, hi: u32, lo: u32) -> u32 {
    debug_assert!(hi >= lo && hi < 32);
    (word >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

/// Decode one instruction starting at `pc` (may consume 4/8/12 bytes).
pub fn decode_at(code: &[u8], pc: u32) -> Result<Inst> {
    decode_at_inner(code, pc).map_err(|t| t.to_error())
}

/// Decode a single already-fetched word (traps on multi-word forms).
pub fn decode_word(word: u32, pc: u32) -> Result<Inst> {
    let enc9 = bits(word, 31, 23);
    if enc9 == SOPP_ENC_HI {
        return decode_sopp(word, pc).map_err(|t| t.to_error());
    }
    if enc9 == SOPC_ENC_HI {
        return decode_sopc(word, pc).map_err(|t| t.to_error());
    }
    if enc9 == SOP1_ENC_HI {
        return decode_sop1(word, pc).map_err(|t| t.to_error());
    }
    if bits(word, 31, 28) == SOPK_ENC_TOP {
        return decode_sopk(word, pc).map_err(|t| t.to_error());
    }
    if bits(word, 31, 30) == SOP2_ENC_TOP {
        return decode_sop2(word, pc).map_err(|t| t.to_error());
    }
    if bits(word, 31, 25) == VOP1_ENC {
        return decode_vop1(word, pc).map_err(|t| t.to_error());
    }
    if bits(word, 31, 25) == VOPC_ENC {
        return decode_vopc(word, pc).map_err(|t| t.to_error());
    }
    if bits(word, 31, 31) == 0 {
        if let Ok(inst) = try_decode_vop2(word, pc) {
            return Ok(inst);
        }
    }
    Err(TrapKind::UnsupportedEncoding { word, pc }.to_error())
}

fn decode_at_inner(code: &[u8], pc: u32) -> std::result::Result<Inst, TrapKind> {
    let word0 = crate::word::fetch_word(code, pc).map_err(|e| {
        if e.message().contains("misaligned") {
            TrapKind::MisalignedPc { pc }
        } else {
            TrapKind::FetchOutOfBounds {
                pc,
                len: code.len(),
            }
        }
    })?;

    let enc9 = bits(word0, 31, 23);
    if enc9 == SOPP_ENC_HI {
        return decode_sopp(word0, pc);
    }
    if enc9 == SOPC_ENC_HI {
        return decode_sopc(word0, pc);
    }
    if enc9 == SOP1_ENC_HI {
        return decode_sop1(word0, pc);
    }
    if bits(word0, 31, 28) == SOPK_ENC_TOP {
        return decode_sopk(word0, pc);
    }
    if bits(word0, 31, 30) == SOP2_ENC_TOP {
        return decode_sop2(word0, pc);
    }

    // SMEM: high byte 0xF4
    if (word0 >> 24) == 0xf4 {
        return decode_smem_load_b64(code, pc, word0);
    }

    if bits(word0, 31, 25) == VOP1_ENC {
        return decode_vop1(word0, pc);
    }
    if bits(word0, 31, 25) == VOPC_ENC {
        return decode_vopc(word0, pc);
    }

    // VOP2 (bit31 clear for e32 forms SoftGPU claims).
    if bits(word0, 31, 31) == 0 {
        if let Ok(inst) = try_decode_vop2(word0, pc) {
            return Ok(inst);
        }
    }

    // GLOBAL (VMEM): high byte 0xEE
    if (word0 >> 24) == 0xee {
        return decode_global(code, pc, word0);
    }

    Err(TrapKind::UnsupportedEncoding { word: word0, pc })
}

fn decode_sopp(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 22, 16);
    let simm16 = bits(word, 15, 0) as u16;
    match op {
        SOPP_OP_NOP => Ok(Inst::SNop { simm16 }),
        SOPP_OP_SLEEP => Ok(Inst::SSleep { simm16 }),
        SOPP_OP_WAITCNT => Ok(Inst::SWaitCnt { simm16 }),
        SOPP_OP_ENDPGM => Ok(Inst::SEndPgm),
        SOPP_OP_CBRANCH_SCC0 => Ok(Inst::SCbranch {
            cond: SBranchCond::Scc0,
            simm16,
        }),
        SOPP_OP_CBRANCH_SCC1 => Ok(Inst::SCbranch {
            cond: SBranchCond::Scc1,
            simm16,
        }),
        SOPP_OP_CBRANCH_EXECZ => Ok(Inst::SCbranch {
            cond: SBranchCond::ExecZ,
            simm16,
        }),
        SOPP_OP_CBRANCH_EXECNZ => Ok(Inst::SCbranch {
            cond: SBranchCond::ExecNz,
            simm16,
        }),
        _ => Err(TrapKind::UnsupportedOpcode {
            format: "SOPP",
            op,
            word,
            pc,
        }),
    }
}

fn decode_sopc(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 22, 16);
    let ssrc0 = ScalarEnc(bits(word, 7, 0) as u8);
    let ssrc1 = ScalarEnc(bits(word, 15, 8) as u8);
    let cmp = match op {
        0x00 => SCmpOp::EqI32,
        0x04 => SCmpOp::LtI32,
        0x06 => SCmpOp::EqU32,
        0x07 => SCmpOp::LgU32,
        0x08 => SCmpOp::GtU32,
        0x09 => SCmpOp::GeU32,
        0x0b => SCmpOp::LeU32,
        _ => {
            return Err(TrapKind::UnsupportedOpcode {
                format: "SOPC",
                op,
                word,
                pc,
            })
        }
    };
    Ok(Inst::SCmp {
        op: cmp,
        ssrc0,
        ssrc1,
    })
}

fn decode_sopk(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 27, 23);
    let sdst = ScalarEnc(bits(word, 22, 16) as u8);
    let simm16 = bits(word, 15, 0) as u16;
    let sopk = match op {
        0x00 => SopkOp::MovkI32,
        0x0f => SopkOp::AddkCoI32,
        0x10 => SopkOp::MulkI32,
        _ => {
            return Err(TrapKind::UnsupportedOpcode {
                format: "SOPK",
                op,
                word,
                pc,
            })
        }
    };
    Ok(Inst::Sopk {
        op: sopk,
        sdst,
        simm16,
    })
}

fn decode_sop1(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 15, 8);
    let sdst = ScalarEnc(bits(word, 22, 16) as u8);
    let ssrc0 = ScalarEnc(bits(word, 7, 0) as u8);
    let sop1 = match op {
        0x00 => Sop1Op::MovB32,
        0x04 => Sop1Op::BrevB32,
        0x1e => Sop1Op::NotB32,
        0x20 => Sop1Op::AndSaveexecB32,
        0x22 => Sop1Op::OrSaveexecB32,
        _ => {
            return Err(TrapKind::UnsupportedOpcode {
                format: "SOP1",
                op,
                word,
                pc,
            })
        }
    };
    Ok(Inst::Sop1 {
        op: sop1,
        sdst,
        ssrc0,
    })
}

fn decode_sop2(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 29, 23);
    let sdst = ScalarEnc(bits(word, 22, 16) as u8);
    let ssrc1 = ScalarEnc(bits(word, 15, 8) as u8);
    let ssrc0 = ScalarEnc(bits(word, 7, 0) as u8);
    let sop2 = match op {
        0x00 => Sop2Op::AddCoU32,
        0x01 => Sop2Op::SubCoU32,
        0x13 => Sop2Op::MinU32,
        0x15 => Sop2Op::MaxU32,
        0x16 => Sop2Op::AndB32,
        0x18 => Sop2Op::OrB32,
        0x1a => Sop2Op::XorB32,
        0x2c => Sop2Op::MulI32,
        _ => {
            return Err(TrapKind::UnsupportedOpcode {
                format: "SOP2",
                op,
                word,
                pc,
            })
        }
    };
    Ok(Inst::Sop2 {
        op: sop2,
        sdst,
        ssrc0,
        ssrc1,
    })
}

fn decode_smem_load_b64(code: &[u8], pc: u32, word0: u32) -> std::result::Result<Inst, TrapKind> {
    let word1 = crate::word::fetch_word(code, pc + 4).map_err(|_| TrapKind::FetchOutOfBounds {
        pc: pc + 4,
        len: code.len(),
    })?;
    if (word1 >> 24) != 0xf8 {
        return Err(TrapKind::UnsupportedEncoding { word: word0, pc });
    }
    let sbase = (bits(word0, 5, 0) * 2) as u8;
    let sdst = bits(word0, 12, 6) as u8;
    let offset = bits(word1, 20, 0);
    Ok(Inst::SLoadB64 {
        sdst,
        sbase,
        offset,
        size: 8,
    })
}

fn decode_vop1(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 16, 9);
    let src0_enc = bits(word, 8, 0) as u16;
    let vdst = bits(word, 24, 17) as u8;
    let vop1 = match op {
        0x01 => Vop1Op::MovB32,
        0x37 => Vop1Op::NotB32,
        0x38 => Vop1Op::BfrevB32,
        _ => {
            return Err(TrapKind::UnsupportedOpcode {
                format: "VOP1",
                op,
                word,
                pc,
            })
        }
    };
    Ok(Inst::Vop1 {
        op: vop1,
        vdst,
        src0_enc,
        size: 4,
    })
}

fn decode_vopc(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 24, 17);
    let src0_enc = bits(word, 8, 0) as u16;
    let src1 = bits(word, 16, 9) as u8;
    let vopc = match op {
        0x49 => VopcOp::LtU32,
        0x4a => VopcOp::EqU32,
        0x4b => VopcOp::LeU32,
        0x4c => VopcOp::GtU32,
        0x4d => VopcOp::NeU32,
        0x4e => VopcOp::GeU32,
        _ => {
            return Err(TrapKind::UnsupportedOpcode {
                format: "VOPC",
                op,
                word,
                pc,
            })
        }
    };
    Ok(Inst::Vopc {
        op: vopc,
        src0_enc,
        src1,
        size: 4,
    })
}

fn try_decode_vop2(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 30, 25);
    let src0_enc = bits(word, 8, 0) as u16;
    let src1 = bits(word, 16, 9) as u8;
    let vdst = bits(word, 24, 17) as u8;
    let vop2 = match op {
        0x01 => Vop2Op::CndmaskB32,
        0x13 => Vop2Op::MinU32,
        0x14 => Vop2Op::MaxU32,
        0x18 => Vop2Op::LshlRevB32,
        0x19 => Vop2Op::LshrRevB32,
        0x1a => Vop2Op::AshrRevI32,
        0x1b => Vop2Op::AndB32,
        0x1c => Vop2Op::OrB32,
        0x1d => Vop2Op::XorB32,
        0x25 => Vop2Op::AddNcU32,
        0x26 => Vop2Op::SubNcU32,
        _ => {
            return Err(TrapKind::UnsupportedOpcode {
                format: "VOP2",
                op,
                word,
                pc,
            })
        }
    };
    Ok(Inst::Vop2 {
        op: vop2,
        vdst,
        src0_enc,
        src1,
        size: 4,
    })
}

fn decode_global(code: &[u8], pc: u32, word0: u32) -> std::result::Result<Inst, TrapKind> {
    let word1 = crate::word::fetch_word(code, pc + 4).map_err(|_| TrapKind::FetchOutOfBounds {
        pc: pc + 4,
        len: code.len(),
    })?;
    let word2 = crate::word::fetch_word(code, pc + 8).map_err(|_| TrapKind::FetchOutOfBounds {
        pc: pc + 8,
        len: code.len(),
    })?;

    let op_hi = (word0 >> 16) & 0xff;
    let saddr = (word0 & 0xff) as u8;
    if op_hi == 0x05 {
        let vdst = (word1 & 0xff) as u8;
        let vaddr = (word2 & 0xff) as u8;
        return Ok(Inst::GlobalLoadB32 {
            vdst,
            vaddr,
            saddr,
            size: 12,
        });
    }
    if op_hi == 0x06 && (word0 & 0x8000) != 0 {
        let vdata = bits(word1, 31, 23) as u8;
        let vaddr = (word2 & 0xff) as u8;
        return Ok(Inst::GlobalStoreB32 {
            vaddr,
            vdata,
            saddr,
            size: 12,
        });
    }
    Err(TrapKind::UnsupportedOpcode {
        format: "GLOBAL",
        op: op_hi,
        word: word0,
        pc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::words_to_code;

    #[test]
    fn decode_known_salu_goldens() {
        assert_eq!(
            decode_word(0xbf80_0000, 0).unwrap(),
            Inst::SNop { simm16: 0 }
        );
        assert_eq!(decode_word(0xbfb0_0000, 0).unwrap(), Inst::SEndPgm);
        assert!(matches!(
            decode_word(0xbf06_0100, 0).unwrap(),
            Inst::SCmp {
                op: SCmpOp::EqU32,
                ..
            }
        ));
    }

    #[test]
    fn decode_tiny_kernel_stream() {
        let code = words_to_code(&[
            0xf400_2002,
            0xf800_0000,
            0xf400_2082,
            0xf800_0008,
            0xbf89_0000,
            0x3002_0082,
            0xee05_0000,
            0x0000_0002,
            0x0000_0001,
            0xbf89_0000,
            0x4a04_0481,
            0xee06_8002,
            0x0100_0000,
            0x0000_0001,
            0xbfb0_0000,
        ]);
        let mut pc = 0u32;
        let mut names = Vec::new();
        while (pc as usize) < code.len() {
            let inst = decode_at(&code, pc).unwrap();
            names.push(inst.mnemonic());
            if matches!(inst, Inst::SEndPgm) {
                break;
            }
            pc += inst.size_bytes();
        }
        assert_eq!(
            names,
            [
                "s_load_b64",
                "s_load_b64",
                "s_waitcnt",
                "v_lshlrev_b32_e32",
                "global_load_b32",
                "s_waitcnt",
                "v_add_nc_u32_e32",
                "global_store_b32",
                "s_endpgm",
            ]
        );
    }
}
