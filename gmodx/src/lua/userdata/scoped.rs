use std::ops::{Deref, DerefMut};
#[cfg(feature = "send")]
use std::sync::Arc;
#[cfg(not(feature = "send"))]
use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

#[cfg(feature = "send")]
use xutex::Mutex;

use crate::lua::{self, AnyUserData, FromLua, ToLua, UserData, Value};

#[cfg(not(feature = "send"))]
type UserDataStorage<T> = Rc<RefCell<Option<T>>>;
#[cfg(feature = "send")]
type UserDataStorage<T> = Arc<Mutex<Option<T>>>;

pub struct ScopedUserDataRef<T: UserData> {
    /// Pointer to the userdata in Lua
    pub(crate) ptr: usize,
    pub(crate) value: UserDataStorage<T>,
    pub(crate) any: AnyUserData,
}

impl<T: UserData> Clone for ScopedUserDataRef<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            value: self.value.clone(),
            any: self.any.clone(),
        }
    }
}

impl<T: UserData> ScopedUserDataRef<T> {
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

    #[must_use]
    #[inline]
    pub fn inner(self) -> UserDataStorage<T> {
        self.value
    }
}

#[cfg(feature = "send")]
impl<T: UserData> ScopedUserDataRef<T> {
    #[inline]
    pub fn lock(&self) -> xutex::MutexGuard<'_, Option<T>> {
        self.value.lock()
    }

    pub async fn lock_async(&self) -> xutex::MutexGuard<'_, Option<T>> {
        self.value.lock_async().await
    }
}

#[cfg(not(feature = "send"))]
impl<T: UserData> ScopedUserDataRef<T> {
    #[must_use]
    pub fn borrow(&self) -> Ref<'_, Option<T>> {
        self.value.borrow()
    }

    #[must_use]
    pub fn borrow_mut(&self) -> RefMut<'_, Option<T>> {
        self.value.borrow_mut()
    }
}

impl<T: UserData> ToLua for ScopedUserDataRef<T> {
    fn push_to_stack(self, l: &lua::State) {
        self.any.push_to_stack(l);
    }

    fn to_value(self, l: &lua::State) -> Value {
        self.any.to_value(l)
    }
}

impl<T: UserData> ToLua for &ScopedUserDataRef<T> {
    fn push_to_stack(self, l: &lua::State) {
        (&self.any).push_to_stack(l);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.any.0.clone()
    }
}

impl<T: UserData> FromLua for ScopedUserDataRef<T> {
    fn try_from_stack(l: &lua::State, index: i32) -> lua::Result<Self> {
        let name = T::name();
        let any = AnyUserData::from_stack_with_type(l, index, name)?;
        any.scoped_cast_to::<T>(l)
            .ok_or_else(|| l.type_error(index, name))
    }
}

impl<T: UserData> From<ScopedUserDataRef<T>> for AnyUserData {
    fn from(udref: ScopedUserDataRef<T>) -> Self {
        udref.any
    }
}

pub struct ScopedUserData<T: UserData>(ScopedUserDataRef<T>);

impl<T: UserData> Deref for ScopedUserData<T> {
    type Target = ScopedUserDataRef<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: UserData> DerefMut for ScopedUserData<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: UserData> ToLua for &ScopedUserData<T> {
    fn push_to_stack(self, l: &lua::State) {
        (&self.0).push_to_stack(l);
    }

    fn to_value(self, l: &lua::State) -> Value {
        (&self.0).to_value(l)
    }
}

impl<T: UserData> Drop for ScopedUserData<T> {
    fn drop(&mut self) {
        super::drop_userdata_at::<T>(self.0.ptr);
        #[cfg(feature = "send")]
        let _ = self.0.value.lock().take();
        #[cfg(not(feature = "send"))]
        let _ = self.0.value.borrow_mut().take();
    }
}

impl lua::State {
    pub fn create_scoped_userdata<T: UserData>(&self, value: T) -> ScopedUserData<T> {
        let value = {
            #[cfg(feature = "send")]
            {
                Arc::new(Mutex::new(Some(value)))
            }
            #[cfg(not(feature = "send"))]
            {
                Rc::new(RefCell::new(Some(value)))
            }
        };
        let (ptr, any) = self.create_userdata_impl::<_, T>(value.clone());
        ScopedUserData(ScopedUserDataRef {
            ptr: ptr as usize,
            value,
            any,
        })
    }
}

impl AnyUserData {
    #[must_use]
    #[inline]
    pub fn scoped_cast_to<T: UserData>(self, l: &lua::State) -> Option<ScopedUserDataRef<T>> {
        if !self.is::<UserDataStorage<T>>(l) {
            return None;
        }
        let ptr = self.ptr();
        // SAFETY: We have checked the type above
        let storage = unsafe { &*(ptr.cast::<UserDataStorage<T>>()) }.clone();
        #[cfg(feature = "send")]
        let valid = storage.lock().is_some();
        #[cfg(not(feature = "send"))]
        let valid = storage.borrow().is_some();
        if !valid {
            return None;
        }
        Some(ScopedUserDataRef {
            ptr: ptr as usize,
            value: storage,
            any: self,
        })
    }
}
