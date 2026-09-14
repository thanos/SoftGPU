fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = std::path::Path::new(&manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let header_root = workspace_root.join("third_party/rocr-headers/hsa");

    println!("cargo:rerun-if-changed=hsa-runtime64.version");
    println!("cargo:rerun-if-changed=link-cdylib.sh");
    println!(
        "cargo:rerun-if-changed={}",
        header_root.join("hsa.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        header_root.join("hsa_ext_amd.h").display()
    );

    let mut build = cc::Build::new();
    build.include(&header_root);
    build.define("HSA_EXPORT", None);
    build.warnings(false);
    build.flag_if_supported("-fvisibility=default");

    for name in ["generated_stubs.c", "generated_amd_stubs.c"] {
        let path = std::path::Path::new(&manifest_dir).join("src").join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        if path.exists() {
            build.file(&path);
        }
    }
    // Suppress cc's automatic `-l` — we link the object files directly so every
    // fail-closed stub is present (an archive would otherwise be GC'd).
    build.cargo_metadata(false);
    build.compile("softgpu_hsa_stubs");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let mut stub_objs = Vec::new();
    for entry in std::fs::read_dir(&out_dir).expect("OUT_DIR read") {
        let entry = entry.expect("OUT_DIR entry");
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.ends_with(".o") && name.contains("generated_") {
            stub_objs.push(path);
        }
    }
    stub_objs.sort();
    assert!(
        !stub_objs.is_empty(),
        "expected generated stub .o files in OUT_DIR"
    );
    for obj in &stub_objs {
        // cdylib-only: do not pull stubs into unrelated link lines.
        println!("cargo:rustc-cdylib-link-arg={}", obj.display());
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    // rustc always injects an anonymous --version-script for Linux cdylibs.
    // A second script makes GNU ld fail ("anonymous version tag cannot be
    // combined with other version tags"). The workspace GNU/Linux linker wrapper
    // (crates/softgpu-hsa/link-cdylib.sh via .cargo/config.toml) replaces rustc's
    // script so SoftGPU can export all hsa_* symbols (Rust + C stubs).
    if target_os == "linux" && target_env == "gnu" {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libhsa-runtime64.so.1");
    }
}
