pub use failure::{Backtrace, Context, Fail, _core};
use std::fmt;

#[derive(Debug)]
pub struct Error<ErrorKindT: Fail> {
    inner: Box<Context<ErrorKindT>>,
}

impl<ErrorKindT: Fail> Error<ErrorKindT> {
    pub fn kind(&self) -> &ErrorKindT {
        self.inner.get_context()
    }
}

impl<ErrorKindT: Fail> Fail for Error<ErrorKindT> {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl<ErrorKindT: Fail> fmt::Display for Error<ErrorKindT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<ErrorKindT: Fail> From<ErrorKindT> for Error<ErrorKindT> {
    fn from(kind: ErrorKindT) -> Self {
        Self::from(Context::new(kind))
    }
}

impl<ErrorKindT: Fail> From<Context<ErrorKindT>> for Error<ErrorKindT> {
    fn from(inner: Context<ErrorKindT>) -> Self {
        Error {
            inner: Box::new(inner),
        }
    }
}

pub trait ResultExt {
    type Success;
    type Error: Fail;

    fn chain_err<ErrorKindT: Fail, MapErrorT: Into<ErrorKindT>>(
        self,
        map: impl FnOnce(&mut Self::Error) -> MapErrorT,
    ) -> Result<Self::Success, Error<ErrorKindT>>;

    fn replace_err<ErrorKindT: Fail, MapErrorT: Into<ErrorKindT>>(
        self,
        map: impl FnOnce(Self::Error) -> MapErrorT,
    ) -> Result<Self::Success, Error<ErrorKindT>>;
}

impl<SuccessT, ErrorT: Fail> ResultExt for Result<SuccessT, ErrorT> {
    type Success = SuccessT;
    type Error = ErrorT;

    fn chain_err<ErrorKindT: Fail, MapErrorT: Into<ErrorKindT>>(
        self,
        map: impl FnOnce(&mut Self::Error) -> MapErrorT,
    ) -> Result<Self::Success, Error<ErrorKindT>> {
        self.map_err(|mut initial_error| {
            let kind = map(&mut initial_error).into();
            initial_error.context(kind).into()
        })
    }

    fn replace_err<ErrorKindT: Fail, MapErrorT: Into<ErrorKindT>>(
        self,
        map: impl FnOnce(Self::Error) -> MapErrorT,
    ) -> Result<Self::Success, Error<ErrorKindT>> {
        self.map_err(|initial_error| map(initial_error).into().into())
    }
}

#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        return Err($e.into());
    };
    ($fmt:expr, $($arg:tt)+) => {
        return Err(format!($fmt, $($arg)+).into());
    };
}
