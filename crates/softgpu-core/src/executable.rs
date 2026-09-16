//! SoftGPU HSA executable / code-object reader state (v0.8).

use softgpu_amd_code_object::{load_agent_image, LaunchAbi, LoadableKernel};
use std::collections::HashMap;

/// SoftGPU code-object reader (owned bytes).
#[derive(Debug, Clone)]
pub struct CodeObjectReader {
    pub bytes: Vec<u8>,
}

/// SoftGPU executable symbol (kernel).
#[derive(Debug, Clone)]
pub struct ExecutableSymbol {
    pub name: String,
    pub symbol: String,
    pub kernel_object: u64,
    pub kernarg_segment_size: u32,
    pub kernarg_segment_align: u32,
    pub group_segment_size: u32,
    pub private_segment_size: u32,
    pub launch_abi: LaunchAbi,
}

/// SoftGPU executable (unfrozen → load → freeze).
#[derive(Debug, Clone)]
pub struct SoftGpuExecutable {
    pub frozen: bool,
    pub symbols: Vec<ExecutableSymbol>,
}

/// Parse SoftGPU-supported agent bytes into loadable kernels.
pub fn parse_agent_kernels(bytes: &[u8]) -> Result<Vec<LoadableKernel>, String> {
    let img = load_agent_image(bytes).map_err(|e| e.to_string())?;
    Ok(img.kernels)
}

/// Index symbols by name and by `.kd` symbol for SoftGPU lookup.
pub fn index_symbols(symbols: &[ExecutableSymbol]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for (i, s) in symbols.iter().enumerate() {
        map.insert(s.name.clone(), i);
        map.insert(s.symbol.clone(), i);
        // HIP often strips `.kd`
        if let Some(stripped) = s.symbol.strip_suffix(".kd") {
            map.insert(stripped.to_string(), i);
        }
    }
    map
}
