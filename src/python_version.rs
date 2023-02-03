use crate::runtime_txt::{self, ReadRuntimeTxtError};
use indoc::formatdoc;
use libherokubuildpack::log::log_info;
use std::fmt::{self, Display};
use std::path::Path;

/// The Python version that will be installed if the project does not specify an explicit version.
pub(crate) const DEFAULT_PYTHON_VERSION: PythonVersion = PythonVersion {
    major: 3,
    minor: 11,
    patch: 1,
};

/// Representation of a specific Python `X.Y.Z` version.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PythonVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl PythonVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl Display for PythonVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Determine the Python version that should be installed for the project.
///
/// If no known version specifier file is found a default Python version will be used.
pub(crate) fn determine_python_version(
    app_dir: &Path,
) -> Result<PythonVersion, PythonVersionError> {
    if let Some(runtime_txt_version) =
        runtime_txt::read_version(app_dir).map_err(PythonVersionError::RuntimeTxt)?
    {
        // TODO: Consider passing this back as a `source` field on PythonVersion
        // so this can be logged by the caller.
        log_info(format!(
            "Using Python version {runtime_txt_version} specified in runtime.txt"
        ));
        return Ok(runtime_txt_version);
    }

    // TODO: Write this content inline, instead of linking out to Dev Center.
    // Also adjust wording to mention pinning as a use-case, not just using a different version.
    log_info(formatdoc! {"
        No Python version specified, using the current default of {DEFAULT_PYTHON_VERSION}.
        To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes"});
    Ok(DEFAULT_PYTHON_VERSION)
}

/// Errors that can occur when determining which Python package manager to use for a project.
#[derive(Debug)]
pub(crate) enum PythonVersionError {
    RuntimeTxt(ReadRuntimeTxtError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_python_version_runtime_txt_valid() {
        assert_eq!(
            determine_python_version(Path::new("tests/fixtures/runtime_txt_python_3.10")).unwrap(),
            PythonVersion::new(3, 10, 9)
        );
        assert_eq!(
            determine_python_version(Path::new(
                "tests/fixtures/runtime_txt_python_version_unavailable"
            ))
            .unwrap(),
            PythonVersion::new(999, 999, 999)
        );
    }

    #[test]
    fn determine_python_version_runtime_txt_error() {
        assert!(matches!(
            determine_python_version(Path::new(
                "tests/fixtures/runtime_txt_python_version_invalid"
            ))
            .unwrap_err(),
            PythonVersionError::RuntimeTxt(ReadRuntimeTxtError::Parse(_))
        ));
    }

    #[test]
    fn determine_python_version_none_specified() {
        assert_eq!(
            determine_python_version(Path::new("tests/fixtures/empty")).unwrap(),
            DEFAULT_PYTHON_VERSION
        );
    }
}
