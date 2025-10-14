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

// Error type aliases for try_* methods (unified across features)
#[cfg(not(feature = "send"))]
pub type XTryBorrowError = std::cell::BorrowError;
#[cfg(not(feature = "send"))]
pub type XTryBorrowMutError = std::cell::BorrowMutError;

#[cfg(feature = "send")]
pub type XTryBorrowError<'a, T> = std::sync::TryLockError<std::sync::RwLockReadGuard<'a, T>>;
#[cfg(feature = "send")]
pub type XTryBorrowMutError<'a, T> = std::sync::TryLockError<std::sync::RwLockWriteGuard<'a, T>>;

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
}

#[cfg(not(feature = "send"))]
impl<T> XRcCell<T> {
    #[inline]
    pub fn try_borrow(&self) -> Result<XRef<'_, T>, XTryBorrowError> {
        self.0.try_borrow()
    }
    #[inline]
    pub fn try_borrow_mut(&self) -> Result<XRefMut<'_, T>, XTryBorrowMutError> {
        self.0.try_borrow_mut()
    }
}

#[cfg(feature = "send")]
impl<T> XRcCell<T> {
    #[inline]
    pub fn try_borrow(&self) -> Result<XRef<'_, T>, XTryBorrowError<'_, T>> {
        self.0.try_read()
    }
    #[inline]
    pub fn try_borrow_mut(&self) -> Result<XRefMut<'_, T>, XTryBorrowMutError<'_, T>> {
        self.0.try_write()
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
            Ok(borrowed) => write!(f, "XRcCell({:?})", &*borrowed),
            _ => write!(f, "XRcCell(<borrowed>)"),
        }
    }
}

impl<T> From<T> for XRcCell<T> {
    fn from(value: T) -> Self {
        XRcCell::new(value)
    }
}

impl<T> From<XRc<XCell<T>>> for XRcCell<T> {
    fn from(inner: XRc<XCell<T>>) -> Self {
        XRcCell(inner)
    }
}

impl<T> From<XRcCell<T>> for XRc<XCell<T>> {
    fn from(cell: XRcCell<T>) -> Self {
        cell.0
    }
}
