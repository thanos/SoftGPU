# AMD code-object fixtures — provenance

**Access date:** 2026-09-14

## SoftGPU-synthetic (host tests)

Generated at test time by `softgpu_amd_code_object::fixture::*`:

| Builder | Purpose |
| --- | --- |
| `fixture_tiny_add_gfx1201` | one kernel, two global pointer args, V5-style version `[1,2]` |
| `fixture_multi_kernel_gfx1201` | two kernels; group/private segment sizes |
| `fixture_unsupported_target` | `gfx900` → fail closed |
| `fixture_unsupported_version` | `amdhsa.version [2,0]` → fail closed |

On-disk copies (regenerate with the example above):

| File | SHA-256 |
| --- | --- |
| `tiny-add-gfx1201.softgpu.co` | `ffab7bca9422ff164c49a6c8b832eda2c18913eb50a9d04099b2d6d0a8171374` |
| `multi-kernel-gfx1201.softgpu.co` | `8e4091a0b88427f4b6ad04a8a78538ed509155bbb50a65364aba51bd1ce9afd1` |
| `unsupported-target.softgpu.co` | `8e7b44e538ea6560f52addaa51179c123044005a9710dcadd215d807d24a5160` |
| `unsupported-version.softgpu.co` | `9b97d607fb98429accbcde8a15c7ca920ada8f3b467975cbbcc602ccfe039add` |

## Official compiler (optional ROCm gate)

When the pinned ROCm image is available, `environments/rocm-x86_64/run-phase5-code-object.sh`:

1. Compiles a tiny HIP kernel with `hipcc --offload-arch=gfx1201`
2. Extracts / locates the device code object when tools allow
3. Runs `softgpu inspect-code-object`
4. Cross-checks note presence with `llvm-readelf -n` when installed

Record image digest + compiler version in the script output. SoftGPU still does
**not** claim ISA execution.
