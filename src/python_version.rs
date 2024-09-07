use crate::runtime_txt::{self, RuntimeTxtError};
use indoc::formatdoc;
use libcnb::Target;
use libherokubuildpack::log::log_info;
use std::fmt::{self, Display};
use std::path::Path;

/// The Python version that will be installed if the project does not specify an explicit version.
pub(crate) const DEFAULT_PYTHON_VERSION: PythonVersion = PythonVersion {
    major: 3,
    minor: 12,
    patch: 6,
};

/// Representation of a specific Python `X.Y.Z` version.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PythonVersion {
    pub(crate) major: u16,
    pub(crate) minor: u16,
    pub(crate) patch: u16,
}

impl PythonVersion {
    pub(crate) fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    // TODO: (W-11474658) Switch to tracking versions/URLs via a manifest file.
    pub(crate) fn url(&self, target: &Target) -> String {
        let Self {
            major,
            minor,
            patch,
        } = self;
        let Target {
            arch,
            distro_name,
            distro_version,
            ..
        } = target;
        format!(
            "https://heroku-buildpack-python.s3.us-east-1.amazonaws.com/python-{major}.{minor}.{patch}-{distro_name}-{distro_version}-{arch}.tar.zst"
        )
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

    // TODO: (W-12613425) Write this content inline, instead of linking out to Dev Center.
    // Also adjust wording to mention pinning as a use-case, not just using a different version.
    log_info(formatdoc! {"
        No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
        To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes"});
    Ok(DEFAULT_PYTHON_VERSION)
}

/// Errors that can occur when determining which Python version to use for a project.
#[derive(Debug)]
pub(crate) enum PythonVersionError {
    /// Errors reading and parsing a `runtime.txt` file.
    RuntimeTxt(RuntimeTxtError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_version_url() {
        assert_eq!(
            PythonVersion::new(3, 11, 0).url(&Target {
                os: "linux".to_string(),
                arch: "amd64".to_string(),
                arch_variant: None,
                distro_name: "ubuntu".to_string(),
                distro_version: "22.04".to_string()
            }),
            "https://heroku-buildpack-python.s3.us-east-1.amazonaws.com/python-3.11.0-ubuntu-22.04-amd64.tar.zst"
        );
        assert_eq!(
            PythonVersion::new(3, 12, 2).url(&Target {
                os: "linux".to_string(),
                arch: "arm64".to_string(),
                arch_variant: None,
                distro_name: "ubuntu".to_string(),
                distro_version: "24.04".to_string()
            }),
            "https://heroku-buildpack-python.s3.us-east-1.amazonaws.com/python-3.12.2-ubuntu-24.04-arm64.tar.zst"
        );
    }

    #[test]
    fn determine_python_version_runtime_txt_valid() {
        assert_eq!(
            determine_python_version(Path::new("tests/fixtures/python_3.7")).unwrap(),
            PythonVersion::new(3, 7, 17)
        );
        assert_eq!(
            determine_python_version(Path::new("tests/fixtures/runtime_txt_non_existent_version"))
                .unwrap(),
            PythonVersion::new(999, 888, 777)
        );
    }

    #[test]
    fn determine_python_version_runtime_txt_error() {
        assert!(matches!(
            determine_python_version(Path::new("tests/fixtures/runtime_txt_invalid_version"))
                .unwrap_err(),
            PythonVersionError::RuntimeTxt(RuntimeTxtError::Parse(_))
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
