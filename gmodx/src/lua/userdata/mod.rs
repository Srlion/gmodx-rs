use std::any::{TypeId, type_name};
use std::ffi::{CStr, CString, c_void};
use std::sync::{LazyLock, Mutex};

use rustc_hash::{FxBuildHasher, FxHashMap};

mod methods;
pub use methods::MethodsBuilder;

mod any;
pub use any::AnyUserData;

mod r#ref;
pub use r#ref::UserDataRef;

mod scoped;
pub use scoped::{ScopedUserData, ScopedUserDataRef};

use crate::lua::{self, ffi::lua_State};
use crate::lua::{Table, ToLua, Value, ffi};

pub(crate) static TYPES: Mutex<FxHashMap<usize, TypeId>> =
    Mutex::new(FxHashMap::with_hasher(FxBuildHasher));

pub(crate) fn drop_userdata_at<T>(ptr: usize) {
    let type_id = TYPES.lock().unwrap().remove(&ptr);
    if type_id.is_some() {
        unsafe {
            std::ptr::drop_in_place(ptr as *mut T);
        }
    }
}

fn unique_id<T: ?Sized>() -> &'static CStr {
    static IDS: LazyLock<Mutex<FxHashMap<TypeId, &'static CStr>>> =
        LazyLock::new(|| Mutex::new(FxHashMap::default()));

    let type_id = typeid::of::<T>();
    IDS.lock().unwrap().entry(type_id).or_insert_with(|| {
        let cstring =
            CString::new(format!("{}_{:?}", gmodx_macros::unique_id!(), type_id)).unwrap();
        Box::leak(cstring.into_boxed_c_str())
    })
}

pub trait UserData {
    fn meta_methods(_: &mut MethodsBuilder) {}
    fn methods(_: &mut MethodsBuilder) {}

    #[must_use]
    fn name() -> &'static str {
        type_name::<Self>()
            .rsplit("::")
            .next()
            .unwrap_or_else(type_name::<Self>)
    }

    #[must_use]
    fn unique_id() -> &'static CStr {
        unique_id::<Self>()
    }

    /// By default we lazily initialize the methods table.
    /// Use this function to initialize the methods table before it is used.
    #[must_use]
    fn init_methods_table(state: &lua::State) -> Table
    where
        Self: Sized,
    {
        push_methods_table::<Self>(state);
        Table(Value::pop_from_stack(state))
    }
}

fn push_methods_table<T: UserData>(state: &lua::State) {
    if ffi::luaL_newmetatable(state.0, unique_id::<T>().as_ptr()) {
        let mut mb = MethodsBuilder::new();
        T::methods(&mut mb);
        for (name, func) in mb.0 {
            func.push_to_stack(state);
            ffi::lua_setfield(state.0, -2, name.as_ptr());
        }
    }
}

impl lua::State {
    // We take `I` because create_userdata_impl is used with RefCell<T> and other wrappers.
    // So we need the actual UserData type separately for underlying methods.
    pub(crate) fn create_userdata_impl<T, I: UserData>(&self, ud: T) -> (*mut c_void, AnyUserData) {
        extern "C-unwind" fn __gc<T>(state: *mut lua_State) -> i32 {
            let ud_ptr = ffi::lua_touserdata(state, -1);
            let ud_ptr = ud_ptr.cast_const();
            drop_userdata_at::<T>(ud_ptr as usize);
            0
        }

        // Userdata: 1
        let ud_ptr = ffi::lua_newuserdata(self.0, std::mem::size_of::<T>());

        // SAFETY: We just created the userdata, so it's safe to write to it.
        unsafe {
            std::ptr::write(ud_ptr.cast::<T>(), ud);
        }

        {
            TYPES
                .lock()
                .unwrap()
                .insert(ud_ptr as usize, typeid::of::<T>());
        }

        // UserData metatable: 2
        let mut mb = MethodsBuilder::new();
        I::meta_methods(&mut mb);

        ffi::lua_createtable(self.0, 0, mb.0.len() as i32);
        {
            for (name, func) in mb.0 {
                func.push_to_stack(self);
                ffi::lua_setfield(self.0, -2, name.as_ptr());
            }
            ffi::lua_pushcclosure(self.0, Some(__gc::<T>), 0);
            ffi::lua_setfield(self.0, -2, c"__gc".as_ptr());
        }

        // Store table: 3
        ffi::lua_createtable(self.0, 0, 0);

        // Store's metatable: 4
        ffi::lua_createtable(self.0, 0, 1);

        // Methods table: 5
        push_methods_table::<I>(self);

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

        (ud_ptr, AnyUserData(Value::pop_from_stack(self)))
    }
}

impl<T: UserData + 'static> ToLua for T {
    fn push_to_stack(self, state: &lua::State) {
        state.create_userdata(self).push_to_stack(state);
    }
}
