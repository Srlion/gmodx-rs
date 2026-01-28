use std::collections::HashMap;
use std::ffi::CString;

use bstr::ByteSlice as _;
use bstr::{BStr, BString};

use crate::lua::traits::{FromLua, ToLua};
use crate::lua::value::ValueInner;
use crate::lua::{self, FromLuaMulti, LightUserData, Nil, Result, Table, ToLuaMulti, ffi};

impl<T: ToLua> ToLua for Option<T> {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        match self {
            Some(v) => v.push_to_stack(l),
            None => ffi::lua_pushnil(l.0),
        }
    }
}

impl<T: FromLua> FromLua for Option<T> {
    #[inline]
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        match ffi::lua_type(l.0, index) {
            ffi::LUA_TNIL | ffi::LUA_TNONE => Ok(None),
            _ => T::try_from_stack(l, index).map(Some),
        }
    }
}

impl ToLua for () {
    #[inline]
    fn push_to_stack(self, _: &lua::State) {
        // do nothing
    }
}

impl FromLua for () {
    #[inline]
    fn try_from_stack(_: &lua::State, _index: i32) -> Result<Self> {
        Ok(())
    }
}

impl ToLua for Nil {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        ffi::lua_pushnil(l.0);
    }
}

impl FromLua for Nil {
    #[inline]
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        match ffi::lua_type(l.0, index) {
            ffi::LUA_TNIL | ffi::LUA_TNONE => Ok(Self),
            _ => Err(l.type_error(index, "nil")),
        }
    }

    #[inline]
    fn try_from_value(value: lua::Value, l: &lua::State) -> Result<Self> {
        match value.0 {
            ValueInner::Nil => Ok(Self),
            ValueInner::Ref(r) => Self::try_from_stack(&r.ref_state(), r.index()),
            _ => Err(l.type_error(value.index().unwrap_or(-1), "nil")),
        }
    }
}

impl ToLua for bool {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        ffi::lua_pushboolean(l.0, i32::from(self));
    }
}

impl FromLua for bool {
    #[inline]
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        match ffi::lua_type(l.0, index) {
            ffi::LUA_TBOOLEAN => Ok(ffi::lua_toboolean(l.0, index)),
            ffi::LUA_TNIL => Ok(false), // it's fine if we treat nil as false
            _ => Err(l.type_error(index, "boolean")),
        }
    }
}

impl ToLua for LightUserData {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        ffi::lua_pushlightuserdata(l.0, self.0);
    }
}

impl FromLua for LightUserData {
    #[inline]
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        match ffi::lua_type(l.0, index) {
            ffi::LUA_TLIGHTUSERDATA => Ok(Self(ffi::lua_touserdata(l.0, index))),
            _ => Err(l.type_error(index, "lightuserdata")),
        }
    }
}

impl ToLua for &str {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        ffi::lua_pushlstring(l.0, self.as_ptr().cast::<i8>(), self.len());
    }
}

impl ToLua for String {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        self.as_str().push_to_stack(l);
    }
}

impl ToLua for &String {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        self.as_str().push_to_stack(l);
    }
}

impl ToLua for &BStr {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        ffi::lua_pushlstring(l.0, self.as_ptr().cast::<i8>(), self.len());
    }
}

impl ToLua for BString {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        self.as_bstr().push_to_stack(l);
    }
}

impl ToLua for &BString {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        self.as_bstr().push_to_stack(l);
    }
}

impl FromLua for BString {
    #[inline]
    fn try_from_stack(l: &lua::State, mut index: i32) -> Result<Self> {
        let _sg = l.stack_guard(); // to pop any extra values we push
        match ffi::lua_type(l.0, index) {
            t @ (ffi::LUA_TSTRING | ffi::LUA_TNUMBER) => {
                if t == ffi::LUA_TNUMBER {
                    ffi::lua_pushvalue(l.0, index); // to avoid confusing lua_next
                    index = -1;
                }

                let mut len = 0;
                let ptr = ffi::lua_tolstring(l.0, index, &mut len);
                if ptr.is_null() {
                    return Ok(Self::default()); // what happened wtf?
                }

                let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
                Ok(Self::from(bytes))
            }
            _ => Err(l.type_error(index, "string")),
        }
    }
}

impl ToLua for &std::ffi::CStr {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        let bytes = self.to_bytes();
        ffi::lua_pushlstring(l.0, bytes.as_ptr().cast::<i8>(), bytes.len());
    }
}

impl ToLua for CString {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        self.as_ref().push_to_stack(l);
    }
}

impl ToLua for &CString {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        self.as_ref().push_to_stack(l);
    }
}

impl<T> ToLua for &Vec<T>
where
    for<'a> &'a T: ToLua,
{
    fn push_to_stack(self, l: &lua::State) {
        let table = l.create_table_with_capacity(self.len() as i32, 0);
        for (i, item) in self.iter().enumerate() {
            table.raw_set(l, i + 1, item); // Lua arrays are 1-indexed
        }
        table.push_to_stack(l);
    }
}

impl<T: ToLua> ToLua for Vec<T> {
    fn push_to_stack(self, l: &lua::State) {
        let table = l.create_table_with_capacity(self.len() as i32, 0);
        for (i, item) in self.into_iter().enumerate() {
            table.raw_set(l, i + 1, item); // Lua arrays are 1-indexed
        }
        table.push_to_stack(l);
    }
}

impl<T: ToLua + Clone> ToLua for &[T] {
    fn push_to_stack(self, l: &lua::State) {
        let table = l.create_table_with_capacity(self.len() as i32, 0);
        for (i, item) in self.iter().enumerate() {
            table.raw_set(l, i + 1, item.clone());
        }
        table.push_to_stack(l);
    }
}

impl<T: FromLua> FromLua for Vec<T> {
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        let table = Table::try_from_stack(l, index)?;
        let len = table.raw_len(l);
        let mut vec = Self::with_capacity(len);
        for i in 1..=len {
            let value = table.raw_get(l, i)?;
            vec.push(value);
        }
        Ok(vec)
    }
}

impl<K: ToLua, V: ToLua> ToLua for HashMap<K, V> {
    fn push_to_stack(self, l: &lua::State) {
        let table = l.create_table_with_capacity(0, self.len() as i32);
        for (k, v) in self {
            table.raw_set(l, k, v);
        }
        table.push_to_stack(l);
    }
}

impl<K, V> ToLua for &HashMap<K, V>
where
    for<'a> &'a K: ToLua,
    for<'a> &'a V: ToLua,
{
    fn push_to_stack(self, l: &lua::State) {
        let table = l.create_table_with_capacity(0, self.len() as i32);
        for (k, v) in self {
            table.raw_set(l, k, v);
        }
        table.push_to_stack(l);
    }
}

impl<K: FromLua + Eq + std::hash::Hash, V: FromLua> FromLua for HashMap<K, V> {
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        if ffi::lua_type(l.0, index) != ffi::LUA_TTABLE {
            return Err(l.type_error(index, "table"));
        }
        let _sg = l.stack_guard(); // to pop any extra values we push
        let mut map = Self::new();
        let abs_idx = ffi::lua_absindex(l.0, index);
        // push nil onto the stack to indicate that we want to start iterating
        ffi::lua_pushnil(l.0);
        while ffi::lua_next(l.0, abs_idx) != 0 {
            let v = V::try_from_stack(l, -1)?;
            let k = K::try_from_stack(l, -2)?;
            // pop the value, keep the key for the next iteration
            ffi::lua_pop(l.0, 1);
            map.insert(k, v);
        }
        Ok(map)
    }
}

#[inline]
fn from_lua_f64(l: &lua::State, index: i32) -> Result<f64> {
    match ffi::lua_type(l.0, index) {
        ffi::LUA_TNUMBER | ffi::LUA_TSTRING => Ok(ffi::lua_tonumber(l.0, index)),
        _ => Err(l.type_error(index, "number")),
    }
}

macro_rules! impl_num_from_lua {
    ($($t:ty),*) => {$(
        impl FromLua for $t {
            #[inline]
            fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
                #[allow(clippy::cast_possible_truncation)]
                Ok(from_lua_f64(l, index)? as $t)
            }
        }
    )*};
}

macro_rules! impl_big_from_lua {
    (signed: $($t:ty),*) => {$(
        impl FromLua for $t {
            #[inline]
            fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
                match ffi::lua_type(l.0, index) {
                    #[allow(clippy::cast_possible_truncation)]
                    ffi::LUA_TNUMBER => Ok(ffi::lua_tonumber(l.0, index) as $t),
                    ffi::LUA_TSTRING => BString::try_from_stack(l, index)?
                        .to_str()
                        .map_err(|_| l.type_error(index, "number"))?
                        .parse()
                        .map_err(|_| l.type_error(index, "number")),
                    _ => Err(l.type_error(index, "number")),
                }
            }
        }
    )*};
    (unsigned: $($t:ty),*) => {$(
        impl FromLua for $t {
            #[inline]
            fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
                match ffi::lua_type(l.0, index) {
                    #[allow(clippy::cast_possible_truncation)]
                    ffi::LUA_TNUMBER => Ok(ffi::lua_tonumber(l.0, index) as $t),
                    ffi::LUA_TSTRING => {
                        let s = BString::try_from_stack(l, index)?;
                        let s = s.to_str().map_err(|_| l.type_error(index, "number"))?;
                        if s.trim_start().starts_with('-') {
                            Ok(0)
                        } else {
                            s.parse().map_err(|_| l.type_error(index, "number"))
                        }
                    }
                    _ => Err(l.type_error(index, "number")),
                }
            }
        }
    )*};
}

macro_rules! impl_num_to_lua {
    ($($t:ty),*) => {$(
        impl ToLua for $t {
            #[inline]
            fn push_to_stack(self, l: &lua::State) {
                #[allow(clippy::cast_possible_truncation)]
                ffi::lua_pushnumber(l.0, self as f64);
            }
        }
    )*};
}

macro_rules! impl_big_to_lua {
    (signed: $($t:ty),*) => {$(
        impl ToLua for $t {
            #[inline]
            fn push_to_stack(self, l: &lua::State) {
                if (-9007199254740991..=9007199254740991).contains(&self) {
                    #[allow(clippy::cast_possible_truncation)]
                    f64::push_to_stack(self as f64, l) // fits in f64
                } else {
                    self.to_string().push_to_stack(l) // too big, use string
                }
            }
        }
    )*};
    (unsigned: $($t:ty),*) => {$(
        impl ToLua for $t {
            #[inline]
            fn push_to_stack(self, l: &lua::State) {
                if self <= 9007199254740991 {
                    #[allow(clippy::cast_possible_truncation)]
                    f64::push_to_stack(self as f64, l) // fits in f64
                } else {
                    self.to_string().push_to_stack(l) // too big, use string
                }
            }
        }
    )*};
}

impl_num_from_lua!(f32, f64, i8, u8, i16, u16, i32, u32);
impl_num_to_lua!(f32, f64, i8, u8, i16, u16, i32, u32);
#[cfg(target_pointer_width = "32")]
impl_num_from_lua!(isize, usize);
#[cfg(target_pointer_width = "32")]
impl_num_to_lua!(isize, usize);

impl_big_from_lua!(signed: i64, i128);
impl_big_from_lua!(unsigned: u64, u128);
impl_big_to_lua!(signed: i64, i128);
impl_big_to_lua!(unsigned: u64, u128);
#[cfg(target_pointer_width = "64")]
impl_big_from_lua!(signed: isize);
#[cfg(target_pointer_width = "64")]
impl_big_from_lua!(unsigned: usize);
#[cfg(target_pointer_width = "64")]
impl_big_to_lua!(signed: isize);
#[cfg(target_pointer_width = "64")]
impl_big_to_lua!(unsigned: usize);

#[cfg(feature = "rust_decimal")]
impl FromLua for rust_decimal::Decimal {
    #[inline]
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        match ffi::lua_type(l.0, index) {
            ffi::LUA_TNUMBER => rust_decimal::Decimal::try_from(ffi::lua_tonumber(l.0, index))
                .map_err(|_| l.type_error(index, "decimal")),
            ffi::LUA_TSTRING => BString::try_from_stack(l, index)?
                .to_str()
                .map_err(|_| l.type_error(index, "decimal"))?
                .parse()
                .map_err(|_| l.type_error(index, "decimal")),
            _ => Err(l.type_error(index, "decimal")),
        }
    }
}

#[cfg(feature = "rust_decimal")]
impl ToLua for &rust_decimal::Decimal {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        use rust_decimal::prelude::ToPrimitive;
        if let Some(f) = self.to_f64() {
            if rust_decimal::Decimal::try_from(f).is_ok_and(|d| d == *self) {
                return f.push_to_stack(l);
            }
        }
        self.to_string().push_to_stack(l)
    }
}

#[cfg(feature = "rust_decimal")]
impl ToLua for rust_decimal::Decimal {
    #[inline]
    fn push_to_stack(self, l: &lua::State) {
        (&self).push_to_stack(l)
    }
}

macro_rules! impl_tuple_lua_multi {
    ($($name:ident),+) => {
        impl<$($name),+> ToLuaMulti for ($($name,)+)
        where
            $($name: ToLuaMulti,)+
        {
            #[inline]
            fn push_to_stack_multi(self, l: &lua::State) {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                $(
                    $name.push_to_stack_multi(l);
                )+
            }
        }

        impl<$($name),+> FromLuaMulti for ($($name,)+)
        where
            $($name: FromLuaMulti,)+
        {
            #[inline]
            fn try_from_stack_multi(l: &lua::State, start: i32, count: i32) -> Result<(Self, i32)> {
                let mut index = 0;
                let mut remaining = count;
                $(
                    #[allow(unused_assignments)]
                    #[allow(non_snake_case)]
                    let $name = {
                        let (result, consumed) = $name::try_from_stack_multi(l, start + index, remaining)?;
                        index += consumed;
                        remaining -= consumed;
                        result
                    };
                )+
                Ok((($($name,)+), index))
            }
        }
    };
}

impl_tuple_lua_multi!(A);
impl_tuple_lua_multi!(A, B);
impl_tuple_lua_multi!(A, B, C);
impl_tuple_lua_multi!(A, B, C, D);
impl_tuple_lua_multi!(A, B, C, D, E);
impl_tuple_lua_multi!(A, B, C, D, E, F);
impl_tuple_lua_multi!(A, B, C, D, E, F, G);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H, I);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H, I, J);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H, I, J, K);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_tuple_lua_multi!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
