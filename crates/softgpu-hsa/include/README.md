# Vendored HSA headers for crates.io builds

Full copy of `third_party/rocr-headers/hsa/` so `softgpu-hsa` builds from the
published crate without the workspace tree. Keep in sync when bumping the ROCR
header pin (regenerate stubs from the workspace `third_party/` copy).
