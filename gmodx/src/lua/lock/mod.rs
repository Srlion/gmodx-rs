use std::cell::UnsafeCell;

use crate::lua::lock::reentrant_mutex::{ReentrantMutex, ReentrantMutexGuard};
use crate::lua::{self};

mod reentrant_mutex;

struct MainThreadCell<T>(UnsafeCell<T>);

// SAFETY: Only accessed from main thread
unsafe impl<T> Sync for MainThreadCell<T> {}

impl<T> MainThreadCell<T> {
    const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }

    fn set(&self, val: T) {
        // SAFETY: Only accessed from main thread
        unsafe {
            *self.0.get() = val;
        }
    }

    fn as_mut(&self) -> &mut T {
        // SAFETY: Only accessed from main thread
        unsafe { &mut *self.0.get() }
    }
}

static LUA_LOCK: ReentrantMutex = ReentrantMutex::new();
static LUA_STATE_PTR: MainThreadCell<*mut lua::ffi::lua_State> =
    MainThreadCell::new(std::ptr::null_mut());
static MAIN_GUARD: MainThreadCell<Option<ReentrantMutexGuard<'static>>> = MainThreadCell::new(None);

pub struct StateGuard {
    _g: ReentrantMutexGuard<'static>,
    state: lua::State,
}

impl std::ops::Deref for StateGuard {
    type Target = lua::State;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[inline(always)]
fn main_lock() {
    MAIN_GUARD.set(Some(LUA_LOCK.lock()));
}

#[inline(always)]
fn main_unlock() {
    MAIN_GUARD.set(None);
}

#[inline(always)]
fn yield_lock(_: &lua::State) {
    if let Some(guard) = MAIN_GUARD.as_mut().as_mut() {
        ReentrantMutexGuard::bump(guard);
    }
}

#[inline(always)]
fn get_state_guard(guard: ReentrantMutexGuard<'static>) -> Option<StateGuard> {
    // SAFETY: We hold the lock, so no other thread can modify the pointer
    let ptr = unsafe { *LUA_STATE_PTR.0.get() };
    if ptr.is_null() {
        return None;
    }
    Some(StateGuard {
        _g: guard,
        state: lua::State(ptr),
    })
}

pub fn lock() -> Option<StateGuard> {
    get_state_guard(LUA_LOCK.lock())
}

pub fn with_lock<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&lua::State) -> R,
{
    let guard = lock()?;
    Some(f(&guard))
}

pub async fn lock_async() -> Option<StateGuard> {
    get_state_guard(LUA_LOCK.lock_async().await)
}

pub async fn with_lock_async<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&lua::State) -> R,
{
    let guard = lock_async().await?;
    Some(f(&guard))
}

inventory::submit! {
    crate::open_close::new(
        1,
        "lock",
        |l| {
            main_lock();
            LUA_STATE_PTR.set(l.0);

            crate::timer::create(
                "gmodx_state_lock_timer",
                0,
                0,
                &l.create_function(yield_lock),
            );
        },
        |_| {
            // Lock to ensure no other threads are using the state
            main_lock();
            LUA_STATE_PTR.set(std::ptr::null_mut());
            main_unlock();
        },
    )
}
