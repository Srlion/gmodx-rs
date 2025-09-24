#[macro_export]
macro_rules! cstr {
    ($cstring:expr) => {{
        let cstring_ptr = $cstring;
        let cstr = unsafe { std::ffi::CStr::from_ptr(cstring_ptr) };
        cstr.to_str().expect("Couldn't unwrap CString")
    }};
}

#[macro_export]
macro_rules! lua_regs {
	() => {
        &[
            $crate::luaL_Reg {
                name: std::ptr::null(),
                func: None,
            }
        ]
    };
    (
        $(
            $name:literal => $func:expr
        ),* $(,)?
    ) => {
        &[
            $(
                $crate::luaL_Reg {
                    name: concat!($name, "\0").as_ptr() as *const i8,
                    func: Some($func),
                }
            ),*,
            $crate::luaL_Reg {
                name: std::ptr::null(),
                func: None,
            }
        ]
    };
}
