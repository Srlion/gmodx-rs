use crate::lua::{self, ffi};

pub struct StackGuard {
    state: *mut ffi::lua_State,
    top: i32,
}

impl StackGuard {
    // Creates a StackGuard instance with record of the stack size, and on Drop will check the
    // stack size and drop any extra elements. If the stack size at the end is *smaller* than at
    // the beginning, this is considered a fatal logic error and will result in a panic.
    #[inline]
    pub fn new(state: *mut ffi::lua_State) -> Self {
        Self {
            state,
            top: ffi::lua_gettop(state),
        }
    }

    // Same as `new()`, but allows specifying the expected stack size at the end of the scope.
    #[inline]
    pub const fn with_top(state: *mut ffi::lua_State, top: i32) -> Self {
        Self { state, top }
    }

    #[inline]
    pub const fn keep(&mut self, n: i32) {
        self.top += n;
    }

    #[inline]
    #[must_use]
    pub const fn top(&self) -> i32 {
        self.top
    }
}

impl lua::State {
    #[must_use]
    pub fn stack_guard(&self) -> StackGuard {
        StackGuard::new(self.0)
    }
}

impl Drop for StackGuard {
    #[track_caller]
    fn drop(&mut self) {
        let top = ffi::lua_gettop(self.state);
        if top < self.top {
            gmodx_panic!("{} too many stack values popped", self.top - top)
        }
        if top > self.top {
            ffi::lua_settop(self.state, self.top);
        }
    }
}
