#include "lua.h"

static BridgeCallbacks callbacks = {0}; // Initialize all fields to NULL

void set_bridge_callbacks(const BridgeCallbacks *napi)
{
    callbacks = napi ? *napi : (BridgeCallbacks){0};
}

static int call_rust_function(lua_State *L)
{
    int function_result = 0;
    if (!callbacks.rust_function_callback(L, &function_result))
    {
        callbacks.lua_error(L);
        return 0;
    }
    return function_result;
}
lua_CFunction get_call_rust_function(void) { return &call_rust_function; }

static int call_rust_closure(lua_State *L)
{
    int closure_result = 0;
    if (!callbacks.rust_closure_callback(L, &closure_result))
    {
        callbacks.lua_error(L);
        return 0;
    }
    return closure_result;
}
lua_CFunction get_call_rust_closure(void) { return &call_rust_closure; }

static int gettable_bridge(lua_State *L)
{
    callbacks.lua_pushvalue(L, 2);
    callbacks.lua_gettable(L, 1);
    return 1;
}
lua_CFunction get_gettable_bridge(void) { return &gettable_bridge; }

static int settable_bridge(lua_State *L)
{
    callbacks.lua_settable(L, 1);
    return 0;
}
lua_CFunction get_settable_bridge(void) { return &settable_bridge; }
