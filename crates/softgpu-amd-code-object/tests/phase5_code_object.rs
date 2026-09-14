//! Phase 5 charter tests: fixtures, fail-closed versions/targets, fuzz floor.

use softgpu_amd_code_object::fixture::{
    fixture_multi_kernel_gfx1201, fixture_tiny_add_gfx1201, fixture_unsupported_target,
    fixture_unsupported_version,
};
use softgpu_amd_code_object::{inspect_bytes, CodeObjectError, SUPPORTED_GFX_SUBSTRING};
use std::time::{Duration, Instant};

#[test]
fn tiny_add_fixture_parses_kernarg_and_target() {
    let bytes = fixture_tiny_add_gfx1201();
    let info = inspect_bytes(&bytes).unwrap();
    assert!(info.target.contains(SUPPORTED_GFX_SUBSTRING));
    assert_eq!(info.metadata_version.major, 1);
    assert_eq!(info.metadata_version.minor, 2);
    assert_eq!(info.kernels.len(), 1);
    let k = &info.kernels[0];
    assert_eq!(k.name, "tiny_add");
    assert_eq!(k.kernarg_segment_size, 16);
    assert_eq!(k.kernarg_segment_align, 8);
    assert_eq!(k.args.len(), 2);
    assert_eq!(k.args[0].offset, 0);
    assert_eq!(k.args[0].size, 8);
    assert_eq!(k.args[0].value_kind, "global_buffer");
    assert_eq!(info.fidelity, "abi");
    assert_eq!(info.note, "metadata_only_no_isa_execution");
}

#[test]
fn multi_kernel_segments() {
    let info = inspect_bytes(&fixture_multi_kernel_gfx1201()).unwrap();
    assert_eq!(info.kernels.len(), 2);
    assert_eq!(info.kernels[1].name, "reduce");
    assert_eq!(info.kernels[1].group_segment_fixed_size, 256);
    assert_eq!(info.kernels[1].private_segment_fixed_size, 64);
}

#[test]
fn unsupported_target_fails_closed() {
    let err = inspect_bytes(&fixture_unsupported_target()).unwrap_err();
    assert!(matches!(err, CodeObjectError::UnsupportedTarget { .. }));
}

#[test]
fn unsupported_version_fails_closed() {
    let err = inspect_bytes(&fixture_unsupported_version()).unwrap_err();
    assert!(matches!(
        err,
        CodeObjectError::UnsupportedMetadataVersion { major: 2, minor: 0 }
    ));
}

#[test]
fn malformed_elf_fails_closed() {
    assert!(inspect_bytes(b"").is_err());
    assert!(inspect_bytes(b"\x7fELF").is_err());
    let mut junk = fixture_tiny_add_gfx1201();
    junk.truncate(32);
    assert!(inspect_bytes(&junk).is_err());
}

#[test]
fn fuzz_smoke_no_panic_bounded() {
    // Charter floor: fixed mutation budget + wall-clock cap; no crash/hang/OOM.
    let seed = fixture_tiny_add_gfx1201();
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut mutations = 0u32;
    let mut state: u64 = 0xC0FFEE_u64;
    while Instant::now() < deadline && mutations < 2_000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut buf = seed.clone();
        let n_flips = (state % 8) as usize + 1;
        for i in 0..n_flips {
            let idx = ((state >> (i * 8)) as usize) % buf.len().max(1);
            buf[idx] ^= (state as u8).wrapping_add(i as u8);
        }
        if buf.len() > 4 && (state & 0xf) == 0 {
            let cut = ((state >> 16) as usize) % buf.len();
            buf.truncate(cut);
        }
        let _ = inspect_bytes(&buf);
        mutations += 1;
    }
    assert!(
        mutations >= 100,
        "fuzz floor too low: only {mutations} mutations"
    );
}
