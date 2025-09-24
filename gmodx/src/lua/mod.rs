mod error;
pub use error::Error;

mod state;
pub use state::State;

mod user_data;
pub use user_data::{UserData, UserDataMethods};

mod function;
pub use function::{CFunction, FunctionReturn, RawCFunction, RustFunction, RustFunctionResult};

mod push;
pub use push::Push;

mod is_number;
pub use is_number::IsNumber;

use crate::lua_shared;
pub use lua_shared::LUA_GLOBALSINDEX;

pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;
pub const MIN_SAFE_INTEGER: i64 = -MAX_SAFE_INTEGER;

pub const fn upvalue_index(i: i32) -> i32 {
    LUA_GLOBALSINDEX - i
}

pub const TNONE: i32 = lua_shared::LUA_TNONE;
pub const TNIL: u32 = lua_shared::LUA_TNIL;
pub const TBOOLEAN: u32 = lua_shared::LUA_TBOOLEAN;
pub const TLIGHTUSERDATA: u32 = lua_shared::LUA_TLIGHTUSERDATA;
pub const TNUMBER: u32 = lua_shared::LUA_TNUMBER;
pub const TSTRING: u32 = lua_shared::LUA_TSTRING;
pub const TTABLE: u32 = lua_shared::LUA_TTABLE;
pub const TFUNCTION: u32 = lua_shared::LUA_TFUNCTION;
pub const TUSERDATA: u32 = lua_shared::LUA_TUSERDATA;
pub const TTHREAD: u32 = lua_shared::LUA_TTHREAD;

pub const MULTRET: i32 = lua_shared::LUA_MULTRET;

pub const OK: u32 = lua_shared::LUA_OK;
pub const YIELD: u32 = lua_shared::LUA_YIELD;
pub const ERRRUN: u32 = lua_shared::LUA_ERRRUN;
pub const ERRSYNTAX: u32 = lua_shared::LUA_ERRSYNTAX;
pub const ERRMEM: u32 = lua_shared::LUA_ERRMEM;
pub const ERRERR: u32 = lua_shared::LUA_ERRERR;
pub const ERRFILE: u32 = lua_shared::LUA_ERRFILE;

pub const REGISTRYINDEX: i32 = lua_shared::LUA_REGISTRYINDEX;
pub const ENVIRONINDEX: i32 = lua_shared::LUA_ENVIRONINDEX;
pub const GLOBALSINDEX: i32 = lua_shared::LUA_GLOBALSINDEX;

pub type NUMBER = lua_shared::lua_Number;
pub type Number = NUMBER;

pub const IDSIZE: u32 = lua_shared::LUA_IDSIZE;

pub type CStr<'a> = &'a std::ffi::CStr;

pub use lua_shared::lua_Debug as Debug;
