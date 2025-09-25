use crate::{
    lua::{self, CFunction},
    lua_shared,
};

unsafe extern "C" {
    pub fn get_call_rust_function() -> CFunction;
    pub fn get_call_rust_closure() -> CFunction;
    pub fn get_gettable_bridge() -> CFunction;
    pub fn get_settable_bridge() -> CFunction;
    pub fn set_bridge_callbacks(napi: *const lua::raw::BridgeCallbacks);
}

inventory::submit! {
    crate::open_close::new(
        1, // Load after lua_shared
        "lua/bridge",
        |_| unsafe {
            set_bridge_callbacks(&lua::raw::BridgeCallbacks {
                lua_error: Some(lua_shared::lua_shared().lua_error),
                lua_pushvalue: Some(lua_shared::lua_shared().lua_pushvalue),
                lua_gettable: Some(lua_shared::lua_shared().lua_gettable),
                lua_settable: Some(lua_shared::lua_shared().lua_settable),

                rust_function_callback: Some(lua::function::rust_function_callback),
                rust_closure_callback: Some(lua::function::rust_closure_callback),
            });
        },
        |_| unsafe {
            set_bridge_callbacks(std::ptr::null());
        },
    )
}
