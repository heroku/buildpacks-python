use crate::python_version::{PythonVersionOrigin, RequestedPythonVersion};

/// Parse the contents of a `.python-version` file into a [`RequestedPythonVersion`].
///
/// The file is expected to contain a string of form `X.Y` or `X.Y.Z`. Leading and trailing
/// whitespace will be removed from each line. Lines which are either comments (that begin
/// with `#`) or are empty will be ignored. Multiple Python versions are not permitted.
pub(crate) fn parse(contents: &str) -> Result<RequestedPythonVersion, ParsePythonVersionFileError> {
    let versions = contents
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<String>>();

    match versions.as_slice() {
        [version] => match version
            .split('.')
            .map(str::parse)
            .collect::<Result<Vec<u16>, _>>()
            .unwrap_or_default()[..]
        {
            [major, minor, patch] => Ok(RequestedPythonVersion {
                major,
                minor,
                patch: Some(patch),
                origin: PythonVersionOrigin::PythonVersionFile,
            }),
            [major, minor] => Ok(RequestedPythonVersion {
                major,
                minor,
                patch: None,
                origin: PythonVersionOrigin::PythonVersionFile,
            }),
            _ => Err(ParsePythonVersionFileError::InvalidVersion(version.clone())),
        },
        [] => Err(ParsePythonVersionFileError::NoVersion),
        _ => Err(ParsePythonVersionFileError::MultipleVersions(versions)),
    }
}

/// Errors that can occur when parsing the contents of a `.python-version` file.
#[derive(Debug, PartialEq)]
pub(crate) enum ParsePythonVersionFileError {
    InvalidVersion(String),
    MultipleVersions(Vec<String>),
    NoVersion,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid() {
        assert_eq!(
            parse("1.2"),
            Ok(RequestedPythonVersion {
                major: 1,
                minor: 2,
                patch: None,
                origin: PythonVersionOrigin::PythonVersionFile,
            })
        );
        assert_eq!(
            parse("987.654.3210"),
            Ok(RequestedPythonVersion {
                major: 987,
                minor: 654,
                patch: Some(3210),
                origin: PythonVersionOrigin::PythonVersionFile,
            })
        );
        assert_eq!(
            parse("1.2\n"),
            Ok(RequestedPythonVersion {
                major: 1,
                minor: 2,
                patch: None,
                origin: PythonVersionOrigin::PythonVersionFile,
            })
        );
        assert_eq!(
            parse("  # Comment 1\n  1.2.3  \n  # Comment 2"),
            Ok(RequestedPythonVersion {
                major: 1,
                minor: 2,
                patch: Some(3),
                origin: PythonVersionOrigin::PythonVersionFile,
            })
        );
    }

    #[test]
    fn parse_invalid_version() {
        assert_eq!(
            parse("1"),
            Err(ParsePythonVersionFileError::InvalidVersion("1".to_string()))
        );
        assert_eq!(
            parse("1.2.3.4"),
            Err(ParsePythonVersionFileError::InvalidVersion(
                "1.2.3.4".to_string()
            ))
        );
        assert_eq!(
            parse("1..3"),
            Err(ParsePythonVersionFileError::InvalidVersion(
                "1..3".to_string()
            ))
        );
        assert_eq!(
            parse("1.2.3."),
            Err(ParsePythonVersionFileError::InvalidVersion(
                "1.2.3.".to_string()
            ))
        );
        assert_eq!(
            parse("1.2rc1"),
            Err(ParsePythonVersionFileError::InvalidVersion(
                "1.2rc1".to_string()
            ))
        );
        assert_eq!(
            parse("1.2.3-dev"),
            Err(ParsePythonVersionFileError::InvalidVersion(
                "1.2.3-dev".to_string()
            ))
        );
        // We don't support the `python-` prefix form since it's undocumented and will likely
        // be deprecated: https://github.com/pyenv/pyenv/issues/3054#issuecomment-2341316638
        assert_eq!(
            parse("python-1.2.3"),
            Err(ParsePythonVersionFileError::InvalidVersion(
                "python-1.2.3".to_string()
            ))
        );
        assert_eq!(
            parse("system"),
            Err(ParsePythonVersionFileError::InvalidVersion(
                "system".to_string()
            ))
        );
        assert_eq!(
            parse("  # Comment 1\n  1 2 3  \n  # Comment 2"),
            Err(ParsePythonVersionFileError::InvalidVersion(
                "1 2 3".to_string()
            ))
        );
    }

    #[test]
    fn parse_no_version() {
        assert_eq!(parse(""), Err(ParsePythonVersionFileError::NoVersion));
        assert_eq!(parse("\n"), Err(ParsePythonVersionFileError::NoVersion));
        assert_eq!(
            parse("# Comment 1\n  \n  # Comment 2"),
            Err(ParsePythonVersionFileError::NoVersion)
        );
    }

    #[test]
    fn parse_multiple_versions() {
        assert_eq!(
            parse("1.2\n3.4"),
            Err(ParsePythonVersionFileError::MultipleVersions(vec![
                "1.2".to_string(),
                "3.4".to_string()
            ]))
        );
        assert_eq!(
            parse("  # Comment 1\n  1.2  \n  # Comment 2\npython-3.4"),
            Err(ParsePythonVersionFileError::MultipleVersions(vec![
                "1.2".to_string(),
                "python-3.4".to_string()
            ]))
        );
    }
}
