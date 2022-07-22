use crate::python_version::PythonVersion;
use crate::utils;
use std::io;
use std::path::Path;

// TODO: Add tests for `get_version`? Or test caller? Or integration test?
//
// Possible tests:
// - some IO error -> Err(RuntimeTxtError::Io)
// - file present but invalid -> Err(RuntimeTxtError::Parse)
// - file present and valid -> Ok(Some(python_version))
// - file not present -> Ok(None)
pub(crate) fn get_version(app_dir: &Path) -> Result<Option<PythonVersion>, RuntimeTxtError> {
    let runtime_txt_path = app_dir.join("runtime.txt");

    utils::read_optional_file(&runtime_txt_path)
        .map_err(RuntimeTxtError::Io)?
        .map(|contents| parse(&contents).map_err(RuntimeTxtError::Parse))
        .transpose()
}

/// Parse the contents of a `runtime.txt` file into a [`PythonVersion`].
///
/// The file is expected to contain a string of form `python-X.Y.Z`.
fn parse(contents: &str) -> Result<PythonVersion, RuntimeTxtParseError> {
    let trimmed_contents = contents.trim();

    // Ease debugging when non-ascii characters have inadvertently been used,
    // such as when an editor has auto-corrected the hyphen to an en/em dash.
    if !trimmed_contents.is_ascii() {
        return Err(RuntimeTxtParseError::NotAscii {
            escaped_file_contents: trimmed_contents.escape_default().to_string(),
        });
    }

    let (runtime, version) = trimmed_contents.split_once('-').ok_or_else(|| {
        RuntimeTxtParseError::MissingRuntimePrefix {
            file_contents: trimmed_contents.to_string(),
        }
    })?;

    match runtime {
        "python" => {
            match version
                .split('.')
                .map(str::parse)
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default()
                .as_slice()
            {
                &[major, minor, patch] => Ok(PythonVersion::new(major, minor, patch)),
                _ => Err(RuntimeTxtParseError::InvalidVersion {
                    version: version.to_string(),
                    file_contents: trimmed_contents.to_string(),
                }),
            }
        }
        _ if runtime.starts_with("pypy") => Err(RuntimeTxtParseError::PyPyNotSupported),
        _ => Err(RuntimeTxtParseError::InvalidRuntimePrefix {
            runtime: runtime.to_string(),
            file_contents: trimmed_contents.to_string(),
        }),
    }
}

#[derive(Debug)]
pub(crate) enum RuntimeTxtError {
    Io(io::Error),
    Parse(RuntimeTxtParseError),
}

#[derive(Debug, PartialEq)]
pub(crate) enum RuntimeTxtParseError {
    InvalidRuntimePrefix {
        runtime: String,
        file_contents: String,
    },
    InvalidVersion {
        version: String,
        file_contents: String,
    },
    MissingRuntimePrefix {
        file_contents: String,
    },
    NotAscii {
        escaped_file_contents: String,
    },
    PyPyNotSupported,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid() {
        assert_eq!(parse("python-1.2.3"), Ok(PythonVersion::new(1, 2, 3)));
        assert_eq!(parse("python-98.76.54"), Ok(PythonVersion::new(98, 76, 54)));
        assert_eq!(
            parse("\n   python-1.2.3   \n"),
            Ok(PythonVersion::new(1, 2, 3))
        );
    }

    #[test]
    fn parse_not_ascii() {
        // En dash.
        assert_eq!(
            parse("python–1.2.3"),
            Err(RuntimeTxtParseError::NotAscii {
                escaped_file_contents: "python\\u{2013}1.2.3".to_string()
            })
        );
        // Em dash.
        assert_eq!(
            parse("python—1.2.3"),
            Err(RuntimeTxtParseError::NotAscii {
                escaped_file_contents: "python\\u{2014}1.2.3".to_string()
            })
        );
    }

    #[test]
    fn parse_missing_runtime_prefix() {
        assert_eq!(
            parse(""),
            Err(RuntimeTxtParseError::MissingRuntimePrefix {
                file_contents: "".to_string()
            })
        );
        assert_eq!(
            parse("1.2.3"),
            Err(RuntimeTxtParseError::MissingRuntimePrefix {
                file_contents: "1.2.3".to_string()
            })
        );
        assert_eq!(
            parse("\n   python 1.2.3   \n"),
            Err(RuntimeTxtParseError::MissingRuntimePrefix {
                file_contents: "python 1.2.3".to_string()
            })
        );
    }

    #[test]
    fn parse_unsupported_runtime_prefix() {
        assert_eq!(
            parse("pypy-1.2.3"),
            Err(RuntimeTxtParseError::PyPyNotSupported)
        );
        assert_eq!(
            parse("pypy3.6-1.2.3"),
            Err(RuntimeTxtParseError::PyPyNotSupported)
        );
    }

    #[test]
    fn parse_invalid_runtime_prefix() {
        assert_eq!(
            parse("abc-1.2.3"),
            Err(RuntimeTxtParseError::InvalidRuntimePrefix {
                runtime: "abc".to_string(),
                file_contents: "abc-1.2.3".to_string()
            })
        );
        assert_eq!(
            parse("python - 1.2.3"),
            Err(RuntimeTxtParseError::InvalidRuntimePrefix {
                runtime: "python ".to_string(),
                file_contents: "python - 1.2.3".to_string()
            })
        );
        assert_eq!(
            parse("\n  -1.2.3  \n"),
            Err(RuntimeTxtParseError::InvalidRuntimePrefix {
                runtime: "".to_string(),
                file_contents: "-1.2.3".to_string()
            })
        );
    }

    #[test]
    fn parse_invalid_version() {
        assert_eq!(
            parse("python-1"),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: "1".to_string(),
                file_contents: "python-1".to_string()
            })
        );
        assert_eq!(
            parse("python-1.2"),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: "1.2".to_string(),
                file_contents: "python-1.2".to_string()
            })
        );
        assert_eq!(
            parse("python-1.2.3.4"),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: "1.2.3.4".to_string(),
                file_contents: "python-1.2.3.4".to_string()
            })
        );
        assert_eq!(
            parse("python-1..3"),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: "1..3".to_string(),
                file_contents: "python-1..3".to_string()
            })
        );
        assert_eq!(
            parse("python-1.2.3."),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: "1.2.3.".to_string(),
                file_contents: "python-1.2.3.".to_string()
            })
        );
        assert_eq!(
            parse("python- 1.2.3"),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: " 1.2.3".to_string(),
                file_contents: "python- 1.2.3".to_string()
            })
        );
        assert_eq!(
            parse("\n   python-1.2.3a   \n"),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: "1.2.3a".to_string(),
                file_contents: "python-1.2.3a".to_string()
            })
        );
        // These are valid semver versions, but not supported Python versions.
        assert_eq!(
            parse("python-1.2.3-dev"),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: "1.2.3-dev".to_string(),
                file_contents: "python-1.2.3-dev".to_string()
            })
        );
        assert_eq!(
            parse("python-1.2.3+abc"),
            Err(RuntimeTxtParseError::InvalidVersion {
                version: "1.2.3+abc".to_string(),
                file_contents: "python-1.2.3+abc".to_string()
            })
        );
    }
}
