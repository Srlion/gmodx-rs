use std::cell::UnsafeCell;

use xutex::{ReentrantMutex, ReentrantMutexGuard};

use crate::lua::{self};
use crate::open_close::get_main_lua_state;

// pub(crate) mod reentrant_mutex;

struct GuardHolder(UnsafeCell<Option<ReentrantMutexGuard<'static, ()>>>);
unsafe impl Sync for GuardHolder {}

static LUA_LOCK: ReentrantMutex<()> = ReentrantMutex::new(());
static MAIN_GUARD: GuardHolder = GuardHolder(UnsafeCell::new(None));
pub struct StateGuard {
    _g: ReentrantMutexGuard<'static, ()>,
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
    unsafe {
        *MAIN_GUARD.0.get() = Some(LUA_LOCK.lock());
    }
}

#[inline(always)]
fn main_unlock() {
    unsafe {
        *MAIN_GUARD.0.get() = None;
    }
}

#[inline(always)]
fn yield_lock(_: &lua::State) {
    if let Some(guard) = unsafe { (*MAIN_GUARD.0.get()).as_mut() } {
        ReentrantMutexGuard::bump(guard);
    }
}

#[inline(always)]
fn get_state_guard(guard: ReentrantMutexGuard<'static, ()>) -> Option<StateGuard> {
    if crate::is_closed() {
        return None;
    }
    Some(StateGuard {
        _g: guard,
        state: get_main_lua_state(),
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
        0,
        "lock",
        |_| {
            main_lock();
            crate::timer::create("gmodx_state_lock_timer", 0, 0, yield_lock);
        },
        |_| {
            // Lock to ensure no other threads are using the state
            main_lock();
            main_unlock();
        },
    )
}
