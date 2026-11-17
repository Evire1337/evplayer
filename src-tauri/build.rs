fn main() {
    // The song splitter's ONNX Runtime (stems.rs) is linked into the program on Windows together
    // with the import of DirectML.dll, a video-card path this program never uses. Loading that
    // DLL only when it is first called keeps the program starting without it.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/DELAYLOAD:DirectML.dll");
        println!("cargo:rustc-link-lib=delayimp");
    }
    tauri_build::build()
}
