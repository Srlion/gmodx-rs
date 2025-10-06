use std::ffi::CString;

use bstr::ByteSlice as _;
use bstr::{BStr, BString};
use num_traits::cast;

use crate::lua::traits::{FromLua, ToLua};
use crate::lua::types::Nil;
use crate::lua::{self, Error, FromLuaMulti, LightUserData, Result, ToLuaMulti, ffi};

impl<T: ToLua> ToLua for Option<T> {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        match self {
            Some(v) => v.push_to_stack(state),
            None => ffi::lua_pushnil(state.0),
        }
    }
}

impl<T: FromLua> FromLua for Option<T> {
    #[inline]
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        if ffi::lua_type(state.0, index) == ffi::LUA_TNIL {
            Ok(None)
        } else {
            T::try_from_stack(state, index).map(Some)
        }
    }
}

impl ToLua for Nil {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        ffi::lua_pushnil(state.0);
    }
}

impl FromLua for Nil {
    #[inline]
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        match ffi::lua_type(state.0, index) {
            ffi::LUA_TNIL => Ok(Nil),
            _ => Err(state.type_error(index, "nil")),
        }
    }
}

impl ToLua for bool {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        ffi::lua_pushboolean(state.0, if self { 1 } else { 0 });
    }
}

impl FromLua for bool {
    #[inline]
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        match ffi::lua_type(state.0, index) {
            ffi::LUA_TBOOLEAN => Ok(ffi::lua_toboolean(state.0, index)),
            ffi::LUA_TNIL => Ok(false), // it's fine if we treat nil as false
            _ => Err(state.type_error(index, "boolean")),
        }
    }
}

impl ToLua for LightUserData {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        ffi::lua_pushlightuserdata(state.0, self.0);
    }
}

impl FromLua for LightUserData {
    #[inline]
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        match ffi::lua_type(state.0, index) {
            ffi::LUA_TLIGHTUSERDATA => Ok(LightUserData(ffi::lua_touserdata(state.0, index))),
            _ => Err(state.type_error(index, "lightuserdata")),
        }
    }
}

impl ToLua for &str {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        ffi::lua_pushlstring(state.0, self.as_ptr() as *const i8, self.len());
    }
}

impl ToLua for String {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        self.as_str().push_to_stack(state);
    }
}

impl ToLua for &String {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        self.as_str().push_to_stack(state);
    }
}

impl ToLua for &BStr {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        ffi::lua_pushlstring(state.0, self.as_ptr() as *const i8, self.len());
    }
}

impl ToLua for BString {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        self.as_bstr().push_to_stack(state);
    }
}

impl ToLua for &BString {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        self.as_bstr().push_to_stack(state);
    }
}

impl FromLua for BString {
    #[inline]
    fn try_from_stack(state: &lua::State, mut index: i32) -> Result<Self> {
        let _sg = state.stack_guard(); // to pop any extra values we push
        match ffi::lua_type(state.0, index) {
            t @ (ffi::LUA_TSTRING | ffi::LUA_TNUMBER) => {
                if t == ffi::LUA_TNUMBER {
                    ffi::lua_pushvalue(state.0, index); // to avoid confusing lua_next
                    index = -1;
                }

                let mut len = 0;
                let ptr = ffi::lua_tolstring(state.0, index, &mut len);
                if ptr.is_null() {
                    return Ok(BString::default()); // what happened wtf?
                }

                let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
                Ok(BString::from(bytes))
            }
            _ => Err(state.type_error(index, "string")),
        }
    }
}

impl ToLua for &std::ffi::CStr {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        let bytes = self.to_bytes();
        ffi::lua_pushlstring(state.0, bytes.as_ptr() as *const i8, bytes.len());
    }
}

impl ToLua for CString {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        self.as_ref().push_to_stack(state)
    }
}

impl ToLua for &CString {
    #[inline]
    fn push_to_stack(self, state: &lua::State) {
        self.as_ref().push_to_stack(state)
    }
}

#[inline]
fn from_lua_f64(state: &lua::State, index: i32) -> Result<f64> {
    match ffi::lua_type(state.0, index) {
        ffi::LUA_TNUMBER | ffi::LUA_TSTRING => Ok(ffi::lua_tonumber(state.0, index)),
        _ => Err(state.type_error(index, "number")),
    }
}

// Converts numeric types to/from Lua's f64, clamping to the target type's range on conversion back.
macro_rules! impl_lua_number_fit {
    (float: $($t:ty),*) => {$(
        impl ToLua for $t {
            #[inline]
            fn push_to_stack(self, state: &lua::State) {
                ffi::lua_pushnumber(state.0, self as f64);
            }
        }

        impl FromLua for $t {
            #[inline]
            fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
                let n = from_lua_f64(state, index)?;
                cast(n).ok_or_else(|| {
                    lua::Error::Message(format!("failed to convert Lua number {n} to {}", stringify!($t)))
                })
            }
        }
    )*};
    (int: $($t:ty),*) => {$(
        impl ToLua for $t {
            #[inline]
            fn push_to_stack(self, state: &lua::State) {
                ffi::lua_pushnumber(state.0, self as f64);
            }
        }

        impl FromLua for $t {
            #[inline]
            fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
                let n = from_lua_f64(state, index)?;
                let n = if n.is_nan() {
                    0.0
                } else if n.is_infinite() {
                    if n.is_sign_positive() { <$t>::MAX as f64 } else { <$t>::MIN as f64 }
                } else {
                    n
                };
                Ok(n.clamp(<$t>::MIN as f64, <$t>::MAX as f64) as $t)
            }
        }
    )*};
}
impl_lua_number_fit!(int: i8, u8, i16, u16, i32, u32);
impl_lua_number_fit!(float: f32, f64);

#[cfg(target_pointer_width = "32")]
impl_lua_number_fit!(int: isize);
#[cfg(target_pointer_width = "32")]
impl_lua_number_fit!(int: usize);

macro_rules! impl_lua_number_big {
    (signed: $($t:ty),*) => {$(
        impl ToLua for $t {
            #[inline]
            fn push_to_stack(self, state: &lua::State) {
                if (-9007199254740991..=9007199254740991).contains(&self) {
                    f64::push_to_stack(self as f64, state) // fits in f64
                } else {
                    self.to_string().push_to_stack(state) // too big, use string
                }
            }
        }
        impl_lua_number_big!(@from_lua $t);
    )*};
    (unsigned: $($t:ty),*) => {$(
        impl ToLua for $t {
            #[inline]
            fn push_to_stack(self, state: &lua::State) {
                if self <= 9007199254740991 {
                    f64::push_to_stack(self as f64, state) // fits in f64
                } else {
                    self.to_string().push_to_stack(state) // too big, use string
                }
            }
        }
        impl_lua_number_big!(@from_lua $t);
    )*};
    (@from_lua $t:ty) => {
        impl FromLua for $t {
            #[inline]
            fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
                match ffi::lua_type(state.0, index) {
                    ffi::LUA_TNUMBER => {
                        let n = ffi::lua_tonumber(state.0, index);
                        if n.is_nan() {
                            Ok(0)
                        } else if n.is_infinite() {
                            Ok(if n.is_sign_positive() { <$t>::MAX } else { <$t>::MIN })
                        } else {
                            cast(n).ok_or_else(|| {
                                Error::Message(format!(
                                    "failed to convert number {n:?} to {}",
                                    stringify!(<$t>)
                                ))
                            })
                        }
                    },
                    ffi::LUA_TSTRING => BString::try_from_stack(state, index)?
                        .to_str()
                        .map_err(|_| state.type_error(index, "number"))?
                        .parse()
                        .map_err(|_| state.type_error(index, "number")),
                    _ => Err(state.type_error(index, "number")),
                }
            }
        }
    };
}
impl_lua_number_big!(signed: i64, i128);
impl_lua_number_big!(unsigned: u64, u128);

#[cfg(target_pointer_width = "64")]
impl_lua_number_big!(signed: isize);
#[cfg(target_pointer_width = "64")]
impl_lua_number_big!(unsigned: usize);

impl ToLuaMulti for () {
    fn push_to_stack_multi(self, _: &lua::State) -> i32 {
        0
    }
}

impl FromLuaMulti for () {
    fn try_from_stack_multi(_: &lua::State, _: i32, _: i32) -> Result<(Self, i32)> {
        Ok(((), 0))
    }
}

macro_rules! impl_tuple_lua_multi {
    ($($name:ident),+) => {
        impl<$($name),+> ToLuaMulti for ($($name,)+)
        where
            $($name: ToLuaMulti,)+
        {
            #[inline]
            fn push_to_stack_multi(self, state: &lua::State) -> i32 {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                let mut count = 0;
                $(
                    count += $name.push_to_stack_multi(state);
                )+
                count
            }
        }

        impl<$($name),+> FromLuaMulti for ($($name,)+)
        where
            $($name: FromLuaMulti,)+
        {
            #[inline]
            fn try_from_stack_multi(state: &lua::State, start: i32, count: i32) -> Result<(Self, i32)> {
                let mut index = 0;
                let mut remaining = count;
                $(
                    #[allow(unused_assignments)]
                    #[allow(non_snake_case)]
                    let $name = {
                        let (result, consumed) = $name::try_from_stack_multi(state, start + index, remaining)?;
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
