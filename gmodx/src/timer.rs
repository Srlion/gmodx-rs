use crate::lua::{IntoLuaFunction, ObjectLike as _, Table, with_lock};

pub fn create<Marker>(
    timer_name: &str,
    delay: i32,
    reps: i32,
    callback: impl IntoLuaFunction<Marker>,
) {
    with_lock(|l| {
        let callback = callback.into_function();
        l.globals()
            .get(l, "timer")
            .and_then(|t: Table| t.call::<()>(l, "Create", (timer_name, delay, reps, callback)))
            .inspect_err(|e| eprintln!("[gmodx] failed to create next tick timer: {e}"))
            .ok();
    });
}

pub fn remove(timer_name: &str) {
    with_lock(|l| {
        l.globals()
            .get(l, "timer")
            .and_then(|t: Table| t.call::<()>(l, "Remove", timer_name))
            .inspect_err(|e| eprintln!("[gmodx] failed to remove next tick timer: {e}"))
            .ok();
    });
}
