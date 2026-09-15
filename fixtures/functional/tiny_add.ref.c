/* SoftGPU Phase 6 host reference for tiny_add.
 *
 * Provenance path (disclosed):
 *   this C reference → hand-translated softgpu-sfir-v1 (see tiny_add.sfir.json
 *   and softgpu_functional::kernels::tiny_add).
 *
 * SoftGPU does NOT claim this was recovered from an AMDGPU code object.
 */
#include <stdint.h>
#include <stddef.h>

void tiny_add_host(const int32_t *a, int32_t *b, size_t n) {
  for (size_t i = 0; i < n; i++) {
    b[i] = a[i] + 1;
  }
}
