use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_queue::SegQueue;

use crate::lua::{self, ObjectLike, Table};

type Hook = Box<dyn Fn(&lua::State) -> bool + Send>;
type Task = Box<dyn FnOnce(&lua::State) + Send>;

const DEFAULT_TICK_RATE: f64 = 66.6667;
const BUDGET_FRACTION: f64 = 0.06; // 6% of tick interval

static HOOKS: Mutex<Vec<Hook>> = Mutex::new(Vec::new());
static PENDING_HOOKS: SegQueue<Hook> = SegQueue::new();
static ONESHOT_HOOKS: SegQueue<Task> = SegQueue::new();
static TICK_RATE: AtomicU64 = AtomicU64::new(DEFAULT_TICK_RATE.to_bits());

#[inline]
fn budget() -> Duration {
    let rate = f64::from_bits(TICK_RATE.load(Ordering::Relaxed));
    let micros = ((1_000_000.0 / rate) * BUDGET_FRACTION).max(0.0) as u64;
    Duration::from_micros(micros)
}

#[inline(never)]
pub fn on_tick(f: impl Fn(&lua::State) -> bool + Send + 'static) {
    PENDING_HOOKS.push(Box::new(f));
}

#[inline(never)]
pub fn next_tick(f: impl FnOnce(&lua::State) + Send + 'static) {
    ONESHOT_HOOKS.push(Box::new(f));
}

#[inline]
fn flush_pending_hooks() {
    if !PENDING_HOOKS.is_empty() {
        let mut hooks = HOOKS.lock().unwrap();
        while let Some(h) = PENDING_HOOKS.pop() {
            hooks.push(h);
        }
    }
}

#[inline(never)]
pub fn flush_next_tick(l: &lua::State) {
    let deadline = Instant::now() + budget();
    while let Some(f) = ONESHOT_HOOKS.pop() {
        f(l);
        if Instant::now() >= deadline {
            break;
        }
    }
}

fn run_tick_hooks(l: &lua::State) {
    flush_pending_hooks();
    HOOKS.lock().unwrap().retain(|f| !f(l));
    flush_next_tick(l);
}

inventory::submit! {
    crate::open_close::new(
        1,
        "ticks",
        |l| {
            HOOKS.lock().unwrap().clear();
            while PENDING_HOOKS.pop().is_some() {}
            while ONESHOT_HOOKS.pop().is_some() {}

            if let Ok(tick_interval) = l
                .get_global::<Table>("engine")
                .and_then(|engine| engine.call::<f64>(l, "TickInterval", ()))
            {
                TICK_RATE.store((1.0 / tick_interval).to_bits(), Ordering::Relaxed);
            }

            crate::timer::create(&format!("gmodx_ticks-{}", gmodx_macros::unique_id!()), 0, 0, run_tick_hooks);
        },
        |_| {},
    )
}
