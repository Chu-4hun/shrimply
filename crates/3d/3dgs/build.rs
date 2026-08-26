use std::{env, fs, path::PathBuf};

use serde_json::Value;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../slang-build/reflect.cpp");
    println!("cargo:rerun-if-changed=shaders");
    println!("cargo:rerun-if-env-changed=SLANG_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=SLANG_BUILD_DIR");

    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let shader_directory = manifest.join("shaders");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let compiler = shrimply_slang_build::Compiler::new(&manifest, &output);
    let mut bindings = String::from("// @generated from dedicated 3DGS Slang reflection.\n");
    for source in shrimply_slang_build::shader_sources(&shader_directory) {
        println!("cargo:rerun-if-changed={}", source.display());
        let module = source
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("3DGS Slang module filename must be UTF-8");
        let artifacts = compiler.compile(&shader_directory, &source, &output);
        let reflected: Value = serde_json::from_slice(&artifacts.reflection)
            .unwrap_or_else(|error| panic!("parse 3DGS Slang reflection: {error}"));
        bindings.push_str(&shrimply_slang_build::generate_module(
            module,
            &artifacts.spirv_filename,
            &reflected,
            &artifacts.abi,
        ));
    }
    fs::write(output.join("slang_bindings.rs"), bindings)
        .expect("write reflected 3DGS Slang Rust bindings");
}
