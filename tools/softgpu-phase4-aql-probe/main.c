/* SoftGPU Phase 4 AQL dispatch interception probe (SoftGPU-controlled subset).
 *
 * Submits a lawful golden kernel-dispatch packet, rings the doorbell, and
 * checks SoftGPU's experimental diagnostic completion contract:
 *   - completion signal stored to 0 (not kernel success)
 *   - HSA read_index advanced
 * Does not execute kernels and does not claim HIP launch success.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <hsa/hsa.h>

#if defined(__linux__)
#include <link.h>
#else
#error "Phase 4 AQL probe requires Linux"
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
  const char *libdir = getenv("SOFTGPU_HSA_LIBDIR");
  if (libdir && strstr(info->dlpi_name, libdir)) st->saw_softgpu_hsa = 1;
  if (strstr(info->dlpi_name, "/opt/rocm")) st->saw_system_hsa = 1;
  return 0;
}

static hsa_status_t on_agent(hsa_agent_t agent, void *data) {
  hsa_device_type_t device;
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_DEVICE, &device) != HSA_STATUS_SUCCESS) {
    return HSA_STATUS_ERROR;
  }
  if (device != HSA_DEVICE_TYPE_GPU) {
    return HSA_STATUS_SUCCESS;
  }
  *(hsa_agent_t *)data = agent;
  return HSA_STATUS_INFO_BREAK;
}

/* SoftGPU golden 1D kernel-dispatch packet (64 bytes, little-endian). */
static void write_golden_kernel_dispatch(void *slot, uint64_t completion_handle) {
  uint8_t *b = (uint8_t *)slot;
  memset(b, 0, 64);
  b[0] = (uint8_t)HSA_PACKET_TYPE_KERNEL_DISPATCH;
  b[2] = 1; /* dimensions = 1 */
  uint16_t wg = 64;
  uint16_t one16 = 1;
  uint32_t grid = 256;
  uint32_t one32 = 1;
  uint64_t kernel_object = 0xABCDULL;
  memcpy(b + 4, &wg, 2);
  memcpy(b + 6, &one16, 2);
  memcpy(b + 8, &one16, 2);
  memcpy(b + 12, &grid, 4);
  memcpy(b + 16, &one32, 4);
  memcpy(b + 20, &one32, 4);
  memcpy(b + 32, &kernel_object, 8);
  memcpy(b + 56, &completion_handle, 8);
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

  if (hsa_init() != HSA_STATUS_SUCCESS) {
    fprintf(stderr, "FAIL: hsa_init\n");
    return 1;
  }

  hsa_agent_t agent = {0};
  hsa_status_t st = hsa_iterate_agents(on_agent, &agent);
  if (!(st == HSA_STATUS_SUCCESS || st == HSA_STATUS_INFO_BREAK) || agent.handle == 0) {
    fprintf(stderr, "FAIL: no GPU agent\n");
    hsa_shut_down();
    return 1;
  }

  uint32_t qmin = 0;
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_QUEUE_MIN_SIZE, &qmin) != HSA_STATUS_SUCCESS) {
    fprintf(stderr, "FAIL: QUEUE_MIN_SIZE\n");
    hsa_shut_down();
    return 1;
  }

  hsa_signal_t completion = {0};
  if (hsa_signal_create(1, 0, NULL, &completion) != HSA_STATUS_SUCCESS) {
    fprintf(stderr, "FAIL: hsa_signal_create\n");
    hsa_shut_down();
    return 1;
  }

  hsa_queue_t *queue = NULL;
  if (hsa_queue_create(agent, qmin, HSA_QUEUE_TYPE_MULTI, NULL, NULL, 0, 0, &queue) !=
          HSA_STATUS_SUCCESS ||
      !queue) {
    fprintf(stderr, "FAIL: hsa_queue_create\n");
    hsa_signal_destroy(completion);
    hsa_shut_down();
    return 1;
  }

  write_golden_kernel_dispatch(queue->base_address, completion.handle);
  hsa_queue_store_write_index_screlease(queue, 1);
  hsa_signal_store_screlease(queue->doorbell_signal, 1);

  hsa_signal_value_t done = hsa_signal_load_scacquire(completion);
  uint64_t read_idx = hsa_queue_load_read_index_scacquire(queue);
  if (done != 0) {
    fprintf(stderr, "FAIL: diagnostic completion expected 0, got %lld\n",
            (long long)done);
    hsa_queue_destroy(queue);
    hsa_signal_destroy(completion);
    hsa_shut_down();
    return 1;
  }
  if (read_idx != 1) {
    fprintf(stderr, "FAIL: read_index expected 1, got %llu\n",
            (unsigned long long)read_idx);
    hsa_queue_destroy(queue);
    hsa_signal_destroy(completion);
    hsa_shut_down();
    return 1;
  }

  printf("softgpu-phase4-aql: PASS fidelity=abi phase=4\n");
  printf("softgpu-phase4-aql: contract=diagnostic_complete_no_execution\n");
  printf("softgpu-phase4-aql: note=not_kernel_success\n");

  hsa_queue_destroy(queue);
  hsa_signal_destroy(completion);
  hsa_shut_down();
  return 0;
}
