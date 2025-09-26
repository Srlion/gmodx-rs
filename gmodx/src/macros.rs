#[macro_export]
macro_rules! cstr_to_str {
    ($cstring:expr) => {{
        let cstring_ptr = $cstring;
        let cstr = unsafe { std::ffi::CStr::from_ptr(cstring_ptr) };
        cstr.to_str().expect("Couldn't unwrap CString")
    }};
}

#[macro_export]
macro_rules! cstr {
    ($s:expr) => {{ std::ffi::CString::new($s).expect("CString::new failed") }};
}

#[macro_export]
macro_rules! cstr_from_args {
    ($($arg:expr),+) => {{
        use std::ffi::{c_char, CStr};
        const BYTES: &[u8] = const_str::concat!($($arg),+, "\0").as_bytes();
        let ptr: *const c_char = BYTES.as_ptr().cast();
        unsafe { CStr::from_ptr(ptr) }
    }};
}
