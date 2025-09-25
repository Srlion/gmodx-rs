use crate::lua::raw::LuaShared;
use std::sync::OnceLock;

// The reason for OnceLock over LazyLock is to have more control over initialization timing.
// So other open_close hooks can run before it if needed.
static LUA_SHARED: OnceLock<LuaShared> = OnceLock::new();

#[inline(always)]
pub fn lua_shared() -> &'static LuaShared {
    LUA_SHARED.get().expect("LUA_SHARED not initialized")
}

#[inline(always)]
pub fn is_loaded() -> bool {
    LUA_SHARED.get().is_some()
}

inventory::submit! {
    crate::open_close::new(
        0,
        "lua_shared",
        |_| {
            LUA_SHARED.get_or_init(do_load);
        },
        |_| {
            // Keep the library loaded for the entire program duration
        },
    )
}

fn do_load() -> LuaShared {
    let paths = get_paths();
    for path in &paths {
        match unsafe { LuaShared::new(path) } {
            Ok(lib) => return lib,
            Err(_) => continue,
        }
    }
    panic!(
        "Failed to load lua_shared library from any known location.\nPaths tried:\n{}",
        paths.join("\n")
    );
}

#[cfg(target_os = "windows")]
fn get_paths() -> Vec<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => vec!["bin/win64/lua_shared.dll", "lua_shared.dll"],
        "x86" => vec![
            "garrysmod/bin/lua_shared.dll",
            "bin/lua_shared.dll",
            "lua_shared.dll",
        ],
        _ => vec!["lua_shared.dll"],
    }
}

#[cfg(target_os = "linux")]
fn get_paths() -> Vec<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => vec!["bin/linux64/lua_shared.so", "lua_shared.so"],
        "x86" => vec![
            "garrysmod/bin/lua_shared_srv.so",
            "bin/linux32/lua_shared.so",
            "garrysmod/bin/lua_shared.so",
            "lua_shared.so",
            "lua_shared_srv.so",
        ],
        _ => vec!["lua_shared.so"],
    }
}

#[cfg(target_os = "macos")]
fn get_paths() -> Vec<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => vec![
            "lua_shared.dylib",
            "GarrysMod_Signed.app/Contents/MacOS/lua_shared.dylib",
        ],
        "x86" => vec!["lua_shared.dylib", "garrysmod/bin/lua_shared.dylib"],
        _ => vec!["lua_shared.dylib"],
    }
}
