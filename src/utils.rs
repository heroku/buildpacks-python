use flate2::read::GzDecoder;
use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use std::{fs, io};
use tar::Archive;

/// Filenames that if found in a project mean it should be treated as a Python project,
/// and so pass this buildpack's detection phase.
///
/// This list is deliberately larger than just the list of supported package manager files,
/// so that Python projects that are missing some of the required files still pass detection,
/// allowing us to show a more detailed error message during the build phase than is possible
/// during detect.
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

/// Returns whether the specified project directory is that of a Python project, and so
/// should pass buildpack detection.
pub(crate) fn is_python_project(app_dir: &Path) -> io::Result<bool> {
    // Until `Iterator::try_find` is stabilised, this is cleaner as a for loop.
    for filename in KNOWN_PYTHON_PROJECT_FILES {
        if app_dir.join(filename).try_exists()? {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Read the contents of the provided filepath if the file exists, gracefully handling
/// the file not being present, but still returning any other form of IO error.
pub(crate) fn read_optional_file(path: &Path) -> io::Result<Option<String>> {
    fs::read_to_string(path)
        .map(Some)
        .or_else(|io_error| match io_error.kind() {
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(io_error),
        })
}

/// Download a gzipped tar file and unpack it to the specified directory.
pub(crate) fn download_and_unpack_gzipped_archive(
    uri: &str,
    destination: &Path,
) -> Result<(), DownloadUnpackArchiveError> {
    // TODO: (W-12613141) Add a timeout: https://docs.rs/ureq/latest/ureq/struct.AgentBuilder.html?search=timeout
    // TODO: (W-12613168) Add retries for certain failure modes, eg: https://github.com/algesten/ureq/blob/05b9a82a380af013338c4f42045811fc15689a6b/src/error.rs#L39-L63
    let response = ureq::get(uri)
        .call()
        .map_err(DownloadUnpackArchiveError::Request)?;
    let gzip_decoder = GzDecoder::new(response.into_reader());
    Archive::new(gzip_decoder)
        .unpack(destination)
        .map_err(DownloadUnpackArchiveError::Unpack)
}

/// Errors that can occur when downloading and unpacking an archive using `download_and_unpack_gzipped_archive`.
#[derive(Debug)]
pub(crate) enum DownloadUnpackArchiveError {
    Request(ureq::Error),
    Unpack(io::Error),
}

/// A helper for running an external process using [`Command`], that streams stdout/stderr
/// to the user and checks that the exit status of the process was non-zero.
pub(crate) fn run_command_and_stream_output(
    command: &mut Command,
) -> Result<(), StreamedCommandError> {
    command
        .status()
        .map_err(StreamedCommandError::Io)
        .and_then(|exit_status| {
            if exit_status.success() {
                Ok(())
            } else {
                Err(StreamedCommandError::NonZeroExitStatus(exit_status))
            }
        })
}

/// A helper for running an external process using [`Command`], that captures stdout/stderr
/// and checks that the exit status of the process was non-zero.
pub(crate) fn run_command_and_capture_output(
    command: &mut Command,
) -> Result<Output, CapturedCommandError> {
    command
        .output()
        .map_err(CapturedCommandError::Io)
        .and_then(|output| {
            if output.status.success() {
                Ok(output)
            } else {
                Err(CapturedCommandError::NonZeroExitStatus(output))
            }
        })
}

/// Errors that can occur when running an external process using `run_command_and_stream_output`.
#[derive(Debug)]
pub(crate) enum StreamedCommandError {
    Io(io::Error),
    NonZeroExitStatus(ExitStatus),
}

/// Errors that can occur when running an external process using `run_command_and_capture_output`.
#[derive(Debug)]
pub(crate) enum CapturedCommandError {
    Io(io::Error),
    NonZeroExitStatus(Output),
}

/// Convert a [`libcnb::Env`] to a sorted vector of key-value string slice tuples, for easier
/// testing of the environment variables set in the buildpack layers.
#[cfg(test)]
pub(crate) fn environment_as_sorted_vector(environment: &libcnb::Env) -> Vec<(&str, &str)> {
    let mut result: Vec<(&str, &str)> = environment
        .iter()
        .map(|(k, v)| (k.to_str().unwrap(), v.to_str().unwrap()))
        .collect();

    result.sort_by_key(|kv| kv.0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_manager::PACKAGE_MANAGER_FILE_MAPPING;

    #[test]
    fn is_python_project_valid_project() {
        assert!(is_python_project(Path::new("tests/fixtures/pyproject_toml_only")).unwrap());
    }

    #[test]
    fn is_python_project_empty() {
        assert!(!is_python_project(Path::new("tests/fixtures/empty")).unwrap());
    }

    #[test]
    fn is_python_project_io_error() {
        assert!(is_python_project(Path::new("tests/fixtures/empty/.gitkeep")).is_err());
    }

    #[test]
    fn read_optional_file_valid_file() {
        assert_eq!(
            read_optional_file(Path::new("tests/fixtures/python_3.7/runtime.txt")).unwrap(),
            Some("python-3.7.17\n".to_string())
        );
    }

    #[test]
    fn read_optional_file_missing_file() {
        assert_eq!(
            read_optional_file(Path::new(
                "tests/fixtures/non-existent-dir/non-existent-file"
            ))
            .unwrap(),
            None
        );
    }

    #[test]
    fn read_optional_file_io_error() {
        assert!(read_optional_file(Path::new("tests/fixtures/")).is_err());
    }

    #[test]
    fn known_python_project_files_contains_all_package_manager_files() {
        assert!(PACKAGE_MANAGER_FILE_MAPPING
            .iter()
            .all(|(filename, _)| { KNOWN_PYTHON_PROJECT_FILES.contains(filename) }));
    }
}
