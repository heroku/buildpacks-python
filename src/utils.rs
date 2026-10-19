use crate::python_version::PythonVersion;
use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::{fs, io};
use tar::Archive;
use zstd::Decoder as ZstdDecoder;

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
    // TODO: (W-12613168) Add retries for certain failure modes, e.g.: https://github.com/algesten/ureq/blob/05b9a82a380af013338c4f42045811fc15689a6b/src/error.rs#L39-L63
    let response = ureq::get(uri)
        .call()
        .map_err(DownloadUnpackArchiveError::Request)?;
    let zstd_decoder = ZstdDecoder::new(response.into_body().into_reader())
        .map_err(DownloadUnpackArchiveError::Unpack)?;
    Archive::new(zstd_decoder)
        .unpack(destination)
        .map_err(DownloadUnpackArchiveError::Unpack)
}

/// Download a gzip compressed tar file and unpack it to the specified directory, with optional
/// flattening of the directory structure (similar to `tar --strip-components`).
// The only consumer of this API is the `uv` layer, since uv's archive (a) uses gzip rather than zstd,
// (b) the contents are also nested within a subdirectory. Having to strip components of the path
// means we cannot use the much simpler `Archive::unpack` API and instead have to reimplement it.
// TODO: File an upstream uv issue proposing switching to a flattened archive.
// TODO: Add timeouts and retries at the same time as doing so for `download_and_unpack_zstd_archive`.
pub(crate) fn download_and_unpack_nested_gzip_archive(
    uri: &str,
    destination: &Path,
    strip_components: usize,
) -> Result<(), DownloadUnpackArchiveError> {
    let response = ureq::get(uri)
        .call()
        .map_err(DownloadUnpackArchiveError::Request)?;
    let gzip_decoder = GzDecoder::new(response.into_body().into_reader());

    fs::create_dir_all(destination).map_err(DownloadUnpackArchiveError::Unpack)?;

    for entry in Archive::new(gzip_decoder)
        .entries()
        .map_err(DownloadUnpackArchiveError::Unpack)?
    {
        let mut entry = entry.map_err(DownloadUnpackArchiveError::Unpack)?;
        let path = entry.path().map_err(DownloadUnpackArchiveError::Unpack)?;
        let stripped_path = path
            .components()
            .skip(strip_components)
            .collect::<PathBuf>();

        // The path will be empty here if there were files shallower than the strip_components depth.
        if !stripped_path.as_os_str().is_empty() {
            entry
                .unpack(destination.join(stripped_path))
                .map_err(DownloadUnpackArchiveError::Unpack)?;
        }
    }

    Ok(())
}

/// Errors that can occur when downloading and unpacking an archive.
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
/// to the user and checks that the exit status of the process was zero.
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
/// and checks that the exit status of the process was zero.
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
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::time::Duration;

    /// Starts a bare-bones HTTP server on an OS-assigned loopback port that accepts exactly one
    /// connection, reads its request line, sends back a minimal 200 response, and reports the
    /// request line (e.g. `"GET http://127.0.0.2:1234/foo HTTP/1.1"`) over the returned channel.
    /// Used to observe, at the socket level, which address a request actually landed on --
    /// whether that's the real target (no proxy in the way) or a proxy (forwarded, full-URI
    /// request line, per RFC 7230 section 5.3.2).
    fn spawn_observer() -> (std::net::SocketAddr, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            tx.send(request_line.trim_end().to_string()).unwrap();
            write_minimal_response(stream);
        });
        (addr, rx)
    }

    fn write_minimal_response(mut stream: TcpStream) {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
    }

    /// Proves, at the socket level, that a plain `ureq::get(uri).call()` (the same call the
    /// Python/uv downloads in this file make) honours `HTTP_PROXY`/`NO_PROXY` on ureq 3 with no
    /// buildpack-side proxy-handling code at all -- unlike ureq 2, where this same call ignores
    /// both entirely (see #630/#631).
    ///
    /// Not safe to run concurrently with other tests that touch these env vars, hence
    /// `--test-threads=1` (or a `#[serial]`-style guard, if this were merged for real).
    #[test]
    #[allow(unsafe_code)]
    fn ureq_default_agent_honours_http_proxy_and_no_proxy() {
        let (proxy_addr, proxy_rx) = spawn_observer();
        let (direct_addr, direct_rx) = spawn_observer();

        // SAFETY: this test doesn't spawn threads of its own that read these vars concurrently;
        // the two request/response round trips below are strictly sequential.
        unsafe {
            std::env::set_var("HTTP_PROXY", format!("http://{proxy_addr}"));
            // Exact-IP entry: 127.0.0.1's own requests bypass the proxy. The proxy observer
            // above is deliberately also bound to 127.0.0.1 -- that's the proxy's *own* address,
            // which is never matched against NO_PROXY; only a request's *target* host is.
            std::env::set_var("NO_PROXY", "127.0.0.1");
        }

        // Target A: a different loopback address, not covered by NO_PROXY -- expect this
        // request to go to the proxy. ureq 3 tunnels every proxied request via CONNECT
        // (RFC 7231 section 4.3.6), even for a plain http:// target, rather than the older
        // full-URI GET-to-proxy style -- so the proxy sees a CONNECT to the target's host:port.
        let target_host_port = format!("127.0.0.2:{}", direct_addr.port());
        let target_a = format!("http://{target_host_port}/via-proxy");
        let _ = ureq::get(&target_a).call();
        let proxy_request_line = proxy_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            proxy_request_line,
            format!("CONNECT {target_host_port} HTTP/1.1")
        );

        // Target B: 127.0.0.1, covered by NO_PROXY -- expect this request to land directly on
        // the target listener, with an origin-form request line (no scheme/host), and for
        // nothing to arrive at the proxy a second time.
        let target_b = format!("http://{direct_addr}/no-proxy");
        let _ = ureq::get(&target_b).call();
        let direct_request_line = direct_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(direct_request_line, "GET /no-proxy HTTP/1.1");
        assert!(
            proxy_rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "the NO_PROXY'd request should never have reached the proxy"
        );

        // SAFETY: same reasoning as above -- no concurrent readers in this test.
        unsafe {
            std::env::remove_var("HTTP_PROXY");
            std::env::remove_var("NO_PROXY");
        }
    }

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
