//! Bounded little-endian ELF64 reader (code objects only).

use crate::error::{CodeObjectError, Result};
use crate::MAX_CODE_OBJECT_BYTES;

const ELFMAG: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const ET_REL: u16 = 1;
const SHT_NOTE: u32 = 7;
const MAX_SECTIONS: usize = 256;

#[derive(Debug, Clone)]
pub struct Elf64File<'a> {
    pub data: &'a [u8],
    pub etype: u16,
    pub shoff: u64,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Elf64Section {
    pub name_off: u32,
    pub sh_type: u32,
    pub offset: u64,
    pub size: u64,
}

impl<'a> Elf64File<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() > MAX_CODE_OBJECT_BYTES {
            return Err(CodeObjectError::TooLarge {
                size: data.len(),
                max: MAX_CODE_OBJECT_BYTES,
            });
        }
        if data.len() < 64 {
            return Err(CodeObjectError::Truncated {
                detail: "ELF header".into(),
            });
        }
        if data[0..4] != ELFMAG {
            return Err(CodeObjectError::NotElf);
        }
        if data[4] != ELFCLASS64 {
            return Err(CodeObjectError::UnsupportedClass { class: data[4] });
        }
        if data[5] != ELFDATA2LSB {
            return Err(CodeObjectError::UnsupportedEndian { data: data[5] });
        }

        let etype = read_u16(data, 16)?;
        // AMDGPU HSACO is typically ET_DYN; accept REL/EXEC for fixtures.
        if etype != ET_DYN && etype != ET_EXEC && etype != ET_REL {
            return Err(CodeObjectError::UnsupportedElfType { etype });
        }

        let shoff = read_u64(data, 40)?;
        let shentsize = read_u16(data, 58)?;
        let shnum = read_u16(data, 60)?;
        let shstrndx = read_u16(data, 62)?;

        if shentsize as usize != 64 {
            return Err(CodeObjectError::BadHeader {
                detail: format!("shentsize={shentsize}, expected 64"),
            });
        }
        if shnum as usize > MAX_SECTIONS {
            return Err(CodeObjectError::LimitExceeded {
                detail: format!("shnum={shnum} > {MAX_SECTIONS}"),
            });
        }
        if shoff != 0 {
            let table_end = shoff
                .checked_add(u64::from(shnum).checked_mul(u64::from(shentsize)).ok_or(
                    CodeObjectError::BadHeader {
                        detail: "shnum*shentsize overflow".into(),
                    },
                )?)
                .ok_or_else(|| CodeObjectError::BadHeader {
                    detail: "section table overflow".into(),
                })?;
            if table_end as usize > data.len() {
                return Err(CodeObjectError::Truncated {
                    detail: "section header table".into(),
                });
            }
            if (shstrndx as usize) >= (shnum as usize) && shnum != 0 {
                return Err(CodeObjectError::BadHeader {
                    detail: format!("shstrndx={shstrndx} out of range"),
                });
            }
        }

        Ok(Self {
            data,
            etype,
            shoff,
            shentsize,
            shnum,
            shstrndx,
        })
    }

    pub fn section(&self, index: usize) -> Result<Elf64Section> {
        if index >= self.shnum as usize {
            return Err(CodeObjectError::SectionFault {
                detail: format!("index {index}"),
            });
        }
        let off = (self.shoff as usize)
            .checked_add(index.checked_mul(self.shentsize as usize).ok_or(
                CodeObjectError::SectionFault {
                    detail: "offset overflow".into(),
                },
            )?)
            .ok_or_else(|| CodeObjectError::SectionFault {
                detail: "offset overflow".into(),
            })?;
        let hdr = slice(self.data, off, 64)?;
        Ok(Elf64Section {
            name_off: read_u32(hdr, 0)?,
            sh_type: read_u32(hdr, 4)?,
            offset: read_u64(hdr, 24)?,
            size: read_u64(hdr, 32)?,
        })
    }

    pub fn section_bytes(&self, sec: &Elf64Section) -> Result<&'a [u8]> {
        if sec.size == 0 {
            return Ok(&[]);
        }
        let start = usize::try_from(sec.offset).map_err(|_| CodeObjectError::SectionFault {
            detail: "offset too large".into(),
        })?;
        let size = usize::try_from(sec.size).map_err(|_| CodeObjectError::SectionFault {
            detail: "size too large".into(),
        })?;
        slice(self.data, start, size)
    }

    pub fn section_name(&self, sec: &Elf64Section) -> Result<&'a str> {
        if self.shnum == 0 {
            return Ok("");
        }
        let strtab = self.section(self.shstrndx as usize)?;
        if strtab.sh_type == 0 {
            return Ok("");
        }
        let bytes = self.section_bytes(&strtab)?;
        let start = sec.name_off as usize;
        if start >= bytes.len() {
            return Err(CodeObjectError::SectionFault {
                detail: "name offset OOB".into(),
            });
        }
        let end = bytes[start..]
            .iter()
            .position(|&b| b == 0)
            .map(|i| start + i)
            .unwrap_or(bytes.len());
        std::str::from_utf8(&bytes[start..end]).map_err(|_| CodeObjectError::SectionFault {
            detail: "section name not utf8".into(),
        })
    }

    pub fn note_sections(&self) -> Result<Vec<(usize, Elf64Section)>> {
        let mut out = Vec::new();
        for i in 0..self.shnum as usize {
            let sec = self.section(i)?;
            if sec.sh_type == SHT_NOTE {
                out.push((i, sec));
            }
        }
        Ok(out)
    }
}

pub fn read_u16(data: &[u8], off: usize) -> Result<u16> {
    let s = slice(data, off, 2)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

pub fn read_u32(data: &[u8], off: usize) -> Result<u32> {
    let s = slice(data, off, 4)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

pub fn read_u64(data: &[u8], off: usize) -> Result<u64> {
    let s = slice(data, off, 8)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

pub fn slice(data: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    let end = off
        .checked_add(len)
        .ok_or_else(|| CodeObjectError::Truncated {
            detail: "slice overflow".into(),
        })?;
    if end > data.len() {
        return Err(CodeObjectError::Truncated {
            detail: format!("need {end}, have {}", data.len()),
        });
    }
    Ok(&data[off..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::fixture_tiny_add_gfx1201;

    fn elf_header(
        etype: u16,
        shoff: u64,
        shentsize: u16,
        shnum: u16,
        shstrndx: u16,
        class: u8,
        data_enc: u8,
    ) -> Vec<u8> {
        let mut d = vec![0u8; 64];
        d[0..4].copy_from_slice(&ELFMAG);
        d[4] = class;
        d[5] = data_enc;
        d[16..18].copy_from_slice(&etype.to_le_bytes());
        d[40..48].copy_from_slice(&shoff.to_le_bytes());
        d[58..60].copy_from_slice(&shentsize.to_le_bytes());
        d[60..62].copy_from_slice(&shnum.to_le_bytes());
        d[62..64].copy_from_slice(&shstrndx.to_le_bytes());
        d
    }

    #[test]
    fn rejects_non_elf() {
        assert!(matches!(
            Elf64File::parse(b"not elf"),
            Err(CodeObjectError::NotElf) | Err(CodeObjectError::Truncated { .. })
        ));
    }

    #[test]
    fn rejects_truncated_header_and_bad_class_endian_type() {
        assert!(matches!(
            Elf64File::parse(&[0x7f, b'E', b'L', b'F']),
            Err(CodeObjectError::Truncated { .. })
        ));
        let mut h = elf_header(ET_DYN, 0, 64, 0, 0, 1, ELFDATA2LSB);
        assert!(matches!(
            Elf64File::parse(&h),
            Err(CodeObjectError::UnsupportedClass { class: 1 })
        ));
        h = elf_header(ET_DYN, 0, 64, 0, 0, ELFCLASS64, 2);
        assert!(matches!(
            Elf64File::parse(&h),
            Err(CodeObjectError::UnsupportedEndian { data: 2 })
        ));
        h = elf_header(0, 0, 64, 0, 0, ELFCLASS64, ELFDATA2LSB);
        assert!(matches!(
            Elf64File::parse(&h),
            Err(CodeObjectError::UnsupportedElfType { etype: 0 })
        ));
    }

    #[test]
    fn rejects_bad_shentsize_shnum_and_shstrndx() {
        let h = elf_header(ET_DYN, 64, 32, 1, 0, ELFCLASS64, ELFDATA2LSB);
        assert!(matches!(
            Elf64File::parse(&h),
            Err(CodeObjectError::BadHeader { .. })
        ));
        let h = elf_header(ET_DYN, 64, 64, 300, 0, ELFCLASS64, ELFDATA2LSB);
        assert!(matches!(
            Elf64File::parse(&h),
            Err(CodeObjectError::LimitExceeded { .. })
        ));
        // shoff set but buffer too short for section table
        let h = elf_header(ET_DYN, 64, 64, 2, 0, ELFCLASS64, ELFDATA2LSB);
        assert!(matches!(
            Elf64File::parse(&h),
            Err(CodeObjectError::Truncated { .. })
        ));
        // Valid-sized empty table region with bad shstrndx
        let mut buf = elf_header(ET_DYN, 64, 64, 1, 5, ELFCLASS64, ELFDATA2LSB);
        buf.resize(64 + 64, 0);
        assert!(matches!(
            Elf64File::parse(&buf),
            Err(CodeObjectError::BadHeader { .. })
        ));
    }

    #[test]
    fn rejects_too_large_buffer() {
        let mut huge = vec![0u8; MAX_CODE_OBJECT_BYTES + 1];
        huge[0..4].copy_from_slice(&ELFMAG);
        huge[4] = ELFCLASS64;
        huge[5] = ELFDATA2LSB;
        assert!(matches!(
            Elf64File::parse(&huge),
            Err(CodeObjectError::TooLarge { .. })
        ));
    }

    #[test]
    fn fixture_sections_names_and_empty_bytes() {
        let bytes = fixture_tiny_add_gfx1201();
        let elf = Elf64File::parse(&bytes).unwrap();
        assert!(elf.shnum > 0);
        let notes = elf.note_sections().unwrap();
        assert!(!notes.is_empty());
        let (_i, sec) = notes[0];
        let name = elf.section_name(&sec).unwrap();
        assert!(!name.is_empty());
        let payload = elf.section_bytes(&sec).unwrap();
        assert!(!payload.is_empty());

        let empty = Elf64Section {
            name_off: 0,
            sh_type: 0,
            offset: 0,
            size: 0,
        };
        assert!(elf.section_bytes(&empty).unwrap().is_empty());

        assert!(matches!(
            elf.section(elf.shnum as usize),
            Err(CodeObjectError::SectionFault { .. })
        ));
    }

    #[test]
    fn slice_and_readers_fail_closed() {
        assert!(matches!(
            slice(&[1, 2, 3], 2, 4),
            Err(CodeObjectError::Truncated { .. })
        ));
        assert!(matches!(
            slice(&[1, 2, 3], usize::MAX - 1, 4),
            Err(CodeObjectError::Truncated { .. })
        ));
        assert!(read_u16(&[0], 0).is_err());
        assert!(read_u32(&[0, 1], 0).is_err());
        assert!(read_u64(&[0; 4], 0).is_err());
    }

    #[test]
    fn section_name_oob_and_null_strtab() {
        // One section header that is also the shstrtab, but name_off past end.
        let mut buf = elf_header(ET_REL, 64, 64, 1, 0, ELFCLASS64, ELFDATA2LSB);
        buf.resize(128, 0);
        // sh_type = SHT_STRTAB (3) so name lookup doesn't early-return empty
        buf[64 + 4..64 + 8].copy_from_slice(&3u32.to_le_bytes());
        // offset/size of strtab: point at bytes 120..128 (8 bytes of zeros)
        buf[64 + 24..64 + 32].copy_from_slice(&120u64.to_le_bytes());
        buf[64 + 32..64 + 40].copy_from_slice(&8u64.to_le_bytes());
        let elf = Elf64File::parse(&buf).unwrap();
        let sec = Elf64Section {
            name_off: 100, // past 8-byte strtab
            sh_type: 1,
            offset: 0,
            size: 0,
        };
        assert!(matches!(
            elf.section_name(&sec),
            Err(CodeObjectError::SectionFault { .. })
        ));
        // sh_type 0 → empty name shortcut
        let nulltab = Elf64Section {
            name_off: 0,
            sh_type: 1,
            offset: 0,
            size: 0,
        };
        // Force shstrtab type 0 by rewriting
        buf[64 + 4..64 + 8].copy_from_slice(&0u32.to_le_bytes());
        let elf = Elf64File::parse(&buf).unwrap();
        assert_eq!(elf.section_name(&nulltab).unwrap(), "");
    }
}
