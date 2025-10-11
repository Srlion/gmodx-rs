use std::{cell::RefCell, fmt, sync::Mutex};

use rustc_hash::{FxBuildHasher, FxHashMap};

// This awesome idea is from mlua
use crate::{
    lua::{self, ffi},
    next_tick,
    sync::XRc,
};

static FREE_SLOTS: Mutex<FxHashMap<i32, bool>> = Mutex::new(FxHashMap::with_hasher(FxBuildHasher));

struct RefThread {
    state: lua::State,
    stack_top: i32,
}

thread_local! {
    static REF_THREAD: RefCell<RefThread> = const { RefCell::new(RefThread {
        state: lua::State(std::ptr::null_mut()),
        stack_top: 0,
    }) };
}

#[derive(Clone)]
pub struct ValueRef {
    // Same as xrc_index but to have faster access
    pub(crate) index: i32,
    pub(crate) xrc_index: Option<ValueRefIndex>,
}

/// A reference to a Lua value index in the auxiliary thread.
/// It's cheap to clone and can be used to track the number of references to a value.
#[derive(Clone)]
pub(crate) struct ValueRefIndex(pub(crate) XRc<i32>);

fn stack_pop() -> i32 {
    with_ref_thread(|thread| {
        let free = {
            let mut free_slots = FREE_SLOTS.lock().unwrap();
            if let Some((&value, _)) = free_slots.iter().next() {
                free_slots.remove(&value);
                Some(value)
            } else {
                None
            }
        };
        if let Some(free) = free {
            ffi::lua_replace(thread.state.0, free);
            free
        } else {
            thread.stack_top += 1;
            thread.stack_top
        }
    })
}

impl ValueRef {
    #[inline]
    pub(crate) fn new(index: i32) -> Self {
        let xrc_index = Some(ValueRefIndex(XRc::new(index)));
        Self { index, xrc_index }
    }

    pub(crate) fn push(&self, to: &lua::State) {
        with_ref_thread(|thread| {
            ffi::lua_xpush(thread.state.0, to.0, self.index);
        });
    }

    /// Pops a lua value from the ref thread.
    pub(crate) fn pop() -> Self {
        let index = stack_pop();
        // println!("index: {index}");
        Self::new(index)
    }

    /// Pops a lua value from the specified state to the ref thread.
    pub(crate) fn pop_from(from: &lua::State) -> Self {
        with_ref_thread(|thread| {
            ffi::lua_xmove(from.0, thread.state.0, 1);
        });
        Self::pop()
    }

    pub(crate) fn thread(&self) -> lua::State {
        with_ref_thread(|thread| thread.state.clone())
    }

    // the lua state is only used to ensure we are on main thread
    // #[inline]
    // pub fn equals(&self, _: &lua::State, other: &Self) -> bool {
    //     ffi::lua_rawequal(lua::raw::ref_thread(), self.index, other.index)
    // }
}

impl Drop for ValueRef {
    fn drop(&mut self) {
        // It's guaranteed that the inner value returns exactly once.
        // This means in particular that the value is not dropped.
        if let Some(ValueRefIndex(xrc)) = self.xrc_index.take()
            && XRc::into_inner(xrc).is_some()
        {
            let index = self.index;
            {
                let mut free_slots = FREE_SLOTS.lock().unwrap();
                free_slots.insert(index, true);
            }
            // Make sure we only access the ref_thread on the main thread.
            next_tick(move |_| {
                with_ref_thread(|thread| {
                    debug_assert!(
                        ffi::lua_gettop(thread.state.0) >= index,
                        "GC finalizer is not allowed in ref_thread"
                    );
                    let mut free_slots = FREE_SLOTS.lock().unwrap();
                    if let Some(should_nil) = free_slots.get(&index)
                        && *should_nil
                    {
                        // println!("freeing {index}");
                        ffi::lua_pushnil(thread.state.0);
                        ffi::lua_replace(thread.state.0, index);
                        free_slots.insert(index, false);
                        // thread.state.dump_stack();
                    }
                });
            });
        }
    }
}

impl fmt::Debug for ValueRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ValueRef({})", self.index)
    }
}

fn set_ref_thread(state: lua::State) {
    REF_THREAD.with(|c| {
        let mut ref_thread = c.borrow_mut();
        ref_thread.state = state;
        ref_thread.stack_top = 0;
        FREE_SLOTS.lock().unwrap().clear();
    });
}

fn with_ref_thread<R>(f: impl FnOnce(&mut RefThread) -> R) -> R {
    REF_THREAD.with(|c| {
        let mut b = c.borrow_mut();
        if b.state.0.is_null() {
            panic!("RefThread not initialized!");
        }
        f(&mut b)
    })
}

inventory::submit! {
    crate::open_close::new(
        0,
        "lua/reference",
        |l| {
            let thread = ffi::new_thread(l.0);
            // leak the reference thread so it doesn't get GC'd
            ffi::luaL_ref(l.0, ffi::LUA_REGISTRYINDEX);
            set_ref_thread(lua::State(thread));
        },
        |_| {
            set_ref_thread(lua::State(std::ptr::null_mut()));
        },
    )
}
