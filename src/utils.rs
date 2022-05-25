use flate2::read::GzDecoder;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::{fs, io};
use tar::Archive;

pub(crate) fn read_optional_file(path: &Path) -> io::Result<Option<String>> {
    fs::read_to_string(path)
        .map(Some)
        .or_else(|io_error| match io_error.kind() {
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(io_error),
        })
}

pub(crate) fn download_and_unpack_gzip(
    uri: &str,
    destination: &Path,
) -> Result<(), DownloadUnpackError> {
    let response = ureq::get(uri)
        .call()
        .map_err(|err| DownloadUnpackError::Request(Box::new(err)))?;
    let gzip_decoder = GzDecoder::new(response.into_reader());
    Archive::new(gzip_decoder)
        .unpack(destination)
        .map_err(DownloadUnpackError::Io)
}

#[derive(Debug)]
pub(crate) enum DownloadUnpackError {
    Io(io::Error),
    // Boxed to prevent `large_enum_variant` Clippy errors since `ureq::Error` is massive.
    Request(Box<ureq::Error>),
}

pub(crate) fn run_command(command: &mut Command) -> Result<(), CommandError> {
    command
        .status()
        .map_err(CommandError::Io)
        .and_then(|exit_status| {
            if exit_status.success() {
                Ok(())
            } else {
                Err(CommandError::NonZeroExitStatus(exit_status))
            }
        })
}

#[derive(Debug)]
pub(crate) enum CommandError {
    Io(io::Error),
    NonZeroExitStatus(ExitStatus),
}
