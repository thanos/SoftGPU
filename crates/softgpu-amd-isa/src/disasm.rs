//! SoftGPU disassembler for Phase 10/11 named subsets.

use crate::decode::{decode_at, decode_word};
use crate::error::Result;
use crate::inst::{Inst, ScalarEnc};

/// Disassemble one word (SALU / VOP2 single-word forms).
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
        Inst::SMovB32 { sdst, ssrc0 } => {
            format!("s_mov_b32 {}, {}", fmt_dst(sdst), fmt_src(ssrc0))
        }
        Inst::SAddCoU32 { sdst, ssrc0, ssrc1 } => format!(
            "s_add_co_u32 {}, {}, {}",
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
        Inst::VLshlRevB32E32 {
            vdst,
            src0_enc,
            src1,
            ..
        } => format!(
            "v_lshlrev_b32_e32 v{vdst}, {}, v{src1}",
            fmt_vop2_src0(src0_enc)
        ),
        Inst::VAddNcU32E32 {
            vdst,
            src0_enc,
            src1,
            ..
        } => format!(
            "v_add_nc_u32_e32 v{vdst}, {}, v{src1}",
            fmt_vop2_src0(src0_enc)
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
    let e = enc.0 as usize;
    if e < 106 {
        return format!("s{e}");
    }
    if (128..=192).contains(&e) {
        return format!("{}", e - 128);
    }
    if e == 193 {
        return "-1".to_string();
    }
    format!("/*enc:0x{:02x}*/", enc.0)
}

fn fmt_vop2_src0(enc: u16) -> String {
    if (128..=192).contains(&enc) {
        format!("{}", enc - 128)
    } else if enc == 193 {
        "-1".to_string()
    } else {
        format!("/*src0:0x{enc:x}*/")
    }
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
    }
}
