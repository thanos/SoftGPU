//! ELF note walker for AMDGPU metadata.

use crate::elf::{read_u32, slice, Elf64File};
use crate::error::{CodeObjectError, Result};

/// `NT_AMDGPU_METADATA` (LLVM AMDGPUUsage).
pub const NT_AMDGPU_METADATA: u32 = 32;
/// Note owner name for code-object V3+.
pub const NOTE_OWNER_AMDGPU: &str = "AMDGPU";
const MAX_NOTES: usize = 64;

#[derive(Debug, Clone)]
pub struct AmdgpuMetadataNote<'a> {
    pub desc: &'a [u8],
}

/// Find the first `AMDGPU` / `NT_AMDGPU_METADATA` note descriptor.
pub fn find_amdgpu_metadata_note<'a>(elf: &Elf64File<'a>) -> Result<AmdgpuMetadataNote<'a>> {
    let mut seen = 0usize;
    for (_i, sec) in elf.note_sections()? {
        let bytes = elf.section_bytes(&sec)?;
        let mut off = 0usize;
        while off + 12 <= bytes.len() {
            if seen >= MAX_NOTES {
                return Err(CodeObjectError::LimitExceeded {
                    detail: format!("notes > {MAX_NOTES}"),
                });
            }
            seen += 1;
            let namesz = read_u32(bytes, off)? as usize;
            let descsz = read_u32(bytes, off + 4)? as usize;
            let ntype = read_u32(bytes, off + 8)?;
            off += 12;

            let name_pad = align4(namesz);
            let desc_pad = align4(descsz);
            let name_bytes = slice(bytes, off, namesz)?;
            // Name includes trailing NUL in ELF notes.
            let name = std::str::from_utf8(name_bytes)
                .unwrap_or("")
                .trim_end_matches('\0');
            off = off
                .checked_add(name_pad)
                .ok_or_else(|| CodeObjectError::Truncated {
                    detail: "note name".into(),
                })?;
            let desc = slice(bytes, off, descsz)?;
            off = off
                .checked_add(desc_pad)
                .ok_or_else(|| CodeObjectError::Truncated {
                    detail: "note desc".into(),
                })?;

            if name == NOTE_OWNER_AMDGPU && ntype == NT_AMDGPU_METADATA {
                return Ok(AmdgpuMetadataNote { desc });
            }
        }
    }
    Err(CodeObjectError::NoteNotFound)
}

fn align4(n: usize) -> usize {
    n.wrapping_add(3) & !3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align4_works() {
        assert_eq!(align4(0), 0);
        assert_eq!(align4(1), 4);
        assert_eq!(align4(4), 4);
        assert_eq!(align4(5), 8);
    }
}
