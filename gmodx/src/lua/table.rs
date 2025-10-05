use crate::lua::{
    self, FromLuaMulti, Function, Result, ToLuaMulti, Value, ffi,
    traits::{FromLua, ObjectLike, ToLua},
};

#[derive(Clone, Debug)]
pub struct Table(pub(crate) Value);

impl Table {
    pub fn set(&self, state: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()> {
        if !self.has_metatable(state) {
            // If the table has no metatable, we can use rawset directly
            // this is because rawset cannot fail
            self.raw_set(state, key, value);
            return Ok(());
        }

        // Otherwise, we use the protected version because lua can longjmp if __newindex errors
        self.set_protected(state, key, value)
    }

    pub fn get<V: FromLua>(&self, state: &lua::State, key: impl ToLua) -> Result<V> {
        if !self.has_metatable(state) {
            return self.raw_get(state, key);
        }

        self.get_protected(state, key)
    }

    pub fn raw_set(&self, state: &lua::State, key: impl ToLua, value: impl ToLua) {
        let _sg = state.stack_guard();

        self.push_to_stack(state); // push the table
        key.push_to_stack(state); // push the key
        value.push_to_stack(state); // push the value
        ffi::lua_rawset(state.0, -3);
    }

    pub fn raw_get<V: FromLua>(&self, state: &lua::State, key: impl ToLua) -> Result<V> {
        let _sg = state.stack_guard();

        self.push_to_stack(state); // push the table
        key.push_to_stack(state); // push the key
        ffi::lua_rawget(state.0, -2);

        V::try_from_stack(state, -1)
    }

    // the lua state is only used to ensure we are on main thread
    pub fn has_metatable(&self, _: &lua::State) -> bool {
        let thread = self.0.thread();
        if ffi::lua_getmetatable(thread.0, self.0.index()) == 0 {
            false
        } else {
            ffi::lua_pop(thread.0, 1); // pop the metatable
            true
        }
    }

    pub(crate) fn set_protected(
        &self,
        state: &lua::State,
        key: impl ToLua,
        value: impl ToLua,
    ) -> Result<()> {
        let _sg = state.stack_guard();

        unsafe extern "C-unwind" fn safe_settable(state: *mut ffi::lua_State) -> i32 {
            // stack: table, key, value
            ffi::lua_settable(state, -3);
            0
        }

        ffi::lua_pushcfunction(state.0, Some(safe_settable));
        self.push_to_stack(state); // push the table
        key.push_to_stack(state); // push the key
        value.push_to_stack(state); // push the value
        state.protect_lua_call(3, 0)?;

        Ok(())
    }

    pub(crate) fn get_protected<V: FromLua>(
        &self,
        state: &lua::State,
        key: impl ToLua,
    ) -> Result<V> {
        let _sg = state.stack_guard();

        unsafe extern "C-unwind" fn safe_gettable(state: *mut ffi::lua_State) -> i32 {
            // stack: table, key
            ffi::lua_gettable(state, -2);
            1
        }

        ffi::lua_pushcfunction(state.0, Some(safe_gettable));
        self.push_to_stack(state); // push the table
        key.push_to_stack(state); // push the key
        state.protect_lua_call(2, 1)?;

        V::try_from_stack(state, -1)
    }
}

impl lua::State {
    pub fn create_table(&self) -> Table {
        self.create_table_with_capacity(0, 0)
    }

    pub fn create_table_with_capacity(&self, narr: i32, nrec: i32) -> Table {
        lua::ffi::lua_createtable(self.0, narr, nrec);
        Table(Value::pop_from_stack(self))
    }
}

impl ToLua for Table {
    fn push_to_stack(self, state: &lua::State) {
        self.0.push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0
    }
}

impl ToLua for &Table {
    fn push_to_stack(self, state: &lua::State) {
        #[allow(clippy::needless_borrow)]
        (&self.0).push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0.clone()
    }
}

impl FromLua for Table {
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        match lua::ffi::lua_type(state.0, index) {
            lua::ffi::LUA_TTABLE => Ok(Table(Value::from_stack(state, index))),
            _ => Err(state.type_error(index, "table")),
        }
    }
}

impl ObjectLike for Table {
    #[inline]
    fn get<V: FromLua>(&self, state: &lua::State, key: impl ToLua) -> Result<V> {
        self.get(state, key)
    }

    #[inline]
    fn set(&self, state: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()> {
        self.set(state, key, value)
    }

    #[inline]
    fn call<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> lua::Result<R> {
        let func: Function = self.get(state, name)?;
        func.call(state, args)
    }

    #[inline]
    fn call_method<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> lua::Result<R> {
        self.call(state, name, (self, args))
    }
}
