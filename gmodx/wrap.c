// wrapper.c
#include "lua.h"

static ApiTable api = {NULL, NULL, NULL}; // Initialize all fields to NULL

void set_api_table(const ApiTable *napi)
{
    if (napi == NULL)
    {
        api.error = NULL;
        api.rust_function_callback = NULL;
        api.rust_closure_callback = NULL;
    }
    else
    {
        api = *napi; // Struct copy
    }
}

static int call_rust_function(lua_State *L)
{
    int function_result = 0;
    if (!api.rust_function_callback(L, &function_result))
    {
        api.error(L);
        return 0;
    }
    return function_result;
}

static int call_rust_closure(lua_State *L)
{
    int closure_result = 0;
    if (!api.rust_closure_callback(L, &closure_result))
    {
        api.error(L);
        return 0;
    }
    return closure_result;
}

lua_CFunction get_call_rust_function(void)
{
    return &call_rust_function;
}

lua_CFunction get_call_rust_closure(void)
{
    return &call_rust_closure;
}
