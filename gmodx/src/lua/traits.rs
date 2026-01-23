use crate::lua::{self, Result, Value, ffi, private::Sealed};

pub trait ToLua: Sized {
    /// Pushes the value onto the Lua stack.
    fn push_to_stack(self, l: &lua::State);

    fn to_value(self, l: &lua::State) -> Value {
        self.push_to_stack(l); // push the value to the stack
        Value::pop_from_stack(l)
    }
}

pub trait FromLua: Sized {
    /// Creates the value from the Lua stack at the given index.
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self>;

    // the lua state is only used to ensure we are on main thread
    fn try_from_value(value: Value, _: &lua::State) -> Result<Self> {
        Self::try_from_stack(&value.ref_state(), value.index())
    }
}

pub trait ToLuaMulti: Sized {
    fn push_to_stack_multi(self, l: &lua::State);
    fn push_to_stack_multi_count(self, l: &lua::State) -> i32 {
        let base = ffi::lua_gettop(l.0);
        self.push_to_stack_multi(l);
        ffi::lua_gettop(l.0) - base
    }
}

impl<T: ToLua> ToLuaMulti for T {
    fn push_to_stack_multi(self, l: &lua::State) {
        self.push_to_stack(l);
    }
}

pub trait FromLuaMulti: Sized {
    fn try_from_stack_multi(l: &lua::State, start_index: i32, count: i32) -> Result<(Self, i32)>;
}

impl<T: FromLua> FromLuaMulti for T {
    fn try_from_stack_multi(l: &lua::State, start_index: i32, _: i32) -> Result<(Self, i32)> {
        T::try_from_stack(l, start_index).map(|v| (v, 1))
    }
}

pub trait ObjectLike: Sealed {
    /// Gets the value associated to `key` from the object, assuming it has `__index` metamethod.
    fn get<V: FromLua>(&self, l: &lua::State, key: impl ToLua) -> Result<V>;

    /// Sets the value associated to `key` in the object, assuming it has `__newindex` metamethod.
    fn set(&self, l: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()>;

    /// Gets the function associated to key `name` from the object and calls it,
    /// passing `args` as function arguments.
    ///
    /// This might invoke the `__index` metamethod.
    fn call<R: FromLuaMulti>(
        &self,
        l: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> lua::Result<R>;

    /// Gets the function associated to key `name` from the object and calls it,
    /// passing the object itself along with `args` as function arguments.
    fn call_method<R: FromLuaMulti>(
        &self,
        l: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> lua::Result<R>;
}
