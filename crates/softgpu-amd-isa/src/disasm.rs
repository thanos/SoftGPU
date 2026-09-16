//! SoftGPU disassembler for the Phase 10 SALU subset.
//!
//! Mnemonics aim to match `llvm-objdump -d -mtriple=amdgcn-amd-amdhsa -mcpu=gfx1201`
//! for the supported subset (verified against llvm-mc goldens).

use crate::decode::decode_word;
use crate::error::Result;
use crate::inst::{Inst, ScalarEnc};
use crate::word::fetch_word;

/// Disassemble one word at `pc` into an llvm-objdump-like text form.
pub fn disasm_word(word: u32, pc: u32) -> Result<String> {
    let inst = decode_word(word, pc)?;
    Ok(format_inst(&inst))
}

/// Disassemble the instruction at `pc` in `code`.
pub fn disasm_at(code: &[u8], pc: u32) -> Result<String> {
    let word = fetch_word(code, pc)?;
    disasm_word(word, pc)
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
