//! This is a companion crate to the [`failure`](https://boats.gitlab.io/failure/intro.html)
//! crate, which aims to recover the ergonomics of
//! [`error_chain`](https://github.com/rust-lang-nursery/error-chain). It does this
//! by bringing back:
//!   * `chain_err`
//!   * non-verbose `Error`, `ErrorKind` pairs
//!   * support for `bail!` and `ensure!` with custom `ErrorKind`-s
//!
//! The `failure` library recommends three different patterns for errors. This
//! library implementes th complex one (and the most useful one) but without all the [boilerplate](https://boats.gitlab.io/failure/error-errorkind.html).
//!
//! ### What it looks like
//!
//! #### Enum
//! ```rust
//! mod errors {
//!     // errors.rs
//!     use failchain::{BoxedError, ChainErrorKind};
//!     use failure::Fail;
//!     use std::path::PathBuf;
//!     use std::result::Result as StdResult;
//!
//!     pub type Error = BoxedError<ErrorKind>; // Use `UnboxedError` instead for
//!                                             // non-allocating, but bigger `Error`.
//!     pub type Result<T> = StdResult<T, Error>;
//!
//!     #[derive(Clone, Eq, PartialEq, Debug, Fail)]
//!     pub enum ErrorKind {
//!         #[fail(display = "Metadata I/O error {:?}.", 0)]
//!         MetadataIo(PathBuf),
//!
//!         #[fail(display = "Corrupt metadata file: {}", 0)]
//!         CorruptMetadata(String),
//!
//!         #[fail(display = "WAD I/O error {:?}.", 0)]
//!         WadIo(PathBuf),
//!
//!         #[fail(display = "Corrupt WAD file: {}", 0)]
//!         CorruptWad(String),
//!     }
//!
//!     impl ChainErrorKind for ErrorKind {
//!         type Error = Error;
//!     }
//! }
//!
//! mod main {
//!     // main.rs
//!     use super::errors::{ErrorKind, Result};
//!     use failchain::{
//!         bail,
//!         ensure,
//!         ResultExt, // for `chain_err`,
//!     };
//!     use std::fs::File;
//!     use std::io::Read;
//!     use std::path::Path;
//!
//!     fn validate_metadata(path: &Path, metadata: &str) -> Result<()> {
//!         // `ensure` is like `assert` (or failure's ensure), except it allows you to
//!         // specify the `ErrorKind`.
//!         ensure!(
//!             !metadata.is_empty(),
//!             ErrorKind::CorruptMetadata(format!("metadata file {:?} is empty", path))
//!         );
//!
//!         // a special mode of `ensure` works for functions that return `ErrorKind`-s
//!         // and take a single string as argument:
//!         ensure!(
//!             metadata.len() > 100,
//!             ErrorKind::CorruptMetadata, // Any FnOnce(String) -> ErrorKind
//!             "metadata file {:?} is too long", // Rest of arguments are format args.
//!             path,
//!         );
//!
//!         // `bail` is like `ensure`, but without the condition and always returns
//!         // early.
//!         bail!(
//!             ErrorKind::CorruptMetadata,
//!             "validation isn't actually implemented"
//!         );
//!     }
//!
//!     fn load(wad: &Path, metadata: &Path) -> Result<()> {
//!         // `chain_err` stashes the original error as the `cause` of a new error.
//!         let wad_file = File::open(wad).chain_err(|| ErrorKind::WadIo(wad.to_owned()))?;
//!
//!         let mut metadata_content = String::new();
//!
//!         // `chain_inspect_err` stashes the original error as the `cause` of the new
//!         // error, but it first allows the callback to inspect it.
//!         let metadata_file = File::open(metadata)
//!             .and_then(|mut file| file.read_to_string(&mut metadata_content))
//!             .chain_inspect_err(|_io_error| ErrorKind::MetadataIo(metadata.to_owned()))?;
//!
//!         validate_metadata(metadata, &metadata_content)?;
//!
//!         Ok(())
//!     }
//! }
//! ```
use failure::{Backtrace, Context, Fail};
use std::fmt;

/// Trait which must be implemented by `ErrorKind`-s.
///
/// The `Error` associated type should select between `UnboxedError<Self>` or `BoxedError<Self>`.
pub trait ChainErrorKind: Fail + Sized {
    type Error: Fail + From<Context<Self>>;
}

/// An error type which stores the backtrace, cause pointer and error kind inline.
///
/// This is potentially a very large object, but it doesn't allocate on creation unlike
/// `BoxedError`.
#[derive(Debug)]
pub struct UnboxedError<ErrorKindT: Fail> {
    inner: Context<ErrorKindT>,
}

/// An error type which stores the backtrace, cause pointer and error kind behind a `Box`.
///
/// The size of this object is always one pointer. It's therefore smaller than `UnboxedError`, but
/// requires an allocation when created.
#[derive(Debug)]
pub struct BoxedError<ErrorKindT: Fail> {
    inner: Box<Context<ErrorKindT>>,
}

impl<ErrorKindT: Fail> UnboxedError<ErrorKindT> {
    pub fn kind(&self) -> &ErrorKindT {
        self.inner.get_context()
    }
}

/// Extension trait which adds the family of `.chain_err` methods to `Result` objects.
pub trait ResultExt: Sized {
    type Success;
    type Error: Fail;

    /// Replace the error in a Result with a new error built from `map`'s `ErrorKind` output.
    ///
    /// The original error is stored as the `cause`/`source` of the new one.
    fn chain_err<ErrorKindT: ChainErrorKind>(
        self,
        map: impl FnOnce() -> ErrorKindT,
    ) -> Result<Self::Success, ErrorKindT::Error> {
        self.chain_inspect_err(|_| map())
    }

    /// Like `chain_err`, but the callback is given an opportunity to inspect the original error.
    fn chain_inspect_err<ErrorKindT: ChainErrorKind>(
        self,
        map: impl FnOnce(&mut Self::Error) -> ErrorKindT,
    ) -> Result<Self::Success, ErrorKindT::Error>;
}

impl<SuccessT, ErrorT: Fail> ResultExt for Result<SuccessT, ErrorT> {
    type Success = SuccessT;
    type Error = ErrorT;

    fn chain_inspect_err<ErrorKindT: ChainErrorKind>(
        self,
        chain: impl FnOnce(&mut ErrorT) -> ErrorKindT,
    ) -> Result<Self::Success, ErrorKindT::Error> {
        self.map_err(|mut initial_error| {
            let kind = chain(&mut initial_error);
            initial_error.context(kind).into()
        })
    }
}

/// Returns early with an error built from an error kind.
///
/// Examples
/// ---
///
/// ```rust
/// // With an ErrorKind.
/// bail!(ErrorKind::CorruptFile(format!("the file at {:?} is corrupt", file_path)))
///
/// // With an ErrorKind and format string (equivalent ot the above.)
/// bail!(ErrorKind::CorruptFile, "the file at {:?} is corrupt", file_path)
/// ```
#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        return Err($e.into());
    };
    ($e:expr,) => {
        return Err($e.into());
    };
    ($kind:expr, $fmt:expr) => {
        return Err($kind(($fmt).to_owned()).into());
    };
    ($kind:expr, $fmt:expr, $($arg:tt)+) => {
        return Err($kind(format!($fmt, $($arg)+)).into());
    };
}

/// Returns early with an error built from an error kind if a given condition is false.
///
/// Examples
/// ---
///
/// ```rust
/// // With an ErrorKind.
/// ensure!(
///     file_path != corrupt_file_path,
///     ErrorKind::CorruptFile(format!("the file at {:?} is corrupt", file_path))
/// );
///
/// // With an ErrorKind and format string (equivalent ot the above.)
/// ensure!(
///     file_path != corrupt_file_path,
///     ErrorKind::CorruptFile,
///     "the file at {:?} is corrupt",
///     file_path,
/// );
/// ```
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $e:expr, $fmt:expr, $($arg:tt)+) => {
        if !($cond) {
            $crate::bail!($e, $fmt, $($arg)+);
        }
    };
    ($cond:expr, $e:expr, $fmt:expr) => {
        if !($cond) {
            $crate::bail!($e, $fmt);
        }
    };
    ($cond:expr, $e:expr,) => {
        if !($cond) {
            $crate::bail!($e);
        }
    };
    ($cond:expr, $e:expr) => {
        if !($cond) {
            $crate::bail!($e);
        }
    };
}

impl<ErrorKindT: Fail> Fail for UnboxedError<ErrorKindT> {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl<ErrorKindT: Fail> fmt::Display for UnboxedError<ErrorKindT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<ErrorKindT: Fail> From<ErrorKindT> for UnboxedError<ErrorKindT> {
    fn from(kind: ErrorKindT) -> Self {
        Self::from(Context::new(kind))
    }
}

impl<ErrorKindT: Fail> From<Context<ErrorKindT>> for UnboxedError<ErrorKindT> {
    fn from(inner: Context<ErrorKindT>) -> Self {
        Self { inner }
    }
}

impl<ErrorKindT: Fail> BoxedError<ErrorKindT> {
    pub fn kind(&self) -> &ErrorKindT {
        self.inner.get_context()
    }
}

impl<ErrorKindT: Fail> Fail for BoxedError<ErrorKindT> {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl<ErrorKindT: Fail> fmt::Display for BoxedError<ErrorKindT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<ErrorKindT: Fail> From<ErrorKindT> for BoxedError<ErrorKindT> {
    fn from(kind: ErrorKindT) -> Self {
        Self::from(Context::new(kind))
    }
}

impl<ErrorKindT: Fail> From<Context<ErrorKindT>> for BoxedError<ErrorKindT> {
    fn from(inner: Context<ErrorKindT>) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }
}
