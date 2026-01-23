use crate::{
    lua::{self, FromLua, FromLuaMulti, Function, State, ToLua, ToLuaMulti, Value, ffi},
    open_close::get_main_lua_state,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ThreadStatus {
    Resumable,
    Yielded,
    Running,
    Error,
}

pub struct Thread(pub(crate) Value, pub(crate) usize);

impl Clone for Thread {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}

impl Thread {
    #[must_use]
    #[inline]
    pub const fn state(&self, _: &lua::State) -> lua::State {
        lua::State::from_usize(self.1)
    }

    #[allow(unused)]
    #[inline]
    pub(crate) const fn lua_state(&self) -> lua::State {
        lua::State::from_usize(self.1)
    }

    pub fn resume<R: FromLuaMulti>(&self, l: &State, args: impl ToLuaMulti) -> lua::Result<R> {
        let thread_state = &self.state(l);

        Self::resume_impl(thread_state, l, args)?;

        let nresults = ffi::lua_gettop(l.0);
        R::try_from_stack_multi(thread_state, nresults + 1, nresults).map(|(v, _)| v)
    }

    pub(crate) fn resume_impl(
        thread_state: &lua::State,
        l: &State,
        args: impl ToLuaMulti,
    ) -> lua::Result<()> {
        match Self::status_impl(thread_state, l) {
            ThreadStatus::Resumable | ThreadStatus::Yielded => {}
            _ => return Err(lua::Error::CoroutineUnresumable),
        }

        let nargs = args.push_to_stack_multi_count(thread_state);
        let ret = ffi::lua_resume(thread_state.0, nargs);
        match ret {
            ffi::LUA_OK | ffi::LUA_YIELD => Ok(()),
            _ => Err(thread_state.pop_error(ret)),
        }
    }

    #[must_use]
    pub fn status(&self, l: &State) -> ThreadStatus {
        Self::status_impl(&self.state(l), l)
    }

    fn status_impl(thread_state: &lua::State, l: &State) -> ThreadStatus {
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
                ffi::lua_settop(self.state(l).0, 0);
                func.push_to_stack(&self.state(l));
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
    #[must_use]
    pub fn create_thread(&self, func: Function) -> Thread {
        let thread_ptr = ffi::new_thread(self.0);
        let thread_state = Self(thread_ptr);
        func.push_to_stack(&thread_state);
        Thread(Value::pop_from_stack(self), thread_state.as_usize())
    }

    #[must_use]
    pub fn is_thread(&self) -> bool {
        get_main_lua_state().0 != self.0
    }
}

impl ToLua for Thread {
    fn push_to_stack(self, l: &lua::State) {
        self.0.push_to_stack(l);
    }
}

impl ToLua for &Thread {
    fn push_to_stack(self, l: &lua::State) {
        (&self.0).push_to_stack(l);
    }
}

impl FromLua for Thread {
    fn try_from_stack(l: &lua::State, index: i32) -> lua::Result<Self> {
        match ffi::lua_type(l.0, index) {
            ffi::LUA_TTHREAD => {
                let thread_state = lua::State(ffi::lua_tothread(l.0, index));
                Ok(Self(Value::from_stack(l, index), thread_state.as_usize()))
            }
            _ => Err(l.type_error(index, "thread")),
        }
    }
}
