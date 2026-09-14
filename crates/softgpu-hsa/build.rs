fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = std::path::Path::new(&manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let header_root = workspace_root.join("third_party/rocr-headers/hsa");

    println!("cargo:rerun-if-changed=hsa-runtime64.version");
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

    for name in ["generated_stubs.c", "generated_amd_stubs.c"] {
        let path = std::path::Path::new(&manifest_dir).join("src").join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        if path.exists() {
            build.file(&path);
        }
    }
    build.compile("softgpu_hsa_stubs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        let script = std::path::Path::new(&manifest_dir).join("hsa-runtime64.version");
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}",
            script.display()
        );
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libhsa-runtime64.so.1");
    }
}
