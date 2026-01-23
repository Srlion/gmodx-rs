use crate::lua::{
    self, FromLuaMulti, Function, Nil, Result, ToLuaMulti, Value, ffi,
    traits::{FromLua, ObjectLike, ToLua},
};

// () pushes nothing, so we need to ensure at least one value is pushed to not segfault
fn push_atleast_one<T: ToLuaMulti>(state: &lua::State, value: T) {
    let count = value.push_to_stack_multi_count(state);
    if count == 0 {
        Nil.push_to_stack(state);
    }
}

#[derive(Clone, Debug)]
pub struct Table(pub(crate) Value);

impl Table {
    pub fn set(&self, l: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()> {
        if !self.has_metatable(l) {
            // If the table has no metatable, we can use rawset directly
            // this is because rawset cannot fail
            self.raw_set(l, key, value);
            return Ok(());
        }

        // Otherwise, we use the protected version because lua can longjmp if __newindex errors
        self.set_protected(l, key, value)
    }

    pub fn get<V: FromLua>(&self, l: &lua::State, key: impl ToLua) -> Result<V> {
        if !self.has_metatable(l) {
            return self.raw_get(l, key);
        }

        self.get_protected(l, key)
    }

    // TODO: should make it call __len, lua 5.1 does not invoke __len, has to be implemented manually
    pub fn len(&self, l: &lua::State) -> Result<usize> {
        Ok(self.raw_len(l))
    }

    pub fn raw_set(&self, l: &lua::State, key: impl ToLua, value: impl ToLua) {
        let _sg = l.stack_guard();

        self.push_to_stack(l); // push the table
        push_atleast_one(l, key); // push the key
        push_atleast_one(l, value); // push the value
        ffi::lua_rawset(l.0, -3);
    }

    pub fn raw_get<V: FromLua>(&self, l: &lua::State, key: impl ToLua) -> Result<V> {
        let _sg = l.stack_guard();

        self.push_to_stack(l); // push the table
        push_atleast_one(l, key); // push the key
        ffi::lua_rawget(l.0, -2);

        V::try_from_stack(l, -1)
    }

    // the lua state is only used to ensure we are on main thread
    #[must_use]
    pub fn raw_len(&self, _: &lua::State) -> usize {
        ffi::lua_rawlen(self.0.ref_state().0, self.0.index())
    }

    // the lua state is only used to ensure we are on main thread
    #[must_use]
    pub fn has_metatable(&self, _: &lua::State) -> bool {
        let thread = self.0.ref_state();
        if ffi::lua_getmetatable(thread.0, self.0.index()) == 0 {
            false
        } else {
            ffi::lua_pop(thread.0, 1); // pop the metatable
            true
        }
    }

    #[must_use]
    #[inline]
    pub fn ipairs<V: FromLua>(&self, l: &lua::State) -> IPairsIter<V> {
        IPairsIter {
            table: self.clone(),
            state: l.clone(),
            index: 0,
            len: self.raw_len(l),
            _phantom: std::marker::PhantomData,
        }
    }

    #[must_use]
    #[inline]
    pub fn pairs<K: FromLua, V: FromLua>(&self, l: &lua::State) -> PairsIter<K, V> {
        PairsIter {
            table: self.clone(),
            state: l.clone(),
            key: Nil.to_value(l),
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn set_metatable(&self, _: &lua::State, metatable: Option<&Self>) {
        let ref_thread = self.0.ref_state().0;
        if let Some(metatable) = &metatable {
            ffi::lua_pushvalue(ref_thread, metatable.0.index());
        } else {
            ffi::lua_pushnil(ref_thread);
        }
        ffi::lua_setmetatable(ref_thread, self.0.index());
    }

    pub(crate) fn set_protected(
        &self,
        l: &lua::State,
        key: impl ToLua,
        value: impl ToLua,
    ) -> Result<()> {
        unsafe extern "C-unwind" fn safe_settable(l: *mut ffi::lua_State) -> i32 {
            // stack: table, key, value
            ffi::lua_settable(l, -3);
            0
        }

        let _sg = l.stack_guard();

        ffi::lua_pushcfunction(l.0, Some(safe_settable));
        self.push_to_stack(l); // push the table
        push_atleast_one(l, key); // push the key
        push_atleast_one(l, value); // push the value
        l.protect_lua_call(3, 0)?;

        Ok(())
    }

    pub(crate) fn get_protected<V: FromLua>(
        &self,
        state: &lua::State,
        key: impl ToLua,
    ) -> Result<V> {
        unsafe extern "C-unwind" fn safe_gettable(l: *mut ffi::lua_State) -> i32 {
            // stack: table, key
            ffi::lua_gettable(l, -2);
            1
        }

        let _sg = state.stack_guard();

        ffi::lua_pushcfunction(state.0, Some(safe_gettable));
        self.push_to_stack(state); // push the table
        push_atleast_one(state, key); // push the key
        state.protect_lua_call(2, 1)?;

        V::try_from_stack(state, -1)
    }
}

impl lua::State {
    #[must_use]
    pub fn create_table(&self) -> Table {
        self.create_table_with_capacity(0, 0)
    }

    #[must_use]
    pub fn create_table_with_capacity(&self, narr: i32, nrec: i32) -> Table {
        lua::ffi::lua_createtable(self.0, narr, nrec);
        Table(Value::pop_from_stack(self))
    }
}

impl ToLua for Table {
    fn push_to_stack(self, l: &lua::State) {
        self.0.push_to_stack(l);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0
    }
}

impl ToLua for &Table {
    fn push_to_stack(self, l: &lua::State) {
        #[allow(clippy::needless_borrow)]
        (&self.0).push_to_stack(l);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0.clone()
    }
}

impl FromLua for Table {
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        match lua::ffi::lua_type(l.0, index) {
            lua::ffi::LUA_TTABLE => Ok(Self(Value::from_stack(l, index))),
            _ => Err(l.type_error(index, "table")),
        }
    }
}

impl ObjectLike for Table {
    #[inline]
    fn get<V: FromLua>(&self, l: &lua::State, key: impl ToLua) -> Result<V> {
        self.get(l, key)
    }

    #[inline]
    fn set(&self, l: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()> {
        self.set(l, key, value)
    }

    #[inline]
    fn call<R: FromLuaMulti>(
        &self,
        l: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> lua::Result<R> {
        let func: Function = self.get(l, name)?;
        func.call(l, args)
    }

    #[inline]
    fn call_method<R: FromLuaMulti>(
        &self,
        l: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> lua::Result<R> {
        self.call(l, name, (self, args))
    }
}

pub struct IPairsIter<V> {
    table: Table,
    state: lua::State,
    index: usize,
    len: usize,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: FromLua> Iterator for IPairsIter<V> {
    type Item = (usize, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        self.index += 1;

        let _sg = self.state.stack_guard();

        (&self.table).push_to_stack(&self.state);
        ffi::lua_rawgeti(self.state.0, -1, self.index as i32);

        V::try_from_stack(&self.state, -1)
            .ok()
            .map(|value| (self.index, value))
    }
}

pub struct PairsIter<K, V> {
    table: Table,
    state: lua::State,
    key: Value, // current key (starts as Nil)
    done: bool,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K: FromLua, V: FromLua> Iterator for PairsIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let _sg = self.state.stack_guard();
        (&self.table).push_to_stack(&self.state);
        (&self.key).push_to_stack(&self.state);

        if ffi::lua_next(self.state.0, -2) == 0 {
            self.done = true;
            return None;
        }

        // save key for next iteration
        self.key = Value::from_stack(&self.state, -2);

        // stack: table, key, value
        let v = V::try_from_stack(&self.state, -1).ok()?;
        let k = K::try_from_stack(&self.state, -2).ok()?;

        Some((k, v))
    }
}

/// Macro to create Lua tables.
///
/// Examples:
/// ```rust
/// // Empty table
/// let t1 = table!(state);
/// // Array-style
/// let t2 = table!(state, [1, 2, 3]);
/// // Map-style
/// let t3 = table!(state, { "key" => "value", "foo" => "bar" });
/// ```
#[macro_export]
macro_rules! table {
    // Empty table
    ($state:expr) => {
        $state.create_table()
    };
    // Array-style: table!(state, [1, 2, 3])
    ($state:expr, [$($val:expr),* $(,)?]) => {{
        let t = $state.create_table();
        let mut _i = 1;
        $(
            t.raw_set($state, _i, $val);
            _i += 1;
        )*
        t
    }};
    // Map-style: table!(state, { "key" => value, "foo" => bar })
    ($state:expr, { $($key:expr => $val:expr),* $(,)? }) => {{
        let t = $state.create_table();
        $(
            t.raw_set($state, $key, $val);
        )*
        t
    }};
}

pub use table;
