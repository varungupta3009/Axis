use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sysroot_lib = manifest_dir.join("sysroot").join("lib");

    println!("cargo:rustc-link-search=native={}", sysroot_lib.display());
    println!("cargo:rustc-link-lib=dylib=SDL2");
    println!("cargo:rustc-link-lib=dylib=SDL2_image");
    println!("cargo:rustc-link-lib=dylib=SDL2_ttf");

    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib");
    println!("cargo:rustc-link-arg=-Wl,--allow-shlib-undefined");
}
