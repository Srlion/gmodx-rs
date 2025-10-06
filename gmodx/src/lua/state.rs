use std::ffi::OsStr;
use std::path::PathBuf;

use bstr::ByteSlice;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use crate::lua::{self, FromLua, Table, ToLua, Value, ffi};

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq)]
pub struct State(pub(crate) *mut lua::ffi::lua_State);

impl State {
    pub(crate) fn clone(&self) -> Self {
        Self(self.0)
    }

    #[inline]
    pub fn type_of(&self, index: i32) -> i32 {
        ffi::lua_type(self.0, index)
    }

    pub fn type_name(&self, idx: i32) -> String {
        let tp = self.type_of(idx);
        let tp_str = {
            let c_str = ffi::lua_typename(self.0, tp);
            if c_str.is_null() {
                eprintln!(
                    "[gmodx] Warning: lua_typename returned null for type {}",
                    tp
                );
                return "<null>".into();
            }
            unsafe { std::ffi::CStr::from_ptr(c_str) }
        };
        tp_str.to_string_lossy().into_owned()
    }

    pub fn globals(&self) -> Table {
        ffi::lua_pushvalue(self.0, ffi::LUA_GLOBALSINDEX);
        Table(Value::pop_from_stack(self))
    }

    #[inline]
    pub fn set_global<K: ToLua>(&self, key: impl ToLua, value: impl ToLua) -> lua::Result<()> {
        self.globals().set(self, key, value)
    }

    #[inline]
    pub fn get_global<V: FromLua>(&self, key: impl ToLua) -> lua::Result<V> {
        self.globals().get(self, key)
    }

    pub fn caller_source_path(self) -> Option<PathBuf> {
        let dbg_info = self.debug_getinfo_at(1, c"S")?;
        let source = dbg_info.source?;
        let bytes = source.as_bytes();
        if cfg!(unix) {
            Some(PathBuf::from(OsStr::from_bytes(bytes)))
        } else {
            Some(PathBuf::from(String::from_utf8_lossy(bytes).as_ref()))
        }
    }

    #[cold]
    pub fn dump_stack(&self) {
        let _sg = self.stack_guard(); // to pop any extra values we push
        let top = ffi::lua_gettop(self.0);
        println!("\n=== STACK DUMP ===");
        println!("Stack size: {}", top);
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
                _ => println!("{}. {}", i, lua_type_name),
            }
        }
        println!();
    }
}
