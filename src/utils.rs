use flate2::read::GzDecoder;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::{fs, io};
use tar::Archive;

// TODO: Unit test that all files from PACKAGE_MANAGER_FILES are in here.
const KNOWN_PYTHON_PROJECT_FILES: [&str; 9] = [
    ".python-version",
    "main.py",
    "manage.py",
    "Pipfile",
    "poetry.lock",
    "pyproject.toml",
    "requirements.txt",
    "runtime.txt",
    "setup.py",
];

// TODO: Unit test
pub(crate) fn is_python_project(app_dir: &Path) -> io::Result<bool> {
    // Until `Iterator::try_find` is stabilised, this is cleaner as a for loop.
    for filename in KNOWN_PYTHON_PROJECT_FILES {
        if app_dir.join(filename).try_exists()? {
            return Ok(true);
        }
    }

    Ok(false)
}

// TODO: Unit test
pub(crate) fn read_optional_file(path: &Path) -> io::Result<Option<String>> {
    fs::read_to_string(path)
        .map(Some)
        .or_else(|io_error| match io_error.kind() {
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(io_error),
        })
}

pub(crate) fn download_and_unpack_gzipped_archive(
    uri: &str,
    destination: &Path,
) -> Result<(), DownloadUnpackArchiveError> {
    // TODO: Timeouts: https://docs.rs/ureq/latest/ureq/struct.AgentBuilder.html?search=timeout
    // TODO: Retries
    let response = ureq::get(uri)
        .call()
        .map_err(DownloadUnpackArchiveError::Request)?;
    let gzip_decoder = GzDecoder::new(response.into_reader());
    Archive::new(gzip_decoder)
        .unpack(destination)
        .map_err(DownloadUnpackArchiveError::Io)
}

#[derive(Debug)]
pub(crate) enum DownloadUnpackArchiveError {
    Io(io::Error),
    Request(ureq::Error),
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
