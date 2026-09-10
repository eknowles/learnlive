fn main() {
    // `tauri dev` runs the bare binary out of target/debug, not a .app bundle, so macOS TCC has
    // no Info.plist to read and kills the process the first time cpal opens an input device (and
    // likewise for EventKit). Embedding the same plist as a __TEXT,__info_plist section makes the
    // usage strings available whether we're bundled or not. The bundle's own Info.plist still
    // wins for a real .app.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let plist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Info.plist");
        println!("cargo:rerun-if-changed={}", plist.display());
        println!("cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,{}", plist.display());

        // sherpa-onnx and its bundled onnxruntime are dylibs linked as `@rpath/...`, but nothing
        // emits an LC_RPATH, so the binary fails to launch with "no LC_RPATH's found". Cargo puts
        // them next to the executable in target/<profile>/ during development, so point the
        // loader there too; a real .app bundle needs them copied into Contents/Frameworks, which
        // is what tauri.conf.json's bundle.macOS.frameworks does.
        for path in ["@executable_path", "@executable_path/../Frameworks", "@loader_path"] {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{path}");
        }
    }

    tauri_build::build()
}
