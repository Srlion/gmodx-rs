use std::mem::MaybeUninit;

use crate::lua::{self, FromLua as _, Table, Value, ffi};

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

    pub fn debug_getinfo_at(
        &self,
        level: i32,
        what: impl AsRef<std::ffi::CStr>,
    ) -> Option<ffi::lua_Debug> {
        let what = what.as_ref();
        let mut ar = MaybeUninit::zeroed();
        if ffi::lua_getstack(self.0, level, ar.as_mut_ptr()) == 0 {
            return None;
        }
        if ffi::lua_getinfo(self.0, what.as_ptr(), ar.as_mut_ptr()) == 0 {
            return None;
        }
        unsafe { Some(ar.assume_init()) }
    }

    pub fn globals(&self) -> Table {
        ffi::lua_pushvalue(self.0, ffi::LUA_GLOBALSINDEX);
        Table(Value::pop_from_stack(self))
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
