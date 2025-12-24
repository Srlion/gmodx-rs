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
