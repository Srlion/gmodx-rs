use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex, mpsc};

use crate::lua;

type CallbackBoxed = Box<dyn FnOnce(lua::State) + Send>;

static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn next_id() -> usize {
    ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub fn new(l: lua::State) -> NextTickQueue {
    NextTickQueue::new(l)
}

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

    fn flush(&self, l: lua::State) {
        // Collect up to 5 callbacks then drop the lock to avoid deadlocks
        // We max it to avoid starving the main thread OR lagging it
        let callbacks: Vec<_> = { self.rx.lock().unwrap().try_iter().take(5).collect() }; // Lock is dropped here
        for callback in callbacks {
            l.set_top(0); // Clear the stack before each callback
            self.counter.fetch_sub(1, Ordering::Release);
            callback(l);
        }
    }
}

#[derive(Clone)]
pub struct NextTickQueue {
    sender: mpsc::Sender<CallbackBoxed>,
    lua_receiver: Arc<LuaReceiver>,
}

impl NextTickQueue {
    pub fn new(l: lua::State) -> Self {
        let (tx, rx) = mpsc::channel();

        let timer_name = format!("__GMODX_LUA_THINK_{}", next_id());

        let lua_receiver = Arc::new(LuaReceiver {
            rx: Mutex::new(rx),
            counter: AtomicUsize::new(0),
            is_closed: AtomicBool::new(false),
        });

        l.with_nested_field_ignore(None, "timer.Create", || {
            l.pcall_ignore(|| {
                l.push_string(&timer_name);
                l.push_number(0); // interval
                l.push_number(0); // repetitions

                let lua_receiver = lua_receiver.clone();
                l.push_closure(move |l| {
                    if lua_receiver.count() == 0 {
                        // if no more tasks and the task queue is closed, we need to remove the timer
                        if lua_receiver.is_closed() {
                            #[cfg(debug_assertions)]
                            println!(
                                "[gmodx] No more tasks and the queue is closed, removing timer {}",
                                timer_name
                            );
                            remove_timer(l, &timer_name);
                            // The receiver will be dropped when all Arc references are gone
                        }
                        return;
                    }

                    lua_receiver.flush(l);
                });

                0
            });
        });

        Self {
            sender: tx,
            lua_receiver,
        }
    }

    pub fn queue<F>(&self, callback: F)
    where
        F: FnOnce(lua::State) + Send + 'static,
    {
        if super::is_closed() {
            return;
        }
        let _ = self.sender.send(Box::new(callback));
        self.lua_receiver.increment_counter();
    }

    pub fn flush(&self, l: lua::State) {
        self.lua_receiver.flush(l);
    }
}

impl Drop for NextTickQueue {
    fn drop(&mut self) {
        self.lua_receiver.set_closed();
    }
}

fn remove_timer(l: lua::State, timer_name: &str) {
    l.with_nested_field_ignore(None, "timer.Remove", || {
        l.pcall_ignore(|| {
            l.push_string(timer_name);
            0
        });
    });
}
