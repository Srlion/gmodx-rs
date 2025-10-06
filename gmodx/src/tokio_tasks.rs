use std::sync::Mutex;
use std::time::Duration;

use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;

use crate::lua::{self, AnyUserData, ObjectLike as _, Value};

const DEFAULT_MAX_WORKER_THREADS: u16 = 2;
const DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT: u16 = 20;

struct TokioState {
    runtime: Runtime,
    handle: Handle,
    tracker: TaskTracker,
    graceful_shutdown_timeout: u16,
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

fn get_max_worker_threads(state: &lua::State) -> Option<u16> {
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
                "GMODX_WORKER_THREADS",
                DEFAULT_MAX_WORKER_THREADS,
                flags,
                "Number of worker threads for async runtime",
            ),
        )
        .ok()?;

    convar.call_method::<u16>(state, "GetInt", ()).ok()
}

fn get_graceful_shutdown_timeout(state: &lua::State) -> Option<u16> {
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

    convar.call_method::<u16>(state, "GetInt", ()).ok()
}

inventory::submit! {
    crate::open_close::new(
        3,
        "tokio_tasks",
        |state| {
            let worker_threads = get_max_worker_threads(state).unwrap_or(DEFAULT_MAX_WORKER_THREADS) ;
            let graceful_shutdown_timeout = get_graceful_shutdown_timeout(state).unwrap_or(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT);

            let runtime = Builder::new_multi_thread()
                .worker_threads(worker_threads as usize)
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
            });
        },
        |_| {
            // take ownership so we can drop everything cleanly after shutdown
            let s = {
                let mut g = STATE.lock().unwrap();
                g.take()
            };
            let Some(s) = s else { return };

            let timeout = Duration::from_secs(s.graceful_shutdown_timeout as u64);

            // close new task intake and wait for tracked tasks to finish (with timeout)
            s.tracker.close();
            let _ = s
                .runtime
                .block_on(async { tokio::time::timeout(timeout, s.tracker.wait()).await });

            s.runtime.shutdown_timeout(timeout);
        },
    )
}
