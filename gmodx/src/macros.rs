#![allow(unused_macros)]

macro_rules! cstr_to_str {
    ($cstring:expr) => {{
        let cstring_ptr = $cstring;
        let cstr = unsafe { std::ffi::CStr::from_ptr(cstring_ptr) };
        match cstr.to_str() {
            Ok(s) => s,
            Err(_) => panic!("Couldn't unwrap CString"),
        }
    }};
}

macro_rules! cstr {
    ($s:expr) => {{ std::ffi::CString::new($s).expect("CString::new failed") }};
}

#[allow(unused)]
macro_rules! todo_release {
    () => {{
        #[cfg(not(debug_assertions))]
        compile_error!("TODO not allowed in release builds");

        #[cfg(debug_assertions)]
        todo!();
    }};
    ($($arg:tt)*) => {{
        #[cfg(not(debug_assertions))]
        compile_error!(concat!("TODO not allowed in release builds: ", $($arg)*));

        #[cfg(debug_assertions)]
        todo!($($arg)*);
    }};
}

macro_rules! bug_msg {
    ($arg:expr) => {
        concat!(
            "gmodx internal error: ",
            $arg,
            " (this is a bug, please file an issue)"
        )
    };
}

macro_rules! gmodx_panic {
    ($msg:expr) => {
        panic!(bug_msg!($msg))
    };

    ($msg:expr,) => {
        gmodx_panic!($msg)
    };

    ($msg:expr, $($arg:expr),+) => {
        panic!(bug_msg!($msg), $($arg),+)
    };

    ($msg:expr, $($arg:expr),+,) => {
        gmodx_panic!($msg, $($arg),+)
    };
}

macro_rules! gmodx_debug_assert {
    ($cond:expr, $msg:expr) => {
        debug_assert!($cond, bug_msg!($msg));
    };

    ($cond:expr, $msg:expr,) => {
        gmodx_debug_assert!($cond, $msg);
    };

    ($cond:expr, $msg:expr, $($arg:expr),+) => {
        debug_assert!($cond, bug_msg!($msg), $($arg),+);
    };

    ($cond:expr, $msg:expr, $($arg:expr),+,) => {
        gmodx_debug_assert!($cond, $msg, $($arg),+);
    };
}
