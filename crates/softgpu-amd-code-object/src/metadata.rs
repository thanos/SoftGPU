//! AMDHSA MessagePack metadata extraction (code-object V3+ subset).

use crate::elf::Elf64File;
use crate::error::{CodeObjectError, Result};
use crate::msgpack::{self, Value};
use crate::note::find_amdgpu_metadata_note;
use serde::Serialize;
use std::path::Path;

/// SoftGPU-supported `amdhsa.version` pairs (major, minor).
/// Corresponds to LLVM code-object metadata V3–V5 family (`[1,0]`…`[1,2]`).
pub const SUPPORTED_AMDHSA_VERSIONS: &[(u32, u32)] = &[(1, 0), (1, 1), (1, 2)];

/// Required substring in `amdhsa.target` for SoftGPU Phase 5 acceptance.
pub const SUPPORTED_GFX_SUBSTRING: &str = "gfx1201";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MetadataVersion {
    pub major: u32,
    pub minor: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KernelArgInfo {
    pub name: Option<String>,
    pub type_name: Option<String>,
    pub offset: u32,
    pub size: u32,
    pub value_kind: String,
    pub address_space: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KernelInfo {
    pub name: String,
    pub symbol: Option<String>,
    pub kernarg_segment_size: u32,
    pub kernarg_segment_align: u32,
    pub group_segment_fixed_size: u32,
    pub private_segment_fixed_size: u32,
    pub args: Vec<KernelArgInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeObjectInfo {
    pub metadata_version: MetadataVersion,
    pub target: String,
    pub kernels: Vec<KernelInfo>,
    pub note_desc_len: usize,
    pub fidelity: &'static str,
    pub note: &'static str,
}

/// Inspect a code-object file from disk (bounded read).
pub fn inspect_path(path: impl AsRef<Path>) -> Result<CodeObjectInfo> {
    let bytes = std::fs::read(path.as_ref()).map_err(|e| CodeObjectError::Io(e.to_string()))?;
    inspect_bytes(&bytes)
}

/// Inspect owned code-object bytes.
pub fn inspect_bytes(bytes: &[u8]) -> Result<CodeObjectInfo> {
    let elf = Elf64File::parse(bytes)?;
    let note = find_amdgpu_metadata_note(&elf)?;
    let root = msgpack::parse(note.desc)?;
    let Value::Map(map) = root else {
        return Err(CodeObjectError::Metadata {
            detail: "root is not a map".into(),
        });
    };

    let version = parse_version(&map)?;
    if !SUPPORTED_AMDHSA_VERSIONS.contains(&(version.major, version.minor)) {
        return Err(CodeObjectError::UnsupportedMetadataVersion {
            major: version.major,
            minor: version.minor,
        });
    }

    let target = required_string(&map, "amdhsa.target")?;
    if !target.contains(SUPPORTED_GFX_SUBSTRING) {
        return Err(CodeObjectError::UnsupportedTarget { target });
    }

    let kernels = parse_kernels(&map)?;
    Ok(CodeObjectInfo {
        metadata_version: version,
        target,
        kernels,
        note_desc_len: note.desc.len(),
        fidelity: "abi",
        note: "metadata_only_no_isa_execution",
    })
}

fn parse_version(map: &std::collections::BTreeMap<String, Value>) -> Result<MetadataVersion> {
    let Some(Value::Array(arr)) = map.get("amdhsa.version") else {
        return Err(CodeObjectError::Metadata {
            detail: "missing amdhsa.version array".into(),
        });
    };
    if arr.len() != 2 {
        return Err(CodeObjectError::Metadata {
            detail: "amdhsa.version must be [major, minor]".into(),
        });
    }
    Ok(MetadataVersion {
        major: as_u32(&arr[0], "amdhsa.version[0]")?,
        minor: as_u32(&arr[1], "amdhsa.version[1]")?,
    })
}

fn parse_kernels(map: &std::collections::BTreeMap<String, Value>) -> Result<Vec<KernelInfo>> {
    let Some(Value::Array(arr)) = map.get("amdhsa.kernels") else {
        return Err(CodeObjectError::Metadata {
            detail: "missing amdhsa.kernels".into(),
        });
    };
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let Value::Map(kmap) = item else {
            return Err(CodeObjectError::Metadata {
                detail: format!("kernel[{i}] not a map"),
            });
        };
        out.push(parse_kernel(kmap, i)?);
    }
    Ok(out)
}

fn parse_kernel(
    kmap: &std::collections::BTreeMap<String, Value>,
    index: usize,
) -> Result<KernelInfo> {
    let name = required_string(kmap, ".name")?;
    let symbol = optional_string(kmap, ".symbol");
    let kernarg_segment_size = optional_u32(kmap, ".kernarg_segment_size")?.unwrap_or(0);
    let kernarg_segment_align = optional_u32(kmap, ".kernarg_segment_align")?.unwrap_or(1);
    let group_segment_fixed_size = optional_u32(kmap, ".group_segment_fixed_size")?.unwrap_or(0);
    let private_segment_fixed_size =
        optional_u32(kmap, ".private_segment_fixed_size")?.unwrap_or(0);
    let args = match kmap.get(".args") {
        Some(Value::Array(a)) => {
            let mut out = Vec::new();
            for (j, arg) in a.iter().enumerate() {
                let Value::Map(amap) = arg else {
                    return Err(CodeObjectError::Metadata {
                        detail: format!("kernel[{index}].args[{j}] not a map"),
                    });
                };
                out.push(KernelArgInfo {
                    name: optional_string(amap, ".name"),
                    type_name: optional_string(amap, ".type_name"),
                    offset: optional_u32(amap, ".offset")?.unwrap_or(0),
                    size: optional_u32(amap, ".size")?.unwrap_or(0),
                    value_kind: optional_string(amap, ".value_kind")
                        .unwrap_or_else(|| "unknown".into()),
                    address_space: optional_string(amap, ".address_space"),
                });
            }
            out
        }
        Some(_) => {
            return Err(CodeObjectError::Metadata {
                detail: format!("kernel[{index}].args not an array"),
            });
        }
        None => Vec::new(),
    };
    Ok(KernelInfo {
        name,
        symbol,
        kernarg_segment_size,
        kernarg_segment_align,
        group_segment_fixed_size,
        private_segment_fixed_size,
        args,
    })
}

fn required_string(map: &std::collections::BTreeMap<String, Value>, key: &str) -> Result<String> {
    match map.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(CodeObjectError::Metadata {
            detail: format!("{key} not a string"),
        }),
        None => Err(CodeObjectError::Metadata {
            detail: format!("missing {key}"),
        }),
    }
}

fn optional_string(map: &std::collections::BTreeMap<String, Value>, key: &str) -> Option<String> {
    match map.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn optional_u32(map: &std::collections::BTreeMap<String, Value>, key: &str) -> Result<Option<u32>> {
    match map.get(key) {
        None => Ok(None),
        Some(v) => Ok(Some(as_u32(v, key)?)),
    }
}

fn as_u32(v: &Value, ctx: &str) -> Result<u32> {
    match v {
        Value::U64(n) => u32::try_from(*n).map_err(|_| CodeObjectError::Metadata {
            detail: format!("{ctx} out of u32 range"),
        }),
        Value::I64(n) if *n >= 0 => {
            u32::try_from(*n as u64).map_err(|_| CodeObjectError::Metadata {
                detail: format!("{ctx} out of u32 range"),
            })
        }
        _ => Err(CodeObjectError::Metadata {
            detail: format!("{ctx} not an unsigned integer"),
        }),
    }
}
