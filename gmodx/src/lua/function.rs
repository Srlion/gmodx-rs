use std::{ffi::CStr, mem};

use crate::lua::{
    self, FromLuaMulti, ToLuaMulti, Value, ffi,
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
        #[allow(clippy::needless_borrow)]
        (&self.0).push_to_stack(state); // Push the function onto the stack
        let nargs = args.push_to_stack_multi(state);
        match ffi::lua_pcall(state.0, nargs, ffi::LUA_MULTRET, 0) {
            ffi::LUA_OK => {}
            res => return Err(state.pop_error(res)),
        }
        let nresults = ffi::lua_gettop(state.0) - stack_start;
        R::try_from_stack_multi(state, stack_start + 1, nresults).map(|(v, _)| v)
    }

    pub(crate) fn to_callback<F, A, R>(func: F) -> Callback
    where
        F: Fn(&lua::State, A) -> std::result::Result<R, Box<dyn std::error::Error>>
            + MaybeSend
            + 'static,
        A: FromLuaMulti,
        R: ToLuaMulti,
    {
        Box::new(move |state: &lua::State| {
            let nargs = ffi::lua_gettop(state.0);
            let (args, _) = A::try_from_stack_multi(state, 1, nargs)?;
            let ret = func(state, args)?;
            Ok(ret.push_to_stack_multi(state))
        })
    }
}

const CLOSURE_GC_METATABLE_NAME: &CStr = gmodx_macros::unique_id!(cstr);

impl lua::State {
    pub fn create_function<F, A, R>(&self, func: F) -> Function
    where
        F: Fn(&lua::State, A) -> std::result::Result<R, Box<dyn std::error::Error>>
            + MaybeSend
            + 'static,
        A: FromLuaMulti,
        R: ToLuaMulti,
    {
        let callback = Function::to_callback(func);
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
