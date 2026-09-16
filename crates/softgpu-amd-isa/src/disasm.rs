//! SoftGPU disassembler for `softgpu-gfx1201-compute-v2`.

use crate::decode::{decode_at, decode_word};
use crate::error::Result;
use crate::inst::{Inst, ScalarEnc, SopkOp};

/// Disassemble one word (single-word forms).
pub fn disasm_word(word: u32, pc: u32) -> Result<String> {
    let inst = decode_word(word, pc)?;
    Ok(format_inst(&inst))
}

/// Disassemble the instruction at `pc` in `code` (supports multi-word).
pub fn disasm_at(code: &[u8], pc: u32) -> Result<String> {
    let inst = decode_at(code, pc)?;
    Ok(format_inst(&inst))
}

fn format_inst(inst: &Inst) -> String {
    match *inst {
        Inst::SNop { simm16 } => format!("s_nop {simm16}"),
        Inst::SEndPgm => "s_endpgm".to_string(),
        Inst::SSleep { simm16 } => format!("s_sleep {simm16}"),
        Inst::SWaitCnt { simm16 } => format!("s_waitcnt {simm16}"),
        Inst::SCbranch { simm16, .. } => {
            let name = inst.mnemonic();
            let off = simm16 as i16;
            format!("{name} {off}")
        }
        Inst::SCmp { ssrc0, ssrc1, .. } => {
            format!("{} {}, {}", inst.mnemonic(), fmt_src(ssrc0), fmt_src(ssrc1))
        }
        Inst::Sopk { op, sdst, simm16 } => {
            let imm = match op {
                SopkOp::MovkI32 | SopkOp::AddkCoI32 | SopkOp::MulkI32 => {
                    format!("0x{simm16:x}")
                }
            };
            format!("{} {}, {imm}", inst.mnemonic(), fmt_dst(sdst))
        }
        Inst::Sop1 { sdst, ssrc0, .. } => {
            format!("{} {}, {}", inst.mnemonic(), fmt_dst(sdst), fmt_src(ssrc0))
        }
        Inst::Sop2 {
            sdst, ssrc0, ssrc1, ..
        } => format!(
            "{} {}, {}, {}",
            inst.mnemonic(),
            fmt_dst(sdst),
            fmt_src(ssrc0),
            fmt_src(ssrc1)
        ),
        Inst::SLoadB64 {
            sdst,
            sbase,
            offset,
            ..
        } => format!(
            "s_load_b64 s[{sdst}:{}], s[{sbase}:{}], 0x{offset:x}",
            sdst + 1,
            sbase + 1
        ),
        Inst::Vop1 { vdst, src0_enc, .. } => {
            format!("{} v{vdst}, {}", inst.mnemonic(), fmt_src_enc(src0_enc))
        }
        Inst::Vop2 {
            op,
            vdst,
            src0_enc,
            src1,
            ..
        } => {
            use crate::inst::Vop2Op;
            if matches!(op, Vop2Op::CndmaskB32) {
                format!(
                    "v_cndmask_b32_e32 v{vdst}, {}, v{src1}, vcc_lo",
                    fmt_src_enc(src0_enc)
                )
            } else {
                format!(
                    "{} v{vdst}, {}, v{src1}",
                    inst.mnemonic(),
                    fmt_src_enc(src0_enc)
                )
            }
        }
        Inst::Vopc { src0_enc, src1, .. } => format!(
            "{} vcc_lo, {}, v{src1}",
            inst.mnemonic(),
            fmt_src_enc(src0_enc)
        ),
        Inst::GlobalLoadB32 {
            vdst, vaddr, saddr, ..
        } => format!(
            "global_load_b32 v{vdst}, v{vaddr}, s[{saddr}:{}]",
            saddr + 1
        ),
        Inst::GlobalStoreB32 {
            vaddr,
            vdata,
            saddr,
            ..
        } => format!(
            "global_store_b32 v{vaddr}, v{vdata}, s[{saddr}:{}]",
            saddr + 1
        ),
    }
}

fn fmt_dst(enc: ScalarEnc) -> String {
    format!("s{}", enc.0)
}

fn fmt_src(enc: ScalarEnc) -> String {
    fmt_src_enc(enc.0 as u16)
}

fn fmt_src_enc(enc: u16) -> String {
    if enc < 106 {
        return format!("s{enc}");
    }
    if (128..=192).contains(&enc) {
        return format!("{}", enc - 128);
    }
    if enc == 193 {
        return "-1".to_string();
    }
    if (256..512).contains(&enc) {
        return format!("v{}", enc - 256);
    }
    format!("/*src:0x{enc:x}*/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_golden_asm_strings() {
        assert_eq!(disasm_word(0xbf80_0000, 0).unwrap(), "s_nop 0");
        assert_eq!(disasm_word(0xbfb0_0000, 0).unwrap(), "s_endpgm");
        assert_eq!(disasm_word(0xbe80_0081, 0).unwrap(), "s_mov_b32 s0, 1");
        assert_eq!(
            disasm_word(0x8000_0201, 0).unwrap(),
            "s_add_co_u32 s0, s1, s2"
        );
        assert_eq!(disasm_word(0xbf06_0100, 0).unwrap(), "s_cmp_eq_u32 s0, s1");
    }
}
