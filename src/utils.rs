use crate::python_version::PythonVersion;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::{fs, io};
use tar::Archive;
use zstd::Decoder;

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

/// Download a Zstandard compressed tar file and unpack it to the specified directory.
pub(crate) fn download_and_unpack_zstd_archive(
    uri: &str,
    destination: &Path,
) -> Result<(), DownloadUnpackArchiveError> {
    // TODO: (W-12613141) Add a timeout: https://docs.rs/ureq/latest/ureq/struct.AgentBuilder.html?search=timeout
    // TODO: (W-12613168) Add retries for certain failure modes, eg: https://github.com/algesten/ureq/blob/05b9a82a380af013338c4f42045811fc15689a6b/src/error.rs#L39-L63
    let response = ureq::get(uri)
        .call()
        .map_err(DownloadUnpackArchiveError::Request)?;
    let zstd_decoder =
        Decoder::new(response.into_reader()).map_err(DownloadUnpackArchiveError::Unpack)?;
    Archive::new(zstd_decoder)
        .unpack(destination)
        .map_err(DownloadUnpackArchiveError::Unpack)
}

/// Errors that can occur when downloading and unpacking an archive using `download_and_unpack_zstd_archive`.
#[derive(Debug)]
pub(crate) enum DownloadUnpackArchiveError {
    Request(ureq::Error),
    Unpack(io::Error),
}

/// Determine the path to the pip module bundled in Python's standard library.
pub(crate) fn bundled_pip_module_path(
    python_layer_path: &Path,
    python_version: &PythonVersion,
) -> io::Result<PathBuf> {
    let bundled_wheels_dir = python_layer_path.join(format!(
        "lib/python{}.{}/ensurepip/_bundled",
        python_version.major, python_version.minor
    ));

    // The wheel filename includes the pip version (for example `pip-XX.Y-py3-none-any.whl`),
    // which varies from one Python release to the next (including between patch releases).
    // As such, we have to find the wheel based on the known filename prefix of `pip-`.
    for entry in fs::read_dir(bundled_wheels_dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("pip-") {
            let pip_wheel_path = entry.path();
            // The pip module exists inside the pip wheel (which is a zip file), however,
            // Python can load it directly by appending the module name to the zip filename,
            // as though it were a path. For example: `pip-XX.Y-py3-none-any.whl/pip`
            let pip_module_path = pip_wheel_path.join("pip");
            return Ok(pip_module_path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "No files found matching the pip wheel filename prefix",
    ))
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
}
