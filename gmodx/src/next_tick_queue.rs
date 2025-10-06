use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use crate::lua::{self, Function, ObjectLike as _, Table, ffi};

type CallbackBoxed = Box<dyn FnOnce(&lua::State) + Send>;

static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

// We use counter/is_closed for fast checking inside the timer callback, this is because it's called A lot
#[derive(Debug)]
struct LuaReceiver {
    rx: Mutex<mpsc::Receiver<CallbackBoxed>>,
    counter: AtomicUsize,
    is_closed: AtomicBool,
}

impl LuaReceiver {
    fn increment_counter(&self) {
        self.counter.fetch_add(1, Ordering::Release);
    }

    fn count(&self) -> usize {
        self.counter.load(Ordering::Acquire)
    }

    fn set_closed(&self) {
        self.is_closed.store(true, Ordering::Release);
    }

    fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Acquire)
    }

    fn flush(&self, state: &lua::State) {
        // Collect up to 5 callbacks then drop the lock to avoid deadlocks
        // We max it to avoid starving the main thread OR lagging it
        let callbacks: Vec<_> = { self.rx.lock().unwrap().try_iter().take(5).collect() }; // Lock is dropped here
        for callback in callbacks {
            ffi::lua_settop(state.0, 0); // Clear the stack before each callback
            self.counter.fetch_sub(1, Ordering::Release);
            callback(state);
        }
    }
}

#[derive(Clone)]
pub struct NextTickQueue {
    sender: mpsc::Sender<CallbackBoxed>,
    lua_receiver: Arc<LuaReceiver>,
}

impl NextTickQueue {
    pub fn new(state: &lua::State) -> Self {
        let (queue, setup_timer) = Self::new_impl();
        setup_timer(state);
        queue
    }

    pub(crate) fn new_impl() -> (Self, impl FnOnce(&lua::State)) {
        let (tx, rx) = mpsc::channel();

        let lua_receiver = Arc::new(LuaReceiver {
            rx: Mutex::new(rx),
            counter: AtomicUsize::new(0),
            is_closed: AtomicBool::new(false),
        });

        let setup_timer = {
            let lua_receiver = lua_receiver.clone();

            move |state: &lua::State| {
                let timer_name = format!(
                    "{}-{}",
                    gmodx_macros::unique_id!(),
                    ID_COUNTER.fetch_add(1, Ordering::Relaxed)
                );
                let callback = create_callback(state, &timer_name, lua_receiver);
                create_timer(state, &timer_name, &callback);
            }
        };

        let queue = Self {
            sender: tx,
            lua_receiver,
        };

        (queue, setup_timer)
    }

    pub fn queue<F>(&self, callback: F)
    where
        F: FnOnce(&lua::State) + Send + 'static,
    {
        if super::is_closed() {
            return;
        }
        let _ = self.sender.send(Box::new(callback));
        self.lua_receiver.increment_counter();
    }

    pub fn flush(&self, state: &lua::State) {
        self.lua_receiver.flush(state);
    }
}

impl Drop for NextTickQueue {
    fn drop(&mut self) {
        self.lua_receiver.set_closed();
    }
}

fn create_callback(
    state: &lua::State,
    timer_name: &str,
    lua_receiver: Arc<LuaReceiver>,
) -> Function {
    let timer_name = timer_name.to_string();
    state.create_function(move |state, ()| {
        if lua_receiver.count() == 0 {
            if lua_receiver.is_closed() {
                #[cfg(debug_assertions)]
                println!(
                    "[gmodx] No more tasks and the queue is closed, removing timer {timer_name}"
                );
                remove_timer(state, &timer_name);
            }
            return Ok(());
        }

        lua_receiver.flush(state);

        Ok(())
    })
}

fn remove_timer(state: &lua::State, timer_name: &str) {
    state
        .globals()
        .get(state, "timer")
        .and_then(|t: Table| t.call::<()>(state, "Remove", timer_name))
        .inspect_err(|e| eprintln!("[gmodx] failed to remove next tick timer: {e}"))
        .ok();
}

fn create_timer(state: &lua::State, timer_name: &str, callback: &Function) {
    state
        .globals()
        .get(state, "timer")
        .and_then(|t: Table| t.call::<()>(state, "Create", (timer_name, 0, 0, callback)))
        .inspect_err(|e| eprintln!("[gmodx] failed to create next tick timer: {e}"))
        .ok();
}
