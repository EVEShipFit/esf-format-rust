use std::fmt;

/// An esf/1 error, and where: a line of text, or a fit and entry, counting from 1.
///
/// Every error is one of three [`ErrorKind`]s. Check `kind` to handle one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Error {
    /// What kind of error this is.
    pub kind: ErrorKind,
    /// What is wrong.
    pub message: String,
    /// The line of the text.
    pub line: Option<usize>,
    /// The fit, for `save()` and binary documents.
    pub fit: Option<usize>,
    /// The entry of that fit.
    pub entry: Option<usize>,
}

/// What kind of error an [`Error`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The data is not a well-formed esf/1 document.
    ///
    /// The text does not follow the grammar, the binary form or base64url is broken, or a fit
    /// name cannot be written.
    Format,
    /// The catalog does not know a name or ID, or a name matches more than one type.
    ///
    /// This often means the catalog is older than the fit.
    Catalog,
    /// The fit breaks a rule of esf/1.
    ///
    /// The document is well formed and the catalog knows every type, but the fit is not valid.
    /// For example, a module is in the wrong rack.
    Rule,
}

impl Error {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Error {
        Error {
            kind,
            message: message.into(),
            line: None,
            fit: None,
            entry: None,
        }
    }

    pub(crate) fn format(message: impl Into<String>) -> Error {
        Error::new(ErrorKind::Format, message)
    }

    pub(crate) fn catalog(message: impl Into<String>) -> Error {
        Error::new(ErrorKind::Catalog, message)
    }

    pub(crate) fn rule(message: impl Into<String>) -> Error {
        Error::new(ErrorKind::Rule, message)
    }

    pub(crate) fn at(self, line: usize) -> Error {
        Error {
            line: Some(line),
            ..self
        }
    }

    pub(crate) fn in_fit(self, fit: usize, entry: Option<usize>) -> Error {
        Error {
            line: None,
            fit: Some(fit),
            entry,
            ..self
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.fit, self.entry, self.line) {
            (Some(fit), Some(entry), _) => write!(f, "fit {fit}, entry {entry}: ")?,
            (Some(fit), None, _) => write!(f, "fit {fit}: ")?,
            (None, _, Some(line)) => write!(f, "line {line}: ")?,
            (None, _, None) => {}
        }
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
