use crate::python_version::{PythonVersionOrigin, RequestedPythonVersion};

/// Parse the contents of a `runtime.txt` file into a [`RequestedPythonVersion`].
///
/// The file is expected to contain a string of form `python-X.Y.Z` or `python-X.Y`.
/// Any leading or trailing whitespace will be removed.
pub(crate) fn parse(contents: &str) -> Result<RequestedPythonVersion, ParseRuntimeTxtError> {
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
        .collect::<Result<Vec<u16>, _>>()
        .unwrap_or_default()[..]
    {
        [major, minor, patch] => Ok(RequestedPythonVersion {
            major,
            minor,
            patch: Some(patch),
            origin: PythonVersionOrigin::RuntimeTxt,
        }),
        [major, minor] => Ok(RequestedPythonVersion {
            major,
            minor,
            patch: None,
            origin: PythonVersionOrigin::RuntimeTxt,
        }),
        _ => Err(ParseRuntimeTxtError {
            cleaned_contents: cleaned_contents.clone(),
        }),
    }
}

/// Errors that can occur when parsing the contents of a `runtime.txt` file.
#[derive(Debug, PartialEq)]
pub(crate) struct ParseRuntimeTxtError {
    pub(crate) cleaned_contents: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid() {
        assert_eq!(
            parse("python-1.2"),
            Ok(RequestedPythonVersion {
                major: 1,
                minor: 2,
                patch: None,
                origin: PythonVersionOrigin::RuntimeTxt
            })
        );
        assert_eq!(
            parse("python-1.2.3"),
            Ok(RequestedPythonVersion {
                major: 1,
                minor: 2,
                patch: Some(3),
                origin: PythonVersionOrigin::RuntimeTxt
            })
        );
        assert_eq!(
            parse("python-987.654.3210"),
            Ok(RequestedPythonVersion {
                major: 987,
                minor: 654,
                patch: Some(3210),
                origin: PythonVersionOrigin::RuntimeTxt
            })
        );
        assert_eq!(
            parse("\n   python-1.2.3   \n"),
            Ok(RequestedPythonVersion {
                major: 1,
                minor: 2,
                patch: Some(3),
                origin: PythonVersionOrigin::RuntimeTxt
            })
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
}
