use std::sync::Mutex;
use std::time::Duration;

use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;

use crate::lua::{self, AnyUserData, ObjectLike as _, Value};

static DEFAULT_ASYNC_THREADS_COUNT: usize = 1;
static DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT: u64 = 20;

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    Started {
        thread_count: usize,
    },
    ShuttingDown {
        timeout_secs: u64,
        pending_tasks: usize,
    },
    ShutdownComplete,
    ShutdownTimeout,
}

type EventCallback = Box<dyn Fn(RuntimeEvent) + Send + Sync>;

static EVENTS: Mutex<Vec<EventCallback>> = Mutex::new(Vec::new());

pub fn on_event(callback: impl Fn(RuntimeEvent) + Send + Sync + 'static) {
    EVENTS.lock().unwrap().push(Box::new(callback));

    // Emit Started event if runtime is already initialized
    if let Some(state) = STATE.lock().unwrap().as_ref() {
        emit_event(RuntimeEvent::Started {
            thread_count: state.thread_count,
        });
    }
}

fn emit_event(event: RuntimeEvent) {
    for callback in EVENTS.lock().unwrap().iter() {
        callback(event.clone());
    }
}

struct TokioState {
    runtime: Runtime,
    handle: Handle,
    tracker: TaskTracker,
    graceful_shutdown_timeout: u64,
    thread_count: usize,
}

static STATE: Mutex<Option<TokioState>> = Mutex::new(None);

pub fn spawn<F>(fut: F) -> Option<JoinHandle<F::Output>>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let g = STATE.lock().unwrap();
    let s = g.as_ref()?;
    Some(s.handle.spawn(s.tracker.track_future(fut)))
}

pub fn spawn_untracked<F>(fut: F) -> Option<JoinHandle<F::Output>>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let g = STATE.lock().unwrap();
    let s = g.as_ref()?;
    Some(s.handle.spawn(fut))
}

pub fn block_on<F>(fut: F) -> Option<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let g = STATE.lock().unwrap();
    let s = g.as_ref()?;
    Some(s.runtime.block_on(fut))
}

fn load_threads_from_convar(state: &lua::State) -> Option<usize> {
    let globals = state.globals();

    let flags = state.create_table();
    for (idx, name) in ["FCVAR_ARCHIVE", "FCVAR_PROTECTED"].iter().enumerate() {
        let cvar_flag = globals.get::<Value>(state, *name).ok()?;
        flags.set(state, idx + 1, cvar_flag).ok()?;
    }

    let convar: AnyUserData = globals
        .call(
            state,
            "CreateConVar",
            (
                "GMODX_ASYNC_THREADS",
                DEFAULT_ASYNC_THREADS_COUNT,
                flags,
                "Number of async threads",
            ),
        )
        .ok()?;

    convar.call_method(state, "GetInt", ()).ok()
}

fn load_timeout_from_convar(state: &lua::State) -> Option<u64> {
    let globals = state.globals();

    let flags = state.create_table();
    for (idx, name) in ["FCVAR_ARCHIVE", "FCVAR_PROTECTED"].iter().enumerate() {
        let cvar_flag = globals.get::<Value>(state, *name).ok()?;
        flags.set(state, idx + 1, cvar_flag).ok()?;
    }

    let convar: AnyUserData = globals
        .call(
            state,
            "CreateConVar",
            (
                "GMODX_GRACEFUL_SHUTDOWN_TIMEOUT",
                DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
                flags,
                "Timeout for graceful shutdown of the async runtime, in seconds",
            ),
        )
        .ok()?;

    convar.call_method(state, "GetInt", ()).ok()
}

fn initialize(state: &lua::State) {
    let thread_count = load_threads_from_convar(state)
        .unwrap_or(DEFAULT_ASYNC_THREADS_COUNT)
        .max(1);
    let graceful_shutdown_timeout = load_timeout_from_convar(state)
        .unwrap_or(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT)
        .max(1);

    let runtime = Builder::new_multi_thread()
        .worker_threads(thread_count)
        .enable_all()
        .thread_name(format!("gmodx-rs:{}", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("failed to build tokio runtime");

    let tracker = TaskTracker::new();

    let mut g = STATE.lock().unwrap();
    *g = Some(TokioState {
        handle: runtime.handle().clone(),
        runtime,
        tracker,
        graceful_shutdown_timeout,
        thread_count,
    });
}

fn shutdown(_: &lua::State) {
    // take ownership so we can drop everything cleanly after shutdown
    let s = {
        let mut g = STATE.lock().unwrap();
        g.take()
    };
    let Some(s) = s else { return };

    let timeout_secs = Duration::from_secs(s.graceful_shutdown_timeout);

    emit_event(RuntimeEvent::ShuttingDown {
        timeout_secs: s.graceful_shutdown_timeout,
        pending_tasks: s.tracker.len(),
    });

    // close new task intake and wait for tracked tasks to finish (with timeout)
    s.tracker.close();
    let wait_result = s
        .runtime
        .block_on(async { tokio::time::timeout(timeout_secs, s.tracker.wait()).await });

    s.runtime.shutdown_timeout(timeout_secs);

    match wait_result {
        Ok(_) => emit_event(RuntimeEvent::ShutdownComplete),
        Err(_) => emit_event(RuntimeEvent::ShutdownTimeout),
    }

    EVENTS.lock().unwrap().clear();
}

inventory::submit! {
    crate::open_close::new(3, "tokio_tasks", initialize, shutdown)
}
