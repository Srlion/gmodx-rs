use std::{mem::MaybeUninit, os::raw::c_void};

use crate::{
    cstr,
    lua::{self},
    lua_shared::lua_shared,
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State(pub *mut lua::raw::lua_State);

impl State {
    #[inline]
    pub fn is_nil(self, index: i32) -> bool {
        let t = self.get_type(index);
        t == lua::TNIL || t == lua::TNONE
    }

    #[inline]
    pub fn is_boolean(self, index: i32) -> bool {
        self.get_type(index) == lua::TBOOLEAN
    }

    #[inline]
    pub fn is_bool(self, index: i32) -> bool {
        self.is_boolean(index)
    }

    #[inline]
    pub fn is_light_userdata(self, index: i32) -> bool {
        self.get_type(index) == lua::TLIGHTUSERDATA
    }

    #[inline]
    pub fn is_number(self, index: i32) -> bool {
        self.get_type(index) == lua::TNUMBER
    }

    #[inline]
    pub fn is_string(self, index: i32) -> bool {
        self.get_type(index) == lua::TSTRING
    }

    #[inline]
    pub fn is_table(self, index: i32) -> bool {
        self.get_type(index) == lua::TTABLE
    }

    #[inline]
    pub fn is_function(self, index: i32) -> bool {
        self.get_type(index) == lua::TFUNCTION
    }

    #[inline]
    pub fn is_userdata(self, index: i32) -> bool {
        self.get_type(index) == lua::TUSERDATA
    }

    #[inline]
    pub fn is_thread(self, index: i32) -> bool {
        self.get_type(index) == lua::TTHREAD
    }
}

impl State {
    #[inline]
    pub fn check_table(self, index: i32) -> Result<(), lua::Error> {
        if self.is_table(index) {
            Ok(())
        } else {
            Err(lua::Error::Message(self.tag_error(index, lua::TTABLE)))
        }
    }
}

impl State {
    #[inline]
    pub unsafe fn unsafe_get_table(self, index: i32) {
        unsafe { (lua_shared().lua_gettable)(self.0, index) }
    }

    pub fn get_table(&self, index: i32) -> Result<(), lua::Error> {
        let index = self.abs_index(index);

        if self.get_meta_field(index, c"__index") == 0 {
            // SAFETY: Just checked there is no __index metamethod
            unsafe { self.unsafe_get_table(index) };
            return Ok(());
        }
        // Remove the __index metamethod we just pushed
        self.pop();

        // Push the table that we are indexing
        self.push_value(index); // ... key table

        // Swap them
        self.insert(-2); // ... table key

        // Push the bridge to avoid longjmp causing UB
        self.push_cclosure(unsafe { lua::bridge::get_gettable_bridge() }, 0); // ... table key bridge

        // Reorder to: bridge table key
        self.insert(-3); // move top (bridge) to -3: ... bridge table key

        self.direct_pcall(2, 1, 0)
    }

    /// Gets a field from a Lua table. Unsafe: can longjmp via __index metamethod.
    #[inline]
    pub unsafe fn unsafe_get_field(self, index: i32, k: lua::CStr) {
        unsafe { (lua_shared().lua_getfield)(self.0, index, k.as_ptr()) }
    }

    pub fn get_field(&self, index: i32, key: lua::CStr) -> Result<(), lua::Error> {
        let index = self.abs_index(index);

        if self.get_meta_field(index, c"__index") == 0 {
            // SAFETY: Just checked there is no __index metamethod
            unsafe { self.unsafe_get_field(index, key) };
            return Ok(());
        }
        // Remove the __index metamethod we just pushed
        self.pop();

        // Push the bridge that we are going to use to avoid longjmp causing any UB to us
        self.push_cclosure(unsafe { lua::bridge::get_gettable_bridge() }, 0);
        // Push the table that we are indexing
        self.push_value(index);
        // Push the key
        self.push_cstring(key);

        self.direct_pcall(2, 1, 0)
    }
}

impl State {
    #[inline]
    pub unsafe fn unsafe_set_table(self, index: i32) {
        unsafe { (lua_shared().lua_settable)(self.0, index) }
    }

    pub fn set_table(&self, index: i32) -> Result<(), lua::Error> {
        let index = self.abs_index(index);

        if self.get_meta_field(index, c"__newindex") == 0 {
            // SAFETY: Just checked there is no __newindex metamethod
            unsafe { self.unsafe_set_table(index) };
            return Ok(());
        }
        // Remove the __newindex metamethod we just pushed
        self.pop();

        // Push the table we're setting into
        self.push_value(index); // ... key value table

        // Push the bridge to avoid longjmp causing UB
        self.push_cclosure(unsafe { lua::bridge::get_settable_bridge() }, 0); // ... key value table bridge

        // Reorder to: bridge table key value
        self.insert(-4); // move top (bridge) to -4: ... bridge key value table
        self.insert(-3); // move top (table)  to -3: ... bridge table key value

        self.direct_pcall(3, 0, 0)
    }

    #[inline]
    pub unsafe fn unsafe_set_field(self, index: i32, k: lua::CStr) {
        unsafe { (lua_shared().lua_setfield)(self.0, index, k.as_ptr()) }
    }

    pub fn set_field(&self, index: i32, key: lua::CStr) -> Result<(), lua::Error> {
        let index = self.abs_index(index);

        if self.get_meta_field(index, c"__newindex") == 0 {
            // SAFETY: Just checked there is no __newindex metamethod
            unsafe { self.unsafe_set_field(index, key) };
            return Ok(());
        }
        // Remove the __newindex metamethod we just pushed
        self.pop();

        // Push the table and key
        self.push_value(index); // ... value table
        self.push_cstring(key); // ... value table key

        // Push the bridge that avoids longjmp causing UB
        self.push_cclosure(unsafe { lua::bridge::get_settable_bridge() }, 0); // ... value table key bridge

        // Reorder to: bridge table key value
        self.insert(-4); // move top (bridge) to -4: ... bridge value table key
        self.insert(-3); // move top (key)    to -3: ... bridge key value table
        self.insert(-3); // move top (table)  to -3: ... bridge table key value

        self.direct_pcall(3, 0, 0)
    }
}

impl State {
    #[inline]
    pub fn raw_get(self, index: i32) {
        unsafe { (lua_shared().lua_rawget)(self.0, index) }
    }

    pub fn raw_get_field(self, index: i32, k: lua::CStr) {
        let index = self.abs_index(index);

        self.push_cstring(k); // ... table key
        self.raw_get(index);
    }
}

impl State {
    #[inline]
    pub fn raw_set(self, index: i32) {
        unsafe { (lua_shared().lua_rawset)(self.0, index) }
    }

    pub fn raw_set_field(self, index: i32, k: lua::CStr) {
        let index = self.abs_index(index);

        self.push_cstring(k);
        // Move the key below the value
        self.insert(-2); // ... table ... key value
        self.raw_set(index);
    }
}

impl State {
    #[inline]
    pub fn new() -> Result<Self, lua::Error> {
        let state = unsafe { (lua_shared().luaL_newstate)() };
        if state.is_null() {
            Err(lua::Error::MemoryAllocation)
        } else {
            Ok(Self(state))
        }
    }

    #[inline]
    pub fn new_thread(self) -> Self {
        unsafe { (lua_shared().lua_newthread)(self.0) }.into()
    }

    #[inline]
    pub fn get_top(self) -> i32 {
        unsafe { (lua_shared().lua_gettop)(self.0) }
    }

    #[inline]
    pub fn set_top(self, new_top: i32) {
        unsafe { (lua_shared().lua_settop)(self.0, new_top) }
    }

    #[inline]
    pub fn push_value(self, index: i32) {
        unsafe { (lua_shared().lua_pushvalue)(self.0, index) }
    }

    #[inline]
    pub fn remove(self, index: i32) {
        unsafe { (lua_shared().lua_remove)(self.0, index) }
    }

    #[inline]
    pub fn insert(self, index: i32) {
        unsafe { (lua_shared().lua_insert)(self.0, index) }
    }

    #[inline]
    pub fn replace(self, index: i32) {
        unsafe { (lua_shared().lua_replace)(self.0, index) }
    }

    #[inline]
    pub fn check_stack(self, extra: i32) -> bool {
        unsafe { (lua_shared().lua_checkstack)(self.0, extra) != 0 }
    }

    #[inline]
    pub fn get_type(self, index: i32) -> i32 {
        unsafe { (lua_shared().lua_type)(self.0, index) }
    }

    #[inline]
    pub fn type_name(self, tp: i32) -> String {
        let tp_str = unsafe {
            let c_str = (lua_shared().lua_typename)(self.0, tp);
            if c_str.is_null() {
                eprintln!(
                    "[gmodx] Warning: lua_typename returned null for type {}",
                    tp
                );
                return "<null>".to_string();
            }
            std::ffi::CStr::from_ptr(c_str)
        };
        tp_str.to_string_lossy().into_owned()
    }

    #[inline]
    pub fn equal(self, index1: i32, index2: i32) -> bool {
        unsafe { (lua_shared().lua_equal)(self.0, index1, index2) == 1 }
    }

    #[inline]
    pub fn raw_equal(self, index1: i32, index2: i32) -> bool {
        unsafe { (lua_shared().lua_rawequal)(self.0, index1, index2) == 1 }
    }

    #[inline]
    pub fn less_than(self, index1: i32, index2: i32) -> bool {
        unsafe { (lua_shared().lua_lessthan)(self.0, index1, index2) == 1 }
    }

    #[inline]
    pub fn to_number(self, index: i32) -> lua::Number {
        unsafe { (lua_shared().lua_tonumber)(self.0, index) }
    }

    #[inline]
    pub fn to_boolean(self, index: i32) -> bool {
        unsafe { (lua_shared().lua_toboolean)(self.0, index) == 1 }
    }

    #[inline]
    pub fn to_bool(self, index: i32) -> bool {
        self.to_boolean(index)
    }

    #[inline]
    pub fn to_binary_string(self, index: i32) -> Option<Vec<u8>> {
        if !self.is_string(index) {
            return None;
        }

        let mut len: usize = 0;
        let ptr = unsafe { (lua_shared().lua_tolstring)(self.0, index, &mut len) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
            Some(slice.to_owned())
        }
    }

    #[inline]
    pub fn to_string(self, index: i32) -> Option<String> {
        if !self.is_string(index) {
            return None;
        }

        let mut len: usize = 0;
        let ptr = unsafe { (lua_shared().lua_tolstring)(self.0, index, &mut len) };
        if ptr.is_null() {
            return None;
        }

        let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
        match std::str::from_utf8(bytes) {
            Ok(s) => Some(s.to_owned()),
            Err(_) => Some(String::from_utf8_lossy(bytes).into_owned()),
        }
    }

    #[inline]
    pub fn len(self, index: i32) -> usize {
        unsafe { (lua_shared().lua_objlen)(self.0, index) }
    }

    #[inline]
    pub fn to_cfunction(self, index: i32) -> lua::CFunction {
        unsafe { (lua_shared().lua_tocfunction)(self.0, index) }
    }

    #[inline]
    pub fn direct_to_userdata(self, index: i32) -> *mut c_void {
        unsafe { (lua_shared().lua_touserdata)(self.0, index) }
    }

    #[inline]
    pub fn to_thread(self, index: i32) -> Option<State> {
        let state = unsafe { (lua_shared().lua_tothread)(self.0, index) };
        if state.is_null() {
            None
        } else {
            Some(State(state))
        }
    }

    #[inline]
    pub fn to_pointer(self, index: i32) -> *const c_void {
        unsafe { (lua_shared().lua_topointer)(self.0, index) }
    }

    #[inline]
    pub fn push_nil(self) {
        unsafe { (lua_shared().lua_pushnil)(self.0) }
    }

    #[inline]
    pub fn direct_push_number(self, n: lua::Number) {
        unsafe { (lua_shared().lua_pushnumber)(self.0, n) }
    }

    #[inline]
    pub fn push_string(self, data: &str) {
        unsafe { (lua_shared().lua_pushlstring)(self.0, data.as_ptr() as *const i8, data.len()) }
    }

    #[inline]
    pub fn push_cstring(self, data: lua::CStr) {
        unsafe { (lua_shared().lua_pushstring)(self.0, data.as_ptr()) }
    }

    #[inline]
    pub fn push_binary_string(self, data: &[u8]) {
        unsafe { (lua_shared().lua_pushlstring)(self.0, data.as_ptr() as *const i8, data.len()) }
    }

    /// Pushes a C closure to the stack. Unsafe: use safe `push_closure` instead.
    #[inline]
    pub fn push_cclosure(self, func: lua::CFunction, n: i32) {
        unsafe { (lua_shared().lua_pushcclosure)(self.0, func, n) }
    }

    #[inline]
    pub fn push_boolean(self, b: bool) {
        unsafe { (lua_shared().lua_pushboolean)(self.0, if b { 1 } else { 0 }) }
    }

    #[inline]
    pub fn push_bool(self, b: bool) {
        self.push_boolean(b)
    }

    #[inline]
    fn abs_index(&self, i: i32) -> i32 {
        if i > 0 || i <= lua::REGISTRYINDEX {
            i
        } else {
            self.get_top() + i + 1
        }
    }

    /// # SAFETY
    ///
    /// YOUR OWN SAFETY LOL
    #[inline]
    pub unsafe fn push_light_userdata(self, p: *mut c_void) {
        unsafe { (lua_shared().lua_pushlightuserdata)(self.0, p) }
    }

    #[inline]
    pub fn push_thread(self) -> i32 {
        unsafe { (lua_shared().lua_pushthread)(self.0) }
    }

    #[inline]
    pub fn raw_seti(self, index: i32, n: i32) {
        unsafe { (lua_shared().lua_rawseti)(self.0, index, n) }
    }

    #[inline]
    pub fn raw_geti(self, index: i32, n: i32) {
        unsafe { (lua_shared().lua_rawgeti)(self.0, index, n) }
    }

    #[inline]
    pub fn get_global(self, name: lua::CStr) {
        if self.get_field(lua::GLOBALSINDEX, name).is_err() {
            // why would a bitch even do this?
            self.push_nil();
        }
    }

    #[inline(always)]
    pub fn set_global(self, name: lua::CStr) {
        let _ = self.set_field(lua::GLOBALSINDEX, name);
    }

    #[inline]
    pub fn pop_n(self, n: i32) {
        self.set_top(-n - 1);
    }

    #[inline]
    pub fn pop(self) {
        self.pop_n(1);
    }

    #[inline]
    pub fn direct_new_userdata(self, size: usize) -> *mut c_void {
        unsafe { (lua_shared().lua_newuserdata)(self.0, size) }
    }

    /// Returns true if the metatable was created, false if it already existed
    /// Either way, the metatable gets pushed onto the stack
    #[inline]
    pub fn new_metatable(self, tname: lua::CStr) -> bool {
        unsafe { (lua_shared().luaL_newmetatable)(self.0, tname.as_ptr()) == 1 }
    }

    #[inline]
    pub fn set_metatable(self, index: i32) -> i32 {
        unsafe { (lua_shared().lua_setmetatable)(self.0, index) }
    }

    #[inline]
    pub fn get_meta_field(self, index: i32, e: lua::CStr) -> i32 {
        unsafe { (lua_shared().luaL_getmetafield)(self.0, index, e.as_ptr()) }
    }

    #[inline]
    pub fn create_table(self, narr: i32, nrec: i32) {
        unsafe { (lua_shared().lua_createtable)(self.0, narr, nrec) }
    }

    #[inline]
    pub fn direct_pcall(
        self,
        n_args: i32,
        n_results: i32,
        err_func: i32,
    ) -> Result<(), lua::Error> {
        let err_code = unsafe { (lua_shared().lua_pcall)(self.0, n_args, n_results, err_func) };
        if err_code == lua::OK {
            Ok(())
        } else {
            Err(lua::Error::from_lua_state(self, err_code))
        }
    }

    #[inline]
    pub fn direct_pcall_ignore(self, n_args: i32, n_results: i32) -> bool {
        if let Err(e) = self.direct_pcall(n_args, n_results, 0) {
            self.error_no_halt(&e.to_string(), None);
            false
        } else {
            true
        }
    }

    pub fn pcall<F>(self, callback: F) -> Result<(), lua::Error>
    where
        F: FnOnce() -> u16,
    {
        if !self.is_function(-1) {
            return Err(lua::Error::NotAFunction);
        }
        let top = self.get_top();
        let nresults = callback();
        let n_args = self.get_top() - top;
        self.direct_pcall(n_args, nresults as i32, 0)
    }

    pub fn pcall_ignore<F>(self, callback: F) -> bool
    where
        F: FnOnce() -> u16,
    {
        if let Err(err) = self.pcall(callback) {
            self.error_no_halt(&err.to_string(), None);
            false
        } else {
            true
        }
    }

    pub fn debug_getinfo_at(self, level: i32, what: lua::CStr) -> Option<lua::Debug> {
        // SAFETY: lua::Debug must be a POD type that can be safely zero-initialized.
        // This is typically true for C structs from Lua's API.
        let mut ar = MaybeUninit::<lua::Debug>::uninit();

        unsafe {
            // First, check if we can get the stack level
            if (lua_shared().lua_getstack)(self.0, level, ar.as_mut_ptr()) == 0 {
                return None;
            }

            // SAFETY: lua_getstack has initialized the activation record partially.
            // lua_getinfo will complete the initialization based on 'what' string.
            if (lua_shared().lua_getinfo)(self.0, what.as_ptr(), ar.as_mut_ptr()) == 0 {
                return None;
            }

            // SAFETY: Both lua_getstack and lua_getinfo have succeeded.
            // According to Lua's documentation, the lua::Debug structure is now
            // fully initialized for the fields requested by 'what'.
            Some(ar.assume_init())
        }
    }

    #[cold]
    pub fn dump_stack(self) {
        let top = self.get_top();
        println!("\n=== STACK DUMP ===");
        println!("Stack size: {}", top);
        for i in 1..=top {
            let lua_type = self.get_type(i);
            let lua_type_name = self.type_name(lua_type);
            match lua_type_name.as_ref() {
                "string" => println!("{}. {}: {:?}", i, lua_type_name, {
                    self.push_value(i);
                    let str = self.to_string(-1);
                    self.pop();
                    str
                }),
                "boolean" => println!("{}. {}: {:?}", i, lua_type_name, {
                    self.push_value(i);
                    let bool = self.to_bool(-1);
                    self.pop();
                    bool
                }),
                "number" => println!("{}. {}: {:?}", i, lua_type_name, {
                    self.push_value(i);
                    let n = self.to_number(-1);
                    self.pop();
                    n
                }),
                _ => println!("{}. {}", i, lua_type_name),
            }
        }
        println!();
    }
}

impl State {
    pub fn error_no_halt(self, err: &str, traceback: Option<&str>) {
        let formatted_err = match traceback {
            Some(tb) => {
                self.get_global(c"ErrorNoHalt");
                format!("[ERROR] {}\n{}", err, tb)
            }
            None => {
                self.get_global(c"ErrorNoHaltWithStack");
                format!("[ERROR] {}", err)
            }
        };

        // Try to call the Lua error handler, fallback to stderr if unavailable
        if self.is_nil(-1) {
            self.pop();
            eprintln!("{}", formatted_err);
        } else {
            self.push_string(&formatted_err);
            if self.direct_pcall(1, 0, 0).is_err() {
                // Lua call failed, fallback to stderr
                eprintln!("{}", formatted_err);
            }
        }
    }

    pub fn type_error(self, narg: i32, tname: &str) -> String {
        let err = format!(
            "{} expected, got {}",
            tname,
            self.type_name(self.get_type(narg))
        );
        self.err_argmsg(narg, &err)
    }

    pub fn tag_error(self, narg: i32, tag: i32) -> String {
        self.type_error(narg, &self.type_name(tag))
    }

    pub fn err_argmsg(self, mut narg: i32, msg: &str) -> String {
        let mut fname = "?";
        let mut namewhat: Option<&str> = None;

        if let Some(ar) = self.debug_getinfo_at(0, c"n") {
            if !ar.name.is_null() {
                fname = cstr!(ar.name);
            }
            if !ar.namewhat.is_null() {
                namewhat = Some(cstr!(ar.namewhat));
            }
        }

        if narg < 0 && narg > lua::REGISTRYINDEX {
            narg = self.get_top() + narg + 1;
        }

        if let Some(namewhat) = namewhat
            && namewhat == "method"
            && {
                narg -= 1;
                narg == 0
            }
        {
            return format!("bad self parameter in method '{}' ({})", fname, msg);
        }

        format!("bad argument #{} to '{}' ({})", narg, fname, msg)
    }
}

impl State {
    #[inline]
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl std::ops::Deref for State {
    type Target = *mut lua::raw::lua_State;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<*mut lua::raw::lua_State> for State {
    #[inline]
    fn as_ref(&self) -> &*mut lua::raw::lua_State {
        &self.0
    }
}

impl From<State> for *mut lua::raw::lua_State {
    #[inline]
    fn from(val: State) -> Self {
        val.0
    }
}

impl From<*mut lua::raw::lua_State> for State {
    #[inline]
    fn from(ptr: *mut lua::raw::lua_State) -> Self {
        State(ptr)
    }
}
