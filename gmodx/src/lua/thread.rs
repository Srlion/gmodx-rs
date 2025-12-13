// thread.rs
use crate::lua::{
    self, FromLua, FromLuaMulti, Function, StackGuard, ToLua, ToLuaMulti, Value, ffi,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ThreadStatus {
    Resumable,
    Running,
    Finished,
    Error,
}

#[derive(Clone, Copy)]
enum ThreadStatusInner {
    New(i32),
    Running,
    Yielded(i32),
    Finished,
    Error,
}

pub struct Thread(pub(crate) Value, pub(crate) lua::State);

#[cfg(feature = "send")]
unsafe impl Send for Thread {}
#[cfg(feature = "send")]
unsafe impl Sync for Thread {}

impl Clone for Thread {
    fn clone(&self) -> Self {
        Thread(self.0.clone(), self.1.clone())
    }
}

impl Thread {
    #[inline]
    pub fn state(&self, _: &lua::State) -> &lua::State {
        &self.1
    }

    pub fn resume<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        args: impl ToLuaMulti,
    ) -> lua::Result<R> {
        let mut pushed_nargs = match self.status_inner(state) {
            ThreadStatusInner::New(n) | ThreadStatusInner::Yielded(n) => n,
            _ => return Err(lua::Error::CoroutineUnresumable),
        };

        let thread_state = &self.1;
        let _sg = StackGuard::new(state.0);

        // Push args to main state then move to thread
        let nargs = args.push_to_stack_multi_count(state);
        if nargs > 0 {
            ffi::lua_xmove(state.0, thread_state.0, nargs);
            pushed_nargs += nargs;
        }

        // Resume and get results
        let ret = ffi::lua_resume(thread_state.0, pushed_nargs);
        let nresults = ffi::lua_gettop(thread_state.0);

        match ret {
            ffi::LUA_OK | ffi::LUA_YIELD => {
                if nresults > 0 {
                    ffi::lua_xmove(thread_state.0, state.0, nresults);
                }
                R::try_from_stack_multi(state, -nresults, nresults).map(|(v, _)| v)
            }
            _ => {
                // Error: pop error from thread stack
                if nresults > 0 {
                    ffi::lua_xmove(thread_state.0, state.0, 1);
                }
                Err(state.pop_error(ret))
            }
        }
    }

    pub fn status(&self, state: &lua::State) -> ThreadStatus {
        match self.status_inner(state) {
            ThreadStatusInner::New(_) | ThreadStatusInner::Yielded(_) => ThreadStatus::Resumable,
            ThreadStatusInner::Running => ThreadStatus::Running,
            ThreadStatusInner::Finished => ThreadStatus::Finished,
            ThreadStatusInner::Error => ThreadStatus::Error,
        }
    }

    fn status_inner(&self, state: &lua::State) -> ThreadStatusInner {
        let thread_state = &self.1;
        if thread_state.0 == state.0 {
            return ThreadStatusInner::Running;
        }
        let status = ffi::lua_status(thread_state.0);
        let top = ffi::lua_gettop(thread_state.0);
        match status {
            ffi::LUA_YIELD => ThreadStatusInner::Yielded(top),
            ffi::LUA_OK if top > 0 => ThreadStatusInner::New(top - 1),
            ffi::LUA_OK => ThreadStatusInner::Finished,
            _ => ThreadStatusInner::Error,
        }
    }

    pub fn reset(&self, state: &lua::State, func: Function) -> lua::Result<()> {
        let status = self.status_inner(state);
        match status {
            ThreadStatusInner::New(_) | ThreadStatusInner::Finished => {
                ffi::lua_settop(self.1.0, 0);
                func.push_to_stack(state);
                ffi::lua_xmove(state.0, self.1.0, 1);
                Ok(())
            }
            ThreadStatusInner::Running => {
                Err(lua::Error::Message("cannot reset running thread".into()))
            }
            _ => Err(lua::Error::Message(
                "cannot reset non-finished thread".into(),
            )),
        }
    }
}

impl lua::State {
    pub fn create_thread(&self, func: Function) -> Thread {
        let thread_ptr = ffi::new_thread(self.0);
        let thread_state = lua::State(thread_ptr);
        func.push_to_stack(&thread_state);
        Thread(Value::pop_from_stack(self), thread_state)
    }
}

impl ToLua for Thread {
    fn push_to_stack(self, state: &lua::State) {
        self.0.push_to_stack(state);
    }
}

impl ToLua for &Thread {
    fn push_to_stack(self, state: &lua::State) {
        (&self.0).push_to_stack(state);
    }
}

impl FromLua for Thread {
    fn try_from_stack(state: &lua::State, index: i32) -> lua::Result<Self> {
        match ffi::lua_type(state.0, index) {
            ffi::LUA_TTHREAD => {
                let thread_state = lua::State(ffi::lua_tothread(state.0, index));
                Ok(Thread(Value::from_stack(state, index), thread_state))
            }
            _ => Err(state.type_error(index, "thread")),
        }
    }
}
