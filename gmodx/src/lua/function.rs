#![allow(static_mut_refs)]

use std::{error::Error, fmt::Display, os::raw::c_int};

use crate::{
    lua::{self, upvalue_index},
    lua_shared,
};

unsafe extern "C" {
    fn get_lua_call_rust() -> CFunction;
    fn set_api_table(napi: *const lua_shared::ApiTable);
}

#[repr(C)]
struct FunctionData {
    func: RustFunction,
    one_shot: bool,
}

pub type CFunction = lua_shared::lua_CFunction;
pub type RawCFunction = unsafe extern "C" fn(lua::State) -> i32;
pub type RustFunctionResult = Result<i32, Box<dyn Error + Send + Sync>>;
pub type RustFunction = fn(lua::State) -> RustFunctionResult;

pub trait FunctionReturn {
    fn handle_result(self, l: lua::State) -> RustFunctionResult;
}

impl FunctionReturn for i32 {
    #[inline(always)]
    fn handle_result(self, _: lua::State) -> RustFunctionResult {
        Ok(self)
    }
}

impl FunctionReturn for () {
    #[inline(always)]
    fn handle_result(self, _: lua::State) -> RustFunctionResult {
        Ok(0)
    }
}

impl<T, E> FunctionReturn for Result<T, E>
where
    T: FunctionReturn,
    E: Display,
{
    #[inline(always)]
    fn handle_result(self, l: lua::State) -> RustFunctionResult {
        match self {
            Ok(val) => val.handle_result(l),
            Err(err) => Err(err.to_string().into()),
        }
    }
}

impl lua::State {
    #[inline]
    pub fn push_function(self, func: RustFunction) {
        self.push_function_x(func, false);
    }

    #[inline]
    pub fn push_function_x(self, func: RustFunction, one_shot: bool) {
        let data_ptr =
            self.raw_new_userdata(std::mem::size_of::<FunctionData>()) as *mut FunctionData;
        unsafe {
            data_ptr.write(FunctionData { func, one_shot });
        }

        // Now only 1 upvalue instead of 2
        self.push_cclosure(unsafe { get_lua_call_rust() }, 1);
    }
}

// So this way of calling from Lua -> C -> Rust is for maximum safety.
// This is because lua_error can longjmp out of the function, skipping any
// Rust stack unwinding, which is undefined behavior.
extern "C-unwind" fn rust_lua_callback(l: *mut lua_shared::lua_State, result: *mut c_int) -> bool {
    let l = lua::State(l);

    let data_ptr = l.raw_to_userdata(upvalue_index(1)) as *mut FunctionData;
    if data_ptr.is_null() {
        l.push_string("attempt to call a nil value");
        return false;
    }

    let data = unsafe { &mut *data_ptr };

    #[inline(always)]
    fn release(l: lua::State, data: &mut FunctionData) {
        if data.one_shot {
            // Remove the function data from the stack
            l.push_bool(false);
            l.replace(upvalue_index(1));
        }
    }

    match (data.func)(l) {
        Ok(v) => unsafe {
            *result = v;
            release(l, data);
            true
        },
        Err(err) => {
            l.push_string(&err.to_string());
            release(l, data);
            false
        }
    }
}

inventory::submit! {
    crate::open_close::new(
        1, // Load after lua_shared
        "lua/function.rs/api_table",
        |_| unsafe {
            set_api_table(&lua_shared::ApiTable {
                error: Some(lua_shared::lua_shared().lua_error),
                rust_lua_callback: Some(rust_lua_callback),
            });
        },
        |_| unsafe {
            set_api_table(std::ptr::null());
        },
    )
}
