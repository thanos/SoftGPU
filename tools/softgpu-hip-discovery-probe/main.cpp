/* SoftGPU Phase 2 HIP-linked agent discovery probe.
 *
 * Controlled subset: the process is HIP-linked (loads SoftGPU as HSA), then
 * discovers the virtual GPU via HSA iterate/get_info. FEATURE=0 means
 * hipGetDeviceCount may be 0; that is documented, not a silent lie.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <hip/hip_runtime.h>
#include <hsa/hsa.h>

#if defined(__linux__)
#include <link.h>
#else
#error "Phase 2 discovery probe requires Linux"
#endif

struct check_state {
  int saw_softgpu_hsa;
  int saw_system_hsa;
};

static int on_phdr(struct dl_phdr_info *info, size_t size, void *data) {
  (void)size;
  struct check_state *st = (struct check_state *)data;
  if (!info->dlpi_name || !info->dlpi_name[0]) return 0;
  if (strstr(info->dlpi_name, "libhsa-runtime64") == NULL &&
      strstr(info->dlpi_name, "libhsa_runtime64") == NULL) {
    return 0;
  }
  fprintf(stderr, "softgpu-hip-discovery: mapped %s\n", info->dlpi_name);
  const char *libdir = getenv("SOFTGPU_HSA_LIBDIR");
  if (libdir && strstr(info->dlpi_name, libdir)) st->saw_softgpu_hsa = 1;
  if (strstr(info->dlpi_name, "/opt/rocm")) st->saw_system_hsa = 1;
  return 0;
}

struct agent_state {
  int saw_gpu;
  char name[64];
  char vendor[64];
  uint32_t feature;
  hsa_device_type_t device;
};

static hsa_status_t on_agent(hsa_agent_t agent, void *data) {
  struct agent_state *st = (struct agent_state *)data;
  hsa_device_type_t device;
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_DEVICE, &device) != HSA_STATUS_SUCCESS) {
    return HSA_STATUS_ERROR;
  }
  if (device != HSA_DEVICE_TYPE_GPU) {
    return HSA_STATUS_SUCCESS;
  }
  memset(st->name, 0, sizeof(st->name));
  memset(st->vendor, 0, sizeof(st->vendor));
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_NAME, st->name) != HSA_STATUS_SUCCESS) {
    return HSA_STATUS_ERROR;
  }
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_VENDOR_NAME, st->vendor) != HSA_STATUS_SUCCESS) {
    return HSA_STATUS_ERROR;
  }
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_FEATURE, &st->feature) != HSA_STATUS_SUCCESS) {
    return HSA_STATUS_ERROR;
  }
  st->device = device;
  st->saw_gpu = 1;
  return HSA_STATUS_INFO_BREAK;
}

int main(void) {
  const char *libdir = getenv("SOFTGPU_HSA_LIBDIR");
  if (!libdir || !libdir[0]) {
    fprintf(stderr, "SOFTGPU_HSA_LIBDIR required\n");
    return 2;
  }

  struct check_state mapst;
  memset(&mapst, 0, sizeof(mapst));
  dl_iterate_phdr(on_phdr, &mapst);
  if (mapst.saw_system_hsa || !mapst.saw_softgpu_hsa) {
    fprintf(stderr, "FAIL: SoftGPU HSA substitution not confirmed\n");
    return 1;
  }

  /* HIP touch — may fail or report 0 devices under FEATURE=0. */
  hipError_t herr = hipInit(0);
  int hip_devices = -1;
  hipGetDeviceCount(&hip_devices);
  fprintf(stderr, "softgpu-hip-discovery: hipInit=%d hipGetDeviceCount=%d\n",
          (int)herr, hip_devices);

  if (hsa_init() != HSA_STATUS_SUCCESS) {
    fprintf(stderr, "FAIL: hsa_init\n");
    return 1;
  }

  struct agent_state ag;
  memset(&ag, 0, sizeof(ag));
  hsa_status_t st = hsa_iterate_agents(on_agent, &ag);
  if (!(st == HSA_STATUS_SUCCESS || st == HSA_STATUS_INFO_BREAK) || !ag.saw_gpu) {
    fprintf(stderr, "FAIL: no SoftGPU GPU agent discovered via HSA\n");
    hsa_shut_down();
    return 1;
  }
  if (ag.feature != 0) {
    fprintf(stderr, "FAIL: FEATURE=%u (expected 0; no dispatch claim)\n", ag.feature);
    hsa_shut_down();
    return 1;
  }

  fprintf(stderr, "softgpu-hip-discovery: gpu name='%s' vendor='%s' feature=0\n",
          ag.name, ag.vendor);
  printf("softgpu-hip-discovery: PASS fidelity=abi phase=2 agent='%s'\n", ag.name);
  printf("softgpu-hip-discovery: controlled_subset=hsa_iterate_from_hip_linked_process\n");
  printf("softgpu-hip-discovery: note=FEATURE=0 so hip device count may be zero\n");

  hsa_shut_down();
  return 0;
}
