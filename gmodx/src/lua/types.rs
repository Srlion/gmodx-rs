use std::ffi::c_void;

use crate::lua;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LightUserData(pub *mut c_void);

pub type Number = lua::ffi::lua_Number;

pub type String = bstr::BString;

pub struct Nil;

/// A trait that adds `Send` requirement if `send` feature is enabled.
#[cfg(feature = "send")]
pub trait MaybeSend: Send {}
#[cfg(feature = "send")]
impl<T: Send> MaybeSend for T {}

#[cfg(not(feature = "send"))]
pub trait MaybeSend {}
#[cfg(not(feature = "send"))]
impl<T> MaybeSend for T {}

pub type CallbackResult = std::result::Result<i32, Box<dyn std::error::Error>>;

#[cfg(feature = "send")]
type CallbackFn<'a> = dyn Fn(&lua::State) -> CallbackResult + Send + 'a;

#[cfg(not(feature = "send"))]
type CallbackFn<'a> = dyn Fn(&lua::State) -> CallbackResult + 'a;

pub type Callback = Box<CallbackFn<'static>>;
