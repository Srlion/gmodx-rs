use crate::{cstr_to_str, lua};

#[derive(Debug, Clone)]
pub enum Error {
    /// Lua failed to allocate memory for an operation.
    MemoryAllocation,

    /// A syntax error was encountered while parsing Lua source code.
    /// Optionally contains the error message from the Lua parser.
    Syntax(Option<String>),

    /// Lua was unable to load the specified file.
    /// Optionally includes the file name or Lua error message.
    File(Option<String>),

    /// A runtime error occurred during Lua execution.
    /// Optionally contains the error message returned by Lua.
    Runtime(Option<String>),

    /// An error was raised while running the Lua error handler function.
    Handler,

    /// A generic error represented by a string message.
    Message(String),

    /// An unrecognized or unknown Lua error code was returned.
    /// Contains the raw error code from Lua.
    Unknown(i32),
}

impl Error {
    pub fn from_lua_state(lua_state: lua::State, lua_int_error_code: i32) -> Self {
        use Error::*;
        let res = match lua_int_error_code {
            lua::ERRMEM => MemoryAllocation,
            lua::ERRERR => Handler,
            lua::ERRSYNTAX | lua::ERRRUN | lua::ERRFILE => {
                let msg = lua_state.to_string(-1);
                match lua_int_error_code {
                    lua::ERRSYNTAX => Syntax(msg),
                    lua::ERRRUN => Runtime(msg),
                    lua::ERRFILE => File(msg),
                    _ => unreachable!(),
                }
            }
            _ => Unknown(lua_int_error_code),
        };
        lua_state.pop(); // pop the error message
        res
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::MemoryAllocation => write!(f, "failed to allocate memory"),
            Error::Syntax(Some(s)) => write!(f, "{}", s),
            Error::Syntax(None) => write!(f, "Lua syntax error"),
            Error::File(Some(s)) => write!(f, "{}", s),
            Error::File(None) => write!(f, "Lua file error"),
            Error::Runtime(Some(s)) => write!(f, "{}", s),
            Error::Runtime(None) => write!(f, "Lua runtime error"),
            Error::Handler => write!(
                f,
                "Error occurred while running the Lua error handler function"
            ),
            Error::Message(msg) => write!(f, "{}", msg),
            Error::Unknown(code) => write!(f, "Unknown Lua error code: {}", code),
        }
    }
}

impl std::error::Error for Error {}

impl lua::State {
    fn call_error_handler(self, message: &str) {
        if self.is_nil(-1) {
            self.pop();
            eprint!("{}", message);
        } else {
            self.push_string(message);
            if self.direct_pcall(1, 0, 0).is_err() {
                // Lua call failed, fallback to stderr
                eprint!("{}", message);
            }
        }
    }

    pub fn error_no_halt(self, err: &str) {
        self.get_global(c"ErrorNoHalt");
        let formatted = format!("{}\n", err);
        self.call_error_handler(&formatted);
    }

    pub fn error_no_halt_with_stack(&self, err: &str) {
        self.get_global(c"ErrorNoHaltWithStack");
        self.call_error_handler(err);
    }

    pub fn report_lua_error(self, err: &lua::Error) {
        self.error_no_halt_with_stack(&err.to_string());
    }

    pub fn report_error(self, err: &dyn std::error::Error, traceback: Option<&str>) {
        let error_msg = err.to_string();
        if let Some(tb) = traceback {
            self.error_no_halt(&format!("[ERROR] {}\n{}", error_msg, tb));
        } else {
            self.error_no_halt_with_stack(&error_msg);
        }
    }

    pub fn type_error(self, narg: i32, tname: &str) -> String {
        let err = format!(
            "{} expected, got {}",
            tname,
            self.type_name(self.get_type(narg))
        );
        self.err_argmsg(narg, &err)
    }

    pub fn tag_error(self, narg: i32, tag: i32) -> String {
        self.type_error(narg, &self.type_name(tag))
    }

    pub fn err_argmsg(self, mut narg: i32, msg: &str) -> String {
        let mut fname = "?";
        let mut namewhat: Option<&str> = None;

        if let Some(ar) = self.debug_getinfo_at(0, c"n") {
            if !ar.name.is_null() {
                fname = cstr_to_str!(ar.name);
            }
            if !ar.namewhat.is_null() {
                namewhat = Some(cstr_to_str!(ar.namewhat));
            }
        }

        if narg < 0 && narg > lua::REGISTRYINDEX {
            narg = self.get_top() + narg + 1;
        }

        if let Some(namewhat) = namewhat
            && namewhat == "method"
            && {
                narg -= 1;
                narg == 0
            }
        {
            return format!("bad self parameter in method '{}' ({})", fname, msg);
        }

        format!("bad argument #{} to '{}' ({})", narg, fname, msg)
    }
}
