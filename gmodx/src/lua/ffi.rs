#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::sync::LazyLock;

include!(concat!(env!("OUT_DIR"), "/lua.rs"));

pub static FFI: LazyLock<LuaShared> = LazyLock::new(|| {
    fn do_load() -> LuaShared {
        let paths = get_paths();

        let mut errors = Vec::new();
        for path in &paths {
            match unsafe { LuaShared::new(path) } {
                Ok(lib) => return lib,

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

    do_load()
});

#[inline(always)]
pub fn new_thread(l: *mut lua_State) -> *mut lua_State {
    unsafe { FFI.lua_newthread(l) }
}

#[inline(always)]
pub fn lua_pushvalue(l: *mut lua_State, index: i32) {
    unsafe { FFI.lua_pushvalue(l, index) };
}

#[inline(always)]
pub fn lua_xmove(from: *mut lua_State, to: *mut lua_State, n: i32) {
    unsafe { FFI.lua_xmove(from, to, n) };
}

#[inline(always)]
pub fn lua_xpush(from: *mut lua_State, to: *mut lua_State, idx: i32) {
    lua_pushvalue(from, idx);
    lua_xmove(from, to, 1);
}

#[inline(always)]
pub fn lua_replace(l: *mut lua_State, index: i32) {
    unsafe { FFI.lua_replace(l, index) };
}

#[inline(always)]
pub fn lua_pushnil(l: *mut lua_State) {
    unsafe { FFI.lua_pushnil(l) };
}

#[inline(always)]
pub fn lua_gettop(l: *mut lua_State) -> i32 {
    unsafe { FFI.lua_gettop(l) }
}

#[inline(always)]
pub fn lua_settop(l: *mut lua_State, index: i32) {
    unsafe { FFI.lua_settop(l, index) };
}

#[inline(always)]
pub fn lua_type(l: *mut lua_State, index: i32) -> i32 {
    unsafe { FFI.lua_type(l, index) }
}

#[inline(always)]
pub fn lua_tolstring(l: *mut lua_State, index: i32, len: &mut usize) -> *const i8 {
    unsafe { FFI.lua_tolstring(l, index, len as *mut usize) }
}

#[inline(always)]
pub fn lua_touserdata(l: *mut lua_State, index: i32) -> *mut std::ffi::c_void {
    unsafe { FFI.lua_touserdata(l, index) }
}

#[inline(always)]
pub fn lua_rawset(l: *mut lua_State, index: i32) {
    unsafe { FFI.lua_rawset(l, index) };
}

#[inline(always)]
pub fn lua_rawget(l: *mut lua_State, index: i32) {
    unsafe { FFI.lua_rawget(l, index) };
}

#[allow(unused)]
#[inline(always)]
pub fn lua_rawgeti(l: *mut lua_State, index: i32, n: i32) {
    unsafe { FFI.lua_rawgeti(l, index, n) };
}

#[inline(always)]
pub fn lua_createtable(l: *mut lua_State, narr: i32, nrec: i32) {
    unsafe { FFI.lua_createtable(l, narr, nrec) };
}

#[inline(always)]
pub fn luaL_newmetatable(l: *mut lua_State, tname: *const i8) -> bool {
    unsafe { FFI.luaL_newmetatable(l, tname) == 1 }
}

#[inline(always)]
pub fn lua_setfield(l: *mut lua_State, index: i32, k: *const i8) {
    unsafe { FFI.lua_setfield(l, index, k) }
}

#[inline(always)]
pub fn lua_setmetatable(l: *mut lua_State, index: i32) -> i32 {
    unsafe { FFI.lua_setmetatable(l, index) }
}

#[inline(always)]
pub fn lua_newuserdata(l: *mut lua_State, size: usize) -> *mut std::ffi::c_void {
    unsafe { FFI.lua_newuserdata(l, size) }
}

#[allow(unused)]
#[inline(always)]
pub fn lua_topointer(l: *mut lua_State, index: i32) -> *const std::ffi::c_void {
    unsafe { FFI.lua_topointer(l, index) }
}

#[inline(always)]
pub fn lua_pushboolean(l: *mut lua_State, b: i32) {
    unsafe { FFI.lua_pushboolean(l, b) };
}

#[inline(always)]
pub fn lua_pushnumber(l: *mut lua_State, n: lua_Number) {
    unsafe { FFI.lua_pushnumber(l, n) };
}

#[inline(always)]
pub fn lua_pushcclosure(l: *mut lua_State, f: lua_CFunction, n: i32) {
    unsafe { FFI.lua_pushcclosure(l, f, n) };
}

#[inline(always)]
pub fn lua_toboolean(l: *mut lua_State, index: i32) -> bool {
    unsafe { FFI.lua_toboolean(l, index) == 1 }
}

#[inline(always)]
pub fn lua_tonumber(l: *mut lua_State, index: i32) -> lua_Number {
    unsafe { FFI.lua_tonumber(l, index) }
}

#[inline(always)]
pub fn lua_pushlightuserdata(l: *mut lua_State, p: *mut std::ffi::c_void) {
    unsafe { FFI.lua_pushlightuserdata(l, p) };
}

#[inline(always)]
pub fn lua_insert(l: *mut lua_State, index: i32) {
    unsafe { FFI.lua_insert(l, index) };
}

#[inline(always)]
pub fn lua_absindex(L: *mut lua_State, mut idx: i32) -> i32 {
    if idx < 0 && idx > LUA_REGISTRYINDEX {
        idx += lua_gettop(L) + 1;
    }
    idx
}

#[allow(unused)]
#[inline(always)]
pub fn lua_tostring(L: *mut lua_State, i: i32) -> *const i8 {
    unsafe { FFI.lua_tolstring(L, i, std::ptr::null_mut()) }
}

#[inline(always)]
pub fn lua_pushlstring(L: *mut lua_State, s: *const i8, l: usize) {
    if l == 0 {
        unsafe { FFI.lua_pushlstring(L, c"".as_ptr(), 0) };
    } else {
        unsafe { FFI.lua_pushlstring(L, s, l) };
    }
}

#[inline(always)]
pub fn lua_pushstring(L: *mut lua_State, s: *const i8) {
    unsafe { FFI.lua_pushstring(L, s) };
}

#[inline(always)]
pub fn lua_error(L: *mut lua_State) -> ! {
    unsafe { FFI.lua_error(L) };
    unreachable!();
}

#[inline(always)]
pub fn lua_pcall(L: *mut lua_State, nargs: i32, nresults: i32, errfunc: i32) -> i32 {
    unsafe { FFI.lua_pcall(L, nargs, nresults, errfunc) }
}

pub const fn lua_upvalueindex(i: i32) -> i32 {
    LUA_GLOBALSINDEX - i
}

#[inline(always)]
pub fn lua_pushcfunction(L: *mut lua_State, f: lua_CFunction) {
    unsafe { FFI.lua_pushcclosure(L, f, 0) };
}

#[inline(always)]
pub fn lua_pop(L: *mut lua_State, n: i32) {
    lua_settop(L, -n - 1);
}

#[allow(unused)]
#[inline(always)]
pub fn lua_remove(L: *mut lua_State, index: i32) {
    unsafe { FFI.lua_remove(L, index) };
}

#[inline(always)]
pub fn lua_typename(L: *mut lua_State, tp: i32) -> *const i8 {
    unsafe { FFI.lua_typename(L, tp, 0) }
}

#[inline(always)]
pub fn lua_rawequal(L: *mut lua_State, index1: i32, index2: i32) -> bool {
    unsafe { FFI.lua_rawequal(L, index1, index2) == 1 }
}

#[inline(always)]
pub fn lua_getmetatable(L: *mut lua_State, index: i32) -> i32 {
    unsafe { FFI.lua_getmetatable(L, index) }
}

#[inline(always)]
pub fn lua_settable(L: *mut lua_State, index: i32) {
    unsafe { FFI.lua_settable(L, index) };
}

#[inline(always)]
pub fn lua_gettable(L: *mut lua_State, index: i32) {
    unsafe { FFI.lua_gettable(L, index) };
}

#[inline(always)]
pub fn luaL_ref(L: *mut lua_State, t: i32) -> i32 {
    unsafe { FFI.luaL_ref(L, t) }
}

#[inline(always)]
pub fn lua_isnumber(L: *mut lua_State, i: i32) -> i32 {
    unsafe { FFI.lua_isnumber(L, i) }
}

#[inline(always)]
pub fn lua_tonumberx(L: *mut lua_State, i: i32, isnum: *mut i32) -> lua_Number {
    let n = lua_tonumber(L, i);
    if !isnum.is_null() {
        unsafe {
            *isnum = (n != 0.0 || lua_isnumber(L, i) != 0) as i32;
        }
    }
    n
}

#[inline(always)]
pub fn lua_rawseti(L: *mut lua_State, index: i32, n: i32) {
    unsafe { FFI.lua_rawseti(L, index, n) };
}

#[inline(always)]
pub fn lua_rawlen(L: *mut lua_State, index: i32) -> usize {
    unsafe { FFI.lua_objlen(L, index) }
}

#[inline(always)]
pub fn luaL_callmeta(L: *mut lua_State, obj: i32, e: *const i8) -> i32 {
    unsafe { FFI.luaL_callmeta(L, obj, e) }
}

#[inline(always)]
pub fn lua_len(L: *mut lua_State, idx: i32) {
    match lua_type(L, idx) {
        LUA_TSTRING => {
            lua_pushnumber(L, lua_rawlen(L, idx) as lua_Number);
        }
        LUA_TTABLE => {
            if luaL_callmeta(L, idx, c"__len".as_ptr()) == 0 {
                lua_pushnumber(L, lua_rawlen(L, idx) as lua_Number);
            }
        }
        LUA_TUSERDATA if luaL_callmeta(L, idx, c"__len".as_ptr()) != 0 => {}
        _ => unsafe {
            (FFI.luaL_error)(
                L,
                c"attempt to get length of a %s value".as_ptr(),
                lua_typename(L, lua_type(L, idx)),
            );
        },
    }
}

#[inline(always)]
pub fn luaL_len(L: *mut lua_State, idx: i32) -> lua_Number {
    let mut isnum = 0;
    lua_len(L, idx);
    let res = lua_tonumberx(L, -1, &mut isnum);
    lua_pop(L, 1);
    if isnum == 0 {
        unsafe { (FFI.luaL_error)(L, c"object length is not an integer".as_ptr()) };
    }
    res
}

#[inline(always)]
pub fn lua_seti(L: *mut lua_State, mut idx: i32, n: lua_Number) {
    idx = lua_absindex(L, idx);
    lua_pushnumber(L, n);
    lua_insert(L, -2);
    lua_settable(L, idx);
}

#[inline(always)]
pub fn lua_getstack(L: *mut lua_State, level: i32, ar: *mut lua_Debug) -> i32 {
    unsafe { FFI.lua_getstack(L, level, ar) }
}

#[inline(always)]
pub fn lua_getinfo(L: *mut lua_State, what: *const i8, ar: *mut lua_Debug) -> i32 {
    unsafe { FFI.lua_getinfo(L, what, ar) }
}

#[inline(always)]
pub fn luaL_loadbuffer(
    L: *mut lua_State,
    buff: *const ::std::os::raw::c_char,
    sz: usize,
    name: *const ::std::os::raw::c_char,
) -> i32 {
    unsafe { FFI.luaL_loadbuffer(L, buff, sz, name) }
}

#[inline(always)]
pub fn lua_yield(L: *mut lua_State, nresults: i32) -> i32 {
    unsafe { FFI.lua_yield(L, nresults) }
}

#[inline(always)]
pub fn lua_resume(L: *mut lua_State, narg: i32) -> i32 {
    unsafe { FFI.lua_resume_real(L, narg) }
}

#[inline(always)]
pub fn lua_status(L: *mut lua_State) -> i32 {
    unsafe { FFI.lua_status(L) }
}

#[inline(always)]
pub fn lua_tothread(L: *mut lua_State, idx: i32) -> *mut lua_State {
    unsafe { FFI.lua_tothread(L, idx) }
}

#[inline(always)]
pub fn lua_getfenv(L: *mut lua_State, idx: i32) {
    unsafe { FFI.lua_getfenv(L, idx) };
}
