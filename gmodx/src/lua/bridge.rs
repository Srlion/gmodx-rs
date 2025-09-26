use crate::lua::{self, CFunction};

unsafe extern "C" {
    pub fn get_call_rust_function() -> CFunction;
    pub fn get_call_rust_closure() -> CFunction;
    pub fn get_gettable_bridge() -> CFunction;
    pub fn get_settable_bridge() -> CFunction;
    pub fn set_bridge_callbacks(napi: *const lua::raw::BridgeCallbacks);
}

pub fn setup(lua_shared: &lua::raw::LuaShared) {
    unsafe {
        set_bridge_callbacks(&lua::raw::BridgeCallbacks {
            lua_error: Some(lua_shared.lua_error),
            lua_pushvalue: Some(lua_shared.lua_pushvalue),
            lua_gettable: Some(lua_shared.lua_gettable),
            lua_settable: Some(lua_shared.lua_settable),

            rust_function_callback: Some(lua::function::rust_function_callback),
            rust_closure_callback: Some(lua::function::rust_closure_callback),
        });
    }
}
