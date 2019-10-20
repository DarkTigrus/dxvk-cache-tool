use std::io;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ErrorKind {
    IoError(io::ErrorKind),
    InvalidInput,
    InvalidData
}

#[derive(Debug)]
pub struct Error {
    kind:    ErrorKind,
    message: String
}

impl Error {
    pub fn new<S: Into<String>>(kind: ErrorKind, message: S) -> Self {
        Error {
            kind,
            message: message.into()
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error {
            kind:    ErrorKind::IoError(error.kind()),
            message: error.to_string()
        }
    }
}
