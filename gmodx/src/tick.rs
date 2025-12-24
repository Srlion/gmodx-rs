use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use std::time::{Duration, Instant};

use crate::lua::{self, ObjectLike, Table};

type Hook = Box<dyn Fn(&lua::State) -> bool + Send>;
type Task = Box<dyn FnOnce(&lua::State) + Send>;

const DEFAULT_TICK_RATE: f64 = 66.6667;

// Budget as fraction of tick interval
const BUDGET_FRACTION: f64 = 0.03; // 3% of tick interval

// Persistent hooks that run every tick until they return true
static HOOKS: Mutex<Vec<Hook>> = Mutex::new(Vec::new());
// One-shot tasks that run on the next tick
static ONESHOT_HOOKS: Mutex<OneShotHooks> = Mutex::new(OneShotHooks {
    tick_rate: DEFAULT_TICK_RATE,
    hooks: VecDeque::new(),
});
static TASKS: Mutex<Vec<Box<dyn FnOnce() + Send>>> = Mutex::new(Vec::new());

static TASK_COUNT: AtomicUsize = AtomicUsize::new(0);

struct OneShotHooks {
    tick_rate: f64,
    hooks: VecDeque<Task>,
}

impl OneShotHooks {
    #[inline]
    fn budget(&self) -> Duration {
        let tick_interval_us = (1_000_000.0 / self.tick_rate) * BUDGET_FRACTION;
        Duration::from_micros(tick_interval_us as u64)
    }
}

/// Register a function to be called every tick.
///
/// Return true from the function to unregister it.
///
/// Example:
/// ```
/// gmodx::tick::on_tick(|l| {
///     println!("Tick!");
///     false // return true to unregister
/// });
/// ```
#[inline(never)]
pub fn on_tick(f: impl Fn(&lua::State) -> bool + Send + 'static) {
    TASKS.lock().unwrap().push(Box::new(|| {
        HOOKS.lock().unwrap().push(Box::new(f));
    }));
    TASK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Schedule a one-shot function to be called on the next tick.
///
/// Runs up to 20 scheduled functions per tick.
///
/// Example:
/// ```
/// gmodx::tick::next_tick(|l| {
///     println!("This runs on the next tick!");
/// });
/// ```
#[inline(never)]
pub fn next_tick(f: impl FnOnce(&lua::State) + Send + 'static) {
    TASKS.lock().unwrap().push(Box::new(|| {
        ONESHOT_HOOKS.lock().unwrap().hooks.push_back(Box::new(f));
    }));
    TASK_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[inline(never)]
pub fn flush_next_tick(l: &lua::State) {
    let mut deadline = Instant::now();
    let mut set = false;

    loop {
        let f = {
            let oneshot = &mut *ONESHOT_HOOKS.lock().unwrap();
            if !set {
                set = true;
                deadline += oneshot.budget();
            }
            oneshot.hooks.pop_front()
        };
        let Some(f) = f else { break };

        f(l);

        if Instant::now() >= deadline {
            break;
        }
    }
}

fn run_tick_hooks(l: &lua::State) {
    // flush tasks only if any pending
    if TASK_COUNT.load(Ordering::Relaxed) > 0 {
        for f in std::mem::take(&mut *TASKS.lock().unwrap()) {
            TASK_COUNT.fetch_sub(1, Ordering::Relaxed);
            f();
        }
    }

    // run persistent hooks
    HOOKS.lock().unwrap().retain(|f| !f(l));

    flush_next_tick(l);
}

inventory::submit! {
    crate::open_close::new(
        1,
        "ticks",
        |l| {
            HOOKS.lock().unwrap().clear();
            ONESHOT_HOOKS.lock().unwrap().hooks.clear();
            TASKS.lock().unwrap().clear();

            if let Ok(tick_interval) = l
                .get_global::<Table>("engine")
                .and_then(|engine| engine.call::<f64>(l, "TickInterval", ()))
            {
                ONESHOT_HOOKS.lock().unwrap().tick_rate = 1.0 / tick_interval;
            }

            crate::timer::create(&format!("gmodx_ticks-{}", gmodx_macros::unique_id!()), 0, 0, run_tick_hooks);
        },
        |_| {},
    )
}
