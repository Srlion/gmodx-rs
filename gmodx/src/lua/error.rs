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

    /// Attempted to call a Lua value that is not a function.
    NotAFunction,

    /// An unrecognized or unknown Lua error code was returned.
    /// Contains the raw error code from Lua.
    Unknown(i32),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::MemoryAllocation => write!(f, "Lua failed to allocate memory"),
            Error::Syntax(Some(s)) => write!(f, "Lua syntax error: {}", s),
            Error::Syntax(None) => write!(f, "Lua syntax error"),
            Error::File(Some(s)) => write!(f, "Lua file error: {}", s),
            Error::File(None) => write!(f, "Lua file error"),
            Error::Runtime(Some(s)) => write!(f, "Lua runtime error: {}", s),
            Error::Runtime(None) => write!(f, "Lua runtime error"),
            Error::Handler => write!(
                f,
                "Error occurred while running the Lua error handler function"
            ),
            Error::NotAFunction => {
                write!(f, "Attempted to call a Lua value that is not a function")
            }
            Error::Unknown(code) => write!(f, "Unknown Lua error code: {}", code),
        }
    }
}

impl std::error::Error for Error {}
