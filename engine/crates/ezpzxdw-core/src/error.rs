use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// Not a DocuWorks file.
    NotXdw(String),
    /// Damaged or inconsistent data.
    Corrupt(String),
    /// A feature this version does not handle.
    Unsupported(String),
    /// The document is protected (password or certificate): its entries and
    /// properties are encrypted. DocuWorks can remove the protection for
    /// someone who knows the password.
    Protected,
    /// Past one of the [`crate::limits::Limits`]: a crafted or damaged file
    /// that claims far more than it holds, or one too large to handle.
    TooLarge(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotXdw(s) => write!(f, "not a DocuWorks file: {s}"),
            Error::Corrupt(s) => write!(f, "damaged file: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::TooLarge(s) => write!(f, "too large or damaged: {s}"),
            Error::Protected => write!(f, "protected: the document is encrypted (password or certificate); remove the protection in DocuWorks first"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
