use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=lua.h");

    let lua_bindings = bindgen::Builder::default()
        .header("lua.h")
        .allowlist_file("lua.h")
        .allowlist_item("(?i).*lua.*")
        .allowlist_function("(?i).*lua.*")
        .allowlist_type("(?i).*lua.*")
        .allowlist_var("(?i).*lua.*")
        .wrap_unsafe_ops(true)
        .dynamic_library_name("LuaShared")
        .dynamic_link_require_all(true)
        .override_abi(bindgen::Abi::CUnwind, "rust_function_callback")
        .override_abi(bindgen::Abi::CUnwind, "rust_closure_callback")
        .override_abi(bindgen::Abi::CUnwind, "lua_CFunction")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate lua bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    lua_bindings
        .write_to_file(out_path.join("lua.rs"))
        .expect("Couldn't write lua bindings!");
}
