#pragma once

#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

const int LUA_TNONE = -1;
const int LUA_TNIL = 0;
const int LUA_TBOOLEAN = 1;
const int LUA_TLIGHTUSERDATA = 2;
const int LUA_TNUMBER = 3;
const int LUA_TSTRING = 4;
const int LUA_TTABLE = 5;
const int LUA_TFUNCTION = 6;
const int LUA_TUSERDATA = 7;
const int LUA_TTHREAD = 8;

const int LUA_MULTRET = -1;

const int LUA_OK = 0;
const int LUA_YIELD = 1;
const int LUA_ERRRUN = 2;
const int LUA_ERRSYNTAX = 3;
const int LUA_ERRMEM = 4;
const int LUA_ERRERR = 5;
const int LUA_ERRFILE = LUA_ERRERR + 1;

const int LUA_REGISTRYINDEX = (-10000);
const int LUA_ENVIRONINDEX = (-10001);
const int LUA_GLOBALSINDEX = (-10002);
#define lua_upvalueindex(i) (LUA_GLOBALSINDEX - (i))

const int LUA_NOREF = (-2);
const int LUA_REFNIL = (-1);

#define LUA_NUMBER double

typedef struct lua_State lua_State;
typedef double lua_Number;

typedef int (*lua_CFunction)(lua_State *L);

typedef struct luaL_Reg
{
    const char *name;
    lua_CFunction func;
} luaL_Reg;

// Thanks to puffy, I finally can rest after chasing a UB for 3 days with almost no sleep
// GMOD MODIFIES THIS FROM 60 TO 128 AND FOR THAT WHOLE TIME NO ONE KNEW ABOUT IT
#define LUA_IDSIZE 128
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
void lua_close(lua_State *L);
void luaL_openlibs(lua_State *L);
void lua_xmove(lua_State *from, lua_State *to, int n);
int lua_gettop(lua_State *L);
void lua_settop(lua_State *L, int index);
void lua_pushvalue(lua_State *L, int index);
void lua_remove(lua_State *L, int index);
void lua_insert(lua_State *L, int index);
void lua_replace(lua_State *L, int index);
int lua_checkstack(lua_State *L, int extra);
int lua_type(lua_State *L, int index);
const char *lua_typename(lua_State *L, int tp, int idx);
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
void lua_pushstring(lua_State *L, const char *s);
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
int lua_pcall(lua_State *L, int nargs, int nresults, int errfunc);
int lua_cpcall(lua_State *L, lua_CFunction func, void *ud);
int lua_yield(lua_State *L, int nresults);
int lua_resume_real(lua_State *L, int narg);
int lua_status(lua_State *L);
int lua_error(lua_State *L);
int lua_next(lua_State *L, int index);
void lua_concat(lua_State *L, int n);
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
int luaL_getmetafield(lua_State *L, int obj, const char *e);
int lua_isnumber(lua_State *L, int index);
int luaL_error(lua_State *L, const char *fmt, ...);

#define abs_index(L, i) \
    ((i) > 0 || (i) <= LUA_REGISTRYINDEX ? (i) : lua_gettop(L) + (i) + 1)
