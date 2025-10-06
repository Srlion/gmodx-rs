use crate::lua::{self, Function, ffi, traits::FromLua};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum Error {
    /// Lua failed to allocate memory for an operation.
    MemoryAllocation(String),

    /// A syntax error was encountered while parsing Lua source code.
    /// Optionally contains the error message from the Lua parser.
    Syntax(String),

    /// A runtime error occurred during Lua execution.
    /// Optionally contains the error message returned by Lua.
    Runtime(String),

    /// A generic error represented by a string message.
    Message(String),

    /// An unrecognized or unknown Lua error code was returned.
    /// Contains the raw error code from Lua.
    Unknown { code: i32, message: String },

    /// A type mismatch occurred
    Type { expected: String, got: String },

    /// Bad argument to a function
    BadArgument {
        arg_num: i32,
        function: String,
        cause: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::MemoryAllocation(s) => write!(f, "{}", s),
            Error::Syntax(s) => write!(f, "{}", s),
            Error::Runtime(s) => write!(f, "{}", s),
            Error::Message(msg) => write!(f, "{}", msg),
            Error::Unknown { code, message } => {
                write!(f, "Unknown Lua error (code {}): {}", code, message)
            }
            Error::Type { expected, got } => {
                write!(f, "expected {}, got {}", expected, got)
            }
            Error::BadArgument {
                arg_num,
                function,
                cause,
            } => write!(f, "bad argument #{} to '{}' ({})", arg_num, function, cause),
        }
    }
}

impl std::error::Error for Error {}

impl lua::State {
    pub fn error_no_halt(&self, err: &str) {
        let formatted = format!("[ERROR] {}\n", err);
        self.call_error_handler("ErrorNoHalt", &formatted);
    }

    pub fn error_no_halt_with_stack(&self, err: &str) {
        self.call_error_handler("ErrorNoHaltWithStack", err);
    }

    fn call_error_handler(&self, func_name: &str, message: &str) {
        self.get_global::<Function>(func_name)
            .and_then(|func| func.call::<()>(self, message))
            .unwrap_or_else(|_| eprint!("{}", message));
    }

    pub(crate) fn pop_error(&self, err_code: i32) -> Error {
        gmodx_debug_assert!(
            err_code != ffi::LUA_OK && err_code != ffi::LUA_YIELD,
            "pop_error called with non-error return code"
        );

        let err_string = lua::String::try_from_stack(self, -1)
            .expect("this error MUST be a string")
            .to_string();
        ffi::lua_pop(self.0, 1); // pop the error object

        match err_code {
            ffi::LUA_ERRMEM => Error::MemoryAllocation(err_string),
            ffi::LUA_ERRSYNTAX => Error::Syntax(err_string),
            ffi::LUA_ERRRUN | ffi::LUA_ERRERR => Error::Runtime(err_string),
            _ => Error::Unknown {
                code: err_code,
                message: err_string,
            },
        }
    }

    pub(crate) fn protect_lua_call(&self, nargs: i32, nresults: i32) -> Result<()> {
        let ret = ffi::lua_pcall(self.0, nargs, nresults, 0);
        if ret == ffi::LUA_OK {
            Ok(())
        } else {
            Err(self.pop_error(ret))
        }
    }

    pub(crate) fn type_error(&self, narg: i32, expected: &str) -> Error {
        Error::Type {
            expected: expected.to_string(),
            got: self.type_name(narg),
        }
    }
}
