fn main() {
    // the shared modules of src-tauri mark their computer-only parts with `cfg(desktop)`, a
    // flag Tauri's build sets; this build is for computers only
    println!("cargo::rustc-check-cfg=cfg(desktop)");
    println!("cargo::rustc-check-cfg=cfg(mobile)");
    println!("cargo:rustc-cfg=desktop");
    // as in src-tauri/build.rs: ONNX Runtime imports DirectML.dll, a path never used here
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/DELAYLOAD:DirectML.dll");
        println!("cargo:rustc-link-lib=delayimp");
        // a window program: no console behind it in a release build
        if std::env::var("PROFILE").as_deref() == Ok("release") {
            println!("cargo:rustc-link-arg-bins=/SUBSYSTEM:WINDOWS");
            println!("cargo:rustc-link-arg-bins=/ENTRY:mainCRTStartup");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
