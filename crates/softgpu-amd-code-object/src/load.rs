//! SoftGPU agent code-object load (v0.8): metadata + `.text` extract.
//!
//! SoftGPU supports a **declared** HSACO-shaped subset: gfx1201 metadata plus a
//! `.text` section whose bytes SoftGPU can execute under SoftGPU CC. Full AMDHSA
//! `.kd` binary preload is fail-closed unless SoftGPU documents otherwise.

use crate::elf::Elf64File;
use crate::error::{CodeObjectError, Result};
use crate::metadata::{inspect_bytes, CodeObjectInfo, KernelInfo};

/// SoftGPU launch ABI for a loaded kernel image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchAbi {
    /// SoftGPU e2e convention: `s[4:5]` = kernarg, `v0` = global_id_x.
    SoftGpuCc,
}

/// One kernel extracted from a SoftGPU-supported agent code object.
#[derive(Debug, Clone)]
pub struct LoadableKernel {
    pub name: String,
    pub symbol: String,
    pub text: Vec<u8>,
    pub kernarg_segment_size: u32,
    pub kernarg_segment_align: u32,
    pub group_segment_size: u32,
    pub private_segment_size: u32,
    pub launch_abi: LaunchAbi,
}

/// SoftGPU-supported agent image (metadata + shared `.text`).
#[derive(Debug, Clone)]
pub struct LoadableAgentImage {
    pub info: CodeObjectInfo,
    pub text: Vec<u8>,
    pub kernels: Vec<LoadableKernel>,
}

/// Extract `.text` and bind SoftGPU-supported kernels from an agent code object.
pub fn load_agent_image(bytes: &[u8]) -> Result<LoadableAgentImage> {
    let info = inspect_bytes(bytes)?;
    let elf = Elf64File::parse(bytes)?;
    let text = extract_text_section(&elf)?.to_vec();
    if text.is_empty() {
        return Err(CodeObjectError::Unsupported {
            detail: "agent code object has empty .text".into(),
        });
    }
    // SoftGPU v0.8: one shared .text blob for all listed kernels (SoftGPU fixtures).
    // Arbitrary multi-kernel hipcc layouts with per-symbol offsets remain unsupported.
    if info.kernels.len() != 1 {
        return Err(CodeObjectError::Unsupported {
            detail: format!(
                "SoftGPU executable load supports exactly 1 kernel, found {}",
                info.kernels.len()
            ),
        });
    }
    let kernels = info
        .kernels
        .iter()
        .map(|k| LoadableKernel {
            name: k.name.clone(),
            symbol: k.symbol.clone().unwrap_or_else(|| format!("{}.kd", k.name)),
            text: text.clone(),
            kernarg_segment_size: k.kernarg_segment_size,
            kernarg_segment_align: k.kernarg_segment_align,
            group_segment_size: k.group_segment_fixed_size,
            private_segment_size: k.private_segment_fixed_size,
            launch_abi: LaunchAbi::SoftGpuCc,
        })
        .collect();
    Ok(LoadableAgentImage {
        info,
        text,
        kernels,
    })
}

fn extract_text_section<'a>(elf: &Elf64File<'a>) -> Result<&'a [u8]> {
    for i in 0..elf.shnum as usize {
        let sec = elf.section(i)?;
        let name = elf.section_name(&sec)?;
        if name == ".text" {
            return elf.section_bytes(&sec);
        }
    }
    Err(CodeObjectError::Unsupported {
        detail: "missing .text section".into(),
    })
}

/// Convenience: first kernel name for SoftGPU tests.
pub fn primary_kernel(info: &CodeObjectInfo) -> Option<&KernelInfo> {
    info.kernels.first()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::fixture_tiny_add_with_text;

    #[test]
    fn loads_softgpu_tiny_add_text() {
        let bytes = fixture_tiny_add_with_text();
        let img = load_agent_image(&bytes).unwrap();
        assert_eq!(img.kernels.len(), 1);
        assert_eq!(img.kernels[0].name, "tiny_add");
        assert!(!img.text.is_empty());
        assert_eq!(img.kernels[0].launch_abi, LaunchAbi::SoftGpuCc);
    }

    #[test]
    fn rejects_metadata_only_fixture() {
        let bytes = crate::fixture::fixture_tiny_add_gfx1201();
        let err = load_agent_image(&bytes).unwrap_err();
        assert!(err.to_string().contains(".text") || err.to_string().contains("text"));
    }
}
