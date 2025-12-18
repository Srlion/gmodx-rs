use crate::lua::{self, FromLua, FromLuaMulti, Function, State, ToLua, ToLuaMulti, Value, ffi};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ThreadStatus {
    Resumable,
    Yielded,
    Running,
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

    pub fn resume<R: FromLuaMulti>(&self, l: &State, args: impl ToLuaMulti) -> lua::Result<R> {
        self.resume_common(l, args)?;

        let thread_state = &self.1;
        let nresults = ffi::lua_gettop(thread_state.0);
        R::try_from_stack_multi(thread_state, -nresults, nresults).map(|(v, _)| v)
    }

    pub fn resume_void(&self, l: &State, args: impl ToLuaMulti) -> lua::Result<()> {
        self.resume_common(l, args)?;
        Ok(())
    }

    fn resume_common(&self, l: &State, args: impl ToLuaMulti) -> lua::Result<()> {
        match self.status(l) {
            ThreadStatus::Resumable | ThreadStatus::Yielded => {}
            _ => return Err(lua::Error::CoroutineUnresumable),
        };

        let thread_state = &self.1;
        let nargs = args.push_to_stack_multi_count(thread_state);
        let ret = ffi::lua_resume(thread_state.0, nargs);
        match ret {
            ffi::LUA_OK | ffi::LUA_YIELD => Ok(()),
            _ => Err(thread_state.pop_error(ret)),
        }
    }

    pub fn status(&self, l: &State) -> ThreadStatus {
        let thread_state = &self.1;
        if thread_state.0 == l.0 {
            return ThreadStatus::Running;
        }
        let status = ffi::lua_status(thread_state.0);
        // let top = ffi::lua_gettop(thread_state.0);
        match status {
            ffi::LUA_YIELD => ThreadStatus::Yielded,
            ffi::LUA_OK => ThreadStatus::Resumable,
            _ => ThreadStatus::Error,
        }
    }

    pub fn reset(&self, l: &State, func: Function) -> lua::Result<()> {
        let status = self.status(l);
        match status {
            ThreadStatus::Resumable => {
                ffi::lua_settop(self.1.0, 0);
                func.push_to_stack(&self.1);
                Ok(())
            }
            ThreadStatus::Running => Err(lua::Error::Message("cannot reset running thread".into())),
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
