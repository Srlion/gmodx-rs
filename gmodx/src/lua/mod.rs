#![allow(static_mut_refs)]

pub(crate) mod ffi;

pub(crate) mod lock;
pub use lock::{lock, lock_async, with_lock, with_lock_async};

mod state;
pub use state::State;

mod conversion;
mod value_ref;

mod types;
pub use types::{LightUserData, Nil, Number, String};

mod value;
pub use value::{MultiValue, Value, ValueKind};

mod multi_value_of;
pub use multi_value_of::MultiValueOf;

mod error;
pub use error::{Error, Result};

mod stack_guard;
pub use stack_guard::StackGuard;

mod table;
pub use table::{Table, table};

mod thread;
pub use thread::{Thread, ThreadStatus};

mod traits;
pub use traits::{FromLua, FromLuaMulti, ObjectLike, ToLua, ToLuaMulti};

mod function;
pub use function::{Function, IntoLuaFunction};

mod userdata;
pub use userdata::{
    AnyUserData, MethodsBuilder as Methods, ScopedUserData, ScopedUserDataRef, UserData,
    UserDataRef,
};

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
