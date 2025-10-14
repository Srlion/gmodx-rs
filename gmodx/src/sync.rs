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
    use std::sync::{Arc, Mutex, MutexGuard};

    pub type XRc<T> = Arc<T>;
    pub type XCell<T> = Mutex<T>;
    pub type XRef<'a, T> = MutexGuard<'a, T>;
    pub type XRefMut<'a, T> = MutexGuard<'a, T>; // Mutex has only one guard type
}

pub use inner::*;

// Error type aliases for try_* methods (unified across features)
#[cfg(not(feature = "send"))]
pub type XTryBorrowError = std::cell::BorrowError;
#[cfg(not(feature = "send"))]
pub type XTryBorrowMutError = std::cell::BorrowMutError;

#[cfg(feature = "send")]
pub type XTryLockError<'a, T> = std::sync::TryLockError<std::sync::MutexGuard<'a, T>>;

pub struct XRcCell<T>(XRc<XCell<T>>);

impl<T> XRcCell<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(XRc::new(XCell::new(value)))
    }
}

#[cfg(not(feature = "send"))]
impl<T> XRcCell<T> {
    #[inline]
    pub fn borrow(&self) -> XRef<'_, T> {
        self.0.borrow()
    }

    #[inline]
    pub fn borrow_mut(&self) -> XRefMut<'_, T> {
        self.0.borrow_mut()
    }

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
    pub fn lock(&self) -> XRef<'_, T> {
        self.0.lock().expect("Mutex poisoned")
    }

    #[inline]
    pub fn try_lock(&self) -> Result<XRef<'_, T>, XTryLockError<'_, T>> {
        self.0.try_lock()
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
        #[cfg(not(feature = "send"))]
        {
            match self.try_borrow() {
                Ok(borrowed) => write!(f, "XRcCell({:?})", &*borrowed),
                _ => write!(f, "XRcCell(<borrowed>)"),
            }
        }
        #[cfg(feature = "send")]
        {
            match self.try_lock() {
                Ok(guard) => write!(f, "XRcCell({:?})", &*guard),
                _ => write!(f, "XRcCell(<locked>)"),
            }
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
