use std::{
    cell::Cell,
    collections::HashSet,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

use crate::lua::{self, ffi};

static MAIN_LUA_STATE: AtomicPtr<lua::ffi::lua_State> = AtomicPtr::new(std::ptr::null_mut());

thread_local! {
    static IS_MAIN_THREAD: Cell<bool> = const { Cell::new(false) };
}

#[must_use]
pub fn is_main_thread() -> bool {
    IS_MAIN_THREAD.with(Cell::get)
}

static GMOD_CLOSED: AtomicBool = AtomicBool::new(true);

#[inline]
pub fn is_closed() -> bool {
    GMOD_CLOSED.load(Ordering::Acquire)
}

#[inline]
#[must_use]
pub fn is_open() -> bool {
    !is_closed()
}

pub struct OpenClose {
    pub priority: i32, // Lower priority loads first
    pub id: &'static str,
    pub open: fn(&lua::State),
    pub close: fn(&lua::State),
}

pub const fn new(
    priority: i32,
    id: &'static str,
    open: fn(&lua::State),
    close: fn(&lua::State),
) -> OpenClose {
    OpenClose {
        priority,
        id,
        open,
        close,
    }
}

inventory::collect!(OpenClose);

fn get_sorted_modules() -> Vec<&'static OpenClose> {
    let mut modules: Vec<&OpenClose> = inventory::iter::<OpenClose>().collect();

    let mut seen_ids = HashSet::new();
    for module in &modules {
        assert!(
            seen_ids.insert(module.id),
            "Duplicate OpenClose ID: {}",
            module.id
        );
    }

    // Sort by priority (lower priority loads first)
    // For modules with same priority, maintain a stable order
    modules.sort_by_key(|m| (m.priority, m.id));

    modules
}

#[inline]
pub(crate) fn get_main_lua_state() -> lua::State {
    let ptr = MAIN_LUA_STATE.load(Ordering::Acquire);
    lua::State(ptr)
}

#[allow(unused)]
pub fn load_all(l: &lua::State) {
    MAIN_LUA_STATE.store(l.0, Ordering::Release);
    IS_MAIN_THREAD.with(|cell| cell.set(true));
    GMOD_CLOSED.store(false, Ordering::Release);

    let modules = get_sorted_modules();
    for module in &modules {
        ffi::lua_settop(l.0, 1); // Clear the stack, on gmod13_open, there is a string at index 1
        (module.open)(l);
        #[cfg(debug_assertions)]
        println!(
            "[gmodx] Loaded OpenClose '{}' (priority: {})",
            module.id, module.priority
        );
    }
}

#[allow(unused)]
pub fn unload_all(l: &lua::State) {
    GMOD_CLOSED.store(true, Ordering::Release);
    MAIN_LUA_STATE.store(std::ptr::null_mut(), Ordering::Release);

    let modules = get_sorted_modules();
    // Unload in reverse order
    for module in modules.iter().rev() {
        ffi::lua_settop(l.0, 0); // Clear the stack
        (module.close)(l);
        #[cfg(debug_assertions)]
        println!(
            "[gmodx] Unloaded OpenClose '{}' (priority: {})",
            module.id, module.priority
        );
    }

    IS_MAIN_THREAD.with(|cell| cell.set(false));
}
