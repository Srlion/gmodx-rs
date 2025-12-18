use crate::lua::{self, Function, ObjectLike as _, Table};

pub fn create(timer_name: &str, delay: i32, reps: i32, callback: &Function) {
    lua::with_lock(|l| {
        l.globals()
            .get(l, "timer")
            .and_then(|t: Table| t.call::<()>(l, "Create", (timer_name, delay, reps, callback)))
            .inspect_err(|e| eprintln!("[gmodx] failed to create next tick timer: {e}"))
            .ok();
    });
}

pub fn remove(timer_name: &str) {
    lua::with_lock(|l| {
        l.globals()
            .get(l, "timer")
            .and_then(|t: Table| t.call::<()>(l, "Remove", timer_name))
            .inspect_err(|e| eprintln!("[gmodx] failed to remove next tick timer: {e}"))
            .ok();
    });
}
