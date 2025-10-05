use crate::lua::{self, Result, Value, private::Sealed};

pub trait ToLua: Sized {
    /// Pushes the value onto the Lua stack.
    fn push_to_stack(self, state: &lua::State);

    fn to_value(self, state: &lua::State) -> Value {
        self.push_to_stack(state); // push the value to the stack
        Value::pop_from_stack(state)
    }
}

pub trait FromLua: Sized {
    /// Creates the value from the Lua stack at the given index.
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self>;

    // the lua state is only used to ensure we are on main thread
    fn try_from_value(value: Value, _: &lua::State) -> Result<Self> {
        Self::try_from_stack(&value.thread(), value.index())
    }
}

pub trait ToLuaMulti: Sized {
    fn push_to_stack_multi(self, state: &lua::State) -> i32;
}

impl<T: ToLua> ToLuaMulti for T {
    fn push_to_stack_multi(self, state: &lua::State) -> i32 {
        self.push_to_stack(state);
        1
    }
}

pub trait FromLuaMulti: Sized {
    fn try_from_stack_multi(
        state: &lua::State,
        start_index: i32,
        count: i32,
    ) -> Result<(Self, i32)>;
}

impl<T: FromLua> FromLuaMulti for T {
    fn try_from_stack_multi(state: &lua::State, start_index: i32, _: i32) -> Result<(Self, i32)> {
        T::try_from_stack(state, start_index).map(|v| (v, 1))
    }
}

pub trait ObjectLike: Sealed {
    /// Gets the value associated to `key` from the object, assuming it has `__index` metamethod.
    fn get<V: FromLua>(&self, state: &lua::State, key: impl ToLua) -> Result<V>;

    /// Sets the value associated to `key` in the object, assuming it has `__newindex` metamethod.
    fn set(&self, state: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()>;

    /// Gets the function associated to key `name` from the object and calls it,
    /// passing `args` as function arguments.
    ///
    /// This might invoke the `__index` metamethod.
    fn call<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> lua::Result<R>;

    /// Gets the function associated to key `name` from the object and calls it,
    /// passing the object itself along with `args` as function arguments.
    fn call_method<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> lua::Result<R>;
}
