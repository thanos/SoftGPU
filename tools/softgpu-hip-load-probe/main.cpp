/* SoftGPU Phase 1 HIP load probe.
 *
 * Links against the real HIP runtime (hipcc) while resolving libhsa-runtime64
 * from SoftGPU via LD_LIBRARY_PATH. Proves SoftGPU is the loaded HSA library
 * and that /opt/rocm's libhsa-runtime64 is not mapped.
 *
 * No kernel launch.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <hip/hip_runtime.h>

#if defined(__linux__)
#include <link.h>
#else
#error "Phase 1 HIP load probe requires Linux"
#endif

struct check_state {
  int saw_softgpu_hsa;
  int saw_system_hsa;
  char softgpu_path[4096];
  char system_path[4096];
};

static int is_hsa_runtime_name(const char *name) {
  return strstr(name, "libhsa-runtime64") != NULL ||
         strstr(name, "libhsa_runtime64") != NULL;
}

static int on_phdr(struct dl_phdr_info *info, size_t size, void *data) {
  (void)size;
  struct check_state *st = (struct check_state *)data;
  if (!info->dlpi_name || !info->dlpi_name[0]) {
    return 0;
  }
  if (!is_hsa_runtime_name(info->dlpi_name)) {
    return 0;
  }
  fprintf(stderr, "softgpu-hip-load-probe: mapped HSA library: %s\n", info->dlpi_name);

  const char *libdir = getenv("SOFTGPU_HSA_LIBDIR");
  if (libdir && libdir[0] && strstr(info->dlpi_name, libdir) != NULL) {
    st->saw_softgpu_hsa = 1;
    snprintf(st->softgpu_path, sizeof(st->softgpu_path), "%s", info->dlpi_name);
  }
  if (strstr(info->dlpi_name, "/opt/rocm") != NULL) {
    st->saw_system_hsa = 1;
    snprintf(st->system_path, sizeof(st->system_path), "%s", info->dlpi_name);
  }
  return 0;
}

int main(void) {
  const char *libdir = getenv("SOFTGPU_HSA_LIBDIR");
  if (!libdir || !libdir[0]) {
    fprintf(stderr,
            "SOFTGPU_HSA_LIBDIR must point at the directory containing SoftGPU's "
            "libhsa-runtime64.so\n");
    return 2;
  }

  struct check_state st;
  memset(&st, 0, sizeof(st));
  dl_iterate_phdr(on_phdr, &st);

  if (st.saw_system_hsa) {
    fprintf(stderr,
            "FAIL: system ROCr is mapped (%s); SoftGPU substitution did not win\n",
            st.system_path);
    return 1;
  }
  if (!st.saw_softgpu_hsa) {
    fprintf(stderr,
            "FAIL: SoftGPU HSA library under SOFTGPU_HSA_LIBDIR=%s was not mapped\n",
            libdir);
    return 1;
  }

  fprintf(stderr, "softgpu-hip-load-probe: SoftGPU HSA resolved to %s\n", st.softgpu_path);

  hipError_t err = hipInit(0);
  fprintf(stderr, "softgpu-hip-load-probe: hipInit -> %d (%s)\n", (int)err,
          hipGetErrorString(err));
  int devices = 0;
  err = hipGetDeviceCount(&devices);
  fprintf(stderr, "softgpu-hip-load-probe: hipGetDeviceCount -> %d devices=%d (%s)\n",
          (int)err, devices, hipGetErrorString(err));

  printf("softgpu-hip-load-probe: PASS fidelity=abi phase=1 softgpu_hsa=%s\n",
         st.softgpu_path);
  printf("softgpu-hip-load-probe: note=no-kernel-execution\n");
  return 0;
}
