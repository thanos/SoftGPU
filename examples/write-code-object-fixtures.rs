//! Write SoftGPU-synthetic code-object fixtures under fixtures/amd-code-object/.

use softgpu_amd_code_object::fixture::{
    fixture_multi_kernel_gfx1201, fixture_tiny_add_gfx1201, fixture_unsupported_target,
    fixture_unsupported_version,
};
use std::fs;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/amd-code-object");
    fs::create_dir_all(&root).expect("mkdir");
    let files = [
        ("tiny-add-gfx1201.softgpu.co", fixture_tiny_add_gfx1201()),
        (
            "multi-kernel-gfx1201.softgpu.co",
            fixture_multi_kernel_gfx1201(),
        ),
        (
            "unsupported-target.softgpu.co",
            fixture_unsupported_target(),
        ),
        (
            "unsupported-version.softgpu.co",
            fixture_unsupported_version(),
        ),
    ];
    for (name, bytes) in files {
        let path = root.join(name);
        fs::write(&path, &bytes).expect("write");
        println!("wrote {} ({} bytes)", path.display(), bytes.len());
    }
}
