use std::fmt;

/// A broken rule of esf/1, and where: a line of text, or a fit and entry, counting from 1.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Error {
    /// What is wrong.
    pub message: String,
    /// The line of the text.
    pub line: Option<usize>,
    /// The fit, for `save()` and binary documents.
    pub fit: Option<usize>,
    /// The entry of that fit.
    pub entry: Option<usize>,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Error {
        Error {
            message: message.into(),
            line: None,
            fit: None,
            entry: None,
        }
    }

    pub(crate) fn at(line: usize, message: impl Into<String>) -> Error {
        Error {
            line: Some(line),
            ..Error::new(message)
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
