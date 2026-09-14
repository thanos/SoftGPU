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
    let end = off.checked_add(len).ok_or_else(|| CodeObjectError::Truncated {
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

    #[test]
    fn rejects_non_elf() {
        assert!(matches!(
            Elf64File::parse(b"not elf"),
            Err(CodeObjectError::NotElf) | Err(CodeObjectError::Truncated { .. })
        ));
    }
}
