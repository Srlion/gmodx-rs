use std::{
    fmt,
    sync::{
        Mutex,
        atomic::{AtomicI32, AtomicPtr, Ordering},
    },
};

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{
    lua::{self, ffi},
    next_tick,
    sync::XRc,
};

static FREE_SLOTS: Mutex<FxHashMap<i32, bool>> = Mutex::new(FxHashMap::with_hasher(FxBuildHasher));
static REF_STATE: AtomicPtr<ffi::lua_State> = AtomicPtr::new(std::ptr::null_mut());
static REF_STACK_TOP: AtomicI32 = AtomicI32::new(0);

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

#[inline]
fn ref_state() -> lua::State {
    let ptr = REF_STATE.load(Ordering::Acquire);
    assert!(!ptr.is_null(), "RefThread not initialized!");
    lua::State(ptr)
}

fn stack_pop() -> i32 {
    let state = ref_state();
    let free = {
        let mut free_slots = FREE_SLOTS.lock().unwrap();
        free_slots.iter().next().map(|(&v, _)| v).map(|v| {
            free_slots.remove(&v);
            v
        })
    };
    if let Some(free) = free {
        ffi::lua_replace(state.0, free);
        free
    } else {
        REF_STACK_TOP.fetch_add(1, Ordering::AcqRel) + 1
    }
}

impl ValueRef {
    #[inline]
    pub(crate) fn new(index: i32) -> Self {
        Self {
            index,
            xrc_index: Some(ValueRefIndex(XRc::new(index))),
        }
    }

    pub(crate) fn push(&self, to: &lua::State) {
        ffi::lua_xpush(ref_state().0, to.0, self.index);
    }

    pub(crate) fn pop() -> Self {
        Self::new(stack_pop())
    }

    pub(crate) fn pop_from(from: &lua::State) -> Self {
        ffi::lua_xmove(from.0, ref_state().0, 1);
        Self::pop()
    }

    #[inline]
    pub(crate) fn ref_state(&self) -> lua::State {
        ref_state()
    }
}

impl Drop for ValueRef {
    fn drop(&mut self) {
        // It's guaranteed that the inner value returns exactly once.
        // This means in particular that the value is not dropped.
        if let Some(ValueRefIndex(xrc)) = self.xrc_index.take()
            && XRc::into_inner(xrc).is_some()
        {
            let index = self.index;
            FREE_SLOTS.lock().unwrap().insert(index, true);
            // Make sure we only access the ref_thread on the main thread.
            next_tick(move |_| {
                let state = ref_state().0;
                debug_assert!(
                    ffi::lua_gettop(state) >= index,
                    "GC finalizer is not allowed in ref_thread"
                );
                let mut free_slots = FREE_SLOTS.lock().unwrap();
                if let Some(&should_nil) = free_slots.get(&index)
                    && should_nil
                {
                    ffi::lua_pushnil(state);
                    ffi::lua_replace(state, index);
                    free_slots.insert(index, false);
                }
            });
        }
    }
}

impl fmt::Debug for ValueRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ValueRef({})", self.index)
    }
}

inventory::submit! {
    crate::open_close::new(
        0,
        "lua/reference",
        |l| {
            let thread = ffi::new_thread(l.0);
            // leak the reference thread so it doesn't get GC'd
            ffi::luaL_ref(l.0, ffi::LUA_REGISTRYINDEX);
            REF_STACK_TOP.store(0, Ordering::Release);
            REF_STATE.store(thread, Ordering::Release);
        },
        |_| {
            REF_STATE.store(std::ptr::null_mut(), Ordering::Release);
            FREE_SLOTS.lock().unwrap().clear();
        },
    )
}
