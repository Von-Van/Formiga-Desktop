//! The error every contract reads and writes its documents with. Each says whose file went wrong
//! in its own word, and otherwise has exactly the cases of [`RulebookError`](crate::RulebookError),
//! so a rulebook error becomes the contract's case for case.

#[doc(hidden)]
pub use serde_json;

/// Declare a contract's document error: `contract_error!(pub enum TravelError, "travel");`.
///
/// The enum has the cases of [`RulebookError`](crate::RulebookError), each saying "the travel
/// file" (or whichever word is given) where the rulebook says "the file", and converts from it,
/// from [`std::io::Error`] and from [`serde_json::Error`].
#[macro_export]
macro_rules! contract_error {
    ($(#[$meta:meta])* $vis:vis enum $name:ident, $word:literal) => {
        $(#[$meta])*
        #[derive(Debug)]
        $vis enum $name {
            Io(::std::io::Error),
            TooLarge {
                limit: u64,
            },
            Json($crate::contract_error::serde_json::Error),
            WrongFormat {
                expected: &'static str,
                found: String,
            },
            UnsupportedVersion {
                needs: u32,
                reads: u32,
            },
            Invalid(String),
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    Self::Io(error) => write!(
                        f,
                        concat!("the ", $word, " file could not be read or written: {}"),
                        error
                    ),
                    Self::TooLarge { limit } => write!(
                        f,
                        concat!(
                            "the ", $word, " file is larger than any ", $word,
                            " file can be ({} bytes)"
                        ),
                        limit
                    ),
                    Self::Json(error) => {
                        write!(f, concat!("the ", $word, " file could not be read: {}"), error)
                    }
                    Self::WrongFormat { expected, found } => {
                        write!(f, "expected a {} file, found {:?}", expected, found)
                    }
                    Self::UnsupportedVersion { needs, reads } => write!(
                        f,
                        concat!(
                            "the ", $word, " file needs a reader of ", $word,
                            " version {}, and this build reads up to version {}"
                        ),
                        needs, reads
                    ),
                    Self::Invalid(reason) => {
                        write!(f, concat!("the ", $word, " file is not usable: {}"), reason)
                    }
                }
            }
        }

        impl ::std::error::Error for $name {
            fn source(&self) -> Option<&(dyn ::std::error::Error + 'static)> {
                match self {
                    Self::Io(error) => Some(error),
                    Self::Json(error) => Some(error),
                    _ => None,
                }
            }
        }

        impl From<::std::io::Error> for $name {
            fn from(error: ::std::io::Error) -> Self {
                Self::Io(error)
            }
        }

        impl From<$crate::contract_error::serde_json::Error> for $name {
            fn from(error: $crate::contract_error::serde_json::Error) -> Self {
                Self::Json(error)
            }
        }

        impl From<$crate::RulebookError> for $name {
            fn from(error: $crate::RulebookError) -> Self {
                match error {
                    $crate::RulebookError::Io(error) => Self::Io(error),
                    $crate::RulebookError::TooLarge { limit } => Self::TooLarge { limit },
                    $crate::RulebookError::Json(error) => Self::Json(error),
                    $crate::RulebookError::WrongFormat { expected, found } => {
                        Self::WrongFormat { expected, found }
                    }
                    $crate::RulebookError::UnsupportedVersion { needs, reads } => {
                        Self::UnsupportedVersion { needs, reads }
                    }
                    $crate::RulebookError::Invalid(reason) => Self::Invalid(reason),
                }
            }
        }
    };
}
