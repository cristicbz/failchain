mod errors {
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
}

mod main {
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
}

fn main() {}
