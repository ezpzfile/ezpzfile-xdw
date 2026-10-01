use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// Not a DocuWorks file.
    NotXdw(String),
    /// Damaged or inconsistent data.
    Corrupt(String),
    /// A feature this version does not handle.
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotXdw(s) => write!(f, "not a DocuWorks file: {s}"),
            Error::Corrupt(s) => write!(f, "damaged file: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
