use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=bridge.h");
    println!("cargo:rerun-if-changed=bridge.c");
    println!("cargo:rerun-if-changed=lua.h");

    {
        let lua_bindings = bindgen::Builder::default()
            .header("lua.h")
            .blocklist_file(".*stdlib\\.h")
            .blocklist_file(".*stdint\\.h")
            .blocklist_file(".*stdbool\\.h")
            .blocklist_function("alloca")
            .blocklist_function("select")
            .blocklist_function("pselect")
            .wrap_unsafe_ops(true)
            .dynamic_library_name("LuaShared")
            .dynamic_link_require_all(true)
            // .override_abi(bindgen::Abi::CUnwind, "lua_CFunction")
            // Tell cargo to invalidate the built crate whenever any of the
            // included header files changed.
            .override_abi(bindgen::Abi::CUnwind, "rust_function_callback")
            .override_abi(bindgen::Abi::CUnwind, "rust_closure_callback")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .generate()
            .expect("Unable to generate lua bindings");

        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        lua_bindings
            .write_to_file(out_path.join("lua.rs"))
            .expect("Couldn't write lua bindings!");
    }

    {
        cc::Build::new()
            .file("bridge.c")
            .static_flag(true)
            .flag("-fvisibility=hidden")
            .compile("bridge");
    }
}
