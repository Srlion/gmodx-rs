use std::{
    error::Error,
    fmt::Display,
    os::raw::{c_int, c_void},
};

use crate::lua::{self, upvalue_index};

pub type CFunction = lua::raw::lua_CFunction;
pub type RawCFunction = unsafe extern "C" fn(lua::State) -> i32;
pub type RustFunctionResult = Result<i32, Box<dyn Error>>;
pub type RustFunction = fn(lua::State) -> RustFunctionResult;
pub type BoxedRustFunction = Box<dyn FnMut(lua::State) -> RustFunctionResult>;

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
    F: FnMut(lua::State) -> R + 'static,
    R: FunctionReturn,
{
    fn into_rust_function(mut self) -> BoxedRustFunction {
        Box::new(move |l: lua::State| self(l).handle_result(l))
    }
}

impl lua::State {
    #[inline]
    pub fn push_function(self, func: RustFunction) {
        unsafe {
            self.push_light_userdata(func as *mut c_void);
        }
        self.push_cclosure(unsafe { lua::bridge::get_call_rust_function() }, 1);
    }

    const CLOSURE_GC_METATABLE_NAME: lua::CStr<'_> = crate::cstr_from_args!(
        "__gmodx_closure_gc_mt",
        env!("CARGO_PKG_VERSION"),
        gmodx_macros::compile_timestamp!()
    );

    #[inline]
    pub fn push_closure<F, R>(self, func: F)
    where
        F: FnMut(lua::State) -> R + 'static,
        R: FunctionReturn,
    {
        // Create userdata to hold the boxed function
        let func_box = func.into_rust_function();
        let data_ptr = self.direct_new_userdata(std::mem::size_of::<BoxedRustFunction>())
            as *mut BoxedRustFunction;

        unsafe {
            data_ptr.write(func_box);
        }

        // We need a __gc metamethod to drop the Box properly
        if self.new_metatable(Self::CLOSURE_GC_METATABLE_NAME) {
            extern "C" fn gc_rust_function(l: *mut lua::raw::lua_State) -> c_int {
                let l = lua::State(l);
                let data_ptr = l.direct_to_userdata(1) as *mut BoxedRustFunction;
                if !data_ptr.is_null() {
                    unsafe {
                        // Read the Box out and drop it properly
                        // This will deallocate the heap memory and run any destructors
                        std::ptr::drop_in_place(data_ptr);
                    }
                }
                0
            }
            self.push_cclosure(Some(gc_rust_function), 0);
            self.raw_set_field(-2, c"__gc");
        }
        self.set_metatable(-2);

        self.push_cclosure(unsafe { lua::bridge::get_call_rust_closure() }, 1);
    }
}

// So this way of calling from Lua -> C -> Rust is for maximum safety.
// This is because lua_error can longjmp out of the function, skipping any
// Rust stack unwinding, which is undefined behavior.
pub(crate) extern "C-unwind" fn rust_function_callback(
    l: *mut lua::raw::lua_State,
    result: *mut c_int,
) -> bool {
    let l = lua::State(l);
    let func_raw = l.direct_to_userdata(upvalue_index(1));
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

pub(crate) extern "C-unwind" fn rust_closure_callback(
    l: *mut lua::raw::lua_State,
    result: *mut c_int,
) -> bool {
    let l = lua::State(l);
    let data_ptr = l.direct_to_userdata(upvalue_index(1)) as *mut BoxedRustFunction;
    if data_ptr.is_null() {
        l.push_string("attempt to call a nil value");
        return false;
    }
    let func = unsafe { &mut *data_ptr };
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
