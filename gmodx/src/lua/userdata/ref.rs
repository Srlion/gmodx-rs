use std::{
    cell::{Ref, RefCell, RefMut},
    ffi::c_void,
};

use crate::lua::{self, AnyUserData, Error, FromLua, Result, ToLua, UserData, Value};

/// The 'static bound is needed to ensure the userdata lives long enough
#[derive(Debug)]
pub struct UserDataRef<T: UserData + 'static> {
    /// Pointer to the userdata in Lua
    pub(crate) ptr: *const c_void,
    pub(crate) any: AnyUserData,
    pub(crate) _marker: std::marker::PhantomData<T>,
}

impl<T: UserData> Clone for UserDataRef<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            any: self.any.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: UserData> UserDataRef<T> {
    #[inline]
    const fn downcast(&self) -> &RefCell<T> {
        // SAFETY: The pointer is valid as long as the inner value is alive.
        // SAFETY: We type check before initializing UserDataRef.
        unsafe { &*(self.ptr.cast::<RefCell<T>>()) }
    }

    #[must_use]
    #[inline]
    pub fn borrow(&self) -> Ref<'_, T> {
        self.downcast().borrow()
    }

    #[must_use]
    #[inline]
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.downcast().borrow_mut()
    }

    #[inline]
    pub fn try_borrow(&self) -> Result<Ref<'_, T>> {
        self.downcast()
            .try_borrow()
            .map_err(|err| Error::Message(format!("cannot borrow '{}': {}", T::name(), err)))
    }

    #[inline]
    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, T>> {
        self.downcast().try_borrow_mut().map_err(|err| {
            Error::Message(format!("cannot borrow '{}' mutably: {}", T::name(), err))
        })
    }

    #[must_use]
    #[inline]
    pub const fn as_any(&self) -> &AnyUserData {
        &self.any
    }

    #[must_use]
    #[inline]
    pub fn into_any(self) -> AnyUserData {
        self.any
    }
}

impl<T: UserData> ToLua for UserDataRef<T> {
    fn push_to_stack(self, l: &lua::State) {
        self.any.push_to_stack(l);
    }

    fn to_value(self, l: &lua::State) -> Value {
        self.any.to_value(l)
    }
}

impl<T: UserData> ToLua for &UserDataRef<T> {
    fn push_to_stack(self, l: &lua::State) {
        #[allow(clippy::needless_borrow)]
        (&self.any).push_to_stack(l);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.any.0.clone()
    }
}

impl<T: UserData> FromLua for UserDataRef<T> {
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        let name = T::name();
        let any = AnyUserData::from_stack_with_type(l, index, name)?;
        any.cast_to::<T>(l).ok_or_else(|| l.type_error(index, name))
    }
}

impl<T: UserData> From<UserDataRef<T>> for AnyUserData {
    fn from(udref: UserDataRef<T>) -> Self {
        udref.any
    }
}

impl AnyUserData {
    #[must_use]
    #[inline]
    pub fn cast_to<T: UserData>(self, l: &lua::State) -> Option<UserDataRef<T>> {
        if !self.is::<RefCell<T>>(l) {
            return None;
        }
        Some(UserDataRef {
            ptr: self.ptr(),
            any: self,
            _marker: std::marker::PhantomData,
        })
    }
}

impl lua::State {
    pub fn create_userdata<T: UserData>(&self, ud: T) -> UserDataRef<T> {
        let (ptr, any) = self.create_userdata_impl::<_, T>(RefCell::new(ud));
        UserDataRef {
            ptr,
            any,
            _marker: std::marker::PhantomData,
        }
    }
}
