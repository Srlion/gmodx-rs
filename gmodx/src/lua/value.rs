use std::{
    collections::{VecDeque, vec_deque},
    fmt,
    ops::{Deref, DerefMut},
};

use crate::lua::{
    self, FromLuaMulti, Result, ToLuaMulti, ffi,
    traits::{FromLua, ToLua},
    value_ref::ValueRef,
};

#[derive(Clone, Debug)]
pub struct Value {
    /// The Lua type ID of this value.
    pub(crate) type_id: i32,
    /// The inner value reference.
    pub(crate) inner: ValueRef,
}

#[derive(Debug)]
pub enum ValueKind {
    Nil,
    Bool,
    LightUserData,
    Number,
    String,
    Table,
    Function,
    UserData,
    Thread,
    Unknown,
}

impl ValueKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Nil => "nil",
            Self::Bool => "boolean",
            Self::LightUserData => "lightuserdata",
            Self::Number => "number",
            Self::String => "string",
            Self::Table => "table",
            Self::Function => "function",
            Self::UserData => "userdata",
            Self::Thread => "thread",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Value {
    #[must_use]
    pub fn from_stack(state: &lua::State, index: i32) -> Self {
        ffi::lua_pushvalue(state.0, index);
        Self::pop_from_stack(state)
    }

    #[must_use]
    pub fn pop_from_stack(state: &lua::State) -> Self {
        let type_id = ffi::lua_type(state.0, -1);
        Self {
            type_id,
            inner: ValueRef::pop_from(state),
        }
    }

    #[must_use]
    pub const fn type_id(&self) -> i32 {
        self.type_id
    }

    #[must_use]
    pub const fn type_kind(&self) -> ValueKind {
        match self.type_id {
            ffi::LUA_TNIL | ffi::LUA_TNONE => ValueKind::Nil,
            ffi::LUA_TBOOLEAN => ValueKind::Bool,
            ffi::LUA_TLIGHTUSERDATA => ValueKind::LightUserData,
            ffi::LUA_TNUMBER => ValueKind::Number,
            ffi::LUA_TSTRING => ValueKind::String,
            ffi::LUA_TTABLE => ValueKind::Table,
            ffi::LUA_TFUNCTION => ValueKind::Function,
            ffi::LUA_TUSERDATA => ValueKind::UserData,
            ffi::LUA_TTHREAD => ValueKind::Thread,
            _ => ValueKind::Unknown,
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_kind().as_str()
    }

    pub fn to<T: FromLua>(self, state: &lua::State) -> Result<T> {
        T::try_from_value(self, state)
    }

    pub fn push_to_stack(&self, state: &lua::State) {
        self.inner.push(state);
    }

    pub(crate) const fn index(&self) -> i32 {
        self.inner.index
    }

    pub(crate) fn ref_state(&self) -> lua::State {
        self.inner.ref_state()
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value of type {} ({})", self.type_name(), self.type_id())
    }
}

impl ToLua for Value {
    fn push_to_stack(self, state: &lua::State) {
        Self::push_to_stack(&self, state);
    }

    fn to_value(self, _: &lua::State) -> Self {
        self
    }
}

impl ToLua for &Value {
    fn push_to_stack(self, state: &lua::State) {
        Value::push_to_stack(self, state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.clone()
    }
}

impl FromLua for Value {
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        Ok(Self::from_stack(state, index))
    }

    fn try_from_value(value: Value, _: &lua::State) -> Result<Self> {
        Ok(value)
    }
}

#[derive(Default, Debug, Clone)]
pub struct MultiValue(VecDeque<Value>);

impl Deref for MultiValue {
    type Target = VecDeque<Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MultiValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl MultiValue {
    /// Creates an empty `MultiValue` containing no values.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self(VecDeque::new())
    }

    /// Creates an empty `MultiValue` container with space for at least `capacity` elements.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(VecDeque::with_capacity(capacity))
    }

    /// Creates a `MultiValue` container from vector of values.
    ///
    /// This method works in *O*(1) time and does not allocate any additional memory.
    #[must_use]
    #[inline]
    pub fn from_vec(vec: Vec<Value>) -> Self {
        vec.into()
    }

    /// Consumes the `MultiValue` and returns a vector of values.
    ///
    /// This method needs *O*(*n*) data movement if the circular buffer doesn't happen to be at the
    /// beginning of the allocation.
    #[must_use]
    #[inline]
    pub fn into_vec(self) -> Vec<Value> {
        self.into()
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn from_lua_iter<T: ToLua>(
        state: &lua::State,
        iter: impl IntoIterator<Item = T>,
    ) -> Self {
        let iter = iter.into_iter();
        let mut multi_value = Self::with_capacity(iter.size_hint().0);
        for value in iter {
            multi_value.push_back(value.to_value(state));
        }
        multi_value
    }
}

impl From<Vec<Value>> for MultiValue {
    #[inline]
    fn from(value: Vec<Value>) -> Self {
        Self(value.into())
    }
}

impl From<MultiValue> for Vec<Value> {
    #[inline]
    fn from(value: MultiValue) -> Self {
        value.0.into()
    }
}

impl FromIterator<Value> for MultiValue {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
        let mut multi_value = Self::new();
        multi_value.extend(iter);
        multi_value
    }
}

impl IntoIterator for MultiValue {
    type Item = Value;
    type IntoIter = vec_deque::IntoIter<Value>;

    #[inline]
    fn into_iter(mut self) -> Self::IntoIter {
        let deque = std::mem::take(&mut self.0);
        std::mem::forget(self);
        deque.into_iter()
    }
}

impl ToLuaMulti for MultiValue {
    fn push_to_stack_multi(self, state: &lua::State) {
        for value in self {
            value.push_to_stack(state);
        }
    }
}

impl ToLuaMulti for &MultiValue {
    fn push_to_stack_multi(self, state: &lua::State) {
        for value in self.iter() {
            value.push_to_stack(state);
        }
    }
}

impl FromLuaMulti for MultiValue {
    fn try_from_stack_multi(
        state: &lua::State,
        start_index: i32,
        count: i32,
    ) -> Result<(Self, i32)> {
        let mut multi_value = Self::with_capacity(count as usize);
        for i in 0..count {
            multi_value.push_back(Value::from_stack(state, start_index + i));
        }
        Ok((multi_value, count))
    }
}
