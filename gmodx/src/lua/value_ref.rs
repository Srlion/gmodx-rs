use std::{
    fmt,
    mem::ManuallyDrop,
    sync::{
        Mutex,
        atomic::{AtomicPtr, Ordering},
    },
};

use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::{
    lua::{self, ffi},
    next_tick,
    sync::XRc,
};

struct SlotPool {
    available: Vec<i32>,         // nil'd and ready to reuse
    pending_nil: FxHashSet<i32>, // dropped, awaiting nil on main thread
    stack_top: i32,
}

impl SlotPool {
    pub const fn new() -> Self {
        Self {
            available: Vec::new(),
            pending_nil: FxHashSet::with_hasher(FxBuildHasher),
            stack_top: 0,
        }
    }

    /// returns (index, is_reused)
    fn take(&mut self) -> (i32, bool) {
        self.pending_nil
            .iter()
            .next()
            .copied()
            .map(|i| {
                self.pending_nil.remove(&i);
                (i, true)
            })
            .or_else(|| self.available.pop().map(|i| (i, true)))
            .unwrap_or_else(|| {
                self.stack_top += 1;
                (self.stack_top, false)
            })
    }
}

static SLOTS: Mutex<SlotPool> = Mutex::new(SlotPool::new());
static REF_STATE: AtomicPtr<ffi::lua_State> = AtomicPtr::new(std::ptr::null_mut());

#[derive(Clone)]
pub struct ValueRef {
    // Same as xrc_index but to have faster access
    pub(crate) index: i32,
    /// A reference to a Lua value index in the auxiliary thread.
    /// It's cheap to clone and can be used to track the number of references to a value.
    pub(crate) xrc: ManuallyDrop<XRc<i32>>,
}

#[inline]
fn ref_state() -> lua::State {
    let ptr = REF_STATE.load(Ordering::Acquire);
    assert!(!ptr.is_null(), "RefThread not initialized!");
    lua::State(ptr)
}

impl ValueRef {
    #[inline]
    pub(crate) fn new(index: i32) -> Self {
        Self {
            index,
            xrc: ManuallyDrop::new(XRc::new(index)),
        }
    }

    pub(crate) fn push(&self, to: &lua::State) {
        ffi::lua_xpush(ref_state().0, to.0, self.index);
    }

    pub(crate) fn pop() -> Self {
        let state = ref_state();
        let mut slots = SLOTS.lock().unwrap();
        let (index, reused) = slots.take();
        if reused {
            ffi::lua_replace(state.0, index);
        }
        drop(slots);
        Self::new(index)
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
        let xrc = unsafe { ManuallyDrop::take(&mut self.xrc) };
        let Some(index) = XRc::into_inner(xrc) else {
            return;
        };

        SLOTS.lock().unwrap().pending_nil.insert(index);

        next_tick(move |_| {
            let state = ref_state().0;
            debug_assert!(
                ffi::lua_gettop(state) >= index,
                "GC finalizer is not allowed in ref_thread"
            );
            let mut slots = SLOTS.lock().unwrap();
            if slots.pending_nil.remove(&index) {
                ffi::lua_pushnil(state);
                ffi::lua_replace(state, index);
                slots.available.push(index);
            }
        });
    }
}

impl fmt::Debug for ValueRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ValueRef({})", self.index)
    }
}

inventory::submit! {
    crate::open_close::new(
        -999,
        "lua/reference",
        |l| {
            let thread = ffi::new_thread(l.0);
            // leak the reference thread so it doesn't get GC'd
            ffi::luaL_ref(l.0, ffi::LUA_REGISTRYINDEX);
            REF_STATE.store(thread, Ordering::Release);
        },
        |_| {
            REF_STATE.store(std::ptr::null_mut(), Ordering::Release);
            *SLOTS.lock().unwrap() = SlotPool::new();
        },
    )
}
