use crate::sync::{XCell, XRcCell, XRef, XRefMut};
use std::any::{TypeId, type_name};
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_void};
use std::rc::Rc;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::lua::function::IntoLuaFunction;
use crate::lua::traits::ObjectLike;
use crate::lua::types::Callback;
use crate::lua::{self, Result, ffi::lua_State};
use crate::lua::{Error, FromLua, FromLuaMulti, Function, Table, ToLua, ToLuaMulti, Value, ffi};

thread_local! {
    static TYPES: RefCell<FxHashMap<*const c_void, TypeId>> = const { RefCell::new(FxHashMap::with_hasher(FxBuildHasher)) };
}

type UserDataCell<T> = XRcCell<T>;

pub trait UserData: 'static {
    fn meta_methods(_: &mut MethodsBuilder) {}
    fn methods(_: &mut MethodsBuilder) {}

    fn name() -> &'static str {
        type_name::<Self>()
            .rsplit("::")
            .next()
            .unwrap_or(type_name::<Self>())
    }

    fn unique_id() -> Rc<CStr> {
        thread_local! {
            static IDS: RefCell<FxHashMap<TypeId, Rc<CStr>>> =
                const { RefCell::new(FxHashMap::with_hasher(FxBuildHasher)) };
        }
        let type_id = TypeId::of::<Self>();
        IDS.with_borrow_mut(|ids| {
            ids.entry(type_id)
                .or_insert_with(|| {
                    let cstring =
                        CString::new(format!("{}_{:?}", gmodx_macros::unique_id!(), type_id))
                            .unwrap();
                    Rc::from(cstring)
                })
                .clone()
        })
    }

    /// By default we lazily initialize the methods table.
    /// Use this function to initialize the methods table before it is used.
    fn init_methods_table(state: &lua::State) -> Table
    where
        Self: Sized,
    {
        push_methods_table::<Self>(state);
        Table(Value::pop_from_stack(state))
    }
}

impl<T: UserData> UserData for XCell<T> {}

fn push_methods_table<T: UserData>(state: &lua::State) {
    if ffi::luaL_newmetatable(state.0, T::unique_id().as_ptr()) {
        let mut mb = MethodsBuilder::new();
        T::methods(&mut mb);
        for (name, func) in mb.0 {
            state.create_function_impl(func).push_to_stack(state);
            ffi::lua_setfield(state.0, -2, name.as_ptr());
        }
    }
}

impl lua::State {
    pub fn create_userdata<T: UserData>(&self, ud: T) -> AnyUserData {
        // Userdata: 1
        let ud_ptr = ffi::lua_newuserdata(self.0, std::mem::size_of::<UserDataCell<T>>());

        let data = ud_ptr as *mut UserDataCell<T>;
        // SAFETY: We just created the userdata, so it's safe to write to it.
        unsafe {
            std::ptr::write_unaligned(data, UserDataCell::new(ud));
        }
        TYPES.with_borrow_mut(|types| {
            types.insert(ud_ptr, TypeId::of::<T>());
        });

        // UserData metatable: 2
        let mut mb = MethodsBuilder::new();
        T::meta_methods(&mut mb);

        ffi::lua_createtable(self.0, 0, mb.0.len() as i32);
        {
            for (name, func) in mb.0 {
                self.create_function_impl(func).push_to_stack(self);
                ffi::lua_setfield(self.0, -2, name.as_ptr());
            }

            extern "C-unwind" fn __gc<T: UserData>(state: *mut lua_State) -> i32 {
                let ud_ptr = ffi::lua_touserdata(state, -1);
                let ud_ptr = ud_ptr as *const c_void;

                let type_id = TYPES.with_borrow_mut(|types| types.remove(&ud_ptr));
                if type_id.is_none() {
                    return 0;
                }

                // cast back to UserDataCell<T> to drop it
                let ud = unsafe { std::ptr::read(ud_ptr as *mut UserDataCell<T>) };
                drop(ud);
                0
            }
            ffi::lua_pushcclosure(self.0, Some(__gc::<T>), 0);
            ffi::lua_setfield(self.0, -2, c"__gc".as_ptr());
        }

        // Store table: 3
        ffi::lua_createtable(self.0, 0, 0);

        // Store's metatable: 4
        ffi::lua_createtable(self.0, 0, 1);

        // Methods table: 5
        push_methods_table::<T>(self);

        // Set methods table as __index of store's metatable
        ffi::lua_setfield(self.0, -2, c"__index".as_ptr()); // pops methods table

        // Set store's metatable
        ffi::lua_setmetatable(self.0, -2); // pops store's metatable

        // Push store to have it as __index
        ffi::lua_pushvalue(self.0, -1);
        ffi::lua_setfield(self.0, -3, c"__index".as_ptr()); // sets on ud_meta

        // Set store as __newindex
        ffi::lua_setfield(self.0, -2, c"__newindex".as_ptr()); // pops store

        // Set userdata's metatable
        ffi::lua_setmetatable(self.0, -2);

        AnyUserData(Value::pop_from_stack(self))
    }
}

#[derive(Debug)]
pub struct UserDataRef<T: UserData> {
    ptr: usize,
    inner: Value, // to hold the userdata's value and be able to push it to the stack quickly
    _marker: std::marker::PhantomData<T>,
}

impl<T: UserData> UserDataRef<T> {
    #[inline]
    const fn downcast(&self) -> &UserDataCell<T> {
        // SAFETY: The pointer is valid as long as the inner value is alive.
        // SAFETY: We type check before initializing UserDataRef.
        unsafe { &*(self.ptr as *const UserDataCell<T>) }
    }

    #[inline]
    pub fn borrow(&self) -> XRef<'_, T> {
        self.downcast().borrow()
    }

    #[inline]
    pub fn borrow_mut(&self) -> XRefMut<'_, T> {
        self.downcast().borrow_mut()
    }

    #[inline]
    pub fn try_borrow(&self) -> Result<XRef<'_, T>> {
        self.downcast()
            .try_borrow()
            .map_err(|err| Error::Message(format!("cannot borrow '{}': {}", T::name(), err)))
    }

    #[inline]
    pub fn try_borrow_mut(&self) -> Result<XRefMut<'_, T>> {
        self.downcast().try_borrow_mut().map_err(|err| {
            Error::Message(format!("cannot borrow '{}' mutably: {}", T::name(), err))
        })
    }
}

impl<T: UserData> Clone for UserDataRef<T> {
    fn clone(&self) -> Self {
        UserDataRef {
            ptr: self.ptr,
            inner: self.inner.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: UserData> ToLua for UserDataRef<T> {
    fn push_to_stack(self, state: &lua::State) {
        self.inner.push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.inner
    }
}

impl<T: UserData> ToLua for &UserDataRef<T> {
    fn push_to_stack(self, state: &lua::State) {
        #[allow(clippy::needless_borrow)]
        (&self.inner).push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.inner.clone()
    }
}

impl<T: UserData> FromLua for UserDataRef<T> {
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        let name = T::name();
        let any = AnyUserData::from_stack_with_type(state, index, name)?;
        any.cast_to::<T>(state)
            .ok_or_else(|| state.type_error(index, name))
    }
}

#[derive(Clone, Debug)]
pub struct AnyUserData(pub(crate) Value);

// We have to force them to pass lua::State here to ensure they are on the main thread
// without having to check it each time nor have panics at runtime

impl AnyUserData {
    #[inline]
    fn ptr(&self) -> *const c_void {
        ffi::lua_touserdata(self.0.thread().0, self.0.index())
    }

    #[inline]
    pub fn is<T: UserData>(&self, _: &lua::State) -> bool {
        TYPES.with_borrow(|types| {
            types
                .get(&self.ptr())
                .is_some_and(|id| id == &TypeId::of::<T>())
        })
    }

    #[inline]
    pub fn cast_to<T: UserData>(self, state: &lua::State) -> Option<UserDataRef<T>> {
        if !self.is::<T>(state) {
            return None;
        }
        Some(UserDataRef {
            ptr: self.ptr() as usize,
            inner: self.0,
            _marker: std::marker::PhantomData,
        })
    }

    #[inline]
    fn from_stack_with_type(state: &lua::State, index: i32, type_name: &str) -> Result<Self> {
        if ffi::lua_type(state.0, index) == ffi::LUA_TUSERDATA {
            Ok(AnyUserData(Value::from_stack(state, index)))
        } else {
            Err(state.type_error(index, type_name))
        }
    }
}

impl<T: UserData> ToLua for T {
    fn push_to_stack(self, state: &lua::State) {
        state.create_userdata(self).push_to_stack(state);
    }
}

impl ToLua for AnyUserData {
    fn push_to_stack(self, state: &lua::State) {
        self.0.push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0
    }
}

impl ToLua for &AnyUserData {
    fn push_to_stack(self, state: &lua::State) {
        #[allow(clippy::needless_borrow)]
        (&self.0).push_to_stack(state);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.0.clone()
    }
}

impl FromLua for AnyUserData {
    #[inline]
    fn try_from_stack(state: &lua::State, index: i32) -> Result<AnyUserData> {
        AnyUserData::from_stack_with_type(state, index, "userdata")
    }
}

impl<T: UserData> From<UserDataRef<T>> for AnyUserData {
    fn from(udref: UserDataRef<T>) -> Self {
        AnyUserData(udref.inner)
    }
}

impl ObjectLike for AnyUserData {
    fn get<V: FromLua>(&self, state: &lua::State, key: impl ToLua) -> Result<V> {
        Table(self.0.clone()).get_protected(state, key)
    }

    fn set(&self, state: &lua::State, key: impl ToLua, value: impl ToLua) -> Result<()> {
        Table(self.0.clone()).set_protected(state, key, value)
    }

    #[inline]
    fn call<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> Result<R> {
        let func: Function = self.get(state, name)?;
        func.call(state, args)
    }

    #[inline]
    fn call_method<R: FromLuaMulti>(
        &self,
        state: &lua::State,
        name: &str,
        args: impl ToLuaMulti,
    ) -> Result<R> {
        self.call(state, name, (self, args))
    }
}

type Methods = Vec<(&'static CStr, Callback)>;

#[derive(Default)]
pub struct MethodsBuilder(Methods);

impl MethodsBuilder {
    fn new() -> Self {
        Self(Vec::new())
    }

    pub fn add<F, Marker>(&mut self, name: &'static CStr, func: F)
    where
        F: IntoLuaFunction<Marker>,
    {
        let callback = func.into_callback();
        self.0.push((name, callback));
    }
}
