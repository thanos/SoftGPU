/* SoftGPU Phase 1 layout probe against pinned ROCR hsa.h.
 *
 * Build:
 *   cc -I third_party/rocr-headers -o target/hsa-layout-probe tools/hsa-layout-probe/probe.c
 *   ./target/hsa-layout-probe
 */
#include <stdio.h>
#include <stddef.h>
#include <stdint.h>
#include "hsa/hsa.h"

#define CHECK_SIZE(type, expected)                                             \
  do {                                                                         \
    size_t got = sizeof(type);                                                 \
    if (got != (size_t)(expected)) {                                           \
      fprintf(stderr, "FAIL sizeof(" #type ")=%zu expected %d\n", got,         \
              (int)(expected));                                                \
      fails++;                                                                 \
    } else {                                                                   \
      printf("ok sizeof(" #type ")=%zu\n", got);                               \
    }                                                                          \
  } while (0)

#define CHECK_ENUM(name, expected)                                             \
  do {                                                                         \
    if ((int)(name) != (int)(expected)) {                                      \
      fprintf(stderr, "FAIL " #name "=%d expected %d\n", (int)(name),          \
              (int)(expected));                                                \
      fails++;                                                                 \
    } else {                                                                   \
      printf("ok " #name "=%d\n", (int)(name));                                \
    }                                                                          \
  } while (0)

int main(void) {
  int fails = 0;
  CHECK_SIZE(hsa_status_t, 4);
  CHECK_SIZE(hsa_agent_t, 8);
  CHECK_SIZE(uint64_t, 8);
  if (offsetof(hsa_agent_t, handle) != 0) {
    fprintf(stderr, "FAIL offsetof(hsa_agent_t, handle)\n");
    fails++;
  } else {
    printf("ok offsetof(hsa_agent_t, handle)=0\n");
  }

  CHECK_ENUM(HSA_STATUS_SUCCESS, 0x0);
  CHECK_ENUM(HSA_STATUS_ERROR_INVALID_AGENT, 0x1004);
  CHECK_ENUM(HSA_STATUS_ERROR_NOT_INITIALIZED, 0x100B);
  CHECK_ENUM(HSA_AGENT_INFO_NAME, 0);
  CHECK_ENUM(HSA_AGENT_INFO_VENDOR_NAME, 1);
  CHECK_ENUM(HSA_AGENT_INFO_FEATURE, 2);
  CHECK_ENUM(HSA_AGENT_INFO_DEVICE, 17);
  CHECK_ENUM(HSA_AGENT_INFO_VERSION_MAJOR, 21);
  CHECK_ENUM(HSA_AGENT_INFO_VERSION_MINOR, 22);
  CHECK_ENUM(HSA_DEVICE_TYPE_CPU, 0);
  CHECK_ENUM(HSA_DEVICE_TYPE_GPU, 1);

  if (fails) {
    fprintf(stderr, "%d layout probe check(s) failed\n", fails);
    return 1;
  }
  printf("hsa layout probe passed\n");
  return 0;
}
