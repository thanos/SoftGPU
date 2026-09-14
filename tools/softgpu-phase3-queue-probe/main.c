/* SoftGPU Phase 3 HSA memory + queue smoke (HIP-linked optional).
 *
 * Exercises Path C memory and queue create/observe under SoftGPU substitution.
 * Does not execute AQL packets.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <hsa/hsa.h>
#include <hsa/hsa_ext_amd.h>

#if defined(__linux__)
#include <link.h>
#else
#error "Phase 3 queue probe requires Linux"
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

static hsa_status_t on_region(hsa_region_t region, void *data) {
  bool allowed = false;
  if (hsa_region_get_info(region, HSA_REGION_INFO_RUNTIME_ALLOC_ALLOWED, &allowed) !=
      HSA_STATUS_SUCCESS) {
    return HSA_STATUS_SUCCESS;
  }
  if (!allowed) return HSA_STATUS_SUCCESS;
  *(hsa_region_t *)data = region;
  return HSA_STATUS_INFO_BREAK;
}

static hsa_status_t on_pool(hsa_amd_memory_pool_t pool, void *data) {
  bool allowed = false;
  if (hsa_amd_memory_pool_get_info(pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED,
                                   &allowed) != HSA_STATUS_SUCCESS) {
    return HSA_STATUS_SUCCESS;
  }
  if (!allowed) return HSA_STATUS_SUCCESS;
  *(hsa_amd_memory_pool_t *)data = pool;
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

  uint32_t feature = 0;
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_FEATURE, &feature) != HSA_STATUS_SUCCESS ||
      (feature & HSA_AGENT_FEATURE_KERNEL_DISPATCH) == 0) {
    fprintf(stderr, "FAIL: FEATURE missing KERNEL_DISPATCH\n");
    hsa_shut_down();
    return 1;
  }

  hsa_region_t region = {0};
  st = hsa_agent_iterate_regions(agent, on_region, &region);
  if (!(st == HSA_STATUS_SUCCESS || st == HSA_STATUS_INFO_BREAK) || region.handle == 0) {
    fprintf(stderr, "FAIL: no allocable region\n");
    hsa_shut_down();
    return 1;
  }
  void *mem = NULL;
  if (hsa_memory_allocate(region, 4096, &mem) != HSA_STATUS_SUCCESS || !mem) {
    fprintf(stderr, "FAIL: hsa_memory_allocate\n");
    hsa_shut_down();
    return 1;
  }
  hsa_memory_free(mem);

  hsa_amd_memory_pool_t pool = {0};
  st = hsa_amd_agent_iterate_memory_pools(agent, on_pool, &pool);
  if (!(st == HSA_STATUS_SUCCESS || st == HSA_STATUS_INFO_BREAK) || pool.handle == 0) {
    fprintf(stderr, "FAIL: no allocable AMD pool\n");
    hsa_shut_down();
    return 1;
  }
  void *pmem = NULL;
  if (hsa_amd_memory_pool_allocate(pool, 4096, 0, &pmem) != HSA_STATUS_SUCCESS || !pmem) {
    fprintf(stderr, "FAIL: hsa_amd_memory_pool_allocate\n");
    hsa_shut_down();
    return 1;
  }
  hsa_amd_memory_pool_free(pmem);

  uint32_t qmin = 0;
  if (hsa_agent_get_info(agent, HSA_AGENT_INFO_QUEUE_MIN_SIZE, &qmin) != HSA_STATUS_SUCCESS) {
    fprintf(stderr, "FAIL: QUEUE_MIN_SIZE\n");
    hsa_shut_down();
    return 1;
  }
  hsa_queue_t *queue = NULL;
  if (hsa_queue_create(agent, qmin, HSA_QUEUE_TYPE_MULTI, NULL, NULL, 0, 0, &queue) !=
          HSA_STATUS_SUCCESS ||
      !queue) {
    fprintf(stderr, "FAIL: hsa_queue_create\n");
    hsa_shut_down();
    return 1;
  }
  hsa_signal_store_screlease(queue->doorbell_signal, 1);
  hsa_queue_store_write_index_screlease(queue, 1);
  if (hsa_queue_load_write_index_scacquire(queue) != 1) {
    fprintf(stderr, "FAIL: write index observe\n");
    hsa_queue_destroy(queue);
    hsa_shut_down();
    return 1;
  }
  hsa_queue_destroy(queue);

  printf("softgpu-phase3-queue: PASS fidelity=abi phase=3\n");
  printf("softgpu-phase3-queue: note=queue_ABI_observe_only_no_packet_execution\n");
  hsa_shut_down();
  return 0;
}
