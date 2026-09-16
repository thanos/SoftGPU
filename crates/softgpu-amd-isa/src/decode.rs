//! Sourced gfx1201 SALU decoder (SoftGPU Phase 10 subset).
//!
//! Encoding layout facts below were confirmed against llvm-mc goldens
//! (`crates/softgpu-amd-isa/goldens/llvm-mc-gfx1201.json`, access 2026-09-16)
//! and the public LLVM AMDGPUUsage encoding descriptions.

use crate::error::{Result, TrapKind};
use crate::inst::{Inst, ScalarEnc};

/// SOPP encoding key: bits 31:23 == 0x17F → high byte pattern `0xBF`.
const SOPP_ENC_HI: u32 = 0x17F;
/// SOP1 encoding key: bits 31:23 == 0x17D → high byte pattern `0xBE`.
const SOP1_ENC_HI: u32 = 0x17D;
/// SOP2 encoding key: bits 31:30 == 0b10.
const SOP2_ENC_TOP: u32 = 0b10;

// SOPP opcodes (bits 22:16) — observed via llvm-mc -mcpu=gfx1201.
const SOPP_OP_NOP: u32 = 0;
const SOPP_OP_SLEEP: u32 = 3;
const SOPP_OP_WAITCNT: u32 = 9;
const SOPP_OP_ENDPGM: u32 = 0x30;

// SOP1 opcodes (bits 15:8).
const SOP1_OP_MOV_B32: u32 = 0;

// SOP2 opcodes (bits 29:23).
const SOP2_OP_ADD_CO_U32: u32 = 0;

fn bits(word: u32, hi: u32, lo: u32) -> u32 {
    debug_assert!(hi >= lo && hi < 32);
    (word >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

/// Decode one instruction word for SoftGPU `gfx1201` SALU subset.
///
/// On unsupported encodings/opcodes returns `TrapKind` **without** implying
/// any machine-state mutation (caller must not apply effects).
pub fn decode_word(word: u32, pc: u32) -> Result<Inst> {
    decode_word_inner(word, pc).map_err(|t| t.to_error())
}

fn decode_word_inner(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let enc9 = bits(word, 31, 23);
    if enc9 == SOPP_ENC_HI {
        return decode_sopp(word, pc);
    }
    if enc9 == SOP1_ENC_HI {
        return decode_sop1(word, pc);
    }
    if bits(word, 31, 30) == SOP2_ENC_TOP {
        return decode_sop2(word, pc);
    }
    Err(TrapKind::UnsupportedEncoding { word, pc })
}

fn decode_sopp(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 22, 16);
    let simm16 = bits(word, 15, 0) as u16;
    match op {
        SOPP_OP_NOP => Ok(Inst::SNop { simm16 }),
        SOPP_OP_SLEEP => Ok(Inst::SSleep { simm16 }),
        SOPP_OP_WAITCNT => Ok(Inst::SWaitCnt { simm16 }),
        SOPP_OP_ENDPGM => Ok(Inst::SEndPgm),
        _ => Err(TrapKind::UnsupportedOpcode {
            format: "SOPP",
            op,
            word,
            pc,
        }),
    }
}

fn decode_sop1(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 15, 8);
    let sdst = ScalarEnc(bits(word, 22, 16) as u8);
    let ssrc0 = ScalarEnc(bits(word, 7, 0) as u8);
    match op {
        SOP1_OP_MOV_B32 => Ok(Inst::SMovB32 { sdst, ssrc0 }),
        _ => Err(TrapKind::UnsupportedOpcode {
            format: "SOP1",
            op,
            word,
            pc,
        }),
    }
}

fn decode_sop2(word: u32, pc: u32) -> std::result::Result<Inst, TrapKind> {
    let op = bits(word, 29, 23);
    let sdst = ScalarEnc(bits(word, 22, 16) as u8);
    let ssrc1 = ScalarEnc(bits(word, 15, 8) as u8);
    let ssrc0 = ScalarEnc(bits(word, 7, 0) as u8);
    match op {
        SOP2_OP_ADD_CO_U32 => Ok(Inst::SAddCoU32 { sdst, ssrc0, ssrc1 }),
        _ => Err(TrapKind::UnsupportedOpcode {
            format: "SOP2",
            op,
            word,
            pc,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_known_goldens() {
        assert_eq!(
            decode_word(0xbf80_0000, 0).unwrap(),
            Inst::SNop { simm16: 0 }
        );
        assert_eq!(decode_word(0xbfb0_0000, 0).unwrap(), Inst::SEndPgm);
        assert_eq!(
            decode_word(0xbe80_0081, 0).unwrap(),
            Inst::SMovB32 {
                sdst: ScalarEnc(0),
                ssrc0: ScalarEnc(0x81)
            }
        );
        assert_eq!(
            decode_word(0x8000_0201, 0).unwrap(),
            Inst::SAddCoU32 {
                sdst: ScalarEnc(0),
                ssrc0: ScalarEnc(1),
                ssrc1: ScalarEnc(2)
            }
        );
    }

    #[test]
    fn unknown_traps() {
        // Random VALU-looking word
        let err = decode_word(0x7e00_0000, 0).unwrap_err();
        assert!(err.message().contains("unsupported encoding"));
        // SOPP with unknown OP
        let err = decode_word(0xbf81_0000, 0).unwrap_err(); // OP=1
        assert!(err.message().contains("SOPP"));
    }
}
