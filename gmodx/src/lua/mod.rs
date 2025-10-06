#![allow(static_mut_refs)]

pub(crate) mod ffi;

mod state;
pub use state::State;

mod conversion;
mod value_ref;

mod types;
pub use types::{LightUserData, Nil, Number, String};

mod value;
pub use value::{MultiValue, Value};

mod error;
pub use error::{Error, Result};

mod stack_guard;
pub use stack_guard::StackGuard;

mod table;
pub use table::Table;

mod traits;
pub use traits::{FromLua, FromLuaMulti, ObjectLike, ToLua, ToLuaMulti};

mod function;
pub use function::Function;

mod userdata;
pub use userdata::{AnyUserData, MethodsBuilder as Methods, UserData, UserDataRef};

mod debug;

pub(crate) mod private {
    use super::*;

    pub trait Sealed {}

    impl Sealed for Error {}
    impl<T> Sealed for std::result::Result<T, Error> {}
    impl Sealed for State {}
    impl Sealed for Table {}
    impl Sealed for AnyUserData {}
}
