use std::ffi::{CStr, OsStr};
use std::path::PathBuf;

use bstr::ByteSlice;

use crate::lua::value_ref::ValueRef;
use crate::lua::{self, FromLua, Function, Table, ToLua, ffi};

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq)]
pub struct State(pub(crate) *mut lua::ffi::lua_State);

impl State {
    pub(crate) const fn clone(&self) -> Self {
        Self(self.0)
    }

    #[allow(unused)]
    #[inline(always)]
    pub(crate) fn as_usize(&self) -> usize {
        self.0 as usize
    }

    pub(crate) const fn from_usize(u: usize) -> Self {
        Self(u as *mut lua::ffi::lua_State)
    }

    #[must_use]
    #[inline]
    pub fn type_of(&self, index: i32) -> i32 {
        ffi::lua_type(self.0, index)
    }

    #[must_use]
    pub fn type_name(&self, idx: i32) -> String {
        let tp = self.type_of(idx);
        let tp_str = {
            let c_str = ffi::lua_typename(self.0, tp);
            if c_str.is_null() {
                eprintln!("[gmodx] Warning: lua_typename returned null for type {tp}");
                return "<null>".into();
            }
            unsafe { std::ffi::CStr::from_ptr(c_str) }
        };
        tp_str.to_string_lossy().into_owned()
    }

    #[must_use]
    pub fn globals(&self) -> Table {
        ffi::lua_pushvalue(self.0, ffi::LUA_GLOBALSINDEX);
        Table(ValueRef::pop_from(self, lua::ValueKind::Table as i32))
    }

    #[inline]
    pub fn set_global(&self, key: impl ToLua, value: impl ToLua) -> lua::Result<()> {
        self.globals().set(self, key, value)
    }

    #[inline]
    pub fn get_global<V: FromLua>(&self, key: impl ToLua) -> lua::Result<V> {
        self.globals().get(self, key)
    }

    #[must_use]
    pub fn caller_source_path(&self) -> Option<PathBuf> {
        let dbg_info = self.debug_getinfo_at(1, c"S")?;
        let source = dbg_info.source?;
        let bytes = source.as_bytes();
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Some(PathBuf::from(OsStr::from_bytes(bytes)))
        }
        #[cfg(not(unix))]
        {
            Some(PathBuf::from(bytes.to_str_lossy().as_ref()))
        }
    }

    pub fn load_buffer(&self, buff: &[u8], name: &CStr) -> lua::Result<Function> {
        let chunk = ffi::luaL_loadbuffer(
            self.0,
            buff.as_ptr().cast::<i8>(),
            buff.len(),
            name.as_ptr(),
        );
        match chunk {
            ffi::LUA_OK => Ok(Function(ValueRef::pop_from(
                self,
                lua::ValueKind::Function as i32,
            ))),
            res => Err(self.pop_error(res)),
        }
    }

    #[cold]
    pub fn dump_stack(&self) {
        let _sg = self.stack_guard(); // to pop any extra values we push
        let top = ffi::lua_gettop(self.0);
        println!("\n=== STACK DUMP ===");
        println!("Stack size: {top}");
        for i in 1..=top {
            let lua_type_name = self.type_name(i);
            match lua_type_name.as_ref() {
                "string" => println!("{}. {}: {:?}", i, lua_type_name, {
                    ffi::lua_pushvalue(self.0, i);
                    let str = lua::String::try_from_stack(self, -1).unwrap_or_default();
                    ffi::lua_pop(self.0, 1);
                    str
                }),
                "boolean" => println!("{}. {}: {:?}", i, lua_type_name, {
                    ffi::lua_pushvalue(self.0, i);
                    let bool = bool::try_from_stack(self, -1).unwrap_or_default();
                    ffi::lua_pop(self.0, 1);
                    bool
                }),
                "number" => println!("{}. {}: {:?}", i, lua_type_name, {
                    ffi::lua_pushvalue(self.0, i);
                    let n = f64::try_from_stack(self, -1).unwrap_or_default();
                    ffi::lua_pop(self.0, 1);
                    n
                }),
                _ => println!("{i}. {lua_type_name}"),
            }
        }
        println!();
    }
}
