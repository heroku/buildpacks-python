use crate::runtime_txt::{self, RuntimeTxtError};
use libherokubuildpack::log_info;
use std::fmt::{self, Display};
use std::path::Path;

pub(crate) const DEFAULT_PYTHON_VERSION: PythonVersion = PythonVersion {
    major: 3,
    minor: 10,
    patch: 5,
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PythonVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl PythonVersion {
    pub fn new(major: u8, minor: u8, patch: u8) -> Self {
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

// string -> requested python version -> exact python version -> python runtime (incl URL etc)

// resolving python version:
// failure modes: Nonsensical, unknown to buildpack, known but not supported, known and used to be supported but no longer
// Does this occur inside each `get_version` / creation of `PythonVersion`?
// But then each error type needs 3-4 additional enum variants
// Depends on whether we want different error messages for each?
// Though could still vary error message by using `PythonVersion.source` etc

// warnings:
// EOL major version, non-latest minor version, deprecated version specifier?
// output warnings as found during build, or at end of the build log?
// does EOL warnings use requested Python version or resolved version? I suppose resolved since needs EOL date etc, plus range version might still be outdated?

// logging:
// Do we log for version specifier files not found? Or only when found?
// where do we log? In get_version, determine_python_version, or in the caller and have to store the version source in `PythonVersion`?

pub(crate) fn determine_python_version(
    app_dir: &Path,
) -> Result<PythonVersion, PythonVersionError> {
    if let Some(runtime_txt_version) =
        runtime_txt::get_version(app_dir).map_err(PythonVersionError::RuntimeTxt)?
    {
        // TODO: Consider passing this back as a `source` field on PythonVersion
        // so this can be logged by the caller.
        log_info(format!(
            "Using Python version {runtime_txt_version} specified in runtime.txt"
        ));
        return Ok(runtime_txt_version);
    }

    log_info(format!(
        "No Python version specified, using default of {DEFAULT_PYTHON_VERSION}"
    ));
    Ok(DEFAULT_PYTHON_VERSION)
}

pub(crate) fn _determine_python_version2(
    app_dir: &Path,
) -> Result<PythonVersion, PythonVersionError> {
    runtime_txt::get_version(app_dir)
        .map_err(PythonVersionError::RuntimeTxt)
        .transpose()
        .or_else(|| {
            runtime_txt::get_version(app_dir)
                .map_err(PythonVersionError::RuntimeTxt)
                .transpose()
        })
        .unwrap_or(Ok(DEFAULT_PYTHON_VERSION))
}

#[derive(Debug)]
pub(crate) enum PythonVersionError {
    RuntimeTxt(RuntimeTxtError),
}
