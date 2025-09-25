use std::{
    error::Error,
    fmt::Display,
    os::raw::{c_int, c_void},
};

use crate::{
    lua::{self, upvalue_index},
    lua_shared,
};

unsafe extern "C" {
    fn get_call_rust_function() -> CFunction;
    fn get_call_rust_closure() -> CFunction;
    fn set_api_table(napi: *const lua_shared::ApiTable);
}

pub type CFunction = lua_shared::lua_CFunction;
pub type RawCFunction = unsafe extern "C" fn(lua::State) -> i32;
pub type RustFunctionResult = Result<i32, Box<dyn Error>>;
pub type RustFunction = fn(lua::State) -> RustFunctionResult;
pub type BoxedRustFunction = Box<dyn Fn(lua::State) -> RustFunctionResult>;

pub trait FunctionReturn {
    fn handle_result(self, l: lua::State) -> RustFunctionResult;
}

impl FunctionReturn for i32 {
    #[inline]
    fn handle_result(self, _: lua::State) -> RustFunctionResult {
        Ok(self)
    }
}

impl FunctionReturn for () {
    #[inline]
    fn handle_result(self, _: lua::State) -> RustFunctionResult {
        Ok(0)
    }
}

impl<T, E> FunctionReturn for Result<T, E>
where
    T: FunctionReturn,
    E: Display,
{
    #[inline]
    fn handle_result(self, l: lua::State) -> RustFunctionResult {
        match self {
            Ok(val) => val.handle_result(l),
            Err(err) => Err(err.to_string().into()),
        }
    }
}

trait IntoRustFunction<R> {
    fn into_rust_function(self) -> BoxedRustFunction;
}

impl<F, R> IntoRustFunction<R> for F
where
    F: Fn(lua::State) -> R + 'static,
    R: FunctionReturn,
{
    fn into_rust_function(self) -> BoxedRustFunction {
        Box::new(move |l: lua::State| self(l).handle_result(l))
    }
}

impl lua::State {
    #[inline]
    pub fn push_function(self, func: RustFunction) {
        unsafe {
            self.push_light_userdata(func as *mut c_void);
        }
        self.raw_push_cclosure(unsafe { get_call_rust_function() }, 1);
    }

    const CLOSURE_GC_METATABLE_NAME: lua::CStr<'_> = crate::cstr_from_args!(
        "__gmodx_closure_gc_mt",
        env!("CARGO_PKG_VERSION"),
        gmodx_macros::compile_timestamp!()
    );

    #[inline]
    pub fn push_closure<F, R>(self, func: F)
    where
        F: Fn(lua::State) -> R + 'static,
        R: FunctionReturn,
    {
        // Create userdata to hold the boxed function
        let func_box = func.into_rust_function();
        let data_ptr = self.raw_new_userdata(std::mem::size_of::<BoxedRustFunction>())
            as *mut BoxedRustFunction;

        unsafe {
            data_ptr.write(func_box);
        }

        // We need a __gc metamethod to drop the Box properly
        if self.new_metatable(Self::CLOSURE_GC_METATABLE_NAME) {
            self.create_table(0, 1);
            self.raw_push_cclosure(Some(gc_rust_function), 0);
            self.set_field(-2, c"__gc");
        }
        self.set_metatable(-2);

        self.raw_push_cclosure(unsafe { get_call_rust_closure() }, 1);
    }
}

extern "C" fn gc_rust_function(l: *mut lua_shared::lua_State) -> c_int {
    let l = lua::State(l);
    let data_ptr = l.raw_to_userdata(1) as *mut BoxedRustFunction;
    if !data_ptr.is_null() {
        unsafe {
            // Read the Box out and drop it properly
            // This will deallocate the heap memory and run any destructors
            std::ptr::drop_in_place(data_ptr);
        }
    }
    0
}

// So this way of calling from Lua -> C -> Rust is for maximum safety.
// This is because lua_error can longjmp out of the function, skipping any
// Rust stack unwinding, which is undefined behavior.
extern "C-unwind" fn rust_function_callback(
    l: *mut lua_shared::lua_State,
    result: *mut c_int,
) -> bool {
    let l = lua::State(l);
    let func_raw = l.raw_to_userdata(upvalue_index(1));
    if func_raw.is_null() {
        l.push_string("attempt to call a nil value");
        return false;
    }
    let func: RustFunction = unsafe { std::mem::transmute(func_raw) };
    match func(l) {
        Ok(v) => unsafe {
            *result = v;
            true
        },
        Err(err) => {
            l.push_string(&err.to_string());
            false
        }
    }
}

extern "C-unwind" fn rust_closure_callback(
    l: *mut lua_shared::lua_State,
    result: *mut c_int,
) -> bool {
    let l = lua::State(l);
    let data_ptr = l.raw_to_userdata(upvalue_index(1)) as *mut BoxedRustFunction;
    if data_ptr.is_null() {
        l.push_string("attempt to call a nil value");
        return false;
    }
    let func = unsafe { &*data_ptr };
    match func(l) {
        Ok(v) => unsafe {
            *result = v;
            true
        },
        Err(err) => {
            l.push_string(&err.to_string());
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
                rust_function_callback: Some(rust_function_callback),
                rust_closure_callback: Some(rust_closure_callback),
            });
        },
        |_| unsafe {
            set_api_table(std::ptr::null());
        },
    )
}
