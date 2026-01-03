#[cfg(feature = "tokio")]
use std::sync::Mutex;
use std::{ffi::CStr, fmt::Display, mem};

use crate::lua::{
    self, FromLuaMulti, StackGuard, ToLuaMulti, Value, ffi,
    traits::{FromLua, ToLua},
    types::{Callback, MaybeSend},
};

#[cfg(feature = "tokio")]
static THREAD_WRAP: Mutex<Option<Function>> = Mutex::new(None);

#[cfg(feature = "tokio")]
fn get_thread_wrap() -> Function {
    THREAD_WRAP
        .lock()
        .unwrap()
        .clone()
        .expect("THREAD_WRAP is not initialized")
}

#[cfg(feature = "tokio")]
inventory::submit! {
    crate::open_close::new(
        0,
        "async_thread_wrap",
        |l| {
            // 😉 ;-)
            let chunk = l.load_buffer(b"
                local co_resume = coroutine.resume
                local xpcall = xpcall
                local debug_traceback = debug.traceback
                return function(done, f, ...)
                    return done(xpcall(f, debug_traceback, ...))
                end
            ", c"async_thread_wrap").expect("failed to load async thread wrap chunk");
            let func = chunk.call::<Function>(l, ()).expect("failed to get async thread wrap function");
            *THREAD_WRAP.lock().unwrap() = Some(func);
        },
        |_| {
            *THREAD_WRAP.lock().unwrap() = None;
        },
    )
}

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

    #[cfg(feature = "tokio")]
    pub async fn call_async<R: FromLuaMulti + Send + 'static>(
        &self,
        args: impl ToLuaMulti,
    ) -> lua::Result<R> {
        use std::sync::{Arc, Mutex};
        use tokio::sync::Notify;

        use crate::lua::State;

        struct Shared<R> {
            result: Mutex<Option<lua::Result<R>>>,
            notify: Notify,
        }

        let shared = Arc::new(Shared {
            result: Mutex::new(None),
            notify: Notify::new(),
        });
        let notified = shared.notify.notified();

        {
            let Some(l) = lua::lock_async().await else {
                return Err(lua::Error::StateUnavailable);
            };

            let thread = l.create_thread(get_thread_wrap());
            let done = l.create_function({
                let shared = shared.clone();
                move |l: &State| {
                    let nargs = ffi::lua_gettop(l.0);
                    let res = match bool::try_from_stack(l, 1) {
                        Ok(true) => R::try_from_stack_multi(l, 2, nargs - 1).map(|(v, _)| v),
                        Ok(false) => {
                            let err_msg = lua::String::try_from_stack(l, 2)
                                .unwrap_or_else(|e| e.to_string().into());
                            Err(lua::Error::Runtime(err_msg))
                        }
                        Err(e) => Err(e),
                    };
                    *shared.result.lock().unwrap() = Some(res);
                    shared.notify.notify_one();
                }
            });

            let func = self.clone();
            thread.resume::<()>(&l, (done, func, args))?;
        }

        if shared.result.lock().unwrap().is_none() {
            notified.await;
        }

        shared.result.lock().unwrap().take().unwrap()
    }
}

const CLOSURE_GC_METATABLE_NAME: &CStr = gmodx_macros::unique_id!(cstr);

impl lua::State {
    pub fn create_function<F, Marker>(&self, func: F) -> Function
    where
        F: IntoLuaFunction<Marker>,
    {
        func.into_function()
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
    fn into_function(self) -> Function;
}

impl IntoLuaFunction<()> for Function {
    fn into_function(self) -> Function {
        self
    }
}

#[cfg(feature = "tokio")]
pub struct AsyncMarker<T>(std::marker::PhantomData<T>);

macro_rules! impl_into_lua_function {
    ($($name:ident),*) => {
        impl<FF, $($name,)* RR, Ret> IntoLuaFunction<($($name,)*)> for FF
        where
            FF: Fn(&lua::State, $($name,)*) -> Ret + MaybeSend + 'static,
            $($name: FromLuaMulti,)*
            Ret: IntoLuaCallbackResult<Value = RR>,
            RR: ToLuaMulti,
        {
            fn into_function(self) -> Function {
                #[allow(unused)]
                #[allow(non_snake_case)]
                let callback = Box::new(move |state: &lua::State| {
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
                });
                let l = lua::lock().unwrap();
                l.create_function_impl(callback)
            }
        }

        #[cfg(feature = "tokio")]
        impl<FF, Fut, $($name,)* RR, Ret> IntoLuaFunction<AsyncMarker<($($name,)*)>> for FF
            where
                FF: Fn(&lua::State, $($name,)*) -> Fut + MaybeSend + 'static,
                Fut: Future<Output = Ret> + Send + 'static,
                $($name: FromLuaMulti,)*
                Ret: IntoLuaCallbackResult<Value = RR>,
                RR: ToLuaMulti + Send + 'static,
            {
                fn into_function(self) -> Function {
                    use crate::lua::Nil;

                    #[allow(unused, non_snake_case)]
                    let callback = Box::new(move |thread_state: &lua::State| {
                        if !thread_state.is_thread() {
                            Err(lua::Error::Runtime("async functions can only be called from within a Lua coroutine".into()))?;
                        }

                        let nargs = ffi::lua_gettop(thread_state.0);
                        let mut index = 1;
                        let mut remaining = nargs;
                        $(
                            let ($name, consumed) = $name::try_from_stack_multi(thread_state, index, remaining)?;
                            index += consumed;
                            remaining -= consumed;
                        )*

                        let fut = self(thread_state, $($name,)*);

                        let thread_state_ptr = thread_state.as_usize();
                        crate::tokio_tasks::spawn(async move {
                            let result = fut.await.into_callback_result();
                            crate::next_tick(move |l| {
                                let thread_state = lua::State::from_usize(thread_state_ptr);
                                match result {
                                    Ok(ret) => { lua::thread::Thread::resume_impl(&thread_state, l, (Nil, ret)).ok(); }
                                    Err(e) => { lua::thread::Thread::resume_impl(&thread_state, l, e.to_string()).ok(); }
                                }
                            });
                        });

                        Ok(ffi::lua_yield(thread_state.0, 0))
                    });
                    let l = lua::lock().unwrap();
                    l.create_function_impl(callback)
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
