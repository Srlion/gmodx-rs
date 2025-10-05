// Thanks to mlua again

#[cfg(not(feature = "send"))]
mod inner {
    use std::cell::{Ref, RefCell, RefMut};

    pub type XRc<T> = std::rc::Rc<T>;
    pub type XCell<T> = RefCell<T>;
    pub type XRef<'a, T> = Ref<'a, T>;
    pub type XRefMut<'a, T> = RefMut<'a, T>;
}

#[cfg(feature = "send")]
mod inner {
    use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

    pub type XRc<T> = Arc<T>;
    pub type XCell<T> = RwLock<T>;
    pub type XRef<'a, T> = RwLockReadGuard<'a, T>;
    pub type XRefMut<'a, T> = RwLockWriteGuard<'a, T>;
}

pub use inner::*;

pub struct XRcCell<T>(XRc<XCell<T>>);

impl<T> XRcCell<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(XRc::new(XCell::new(value)))
    }

    #[inline]
    pub fn borrow(&self) -> XRef<'_, T> {
        #[cfg(not(feature = "send"))]
        {
            self.0.borrow()
        }
        #[cfg(feature = "send")]
        {
            self.0.read().expect("RwLock poisoned")
        }
    }

    #[inline]
    pub fn borrow_mut(&self) -> XRefMut<'_, T> {
        #[cfg(not(feature = "send"))]
        {
            self.0.borrow_mut()
        }
        #[cfg(feature = "send")]
        {
            self.0.write().expect("RwLock poisoned")
        }
    }

    #[inline]
    pub fn try_borrow(&self) -> Option<XRef<'_, T>> {
        #[cfg(not(feature = "send"))]
        {
            self.0.try_borrow().ok()
        }
        #[cfg(feature = "send")]
        {
            self.0.try_read().ok()
        }
    }

    #[inline]
    pub fn try_borrow_mut(&self) -> Option<XRefMut<'_, T>> {
        #[cfg(not(feature = "send"))]
        {
            self.0.try_borrow_mut().ok()
        }
        #[cfg(feature = "send")]
        {
            self.0.try_write().ok()
        }
    }
}

impl<T> Clone for XRcCell<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Default> Default for XRcCell<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for XRcCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.try_borrow() {
            Some(borrowed) => write!(f, "XRcCell({:?})", &*borrowed),
            None => write!(f, "XRcCell(<borrowed>)"),
        }
    }
}
