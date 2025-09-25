use std::{mem::MaybeUninit, os::raw::c_void};

use crate::{
    cstr,
    lua::{self},
    lua_shared::{lua_State, lua_shared},
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State(pub *mut lua_State);

impl State {
    #[inline]
    pub fn is_nil(self, index: i32) -> bool {
        let t = self.get_type(index);
        t == lua::TNIL as i32 || t == lua::TNONE
    }

    #[inline]
    pub fn is_boolean(self, index: i32) -> bool {
        self.get_type(index) == lua::TBOOLEAN as i32
    }

    #[inline]
    pub fn is_bool(self, index: i32) -> bool {
        self.is_boolean(index)
    }

    #[inline]
    pub fn is_light_userdata(self, index: i32) -> bool {
        self.get_type(index) == lua::TLIGHTUSERDATA as i32
    }

    #[inline]
    pub fn is_number(self, index: i32) -> bool {
        self.get_type(index) == lua::TNUMBER as i32
    }

    #[inline]
    pub fn is_string(self, index: i32) -> bool {
        self.get_type(index) == lua::TSTRING as i32
    }

    #[inline]
    pub fn is_table(self, index: i32) -> bool {
        self.get_type(index) == lua::TTABLE as i32
    }

    #[inline]
    pub fn is_function(self, index: i32) -> bool {
        self.get_type(index) == lua::TFUNCTION as i32
    }

    #[inline]
    pub fn is_userdata(self, index: i32) -> bool {
        self.get_type(index) == lua::TUSERDATA as i32
    }

    #[inline]
    pub fn is_thread(self, index: i32) -> bool {
        self.get_type(index) == lua::TTHREAD as i32
    }
}

impl State {
    #[inline]
    pub fn check_table(self, index: i32) -> Result<(), lua::Error> {
        if self.is_table(index) {
            Ok(())
        } else {
            Err(lua::Error::Message(
                self.tag_error(index, lua::TTABLE as i32),
            ))
        }
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
    pub fn raw_to_userdata(self, index: i32) -> *mut c_void {
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
    pub fn raw_push_number(self, n: lua::Number) {
        unsafe { (lua_shared().lua_pushnumber)(self.0, n) }
    }

    #[inline]
    pub fn push_string(self, data: &str) {
        unsafe { (lua_shared().lua_pushlstring)(self.0, data.as_ptr() as *const i8, data.len()) }
    }

    #[inline]
    pub fn push_binary_string(self, data: &[u8]) {
        unsafe { (lua_shared().lua_pushlstring)(self.0, data.as_ptr() as *const i8, data.len()) }
    }

    #[inline]
    pub fn raw_push_cclosure(self, func: lua::CFunction, n: i32) {
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
    pub fn raw_seti(self, index: i32, n: i32) {
        unsafe { (lua_shared().lua_rawseti)(self.0, index, n) }
    }

    #[inline]
    pub fn raw_geti(self, index: i32, n: i32) {
        unsafe { (lua_shared().lua_rawgeti)(self.0, index, n) }
    }

    /// # SAFETY
    ///
    /// YOUR OWN SAFETY LOL
    #[inline]
    pub unsafe fn push_light_userdata(self, p: *mut c_void) {
        unsafe { (lua_shared().lua_pushlightuserdata)(self.0, p) }
    }

    #[inline]
    pub fn raw_get_field(self, index: i32, k: lua::CStr) {
        unsafe { (lua_shared().lua_getfield)(self.0, index, k.as_ptr()) }
    }

    #[inline]
    pub fn get_global(self, name: lua::CStr) {
        self.raw_get_field(lua::GLOBALSINDEX, name);
    }

    #[inline]
    pub fn set_field(self, index: i32, k: lua::CStr) {
        unsafe { (lua_shared().lua_setfield)(self.0, index, k.as_ptr()) }
    }

    #[inline(always)]
    pub fn set_global(self, name: lua::CStr) {
        self.set_field(lua::GLOBALSINDEX, name)
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
    pub fn raw_new_userdata(self, size: usize) -> *mut c_void {
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
    pub fn create_table(self, narr: i32, nrec: i32) {
        unsafe { (lua_shared().lua_createtable)(self.0, narr, nrec) }
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
    type Target = *mut lua_State;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<*mut lua_State> for State {
    #[inline]
    fn as_ref(&self) -> &*mut lua_State {
        &self.0
    }
}

impl From<State> for *mut lua_State {
    #[inline]
    fn from(val: State) -> Self {
        val.0
    }
}

impl From<*mut lua_State> for State {
    #[inline]
    fn from(ptr: *mut lua_State) -> Self {
        State(ptr)
    }
}
