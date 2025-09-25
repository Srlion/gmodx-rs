#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

#define LUA_TNONE (-1)
#define LUA_TNIL 0
#define LUA_TBOOLEAN 1
#define LUA_TLIGHTUSERDATA 2
#define LUA_TNUMBER 3
#define LUA_TSTRING 4
#define LUA_TTABLE 5
#define LUA_TFUNCTION 6
#define LUA_TUSERDATA 7
#define LUA_TTHREAD 8

#define LUA_MULTRET (-1)

#define LUA_OK 0
#define LUA_YIELD 1
#define LUA_ERRRUN 2
#define LUA_ERRSYNTAX 3
#define LUA_ERRMEM 4
#define LUA_ERRERR 5
#define LUA_ERRFILE (LUA_ERRERR + 1)

#define LUA_REGISTRYINDEX (-10000)
#define LUA_ENVIRONINDEX (-10001)
#define LUA_GLOBALSINDEX (-10002)
#define lua_upvalueindex(i) (LUA_GLOBALSINDEX - (i))

#define LUA_NUMBER double

typedef struct lua_State lua_State;
typedef double lua_Number;

typedef int (*lua_CFunction)(lua_State *L);

typedef struct luaL_Reg
{
    const char *name;
    lua_CFunction func;
} luaL_Reg;

#define LUA_IDSIZE 60
typedef struct lua_Debug
{
    int event;
    const char *name;           /* (n) */
    const char *namewhat;       /* (n) `global', `local', `field', `method' */
    const char *what;           /* (S) `Lua', `C', `main', `tail' */
    const char *source;         /* (S) */
    int currentline;            /* (l) */
    int nups;                   /* (u) number of upvalues */
    int linedefined;            /* (S) */
    int lastlinedefined;        /* (S) */
    char short_src[LUA_IDSIZE]; /* (S) */
    /* private part */
    int i_ci; /* active function */
} lua_Debug;

lua_State *luaL_newstate(void);
lua_State *lua_newthread(lua_State *);
int lua_gettop(lua_State *L);
void lua_settop(lua_State *L, int index);
void lua_pushvalue(lua_State *L, int index);
void lua_remove(lua_State *L, int index);
void lua_insert(lua_State *L, int index);
void lua_replace(lua_State *L, int index);
int lua_checkstack(lua_State *L, int extra);
int lua_type(lua_State *L, int index);
const char *lua_typename(lua_State *L, int tp);
int lua_equal(lua_State *L, int index1, int index2);
int lua_rawequal(lua_State *L, int index1, int index2);
int lua_lessthan(lua_State *L, int index1, int index2);
lua_Number lua_tonumber(lua_State *L, int index);
int lua_toboolean(lua_State *L, int index);
const char *lua_tolstring(lua_State *L, int index, size_t *len);
size_t lua_objlen(lua_State *L, int index);
lua_CFunction lua_tocfunction(lua_State *L, int index);
void *lua_touserdata(lua_State *L, int index);
lua_State *lua_tothread(lua_State *L, int index);
const void *lua_topointer(lua_State *L, int index);
void lua_pushnil(lua_State *L);
void lua_pushnumber(lua_State *L, lua_Number n);
void lua_pushlstring(lua_State *L, const char *s, size_t len);
void lua_pushcclosure(lua_State *L, lua_CFunction fn, int n);
void lua_pushboolean(lua_State *L, int b);
void lua_pushlightuserdata(lua_State *L, void *p);
int lua_pushthread(lua_State *L);
void lua_gettable(lua_State *L, int index);
void lua_getfield(lua_State *L, int index, const char *k);
void lua_rawget(lua_State *L, int index);
void lua_rawgeti(lua_State *L, int index, int n);
void lua_createtable(lua_State *L, int narr, int nrec);
void *lua_newuserdata(lua_State *L, size_t size);
int lua_getmetatable(lua_State *L, int index);
void lua_getfenv(lua_State *L, int index);
void lua_settable(lua_State *L, int index);
void lua_setfield(lua_State *L, int index, const char *k);
void lua_rawset(lua_State *L, int index);
void lua_rawseti(lua_State *L, int index, int n);
int lua_setmetatable(lua_State *L, int index);
int lua_setfenv(lua_State *L, int index);
void lua_call(lua_State *L, int nargs, int nresults);
int lua_pcall(lua_State *L, int nargs, int nresults, int errfunc);
int lua_cpcall(lua_State *L, lua_CFunction func, void *ud);
int lua_yield(lua_State *L, int nresults);
int lua_resume_real(lua_State *L, int narg);
int lua_status(lua_State *L);
int lua_error(lua_State *L);
int lua_next(lua_State *L, int index);
void lua_concat(lua_State *L, int n);
void luaL_openlibs(lua_State *L);
int luaL_callmeta(lua_State *L, int obj, const char *e);
int luaL_newmetatable(lua_State *L, const char *tname);
int luaL_ref(lua_State *L, int t);
void luaL_unref(lua_State *L, int t, int ref);
int luaL_loadbuffer(lua_State *L,
                    const char *buff,
                    size_t sz,
                    const char *name);
int luaL_loadbufferx(lua_State *L, const char *buff, size_t sz, const char *name, const char *mode);
int luaL_loadstring(lua_State *L, const char *s);
int luaL_loadfile(lua_State *L, const char *filename);
const char *luaL_findtable(lua_State *L, int idx, const char *fname, int szhint); /* Functions to be called by the debugger in specific events */
int lua_getstack(lua_State *L, int level, lua_Debug *ar);
int lua_getinfo(lua_State *L, const char *what, lua_Debug *ar);

// This is to pass function pointers from Rust to C
typedef struct
{
    int (*error)(lua_State *L);
    bool (*rust_function_callback)(lua_State *L, int *result);
    bool (*rust_closure_callback)(lua_State *L, int *result);
} ApiTable;
