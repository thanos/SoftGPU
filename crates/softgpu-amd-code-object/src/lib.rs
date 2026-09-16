//! SoftGPU AMDGPU code-object frontend (Phase 5).
//!
//! Bounded ELF64 note walker + MessagePack AMDHSA metadata extraction.
//! Does **not** execute kernels or decode gfx1201 ISA.
//!
//! Provenance: LLVM AMDGPUUsage (`NT_AMDGPU_METADATA` / owner `AMDGPU`).

pub mod elf;
pub mod error;
pub mod fixture;
pub mod load;
pub mod metadata;
pub mod msgpack;
pub mod note;

pub use error::{CodeObjectError, Result};
pub use load::{load_agent_image, LaunchAbi, LoadableAgentImage, LoadableKernel};
pub use metadata::{
    inspect_bytes, inspect_path, CodeObjectInfo, KernelArgInfo, KernelInfo, MetadataVersion,
    SUPPORTED_AMDHSA_VERSIONS, SUPPORTED_GFX_SUBSTRING,
};

/// SoftGPU crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// SoftGPU maximum accepted code-object file size (bytes).
pub const MAX_CODE_OBJECT_BYTES: usize = 16 * 1024 * 1024;
