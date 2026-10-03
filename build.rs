fn main() {
    // Without declared inputs cargo reruns this script, and with it the whole
    // bin crate, whenever any file in the package changes (even index.html).
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=res/logo.ico");

    link_scip_at_runtime();

    // Generate Windows ICO
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("res/logo.ico");
        res.compile().expect("failed to compile Windows resources");
    }
}

/// Make the SCIP shared library findable when the binary runs.
///
/// scip-sys records where it linked SCIP from (`links = "scip"`, surfaced here
/// as `DEP_SCIP_LIBDIR`) and asks for an rpath there - but a dependency's
/// `rustc-link-arg` never reaches a downstream binary, so without this the
/// executable only starts under `cargo run`, which adds the path itself.
///
/// Two entries: the directory SCIP was linked from, so a development build
/// runs from `target/` as-is, and the executable's own directory, so a
/// packaged build can ship `libscip` beside it. Windows has no rpath: there
/// the DLL has to sit beside the executable or on `PATH`.
fn link_scip_at_runtime() {
    println!("cargo:rerun-if-env-changed=DEP_SCIP_LIBDIR");
    let Ok(dir) = std::env::var("DEP_SCIP_LIBDIR") else {
        return;
    };
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("linux") => {
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{dir}");
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
        }
        Ok("macos") => {
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{dir}");
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,@executable_path");
        }
        _ => {}
    }
}
