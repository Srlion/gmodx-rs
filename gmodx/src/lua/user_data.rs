use std::{any::TypeId, os::raw::c_void};

use crate::{lua, lua_shared::lua_State};

pub type UserDataMethods = &'static [(lua::CStr<'static>, lua::RustFunction)];

// This is used as the key to store the userdata inside the lua table
pub const INDEX_KEY: i32 = 1;

#[repr(C)]
struct EmptyUserData;

#[repr(C)]
pub struct TaggedUserData<T: 'static> {
    pub data: T,
    pub type_id: TypeId,
}

impl<T: 'static> TaggedUserData<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            type_id: TypeId::of::<T>(),
        }
    }

    pub fn is(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    pub fn from_ptr<'a>(ptr: *mut c_void) -> Option<&'a mut Self> {
        if ptr.is_null() {
            return None;
        }
        let tagged_ptr = ptr as *mut Self;
        // SAFETY: We've checked ptr is not null.
        // The type_id check below validates the type.
        let tagged = unsafe { &mut *tagged_ptr };
        if !tagged.is() {
            return None;
        }
        Some(tagged)
    }

    fn consume(ptr: *mut c_void) -> Option<TaggedUserData<T>> {
        if ptr.is_null() {
            return None;
        }
        let tagged_ptr = ptr as *mut Self;
        // SAFETY: We've checked ptr is not null
        // The type_id check below validates the type.
        let tagged_ref = unsafe { &mut *tagged_ptr };
        if !tagged_ref.is() {
            return None;
        }
        // Mark the userdata as empty to prevent double free
        // This is for people who manually call __gc on the userdata
        // which is technically undefined behavior but we can at least
        // try to mitigate the damage.
        tagged_ref.type_id = TypeId::of::<EmptyUserData>();
        // SAFETY: We've verified the pointer is valid and of the correct type.
        let tagged = unsafe { std::ptr::read(tagged_ptr) };
        Some(tagged)
    }
}

extern "C" fn userdata_gc<T: UserData>(l: *mut lua_State) -> i32 {
    let l = lua::State(l);
    let tagged = TaggedUserData::<T>::consume(l.raw_to_userdata(-1));
    match tagged {
        Some(t) => drop(t.data), // Explicitly drop the data
        None => {
            #[cfg(debug_assertions)]
            eprintln!("[gmodx] Warning: __gc called on invalid userdata")
        }
    }
    0
}

pub trait UserData: Sized + 'static {
    const METATABLE_NAME: lua::CStr<'_>;
    const METHODS: UserDataMethods = &[];
}

impl lua::State {
    pub fn push_userdata<T: UserData>(self, ud: T) {
        // local table = {}
        self.create_table(1, 0);

        // local ud_ptr = newproxy(false)
        {
            let tagged = TaggedUserData::new(ud);
            let ud_ptr = self.raw_new_userdata(std::mem::size_of::<TaggedUserData<T>>());
            let tagged_ptr = ud_ptr as *mut TaggedUserData<T>;
            // SAFETY: raw_new_userdata returns a valid pointer to uninitialized memory
            // of the requested size. We immediately initialize it with our tagged data.
            unsafe {
                tagged_ptr.write(tagged);
            }
        }

        // local gc_mt = {}
        self.create_table(0, 1);
        // gc_mt.__gc = T.__internal_gc
        {
            self.raw_push_cclosure(Some(userdata_gc::<T>), 0);
            self.set_field(-2, c"__gc");
        }

        // table[INDEX_KEY] = ud_ptr
        self.set_metatable(-2);
        self.raw_seti(-2, INDEX_KEY);

        // local mt = {}
        if self.new_metatable(T::METATABLE_NAME) {
            for (name, func) in T::METHODS {
                // mt[name] = func
                self.push_function(*func);
                self.set_field(-2, name);
            }

            // mt.__index = mt
            self.push_value(-1); // Pushes the metatable to the top of the stack
            self.set_field(-2, c"__index");
        }

        // setmetatable(table, mt)
        self.set_metatable(-2);
    }

    pub fn to_userdata<'a, T: UserData>(self, index: i32) -> Option<&'a mut T> {
        self.check_table(index).ok()?;
        self.raw_geti(index, INDEX_KEY);
        let tagged = TaggedUserData::<T>::from_ptr(self.raw_to_userdata(-1));
        tagged.map(|t| &mut t.data)
    }

    pub fn check_userdata<'a, T: UserData>(self, index: i32) -> Result<&'a mut T, lua::Error> {
        self.to_userdata(index).ok_or_else(|| {
            lua::Error::Message(self.type_error(index, &T::METATABLE_NAME.to_string_lossy()))
        })
    }
}
