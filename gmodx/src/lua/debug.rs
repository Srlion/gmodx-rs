use std::os::raw::c_char;
use std::{ffi::CStr, mem::MaybeUninit};

use crate::lua::{self, ffi};

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub event: i32,
    pub name: Option<lua::String>,
    pub namewhat: Option<lua::String>,
    pub what: Option<lua::String>,
    pub source: Option<lua::String>,
    pub currentline: i32,
    pub nups: i32,
    pub linedefined: i32,
    pub lastlinedefined: i32,
    pub short_src: lua::String,
    /* private part */
    _i_ci: i32,
}

impl DebugInfo {
    pub(crate) unsafe fn from_raw(raw: &ffi::lua_Debug) -> Self {
        Self {
            event: raw.event,
            name: ptr_to_string(raw.name),
            namewhat: ptr_to_string(raw.namewhat),
            what: ptr_to_string(raw.what),
            source: ptr_to_string(raw.source),
            currentline: raw.currentline,
            nups: raw.nups,
            linedefined: raw.linedefined,
            lastlinedefined: raw.lastlinedefined,
            short_src: c_array_to_string(&raw.short_src),
            _i_ci: raw.i_ci,
        }
    }
}

fn ptr_to_string(ptr: *const c_char) -> Option<lua::String> {
    if ptr.is_null() {
        None
    } else {
        Some(lua::String::from(unsafe { CStr::from_ptr(ptr) }.to_bytes()))
    }
}

fn c_array_to_string(arr: &[c_char; 128]) -> lua::String {
    let ptr = arr.as_ptr();
    lua::String::from(unsafe { CStr::from_ptr(ptr) }.to_bytes())
}

impl lua::State {
    pub fn debug_getinfo_at(
        &self,
        level: i32,
        what: impl AsRef<std::ffi::CStr>,
    ) -> Option<DebugInfo> {
        let what = what.as_ref();
        let mut ar = MaybeUninit::zeroed();
        if ffi::lua_getstack(self.0, level, ar.as_mut_ptr()) == 0 {
            return None;
        }
        if ffi::lua_getinfo(self.0, what.as_ptr(), ar.as_mut_ptr()) == 0 {
            return None;
        }
        let lua_debug = unsafe { ar.assume_init() };
        Some(DebugInfo::from(&lua_debug))
    }
}

impl From<&ffi::lua_Debug> for DebugInfo {
    fn from(raw: &ffi::lua_Debug) -> Self {
        unsafe { Self::from_raw(raw) }
    }
}
