use std::thread;

use crate::is_main_thread;
use crate::lua::{self};

use super::next_tick_queue::NextTickQueue;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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
    F: FnOnce(&lua::State) + Send + 'static,
{
    with_next_tick(|q| q.queue(callback));
}

pub fn flush_next_tick(state: &lua::State) {
    with_next_tick(|q| q.flush(state));
}

pub fn block_until_next_tick<F>(f: F)
where
    F: FnOnce(&lua::State) + Send + 'static,
{
    assert!(
        !is_main_thread(),
        "block_until_next_tick must be called from a non-main thread"
    );

    let th = thread::current();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();

    next_tick(move |state| {
        f(state);
        done2.store(true, Ordering::Release);
        th.unpark();
    });

    while !done.load(Ordering::Acquire) {
        thread::park();
    }
}

inventory::submit! {
    crate::open_close::new(
        2,
        "next_tick",
        |l| {
            let (queue, setup_timer) = NextTickQueue::new_impl();
            {
                let mut q = NEXT_TICK.lock().unwrap();
                q.replace(queue);
            }
            setup_timer(l);
        },
        |l| {
            let mut q = NEXT_TICK.lock().unwrap();
            if let Some(q) = q.take() {
                q.flush(l);
            }
        },
    )
}
