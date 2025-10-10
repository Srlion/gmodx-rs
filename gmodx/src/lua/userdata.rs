use std::any::{Any, TypeId, type_name};
use std::cell::{Ref, RefCell, RefMut};
use std::ffi::{CStr, CString, c_void};
use std::rc::Rc;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::lua::function::IntoLuaFunction;
use crate::lua::traits::ObjectLike;
use crate::lua::types::Callback;
use crate::lua::{self, Result, ffi::lua_State};
use crate::lua::{Error, FromLua, FromLuaMulti, Function, Table, ToLua, ToLuaMulti, Value, ffi};

thread_local! {
    static OBJECTS: RefCell<FxHashMap<*mut c_void, Rc<dyn Any>>> =
        const { RefCell::new(FxHashMap::with_hasher(FxBuildHasher)) };
}

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
        IDS.with_borrow_mut(|ids| {
            ids.entry(std::any::TypeId::of::<Self>())
                .or_insert_with(|| {
                    let cstring = CString::new(format!(
                        "{}_{:?}",
                        gmodx_macros::unique_id!(),
                        std::any::TypeId::of::<Self>()
                    ))
                    .unwrap();
                    Rc::from(cstring)
                })
                .clone()
        })
    }
}

impl lua::State {
    pub fn create_userdata<T: UserData>(&self, ud: T) -> AnyUserData {
        // Userdata: 1
        let ud_ptr = ffi::lua_newuserdata(self.0, 0);

        OBJECTS.with_borrow_mut(|objects| {
            let boxed: Rc<dyn Any> = Rc::new(RefCell::new(ud));
            objects.insert(ud_ptr, boxed);
        });

        // UserData metatable: 2
        let meta_methods = {
            let mut mb = MethodsBuilder::new();
            T::meta_methods(&mut mb);
            mb.build()
        };
        ffi::lua_createtable(self.0, 0, meta_methods.len() as i32);
        {
            for (name, func) in meta_methods.into_iter() {
                self.create_function_impl(func).push_to_stack(self);
                ffi::lua_setfield(self.0, -2, name.as_ptr());
            }

            extern "C-unwind" fn __gc(state: *mut lua_State) -> i32 {
                let l = lua::State(state);
                let ud_ptr = ffi::lua_touserdata(l.0, -1);
                OBJECTS.with_borrow_mut(|objects| {
                    objects.remove(&ud_ptr);
                });
                0
            }
            ffi::lua_pushcclosure(self.0, Some(__gc), 0);
            ffi::lua_setfield(self.0, -2, c"__gc".as_ptr());
        }

        // Store table: 3
        ffi::lua_createtable(self.0, 0, 0);

        // Store's metatable: 4
        ffi::lua_createtable(self.0, 0, 1);

        // Methods table: 5
        if ffi::luaL_newmetatable(self.0, T::unique_id().as_ptr()) {
            let methods = {
                let mut mb = MethodsBuilder::new();
                T::methods(&mut mb);
                mb.build()
            };
            for (name, func) in methods.into_iter() {
                self.create_function_impl(func).push_to_stack(self);
                ffi::lua_setfield(self.0, -2, name.as_ptr());
            }
        }

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
pub struct UserDataRef<T: UserData>(Rc<RefCell<T>>);

impl<T: UserData> Clone for UserDataRef<T> {
    fn clone(&self) -> Self {
        UserDataRef(self.0.clone())
    }
}

impl<T: UserData> UserDataRef<T> {
    #[inline]
    pub fn borrow(&self) -> Ref<'_, T> {
        self.0.borrow()
    }

    #[inline]
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }

    #[inline]
    pub fn try_borrow(&self) -> Result<Ref<'_, T>> {
        self.0
            .try_borrow()
            .map_err(|err| Error::Message(format!("cannot borrow '{}': {}", T::name(), err)))
    }

    #[inline]
    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, T>> {
        self.0.try_borrow_mut().map_err(|err| {
            Error::Message(format!("cannot borrow '{}' mutably: {}", T::name(), err))
        })
    }
}

#[derive(Clone, Debug)]
pub struct AnyUserData(pub(crate) Value);

// We have to force them to pass lua::State here to ensure they are on the main thread
// without having to check it each time nor have panics at runtime

impl AnyUserData {
    #[inline]
    fn ptr(&self) -> *mut c_void {
        ffi::lua_touserdata(self.0.thread().0, self.0.index())
    }

    #[inline]
    pub fn is<T: UserData>(&self, _: &lua::State) -> bool {
        OBJECTS.with_borrow(|objects| {
            objects
                .get(&self.ptr())
                .is_some_and(|obj| obj.is::<RefCell<T>>())
        })
    }

    pub fn downcast<T: UserData>(&self, _: &lua::State) -> Option<UserDataRef<T>> {
        OBJECTS
            .with_borrow(|objects| {
                objects
                    .get(&self.ptr())
                    .and_then(|rc| rc.clone().downcast::<RefCell<T>>().ok())
            })
            .map(|rc| UserDataRef(rc))
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

impl<T: UserData> FromLua for UserDataRef<T> {
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        let any_ud = AnyUserData::from_stack_with_type(state, index, T::name())?;
        any_ud
            .downcast::<T>(state)
            .ok_or_else(|| state.type_error(index, T::name()))
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

    fn build(self) -> Methods {
        self.0
    }
}

impl IntoIterator for MethodsBuilder {
    type Item = (&'static CStr, Callback);
    type IntoIter = std::vec::IntoIter<(&'static CStr, Callback)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub struct TypedUserData<T: UserData>(pub AnyUserData, std::marker::PhantomData<T>);

impl<T: UserData> FromLua for TypedUserData<T> {
    fn try_from_stack(state: &lua::State, index: i32) -> Result<Self> {
        let any = AnyUserData::from_stack_with_type(state, index, T::name())?;
        // Type check it
        any.downcast::<T>(state)
            .ok_or_else(|| state.type_error(index, T::name()))?;
        Ok(TypedUserData(any, std::marker::PhantomData))
    }
}

impl<T: UserData> std::ops::Deref for TypedUserData<T> {
    type Target = AnyUserData;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: UserData> std::ops::DerefMut for TypedUserData<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
