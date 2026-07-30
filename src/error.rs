use std::fmt;

/// What can be wrong with a set of exposures.
///
/// Only the arguments a caller takes from its own user are reported this way.
/// A buffer whose length contradicts the dimensions passed with it stays a
/// panic, because nothing downstream can do anything about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Two exposures are not the same size.
    SizeMismatch {
        reference: (usize, usize),
        target: (usize, usize),
    },
    /// The chosen reference is not one of the exposures.
    NoSuchReference { reference: usize, exposures: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::SizeMismatch { reference, target } => write!(
                f,
                "cannot align a {}x{} exposure against a {}x{} one",
                reference.0, reference.1, target.0, target.1
            ),
            Error::NoSuchReference {
                reference,
                exposures,
            } => write!(f, "no exposure {reference} in a stack of {exposures}"),
        }
    }
}

impl std::error::Error for Error {}
