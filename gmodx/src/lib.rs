#[macro_use]
pub mod macros;

pub mod lua;

pub mod open_close;
pub use open_close::{is_closed, is_main_thread, is_open};

pub use gmodx_macros::*;

pub use inventory;

pub mod sync;

mod next_tick_queue;
pub use next_tick_queue::NextTickQueue;

mod next_tick;
pub use next_tick::{async_next_tick, block_until_next_tick, flush_next_tick, next_tick};

#[cfg(feature = "tokio")]
pub mod tokio_tasks;

pub use bstr;
