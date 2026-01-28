use std::ffi::c_void;

use crate::lua::{
    self, FromLua, FromLuaMulti, Function, ObjectLike, Result, Table, ToLua, ToLuaMulti, Value,
    ffi, value_ref::ValueRef,
};

#[derive(Clone, Debug)]
pub struct AnyUserData(pub(crate) ValueRef);

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
        l: &lua::State,
        index: i32,
        type_name: &str,
    ) -> Result<Self> {
        if ffi::lua_type(l.0, index) == ffi::LUA_TUSERDATA {
            Ok(Self(ValueRef::from_stack(
                l,
                index,
                lua::ValueKind::UserData as i32,
            )))
        } else {
            Err(l.type_error(index, type_name))
        }
    }
}

impl ObjectLike for AnyUserData {
    fn get<V: FromLua>(&self, l: &lua::State, key: impl ToLua) -> Result<V> {
        Table(self.0.clone()).get_protected(l, key)
    }

    fn set(&self, l: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()> {
        Table(self.0.clone()).set_protected(l, key, value)
    }

    #[inline]
    fn call<R: FromLuaMulti>(
        &self,
        l: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> Result<R> {
        let func: Function = self.get(l, name)?;
        func.call(l, args)
    }

    #[inline]
    fn call_method<R: FromLuaMulti>(
        &self,
        l: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> Result<R> {
        self.call(l, name, (self, args))
    }
}

impl ToLua for AnyUserData {
    fn push_to_stack(self, l: &lua::State) {
        self.0.push(l);
    }

    fn to_value(self, _: &lua::State) -> Value {
        Value::from_ref(self.0)
    }
}

impl ToLua for &AnyUserData {
    fn push_to_stack(self, l: &lua::State) {
        #[allow(clippy::needless_borrow)]
        (&self.0).push(l);
    }

    fn to_value(self, _: &lua::State) -> Value {
        Value::from_ref(self.0.clone())
    }
}

impl FromLua for AnyUserData {
    #[inline]
    fn try_from_stack(l: &lua::State, index: i32) -> Result<AnyUserData> {
        Self::from_stack_with_type(l, index, "userdata")
    }
}
