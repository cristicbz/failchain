failchain
---

* [crates.io](https://crates.io/crates/failchain)
* [Documentation](https://docs.rs/failchain)

`failchain` is a tiny companion crate to the [`failure`](https://boats.gitlab.io/failure/intro.html)
crate, which aims to recover the ergonomics of
[`error_chain`](https://github.com/rust-lang-nursery/error-chain). It does this
by bringing back:
  * `chain_err`
  * non-verbose `Error`, `ErrorKind` pairs
  * support for `bail!` and `ensure!` with custom `ErrorKind`-s

The `failure` library recommends three different patterns for errors. This
library implementes the most complex one (and the most useful one) but without all the [boilerplate](https://boats.gitlab.io/failure/error-errorkind.html).


### What it looks like

```rust
// errors.rs
use failchain::{BoxedError, ChainErrorKind};
use failure::Fail;
use std::path::PathBuf;
use std::result::Result as StdResult;

pub type Error = BoxedError<ErrorKind>; // Use `UnboxedError` instead for
                                        // non-allocating, but bigger `Error`.
pub type Result<T> = StdResult<T, Error>;

#[derive(Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "Metadata I/O error {:?}.", 0)]
    MetadataIo(PathBuf),

    #[fail(display = "Corrupt metadata file: {}", 0)]
    CorruptMetadata(String),

    #[fail(display = "WAD I/O error {:?}.", 0)]
    WadIo(PathBuf),

    #[fail(display = "Corrupt WAD file: {}", 0)]
    CorruptWad(String),
}

impl ChainErrorKind for ErrorKind {
    type Error = Error;
}


// main.rs
use super::errors::{ErrorKind, Result};
use failchain::{
    bail,
    ensure,
    ResultExt, // for `chain_err`,
};
use std::fs::File;
use std::io::Read;
use std::path::Path;

fn validate_metadata(path: &Path, metadata: &str) -> Result<()> {
    // `ensure` is like `assert` (or failure's ensure), except it allows you to
    // specify the `ErrorKind`.
    ensure!(
        !metadata.is_empty(),
        ErrorKind::CorruptMetadata(format!("metadata file {:?} is empty", path))
    );

    // a special mode of `ensure` works for functions that return `ErrorKind`-s
    // and take a single string as argument:
    ensure!(
        metadata.len() > 100,
        ErrorKind::CorruptMetadata, // Any FnOnce(String) -> ErrorKind
        "metadata file {:?} is too long", // Rest of arguments are format args.
        path,
    );

    // `bail` is like `ensure`, but without the condition and always returns
    // early.
    bail!(
        ErrorKind::CorruptMetadata,
        "validation isn't actually implemented"
    );
}

fn load(wad: &Path, metadata: &Path) -> Result<()> {
    // `chain_err` stashes the original error as the `cause` of a new error.
    let wad_file = File::open(wad).chain_err(|| ErrorKind::WadIo(wad.to_owned()))?;

    let mut metadata_content = String::new();

    // `chain_inspect_err` stashes the original error as the `cause` of the new
    // error, but it first allows the callback to inspect it.
    let metadata_file = File::open(metadata)
        .and_then(|mut file| file.read_to_string(&mut metadata_content))
        .chain_inspect_err(|_io_error| ErrorKind::MetadataIo(metadata.to_owned()))?;

    validate_metadata(metadata, &metadata_content)?;

    Ok(())
}
```

### Future Plans

This library is a proof-of-concept and was developed as part of the `error_chain` ->
`failure` migration of [rust-doom](https://github.com/cristicbz/rust-doom) and inspired by
@burntsushi's error handling solution in [imdb-rename](https://github.com/BurntSushi/imdb-rename/blob/master/imdb-index/src/error.rs
).

The main issue with the current library is that `failchain` must be leaked as a public dependency
via the `Error` type-alias. This would be easily solved with a custom derive which implements the
necessary `From` and `Fail` impl-s e.g.
```rust
#[derive(ChainError)]
pub struct Error(BoxedError<ErrorKind>);


// expanding to
impl Fail for Error { /* delegate */ }
impl From<ErrorKind> for Error { /* delegate */ }
```

The error handling story in Rust is a little up in the air right now, but in my opinion any eventual
solution should include these pieces (as `error_chain` did).

### Versioning

`failure` is a public dependency of `failchain`. The library versioning scheme is
`<failchain_major><failure_version><failchain_minor>`. So `0.1015.2` would be version `0.1.2` of
`failchain` with a dependency on version `0.1.5` of `failure`.
