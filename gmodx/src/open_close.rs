use std::{collections::HashSet, sync::atomic::AtomicBool};

use crate::lua::{self, ffi};

thread_local! {
    static IS_MAIN_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn is_main_thread() -> bool {
    IS_MAIN_THREAD.with(|cell| cell.get())
}

static GMOD_CLOSED: AtomicBool = AtomicBool::new(true);

#[inline]
pub fn is_closed() -> bool {
    GMOD_CLOSED.load(std::sync::atomic::Ordering::Acquire)
}

#[inline]
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
        if !seen_ids.insert(module.id) {
            panic!("Duplicate OpenClose ID: {}", module.id);
        }
    }

    // Sort by priority (lower priority loads first)
    // For modules with same priority, maintain a stable order
    modules.sort_by_key(|m| (m.priority, m.id));

    modules
}

#[allow(unused)]
pub fn load_all(state: &lua::State) {
    IS_MAIN_THREAD.with(|cell| cell.set(true));
    GMOD_CLOSED.store(false, std::sync::atomic::Ordering::Release);

    let modules = get_sorted_modules();
    for module in &modules {
        ffi::lua_settop(state.0, 1); // Clear the stack, on gmod13_open, there is a string at index 1
        (module.open)(state);
        #[cfg(debug_assertions)]
        println!(
            "[gmodx] Loaded OpenClose '{}' (priority: {})",
            module.id, module.priority
        );
    }
}

#[allow(unused)]
pub fn unload_all(state: &lua::State) {
    GMOD_CLOSED.store(true, std::sync::atomic::Ordering::Release);

    let modules = get_sorted_modules();
    // Unload in reverse order
    for module in modules.iter().rev() {
        ffi::lua_settop(state.0, 0); // Clear the stack
        (module.close)(state);
        #[cfg(debug_assertions)]
        println!(
            "[gmodx] Unloaded OpenClose '{}' (priority: {})",
            module.id, module.priority
        );
    }

    IS_MAIN_THREAD.with(|cell| cell.set(false));
}
