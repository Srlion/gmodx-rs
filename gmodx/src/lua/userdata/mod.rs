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

use crate::lua::value_ref::ValueRef;
use crate::lua::{self, ffi::lua_State};
use crate::lua::{Function, Table, ToLua, Value, ffi};

/// 0 = __index
/// 1 = __newindex
static UD_METAMETHODS: Mutex<Option<(i32, i32)>> = Mutex::new(None);

fn get_ud_metamethods() -> (i32, i32) {
    UD_METAMETHODS
        .lock()
        .unwrap()
        .expect("userdata store not initialized")
}

inventory::submit! {
    crate::open_close::new(
        0,
        "userdata_store",
        |l| {
            // 😉 ;-)
            let chunk = l.load_buffer(b"
                local getmetatable = getmetatable
                local STORE = setmetatable({}, { __mode = 'k' })
                local function __index(self, k)
                    local store = STORE[self]
                    if store then
                        local v = store[k]
                        if v ~= nil then
                            return v
                        end
                    end
                    return getmetatable(self)[k]
                end
                local function __newindex(self, k, v)
                    local store = STORE[self]
                    if not store then
                        STORE[self] = {
                            [k] = v
                        }
                        return
                    end
                    store[k] = v
                end
                return STORE, __index, __newindex
            ", c"ud_store").expect("failed to load userdata store chunk");

            let (store, __index, __newindex) = chunk.call::<(Table, Function, Function)>(l, ()).expect("failed to get userdata store");
            store.0.leak_index(); // leak the STORE table to keep it alive forever
            *UD_METAMETHODS.lock().unwrap() = Some((
                __index.0.leak_index(),
                __newindex.0.leak_index(),
            ));        },
        |_| {
            *UD_METAMETHODS.lock().unwrap() = None;
        },
    )
}

pub static TYPES: Mutex<FxHashMap<usize, (TypeId, fn(usize))>> =
    Mutex::new(FxHashMap::with_hasher(FxBuildHasher));

fn register_userdata<T>(ptr: usize) {
    fn drop_fn<T>(ptr: usize) {
        unsafe { std::ptr::drop_in_place(ptr as *mut T) }
    }
    TYPES
        .lock()
        .unwrap()
        .insert(ptr, (typeid::of::<T>(), drop_fn::<T>));
}

pub(crate) fn is_type<T>(ptr: usize) -> bool {
    TYPES
        .lock()
        .unwrap()
        .get(&ptr)
        .is_some_and(|(id, _)| id == &typeid::of::<T>())
}

pub(crate) fn drop_userdata_at(ptr: usize) {
    if let Some((_, drop_fn)) = TYPES.lock().unwrap().remove(&ptr) {
        drop_fn(ptr)
    }
}

fn unique_id<T: ?Sized>() -> &'static CStr {
    static IDS: LazyLock<Mutex<FxHashMap<TypeId, &'static CStr>>> =
        LazyLock::new(|| Mutex::new(FxHashMap::default()));

    let id = typeid::of::<T>();
    IDS.lock().unwrap().entry(id).or_insert_with(|| {
        let cstring = CString::new(format!("{}_{:?}", gmodx_macros::unique_id!(), id)).unwrap();
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
    fn init_methods_table(l: &lua::State) -> Table
    where
        Self: Sized,
    {
        push_methods_table::<Self>(l);
        Table(Value::pop_from_stack(l))
    }
}

fn push_methods_table<T: UserData>(l: &lua::State) {
    extern "C-unwind" fn __gc(l: *mut lua_State) -> i32 {
        let ud_ptr = ffi::lua_touserdata(l, 1) as usize;
        drop_userdata_at(ud_ptr);
        0
    }

    if !ffi::luaL_newmetatable(l.0, unique_id::<T>().as_ptr()) {
        return;
    }

    let mut mb = MethodsBuilder::new();
    T::methods(&mut mb);
    T::meta_methods(&mut mb);

    let mut has_tostring = false;
    for (name, func) in mb.0 {
        assert!(
            name != c"__gc",
            "{}: use Drop instead of __gc",
            type_name::<T>()
        );
        assert!(
            name != c"__index" && name != c"__newindex",
            "{}: __index/__newindex reserved",
            type_name::<T>()
        );
        has_tostring |= name == c"__tostring";

        func.push_to_stack(l);
        ffi::lua_setfield(l.0, -2, name.as_ptr());
    }

    if !has_tostring {
        l.create_function(|_: &_| T::name()).push_to_stack(l);
        ffi::lua_setfield(l.0, -2, c"__tostring".as_ptr());
    }

    let (__index, __newindex) = get_ud_metamethods();
    ValueRef::push_index(l, __index);
    ffi::lua_setfield(l.0, -2, c"__index".as_ptr());

    ValueRef::push_index(l, __newindex);
    ffi::lua_setfield(l.0, -2, c"__newindex".as_ptr());

    ffi::lua_pushcfunction(l.0, Some(__gc));
    ffi::lua_setfield(l.0, -2, c"__gc".as_ptr());
}

impl lua::State {
    // We take `I` because create_userdata_impl is used with RefCell<T> and other wrappers.
    // So we need the actual UserData type separately for underlying methods.
    pub(crate) fn create_userdata_impl<T, I: UserData>(&self, ud: T) -> (*mut c_void, AnyUserData) {
        // Userdata: 1
        let ud_ptr = ffi::lua_newuserdata(self.0, std::mem::size_of::<T>());

        // SAFETY: We just created the userdata, so it's safe to write to it.
        unsafe {
            std::ptr::write(ud_ptr.cast::<T>(), ud);
        }

        register_userdata::<T>(ud_ptr as usize);

        push_methods_table::<I>(self);
        ffi::lua_setmetatable(self.0, -2);

        (ud_ptr, AnyUserData(Value::pop_from_stack(self)))
    }
}

impl<T: UserData + 'static> ToLua for T {
    fn push_to_stack(self, l: &lua::State) {
        l.create_userdata(self).push_to_stack(l);
    }
}
