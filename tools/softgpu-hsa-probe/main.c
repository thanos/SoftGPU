/* SoftGPU-authored HSA discovery probe (not a HIP application).
 *
 * Links against SoftGPU's libhsa-runtime64 to prove agent discovery.
 * Full HIP-linked load proof remains a Linux x86-64 + ROCm CI gate.
 *
 * Example (macOS after cargo build -p softgpu-hsa):
 *   cc -I third_party/rocr-headers \
 *      -o target/softgpu-hsa-probe tools/softgpu-hsa-probe/main.c \
 *      -L target/debug -lhsa-runtime64
 *   DYLD_LIBRARY_PATH=target/debug ./target/softgpu-hsa-probe
 */
#include <stdio.h>
#include <string.h>
#include "hsa/hsa.h"

static int saw_gpu;

static hsa_status_t on_agent(hsa_agent_t agent, void *data) {
  (void)data;
  hsa_device_type_t device;
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_DEVICE, &device) != HSA_STATUS_SUCCESS) {
    return HSA_STATUS_ERROR;
  }
  if (device == HSA_DEVICE_TYPE_GPU) {
    char name[64];
    memset(name, 0, sizeof(name));
    if (hsa_agent_get_info(agent, HSA_AGENT_INFO_NAME, name) != HSA_STATUS_SUCCESS) {
      return HSA_STATUS_ERROR;
    }
    printf("softgpu-hsa-probe: gpu agent name='%s'\n", name);
    saw_gpu = 1;
  }
  return HSA_STATUS_SUCCESS;
}

int main(void) {
  hsa_status_t st = hsa_init();
  if (st != HSA_STATUS_SUCCESS) {
    fprintf(stderr, "hsa_init failed: 0x%x\n", (unsigned)st);
    return 1;
  }
  st = hsa_iterate_agents(on_agent, NULL);
  if (st != HSA_STATUS_SUCCESS) {
    fprintf(stderr, "hsa_iterate_agents failed: 0x%x\n", (unsigned)st);
    hsa_shut_down();
    return 1;
  }
  st = hsa_shut_down();
  if (st != HSA_STATUS_SUCCESS || !saw_gpu) {
    fprintf(stderr, "probe failed (shutdown=0x%x saw_gpu=%d)\n", (unsigned)st, saw_gpu);
    return 1;
  }
  printf("softgpu-hsa-probe: ok fidelity=abi phase=2\n");
  return 0;
}
