use std::{ffi::CStr, fmt::Display, mem};

use crate::lua::{
    self, FromLuaMulti, StackGuard, ToLuaMulti, Value, ffi,
    traits::{FromLua, ToLua},
    types::{Callback, MaybeSend},
};

#[derive(Clone, Debug)]
pub struct Function(pub(crate) Value);

impl Function {
    pub fn call<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        args: impl ToLuaMulti,
    ) -> lua::Result<R> {
        let stack_start = ffi::lua_gettop(state.0);
        let _sg = StackGuard::with_top(state.0, stack_start);
        #[allow(clippy::needless_borrow)]
        (&self.0).push_to_stack(state); // Push the function onto the stack
        args.push_to_stack_multi(state);
        let nargs = ffi::lua_gettop(state.0) - stack_start - 1;
        match ffi::lua_pcall(state.0, nargs, ffi::LUA_MULTRET, 0) {
            ffi::LUA_OK => {}
            res => return Err(state.pop_error(res)),
        }
        let nresults = ffi::lua_gettop(state.0) - stack_start;
        R::try_from_stack_multi(state, stack_start + 1, nresults).map(|(v, _)| v)
    }

    /// Same as [`call`], but logs any errors that occur.
    pub fn call_logged<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        args: impl ToLuaMulti,
    ) -> lua::Result<R> {
        let res = self.call(state, args);
        if let Err(err) = &res {
            state.error_no_halt_with_stack(&err.to_string());
        }
        res
    }

    /// Calls the function with the given arguments, ignoring any return values.
    pub fn call_no_rets(&self, state: &lua::State, args: impl ToLuaMulti) -> lua::Result<()> {
        let stack_start = ffi::lua_gettop(state.0);
        let _sg = StackGuard::with_top(state.0, stack_start);
        #[allow(clippy::needless_borrow)]
        (&self.0).push_to_stack(state); // Push the function onto the stack
        args.push_to_stack_multi(state);
        let nargs = ffi::lua_gettop(state.0) - stack_start - 1;
        match ffi::lua_pcall(state.0, nargs, 0, 0) {
            ffi::LUA_OK => Ok(()),
            res => Err(state.pop_error(res)),
        }
    }

    /// Same as [`call_no_rets`], but logs any errors that occur.
    pub fn call_no_rets_logged(
        &self,
        state: &lua::State,
        args: impl ToLuaMulti,
    ) -> lua::Result<()> {
        let res = self.call_no_rets(state, args);
        if let Err(err) = &res {
            state.error_no_halt_with_stack(&err.to_string());
        }
        res
    }
}

const CLOSURE_GC_METATABLE_NAME: &CStr = gmodx_macros::unique_id!(cstr);

impl lua::State {
    pub fn create_function<F, Marker>(&self, func: F) -> Function
    where
        F: IntoLuaFunction<Marker>,
    {
        let callback = func.into_callback();
        self.create_function_impl(callback)
    }

    pub(crate) fn create_function_impl(&self, func: Callback) -> Function {
        let callback_ptr =
            ffi::lua_newuserdata(self.0, mem::size_of::<Callback>()) as *mut Callback;

        debug_assert_eq!(
            (callback_ptr as usize) % mem::align_of::<Callback>(),
            0,
            "Lua userdata has insufficient alignment for Callback"
        );

        unsafe {
            callback_ptr.write(func);
        }

        if ffi::luaL_newmetatable(self.0, CLOSURE_GC_METATABLE_NAME.as_ptr()) {
            extern "C-unwind" fn gc_rust_function(state: *mut lua::ffi::lua_State) -> i32 {
                let l = lua::State(state);
                let data_ptr = ffi::lua_touserdata(l.0, 1) as *mut Callback;
                if !data_ptr.is_null() {
                    unsafe {
                        // Read the Box out and drop it
                        std::ptr::drop_in_place(data_ptr);
                    }
                }
                0
            }
            ffi::lua_pushcclosure(self.0, Some(gc_rust_function), 0);
            ffi::lua_setfield(self.0, -2, c"__gc".as_ptr());
        }
        ffi::lua_setmetatable(self.0, -2);

        ffi::lua_pushcclosure(self.0, Some(rust_closure_callback), 1);

        Function(Value::pop_from_stack(self))
    }
}

extern "C-unwind" fn rust_closure_callback(state: *mut ffi::lua_State) -> i32 {
    {
        let l = lua::State(state);
        let data_ptr = ffi::lua_touserdata(l.0, ffi::lua_upvalueindex(1)) as *const Callback;
        if !data_ptr.is_null() {
            let func = unsafe { &*data_ptr };
            match func(&l) {
                Ok(v) => return v,
                Err(err) => {
                    let err_str = err.to_string();
                    ffi::lua_pushlstring(l.0, err_str.as_ptr() as *const i8, err_str.len());
                    drop(err_str); // make sure to drop before lua_error
                }
            }
        } else {
            ffi::lua_pushstring(l.0, c"attempt to call a nil value".as_ptr());
        }
    }
    ffi::lua_error(state);
}

impl ToLua for Function {
    fn push_to_stack(self, state: &lua::State) {
        self.0.push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0
    }
}

impl ToLua for &Function {
    fn push_to_stack(self, state: &lua::State) {
        #[allow(clippy::needless_borrow)]
        (&self.0).push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0.clone()
    }
}

impl FromLua for Function {
    fn try_from_stack(state: &lua::State, index: i32) -> lua::Result<Self> {
        match ffi::lua_type(state.0, index) {
            ffi::LUA_TFUNCTION => Ok(Function(Value::from_stack(state, index))),
            _ => Err(state.type_error(index, "function")),
        }
    }
}

pub trait IntoLuaCallbackResult {
    type Value: ToLuaMulti;
    fn into_callback_result(self) -> Result<Self::Value, String>;
}

impl<T, E> IntoLuaCallbackResult for Result<T, E>
where
    T: ToLuaMulti,
    E: Display,
{
    type Value = T;
    #[inline(always)]
    fn into_callback_result(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

impl<T> IntoLuaCallbackResult for T
where
    T: ToLuaMulti,
{
    type Value = Self;
    #[inline(always)]
    fn into_callback_result(self) -> Result<Self, String> {
        Ok(self)
    }
}

pub trait IntoLuaFunction<Marker> {
    fn into_callback(self) -> Callback;
}

macro_rules! impl_into_lua_function {
    ($($name:ident),*) => {
        impl<FF, $($name,)* RR, Ret> IntoLuaFunction<($($name,)*)> for FF
        where
            FF: Fn(&lua::State, $($name,)*) -> Ret + MaybeSend + 'static,
            $($name: FromLuaMulti,)*
            Ret: IntoLuaCallbackResult<Value = RR>,
            RR: ToLuaMulti,
        {
            fn into_callback(self) -> Callback {
                #[allow(unused)]
                #[allow(non_snake_case)]
                Box::new(move |state: &lua::State| {
                    let nargs = ffi::lua_gettop(state.0);
                    let mut index = 1;
                    let mut remaining = nargs;
                    $(
                        let ($name, consumed) = $name::try_from_stack_multi(state, index, remaining)?;
                        index += consumed;
                        remaining -= consumed;
                    )*
                    let ret = self(state, $($name,)*).into_callback_result()?;
                    Ok(ret.push_to_stack_multi_count(state))
                })
            }
        }
    };
}

impl_into_lua_function!();
impl_into_lua_function!(A);
impl_into_lua_function!(A, B);
impl_into_lua_function!(A, B, C);
impl_into_lua_function!(A, B, C, D);
impl_into_lua_function!(A, B, C, D, E);
impl_into_lua_function!(A, B, C, D, E, F);
impl_into_lua_function!(A, B, C, D, E, F, G);
impl_into_lua_function!(A, B, C, D, E, F, G, H);
impl_into_lua_function!(A, B, C, D, E, F, G, H, I);
impl_into_lua_function!(A, B, C, D, E, F, G, H, I, J);
impl_into_lua_function!(A, B, C, D, E, F, G, H, I, J, K);
impl_into_lua_function!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_into_lua_function!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_into_lua_function!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_into_lua_function!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_into_lua_function!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
