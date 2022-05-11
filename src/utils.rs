use flate2::read::GzDecoder;
use std::path::Path;
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

pub(crate) fn _read_optional_file2(path: &Path) -> io::Result<Option<String>> {
    fs::read_to_string(path).map(Some).or_else(|io_error| {
        if io_error.kind() == io::ErrorKind::NotFound {
            Ok(None)
        } else {
            Err(io_error)
        }
    })
}

pub(crate) fn _read_optional_file_option(path: &Path) -> Option<io::Result<String>> {
    match fs::read_to_string(path) {
        Err(io_error) if io_error.kind() == io::ErrorKind::NotFound => None,
        other => Some(other),
    }
}

pub(crate) fn download_and_unpack_gzip(
    uri: &str,
    destination: &Path,
) -> Result<(), DownloadUnpackError> {
    let response = ureq::get(uri)
        .call()
        .map_err(|err| DownloadUnpackError::RequestError(Box::new(err)))?;
    let gzip_decoder = GzDecoder::new(response.into_reader());
    Archive::new(gzip_decoder)
        .unpack(destination)
        .map_err(DownloadUnpackError::IoError)
}

#[derive(Debug)]
pub(crate) enum DownloadUnpackError {
    // Boxed to prevent `large_enum_variant` Clippy errors since `ureq::Error` is massive.
    RequestError(Box<ureq::Error>),
    IoError(io::Error),
}
