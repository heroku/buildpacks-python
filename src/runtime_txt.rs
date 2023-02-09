use crate::python_version::PythonVersion;
use crate::utils;
use std::io;
use std::path::Path;

/// Retrieve a parsed Python version from a `runtime.txt` file if it exists in the
/// specified project directory.
///
/// Returns `Ok(None)` if the file does not exist, but returns the error for all other
/// forms of IO or parsing errors.
pub(crate) fn read_version(app_dir: &Path) -> Result<Option<PythonVersion>, RuntimeTxtError> {
    let runtime_txt_path = app_dir.join("runtime.txt");

    utils::read_optional_file(&runtime_txt_path)
        .map_err(RuntimeTxtError::Io)?
        .map(|contents| parse(&contents).map_err(RuntimeTxtError::Parse))
        .transpose()
}

/// Parse the contents of a `runtime.txt` file into a [`PythonVersion`].
///
/// The file is expected to contain a string of form `python-X.Y.Z`.
/// Any leading or trailing whitespace will be removed.
fn parse(contents: &str) -> Result<PythonVersion, ParseRuntimeTxtError> {
    // All leading/trailing whitespace is trimmed, since that's what the classic buildpack
    // permitted (however it's primarily trailing newlines that we need to support). The
    // string is then escaped, to aid debugging when non-ascii characters have inadvertently
    // been used, such as when an editor has auto-corrected the hyphen to an en/em dash.
    let cleaned_contents = contents.trim().escape_default().to_string();

    let version_substring =
        cleaned_contents
            .strip_prefix("python-")
            .ok_or_else(|| ParseRuntimeTxtError {
                cleaned_contents: cleaned_contents.clone(),
            })?;

    match version_substring
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default()
        .as_slice()
    {
        &[major, minor, patch] => Ok(PythonVersion::new(major, minor, patch)),
        _ => Err(ParseRuntimeTxtError {
            cleaned_contents: cleaned_contents.clone(),
        }),
    }
}

/// Errors that can occur when reading and parsing a `runtime.txt` file.
#[derive(Debug)]
pub(crate) enum RuntimeTxtError {
    Io(io::Error),
    Parse(ParseRuntimeTxtError),
}

/// Errors that can occur when parsing the contents of a `runtime.txt` file.
#[derive(Debug, PartialEq)]
pub(crate) struct ParseRuntimeTxtError {
    pub cleaned_contents: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid() {
        assert_eq!(parse("python-1.2.3"), Ok(PythonVersion::new(1, 2, 3)));
        assert_eq!(
            parse("python-987.654.3210"),
            Ok(PythonVersion::new(987, 654, 3210))
        );
        assert_eq!(
            parse("\n   python-1.2.3   \n"),
            Ok(PythonVersion::new(1, 2, 3))
        );
    }

    #[test]
    fn parse_invalid_prefix() {
        assert_eq!(
            parse(""),
            Err(ParseRuntimeTxtError {
                cleaned_contents: String::new()
            })
        );
        assert_eq!(
            parse("1.2.3"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "1.2.3".to_string()
            })
        );
        assert_eq!(
            parse("python 1.2.3"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python 1.2.3".to_string()
            })
        );
        assert_eq!(
            parse("python -1.2.3"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python -1.2.3".to_string()
            })
        );
        assert_eq!(
            parse("abc-1.2.3"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "abc-1.2.3".to_string()
            })
        );
        assert_eq!(
            parse("\n  -1.2.3  \n"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "-1.2.3".to_string()
            })
        );
        assert_eq!(
            // En dash.
            parse("python–1.2.3"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python\\u{2013}1.2.3".to_string()
            })
        );
        assert_eq!(
            // Em dash.
            parse("python—1.2.3"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python\\u{2014}1.2.3".to_string()
            })
        );
    }

    #[test]
    fn parse_invalid_version() {
        assert_eq!(
            parse("python-1"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python-1".to_string(),
            })
        );
        assert_eq!(
            parse("python-1.2"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python-1.2".to_string(),
            })
        );
        assert_eq!(
            parse("python-1.2.3.4"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python-1.2.3.4".to_string(),
            })
        );
        assert_eq!(
            parse("python-1..3"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python-1..3".to_string(),
            })
        );
        assert_eq!(
            parse("python-1.2.3."),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python-1.2.3.".to_string(),
            })
        );
        assert_eq!(
            parse("python- 1.2.3"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python- 1.2.3".to_string(),
            })
        );
        assert_eq!(
            parse("\n   python-1.2.3a   \n"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python-1.2.3a".to_string(),
            })
        );
        // These are valid semver versions, but not supported Python versions.
        assert_eq!(
            parse("python-1.2.3-dev"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python-1.2.3-dev".to_string(),
            })
        );
        assert_eq!(
            parse("python-1.2.3+abc"),
            Err(ParseRuntimeTxtError {
                cleaned_contents: "python-1.2.3+abc".to_string(),
            })
        );
    }

    #[test]
    fn read_version_valid_runtime_txt() {
        assert_eq!(
            read_version(Path::new("tests/fixtures/runtime_txt_python_3.10")).unwrap(),
            Some(PythonVersion::new(3, 10, 9))
        );
        assert_eq!(
            read_version(Path::new(
                "tests/fixtures/runtime_txt_python_version_unavailable"
            ))
            .unwrap(),
            Some(PythonVersion::new(999, 999, 999))
        );
    }

    #[test]
    fn read_version_runtime_txt_not_present() {
        assert_eq!(
            read_version(Path::new("tests/fixtures/empty")).unwrap(),
            None
        );
    }

    #[test]
    fn read_version_io_error() {
        assert!(matches!(
            read_version(Path::new("tests/fixtures/empty/.gitkeep")).unwrap_err(),
            RuntimeTxtError::Io(_)
        ));
    }

    #[test]
    fn read_version_parse_error() {
        assert!(matches!(
            read_version(Path::new(
                "tests/fixtures/runtime_txt_python_version_invalid"
            ))
            .unwrap_err(),
            RuntimeTxtError::Parse(_)
        ));
    }
}
