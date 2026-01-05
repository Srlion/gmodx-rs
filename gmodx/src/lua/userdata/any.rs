use std::ffi::c_void;

use crate::lua::{
    self, FromLua, FromLuaMulti, Function, ObjectLike, Result, Table, ToLua, ToLuaMulti, Value, ffi,
};

#[derive(Clone, Debug)]
pub struct AnyUserData(pub(crate) Value);

// We have to force them to pass lua::State here to ensure they are on the main thread
// without having to check it each time nor have panics at runtime

impl AnyUserData {
    #[inline]
    pub(crate) fn ptr(&self) -> *const c_void {
        ffi::lua_touserdata(self.0.ref_state().0, self.0.index())
    }

    #[inline]
    pub fn is<T>(&self, _: &lua::State) -> bool {
        super::is_type::<T>(self.ptr() as usize)
    }

    #[inline]
    pub(crate) fn from_stack_with_type(
        state: &lua::State,
        index: i32,
        type_name: &str,
    ) -> Result<Self> {
        if ffi::lua_type(state.0, index) == ffi::LUA_TUSERDATA {
            Ok(Self(Value::from_stack(state, index)))
        } else {
            Err(state.type_error(index, type_name))
        }
    }
}

impl ObjectLike for AnyUserData {
    fn get<V: FromLua>(&self, state: &lua::State, key: impl ToLua) -> Result<V> {
        Table(self.0.clone()).get_protected(state, key)
    }

    fn set(&self, state: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()> {
        Table(self.0.clone()).set_protected(state, key, value)
    }

    #[inline]
    fn call<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> Result<R> {
        let func: Function = self.get(state, name)?;
        func.call(state, args)
    }

    #[inline]
    fn call_method<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> Result<R> {
        self.call(state, name, (self, args))
    }
}

impl ToLua for AnyUserData {
    fn push_to_stack(self, state: &lua::State) {
        self.0.push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0
    }
}

impl ToLua for &AnyUserData {
    fn push_to_stack(self, state: &lua::State) {
        #[allow(clippy::needless_borrow)]
        (&self.0).push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0.clone()
    }
}

impl FromLua for AnyUserData {
    #[inline]
    fn try_from_stack(state: &lua::State, index: i32) -> Result<AnyUserData> {
        Self::from_stack_with_type(state, index, "userdata")
    }
}
