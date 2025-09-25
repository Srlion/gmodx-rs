mod error;
pub use error::Error;

mod state;
pub use state::State;

pub mod bridge;

mod user_data;
pub use user_data::{UserData, UserDataMethods};

mod function;
pub use function::{CFunction, FunctionReturn, RawCFunction, RustFunction, RustFunctionResult};

mod push;
pub use push::Push;

mod is_number;
pub use is_number::IsNumber;

pub use raw::LUA_GLOBALSINDEX;

pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;
pub const MIN_SAFE_INTEGER: i64 = -MAX_SAFE_INTEGER;

pub const fn upvalue_index(i: i32) -> i32 {
    LUA_GLOBALSINDEX - i
}

pub const TNONE: i32 = raw::LUA_TNONE;
pub const TNIL: i32 = raw::LUA_TNIL as i32;
pub const TBOOLEAN: i32 = raw::LUA_TBOOLEAN as i32;
pub const TLIGHTUSERDATA: i32 = raw::LUA_TLIGHTUSERDATA as i32;
pub const TNUMBER: i32 = raw::LUA_TNUMBER as i32;
pub const TSTRING: i32 = raw::LUA_TSTRING as i32;
pub const TTABLE: i32 = raw::LUA_TTABLE as i32;
pub const TFUNCTION: i32 = raw::LUA_TFUNCTION as i32;
pub const TUSERDATA: i32 = raw::LUA_TUSERDATA as i32;
pub const TTHREAD: i32 = raw::LUA_TTHREAD as i32;

pub const MULTRET: i32 = raw::LUA_MULTRET;

pub const OK: i32 = raw::LUA_OK as i32;
pub const YIELD: i32 = raw::LUA_YIELD as i32;
pub const ERRRUN: i32 = raw::LUA_ERRRUN as i32;
pub const ERRSYNTAX: i32 = raw::LUA_ERRSYNTAX as i32;
pub const ERRMEM: i32 = raw::LUA_ERRMEM as i32;
pub const ERRERR: i32 = raw::LUA_ERRERR as i32;
pub const ERRFILE: i32 = raw::LUA_ERRFILE as i32;

pub const REGISTRYINDEX: i32 = raw::LUA_REGISTRYINDEX;
pub const ENVIRONINDEX: i32 = raw::LUA_ENVIRONINDEX;
pub const GLOBALSINDEX: i32 = raw::LUA_GLOBALSINDEX;

pub type NUMBER = raw::lua_Number;
pub type Number = NUMBER;

pub const IDSIZE: u32 = raw::LUA_IDSIZE;

pub type CStr<'a> = &'a std::ffi::CStr;

pub use raw::lua_Debug as Debug;

pub mod raw {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    include!(concat!(env!("OUT_DIR"), "/lua.rs"));
}
