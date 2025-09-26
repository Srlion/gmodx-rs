use crate::lua;
use crate::lua::raw::LuaShared;
use std::sync::LazyLock;

pub static LUA_SHARED: LazyLock<LuaShared> = LazyLock::new(do_load);

fn do_load() -> LuaShared {
    let paths = get_paths();

    let mut errors = Vec::new();
    for path in &paths {
        match unsafe { LuaShared::new(path) } {
            Ok(lib) => {
                lua::bridge::setup(&lib);
                return lib;
            }
            Err(err) => errors.push(err.to_string()),
        }
    }

    panic!(
        "Failed to load lua_shared library from any known location.\nPaths tried:\n{}\nErrors:\n{}",
        paths.join("\n"),
        errors.join("\n")
    );
}

#[cfg(target_os = "windows")]
fn get_paths() -> Vec<String> {
    match std::env::consts::ARCH {
        "x86_64" => vec!["bin/win64/lua_shared.dll", "lua_shared.dll"],
        "x86" => vec![
            "garrysmod/bin/lua_shared.dll",
            "bin/lua_shared.dll",
            "lua_shared.dll",
        ],
        _ => vec!["lua_shared.dll"],
    }
    .into_iter()
    .map(String::from)
    .collect()
}

#[cfg(target_os = "linux")]
fn get_paths() -> Vec<String> {
    match std::env::consts::ARCH {
        "x86_64" => vec!["bin/linux64/lua_shared.so", "lua_shared.so"],
        "x86" => vec![
            "garrysmod/bin/lua_shared_srv.so",
            "bin/linux32/lua_shared.so",
            "garrysmod/bin/lua_shared.so",
            "lua_shared.so",
            "lua_shared_srv.so",
        ],
        _ => vec!["lua_shared.so", "lua_shared_srv.so"],
    }
    .into_iter()
    .map(String::from)
    .collect()
}
