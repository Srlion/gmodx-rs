use crate::lua;

use super::next_tick_queue::NextTickQueue;
use std::sync::Mutex;

static NEXT_TICK: Mutex<Option<NextTickQueue>> = Mutex::new(None);

fn with_next_tick<F>(f: F)
where
    F: FnOnce(&NextTickQueue),
{
    let q = NEXT_TICK.lock().unwrap();
    if let Some(q) = q.as_ref() {
        f(q);
    }
}

pub fn next_tick<F>(callback: F)
where
    F: FnOnce(lua::State) + Send + 'static,
{
    with_next_tick(|q| q.queue(callback));
}

pub fn flush_next_tick(l: lua::State) {
    with_next_tick(|q| q.flush(l));
}

inventory::submit! {
    crate::open_close::new(
        2,
        "next_tick",
        |l| {
            let mut q = NEXT_TICK.lock().unwrap();
            q.replace(NextTickQueue::new(l));
        },
        |l| {
            let mut q = NEXT_TICK.lock().unwrap();
            if let Some(q) = q.take() {
                q.flush(l);
            }
        },
    )
}
