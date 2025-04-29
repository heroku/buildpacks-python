use crate::python_version::PythonVersion;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::{fs, io};
use tar::Archive;
use zstd::Decoder;

/// Check if the specified file exists.
///
/// Returns `Ok(true)` if the file exists, `Ok(false)` if it does not, and
/// an error if it was not possible to determine either way. Permissions are
/// not required on the file itself to determine its existence, only on its
/// parent directories.
///
/// Errors can be caused by:
/// - Insufficient permissions on a parent directory, preventing reading its contents.
///   (Although this is unlikely, since both Git and Pack prevent including directories
///   without read permissions.)
/// - Various other filesystem/OS errors.
///
/// In practice, I don't believe this error can ever be triggered by users unless they
/// use a custom or inline buildpack that changes permissions within the container.
pub(crate) fn file_exists(path: &Path) -> Result<bool, FileExistsError> {
    path.try_exists().or_else(|io_error| match io_error.kind() {
        // The `NotADirectory` case occurs when a *parent directory* in the path turns out
        // to be a file instead. For example, if we were checking for `foo/bar.toml` and
        // the user had created a file named `foo` in the specified directory.
        io::ErrorKind::NotADirectory => Ok(false),
        _ => Err(FileExistsError {
            io_error,
            path: path.to_path_buf(),
        }),
    })
}

/// An I/O error that occurred when checking if the specified file exists.
#[derive(Debug)]
pub(crate) struct FileExistsError {
    pub(crate) io_error: io::Error,
    pub(crate) path: PathBuf,
}

/// Read the contents of the provided filepath if the file exists, gracefully handling
/// the file not being present, but still returning any other form of I/O error.
///
/// Errors can be caused by:
/// - Insufficient permissions to read the file or a parent directory. (Although this is
///   unlikely, since both Git and Pack prevent including files without read permissions.)
/// - The file containing invalid UTF-8.
/// - Various other filesystem/OS errors.
pub(crate) fn read_optional_file(path: &Path) -> Result<Option<String>, ReadOptionalFileError> {
    fs::read_to_string(path)
        .map(Some)
        .or_else(|io_error| match io_error.kind() {
            // The `IsADirectory` case occurs when the user has created a directory with the
            // same name as the file we are trying to read. For example, if we were reading
            // `foo.toml` and there was a directory named `foo.toml` in the app dir.
            // The `NotADirectory` case occurs when a *parent directory* in the path turns out
            // to be a file instead. For example, if we were reading `foo/bar.toml` and the
            // user had created a file named `foo` in the specified directory.
            io::ErrorKind::NotFound
            | io::ErrorKind::IsADirectory
            | io::ErrorKind::NotADirectory => Ok(None),
            _ => Err(ReadOptionalFileError {
                io_error,
                path: path.to_path_buf(),
            }),
        })
}

/// An I/O error that occurred when reading the specified optional file.
#[derive(Debug)]
pub(crate) struct ReadOptionalFileError {
    pub(crate) io_error: io::Error,
    pub(crate) path: PathBuf,
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
///
/// The wheel filename includes the pip version (for example `pip-XX.Y-py3-none-any.whl`),
/// which varies from one Python release to the next (including between patch releases).
/// As such, we have to find the wheel based on the known filename prefix of `pip-`.
pub(crate) fn bundled_pip_module_path(
    python_layer_path: &Path,
    python_version: &PythonVersion,
) -> Result<PathBuf, FindBundledPipError> {
    let bundled_wheels_dir = python_layer_path.join(format!(
        "lib/python{}.{}/ensurepip/_bundled",
        python_version.major, python_version.minor
    ));

    let pip_wheel_path = fs::read_dir(&bundled_wheels_dir)
        .map_err(|io_error| FindBundledPipError {
            bundled_wheels_dir: bundled_wheels_dir.clone(),
            io_error,
        })?
        .find_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_name().to_string_lossy().starts_with("pip-") {
                Some(entry.path())
            } else {
                None
            }
        })
        .ok_or(FindBundledPipError {
            bundled_wheels_dir,
            io_error: io::Error::new(
                io::ErrorKind::NotFound,
                "No files found matching the pip wheel filename prefix",
            ),
        })?;

    // The pip module exists inside the pip wheel (which is a zip file), however,
    // Python can load it directly by appending the module name to the zip filename,
    // as though it were a path. For example: `pip-XX.Y-py3-none-any.whl/pip`
    let pip_module_path = pip_wheel_path.join("pip");

    Ok(pip_module_path)
}

/// Errors that can occur when finding the pip module bundled in Python's standard library.
#[derive(Debug)]
pub(crate) struct FindBundledPipError {
    pub(crate) bundled_wheels_dir: PathBuf,
    pub(crate) io_error: io::Error,
}

/// A helper for running an external process using [`Command`], that streams stdout/stderr
/// to the user and checks that the exit status of the process was non-zero.
pub(crate) fn run_command_and_stream_output(
    command: &mut Command,
) -> Result<(), StreamedCommandError> {
    command
        .status()
        .map_err(|io_error| {
            StreamedCommandError::Io(CommandIoError {
                program: command.get_program().to_string_lossy().to_string(),
                io_error,
            })
        })
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
        .map_err(|io_error| {
            CapturedCommandError::Io(CommandIoError {
                program: command.get_program().to_string_lossy().to_string(),
                io_error,
            })
        })
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
    Io(CommandIoError),
    NonZeroExitStatus(ExitStatus),
}

/// Errors that can occur when running an external process using `run_command_and_capture_output`.
#[derive(Debug)]
pub(crate) enum CapturedCommandError {
    Io(CommandIoError),
    NonZeroExitStatus(Output),
}

/// I/O error that occurred while spawning/waiting on a command,
/// such as when the program wasn't found.
#[derive(Debug)]
pub(crate) struct CommandIoError {
    pub(crate) program: String,
    pub(crate) io_error: io::Error,
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
    fn file_exists_valid_file() {
        assert!(file_exists(Path::new("tests/fixtures/python_3.11/.python-version")).unwrap());
    }

    #[test]
    fn file_exists_missing_file() {
        assert!(
            !file_exists(Path::new(
                "tests/fixtures/non-existent-dir/non-existent-file"
            ))
            .unwrap()
        );
        // Tests the `NotADirectory` case (when a parent directory in the path turns out to be a file instead).
        assert!(!file_exists(Path::new("README.md/non-existent-file")).unwrap());
    }

    #[test]
    fn file_exists_io_error() {
        // It's actually quite hard to force the underlying `Path::try_exists` to return an I/O error,
        // since it returns `Ok(false)` in many cases that you would think might be an error. However,
        // one way to do so is by using invalid characters in the path (such as a NUL byte).
        let path = Path::new("\0/invalid");
        let err = file_exists(path).unwrap_err();
        assert_eq!(err.path, path);
    }

    #[test]
    fn read_optional_file_valid_file() {
        assert_eq!(
            read_optional_file(Path::new("tests/fixtures/python_3.11/.python-version")).unwrap(),
            Some("3.11\n".to_string())
        );
    }

    #[test]
    fn read_optional_file_missing_file() {
        // Tests the `io::ErrorKind::NotFound` case.
        assert_eq!(
            read_optional_file(Path::new(
                "tests/fixtures/non-existent-dir/non-existent-file"
            ))
            .unwrap(),
            None
        );
        // Tests the `io::ErrorKind::IsADirectory` case.
        assert_eq!(
            read_optional_file(Path::new("tests/fixtures/")).unwrap(),
            None
        );
        // Tests the `io::ErrorKind::NotADirectory` case.
        assert_eq!(
            read_optional_file(Path::new("README.md/non-existent-file")).unwrap(),
            None
        );
    }

    #[test]
    fn read_optional_file_io_error() {
        let path = Path::new("tests/fixtures/python_version_file_invalid_unicode/.python-version");
        let err = read_optional_file(path).unwrap_err();
        assert_eq!(err.path, path);
    }

    #[test]
    fn run_command_and_stream_output_success() {
        run_command_and_stream_output(Command::new("bash").args(["-c", "true"])).unwrap();
    }

    #[test]
    fn run_command_and_stream_output_io_error() {
        assert!(matches!(
            run_command_and_stream_output(&mut Command::new("non-existent-command")).unwrap_err(),
            StreamedCommandError::Io(_)
        ));
    }

    #[test]
    fn run_command_and_stream_output_non_zero_exit_status() {
        assert!(matches!(
            run_command_and_stream_output(Command::new("bash").args(["-c", "false"])).unwrap_err(),
            StreamedCommandError::NonZeroExitStatus(_)
        ));
    }

    #[test]
    fn run_command_and_capture_output_success() {
        let output =
            run_command_and_capture_output(Command::new("bash").args(["-c", "echo output"]))
                .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "output\n");
    }

    #[test]
    fn run_command_and_capture_output_io_error() {
        assert!(matches!(
            run_command_and_capture_output(&mut Command::new("non-existent-command")).unwrap_err(),
            CapturedCommandError::Io(_)
        ));
    }

    #[test]
    fn run_command_and_capture_output_non_zero_exit_status() {
        assert!(matches!(
            run_command_and_capture_output(Command::new("bash").args(["-c", "false"])).unwrap_err(),
            CapturedCommandError::NonZeroExitStatus(_)
        ));
    }
}
