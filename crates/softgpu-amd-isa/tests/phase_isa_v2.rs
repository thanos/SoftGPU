//! SoftGPU v0.7 — `softgpu-gfx1201-compute-v2` broader ISA tests.

use softgpu_amd_isa::{
    clamp64_host_ref, run_clamp64_1d, run_salu, run_select_gt50_1d, select_gt50_host_ref,
    words_to_code, Arch, GlobalArena, IsaMemory, MachineState, WaveSize, SUBSET_NAME,
    SUPPORTED_FAMILIES,
};

#[test]
fn subset_is_compute_v2_and_broad() {
    assert_eq!(SUBSET_NAME, "softgpu-gfx1201-compute-v2");
    assert!(SUPPORTED_FAMILIES.len() >= 40);
    assert!(SUPPORTED_FAMILIES.iter().any(|f| f.starts_with("SOPC:")));
    assert!(SUPPORTED_FAMILIES.iter().any(|f| f.starts_with("VOPC:")));
    assert!(SUPPORTED_FAMILIES.iter().any(|f| f.starts_with("VOP1:")));
}

#[test]
fn clamp64_matches_host() {
    let n = 64u32;
    let mut mem = GlobalArena::new(0, 0x4000);
    let kernarg = 0x1000u64;
    let a_addr = 0x2000u64;
    let b_addr = 0x3000u64;
    mem.store_u64(kernarg, a_addr).unwrap();
    mem.store_u64(kernarg + 8, b_addr).unwrap();

    let mut host_a = vec![0u32; n as usize];
    for (i, v) in host_a.iter_mut().enumerate() {
        *v = (i as u32) * 7;
        mem.store_u32(a_addr + (i as u64) * 4, *v).unwrap();
    }
    let mut host_b = vec![0u32; n as usize];
    clamp64_host_ref(&host_a, &mut host_b);

    run_clamp64_1d(&mut mem, kernarg, n, WaveSize::Wave32).unwrap();
    for (i, expected) in host_b.iter().enumerate() {
        let got = mem.load_u32(b_addr + (i as u64) * 4).unwrap();
        assert_eq!(got, *expected, "clamp mismatch at i={i}");
    }
}

#[test]
fn select_gt50_matches_host() {
    let n = 64u32;
    let mut mem = GlobalArena::new(0, 0x4000);
    let kernarg = 0x1000u64;
    let a_addr = 0x2000u64;
    let b_addr = 0x3000u64;
    mem.store_u64(kernarg, a_addr).unwrap();
    mem.store_u64(kernarg + 8, b_addr).unwrap();

    let mut host_a = vec![0u32; n as usize];
    for (i, v) in host_a.iter_mut().enumerate() {
        *v = (i as u32) * 3;
        mem.store_u32(a_addr + (i as u64) * 4, *v).unwrap();
    }
    let mut host_b = vec![0u32; n as usize];
    select_gt50_host_ref(&host_a, &mut host_b);

    run_select_gt50_1d(&mut mem, kernarg, n, WaveSize::Wave32).unwrap();
    for (i, expected) in host_b.iter().enumerate() {
        let got = mem.load_u32(b_addr + (i as u64) * 4).unwrap();
        assert_eq!(got, *expected, "select mismatch at i={i}");
    }
}

#[test]
fn salu_branch_counts_to_five() {
    let code = words_to_code(&[
        0xbe80_0080,
        0xbe81_0085,
        0x8000_8100,
        0xbf07_0100,
        0xbfa2_fffd,
        0xbfb0_0000,
    ]);
    let mut st = MachineState::new(Arch::Gfx1201, WaveSize::Wave32);
    run_salu(&mut st, &code, 64).unwrap();
    assert_eq!(st.sgpr[0], 5);
    assert!(st.halted);
}
