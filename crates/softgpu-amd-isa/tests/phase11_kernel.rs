//! Phase 11 — first end-to-end SoftGPU gfx1201 tiny_add kernel.

use softgpu_amd_isa::{
    run_tiny_add_1d, tiny_add_host_ref, GlobalArena, IsaMemory, WaveSize, SUBSET_NAME,
    TINY_ADD_TEXT,
};

#[test]
fn tiny_add_matches_host_reference() {
    assert_eq!(SUBSET_NAME, "softgpu-gfx1201-e2e-tiny-v1");
    assert_eq!(TINY_ADD_TEXT.len(), 60);

    let n = 64u32;
    // SoftGPU arena layout:
    // [0x1000, kernarg 16 bytes) [0x2000, a[]) [0x3000, b[])
    let mut mem = GlobalArena::new(0, 0x4000);
    let kernarg = 0x1000u64;
    let a_addr = 0x2000u64;
    let b_addr = 0x3000u64;
    mem.store_u64(kernarg, a_addr).unwrap();
    mem.store_u64(kernarg + 8, b_addr).unwrap();

    let mut host_a = vec![0i32; n as usize];
    for (i, v) in host_a.iter_mut().enumerate() {
        *v = (i as i32) * 3 - 1;
        mem.store_u32(a_addr + (i as u64) * 4, *v as u32).unwrap();
    }
    let mut host_b = vec![0i32; n as usize];
    tiny_add_host_ref(&host_a, &mut host_b);

    let steps = run_tiny_add_1d(&mut mem, kernarg, n, WaveSize::Wave32).unwrap();
    assert!(steps > 0);

    for (i, expected) in host_b.iter().enumerate() {
        let got = mem.load_u32(b_addr + (i as u64) * 4).unwrap() as i32;
        assert_eq!(got, *expected, "mismatch at i={i}");
    }
}

#[test]
fn unsupported_code_traps() {
    let mut mem = GlobalArena::new(0, 64);
    // Random unsupported word as "kernel"
    let code = [0x11, 0x22, 0x33, 0x44];
    let err = softgpu_amd_isa::run_code_1d(&code, &mut mem, 0, 1, WaveSize::Wave32).unwrap_err();
    assert!(err.message().contains("unsupported") || err.message().contains("Trap"));
}
