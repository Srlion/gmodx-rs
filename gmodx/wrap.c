// wrapper.c
#include "lua.h"

static ApiTable api = {NULL, NULL}; // Initialize all fields to NULL

void set_api_table(const ApiTable *napi)
{
    if (napi == NULL)
    {
        api.error = NULL;
        api.rust_lua_callback = NULL;
    }
    else
    {
        api = *napi; // Struct copy
    }
}

static int result = 0;
static int lua_call_rust(lua_State *L)
{
    if (!api.rust_lua_callback(L, &result))
    {
        api.error(L);
        return 0;
    }
    return result;
}

lua_CFunction get_lua_call_rust(void)
{
    return &lua_call_rust;
}
