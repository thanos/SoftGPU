//! Little-endian instruction-word reader for SoftGPU ISA code streams.

use crate::error::{IsaError, IsaErrorKind, Result, TrapKind};

/// Read one LE u32 instruction word at `pc` (byte offset).
pub fn fetch_word(code: &[u8], pc: u32) -> Result<u32> {
    if pc % 4 != 0 {
        return Err(TrapKind::MisalignedPc { pc }.to_error());
    }
    let off = pc as usize;
    if off
        .checked_add(4)
        .map(|end| end > code.len())
        .unwrap_or(true)
    {
        return Err(TrapKind::FetchOutOfBounds {
            pc,
            len: code.len(),
        }
        .to_error());
    }
    let w = u32::from_le_bytes([code[off], code[off + 1], code[off + 2], code[off + 3]]);
    Ok(w)
}

/// Parse a hex word string (`0xbf800000` or `bf800000`).
pub fn parse_hex_word(s: &str) -> Result<u32> {
    let t = s.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    u32::from_str_radix(t, 16).map_err(|_| {
        IsaError::new(
            IsaErrorKind::Config,
            format!("invalid hex instruction word '{s}'"),
        )
        .with_remediation("expected 8 hex digits, e.g. 0xbf800000")
    })
}

/// Encode a word as four little-endian bytes.
pub fn word_to_le_bytes(word: u32) -> [u8; 4] {
    word.to_le_bytes()
}

/// Build a code buffer from LE words.
pub fn words_to_code(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for w in words {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_roundtrip() {
        let code = words_to_code(&[0xbf80_0000, 0xbe80_0081]);
        assert_eq!(fetch_word(&code, 0).unwrap(), 0xbf80_0000);
        assert_eq!(fetch_word(&code, 4).unwrap(), 0xbe80_0081);
    }

    #[test]
    fn misaligned_and_oob() {
        let code = words_to_code(&[0xbf80_0000]);
        assert!(matches!(
            fetch_word(&code, 1).unwrap_err().kind(),
            IsaErrorKind::Trap
        ));
        assert!(matches!(
            fetch_word(&code, 4).unwrap_err().kind(),
            IsaErrorKind::Trap
        ));
    }
}
