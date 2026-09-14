// SoftGPU Phase 5 tiny HIP kernel (official compiler path when hipcc available).
// Not executed by SoftGPU — used to produce AMDGPU metadata for inspection.
#include <hip/hip_runtime.h>

__global__ void tiny_add(const float *a, float *b) {
  size_t i = hipBlockDim_x * hipBlockIdx_x + hipThreadIdx_x;
  b[i] = a[i] + 1.0f;
}

int main() {
  // Host stub so hipcc produces a linkable binary; SoftGPU does not launch.
  return 0;
}
