use std::fmt;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

impl Error {
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub enum ErrorKind {
    Io(std::io::ErrorKind),
    NotARepository,
    Unsupported(&'static str),
    Backend(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(kind) => write!(f, "I/O error: {kind}"),
            Self::NotARepository => f.write_str("Not a repository"),
            Self::Unsupported(message) => write!(f, "Unsupported: {message}"),
            Self::Backend(message) => f.write_str(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorKind};

    #[test]
    fn backend_error_kind_display_is_human_readable() {
        let kind = ErrorKind::Backend("message".to_string());
        assert_eq!(kind.to_string(), "message");
    }

    #[test]
    fn error_display_uses_error_kind_display() {
        let error = Error::new(ErrorKind::Backend("message".to_string()));
        assert_eq!(error.to_string(), "message");
    }
}
