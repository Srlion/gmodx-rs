use std::ops::{Deref, DerefMut};

use crate::lua::{self, FromLua, FromLuaMulti, ToLua, ToLuaMulti};

#[derive(Default, Debug, Clone)]
pub struct MultiValueOf<T>(pub Vec<T>);

impl<T> Deref for MultiValueOf<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for MultiValueOf<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> MultiValueOf<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<T> From<Vec<T>> for MultiValueOf<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}

impl<T> IntoIterator for MultiValueOf<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: ToLua> ToLuaMulti for MultiValueOf<T> {
    fn push_to_stack_multi(self, l: &lua::State) {
        for v in self.0 {
            v.push_to_stack(l);
        }
    }
}

impl<T: FromLua> FromLuaMulti for MultiValueOf<T> {
    fn try_from_stack_multi(l: &lua::State, start: i32, count: i32) -> lua::Result<(Self, i32)> {
        let mut vec = Vec::with_capacity(count as usize);
        for i in 0..count {
            vec.push(T::try_from_stack(l, start + i)?);
        }
        Ok((Self(vec), count))
    }
}
