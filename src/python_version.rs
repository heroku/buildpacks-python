use crate::python_version_file::{self, ParsePythonVersionFileError};
use crate::runtime_txt::{self, ParseRuntimeTxtError};
use crate::utils;
use libcnb::Target;
use std::fmt::{self, Display};
use std::io;
use std::path::Path;

/// The Python version that will be installed if the project does not specify an explicit version.
pub(crate) const DEFAULT_PYTHON_VERSION: RequestedPythonVersion = RequestedPythonVersion {
    major: 3,
    minor: 13,
    patch: None,
    origin: PythonVersionOrigin::BuildpackDefault,
};
pub(crate) const DEFAULT_PYTHON_FULL_VERSION: PythonVersion = LATEST_PYTHON_3_13;

pub(crate) const OLDEST_SUPPORTED_PYTHON_3_MINOR_VERSION: u16 = 9;
pub(crate) const NEWEST_SUPPORTED_PYTHON_3_MINOR_VERSION: u16 = 13;
pub(crate) const NEXT_UNRELEASED_PYTHON_3_MINOR_VERSION: u16 =
    NEWEST_SUPPORTED_PYTHON_3_MINOR_VERSION + 1;

pub(crate) const LATEST_PYTHON_3_9: PythonVersion = PythonVersion::new(3, 9, 21);
pub(crate) const LATEST_PYTHON_3_10: PythonVersion = PythonVersion::new(3, 10, 16);
pub(crate) const LATEST_PYTHON_3_11: PythonVersion = PythonVersion::new(3, 11, 11);
pub(crate) const LATEST_PYTHON_3_12: PythonVersion = PythonVersion::new(3, 12, 8);
pub(crate) const LATEST_PYTHON_3_13: PythonVersion = PythonVersion::new(3, 13, 1);

/// The Python version that was requested for a project.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RequestedPythonVersion {
    pub(crate) major: u16,
    pub(crate) minor: u16,
    pub(crate) patch: Option<u16>,
    pub(crate) origin: PythonVersionOrigin,
}

impl Display for RequestedPythonVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            major,
            minor,
            patch,
            ..
        } = self;
        if let Some(patch) = patch {
            write!(f, "{major}.{minor}.{patch}")
        } else {
            write!(f, "{major}.{minor}")
        }
    }
}

/// The origin of the [`RequestedPythonVersion`].
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PythonVersionOrigin {
    BuildpackDefault,
    PythonVersionFile,
    RuntimeTxt,
}

impl Display for PythonVersionOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildpackDefault => write!(f, "buildpack default"),
            Self::PythonVersionFile => write!(f, ".python-version"),
            Self::RuntimeTxt => write!(f, "runtime.txt"),
        }
    }
}

/// Representation of a specific Python `X.Y.Z` version.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PythonVersion {
    pub(crate) major: u16,
    pub(crate) minor: u16,
    pub(crate) patch: u16,
}

impl PythonVersion {
    pub(crate) const fn new(major: u16, minor: u16, patch: u16) -> Self {
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
        let Self {
            major,
            minor,
            patch,
        } = self;
        write!(f, "{major}.{minor}.{patch}")
    }
}

/// Determine the Python version that has been requested for the project.
///
/// If no known version specifier file is found a default Python version will be used.
pub(crate) fn read_requested_python_version(
    app_dir: &Path,
) -> Result<RequestedPythonVersion, RequestedPythonVersionError> {
    if let Some(contents) = utils::read_optional_file(&app_dir.join("runtime.txt"))
        .map_err(RequestedPythonVersionError::ReadRuntimeTxt)?
    {
        runtime_txt::parse(&contents).map_err(RequestedPythonVersionError::ParseRuntimeTxt)
    } else if let Some(contents) = utils::read_optional_file(&app_dir.join(".python-version"))
        .map_err(RequestedPythonVersionError::ReadPythonVersionFile)?
    {
        python_version_file::parse(&contents)
            .map_err(RequestedPythonVersionError::ParsePythonVersionFile)
    } else {
        Ok(DEFAULT_PYTHON_VERSION)
    }
}

/// Errors that can occur when determining which Python version was requested for a project.
#[derive(Debug)]
pub(crate) enum RequestedPythonVersionError {
    /// Errors parsing a `.python-version` file.
    ParsePythonVersionFile(ParsePythonVersionFileError),
    /// Errors parsing a `runtime.txt` file.
    ParseRuntimeTxt(ParseRuntimeTxtError),
    /// Errors reading a `.python-version` file.
    ReadPythonVersionFile(io::Error),
    /// Errors reading a `runtime.txt` file.
    ReadRuntimeTxt(io::Error),
}

pub(crate) fn resolve_python_version(
    requested_python_version: &RequestedPythonVersion,
) -> Result<PythonVersion, ResolvePythonVersionError> {
    let &RequestedPythonVersion {
        major,
        minor,
        patch,
        ..
    } = requested_python_version;

    match (major, minor, patch) {
        (..3, _, _) | (3, ..OLDEST_SUPPORTED_PYTHON_3_MINOR_VERSION, _) => Err(
            ResolvePythonVersionError::EolVersion(requested_python_version.clone()),
        ),
        (3, NEXT_UNRELEASED_PYTHON_3_MINOR_VERSION.., _) | (4.., _, _) => Err(
            ResolvePythonVersionError::UnknownVersion(requested_python_version.clone()),
        ),
        (3, 9, None) => Ok(LATEST_PYTHON_3_9),
        (3, 10, None) => Ok(LATEST_PYTHON_3_10),
        (3, 11, None) => Ok(LATEST_PYTHON_3_11),
        (3, 12, None) => Ok(LATEST_PYTHON_3_12),
        (3, 13, None) => Ok(LATEST_PYTHON_3_13),
        (major, minor, Some(patch)) => Ok(PythonVersion::new(major, minor, patch)),
    }
}

/// Errors that can occur when resolving a requested Python version to a specific Python version.
#[derive(Debug, PartialEq)]
pub(crate) enum ResolvePythonVersionError {
    EolVersion(RequestedPythonVersion),
    UnknownVersion(RequestedPythonVersion),
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
    fn read_requested_python_version_runtime_txt() {
        assert_eq!(
            read_requested_python_version(Path::new(
                "tests/fixtures/runtime_txt_and_python_version_file"
            ))
            .unwrap(),
            RequestedPythonVersion {
                major: 3,
                minor: 9,
                patch: Some(0),
                origin: PythonVersionOrigin::RuntimeTxt,
            }
        );
        assert!(matches!(
            read_requested_python_version(Path::new("tests/fixtures/runtime_txt_invalid_unicode"))
                .unwrap_err(),
            RequestedPythonVersionError::ReadRuntimeTxt(_)
        ));
        assert!(matches!(
            read_requested_python_version(Path::new("tests/fixtures/runtime_txt_invalid_version"))
                .unwrap_err(),
            RequestedPythonVersionError::ParseRuntimeTxt(_)
        ));
    }

    #[test]
    fn read_requested_python_version_python_version_file() {
        assert_eq!(
            read_requested_python_version(Path::new("tests/fixtures/python_3.9")).unwrap(),
            RequestedPythonVersion {
                major: 3,
                minor: 9,
                patch: None,
                origin: PythonVersionOrigin::PythonVersionFile,
            }
        );
        assert!(matches!(
            read_requested_python_version(Path::new(
                "tests/fixtures/python_version_file_invalid_unicode"
            ))
            .unwrap_err(),
            RequestedPythonVersionError::ReadPythonVersionFile(_)
        ));
        assert!(matches!(
            read_requested_python_version(Path::new(
                "tests/fixtures/python_version_file_invalid_version"
            ))
            .unwrap_err(),
            RequestedPythonVersionError::ParsePythonVersionFile(_)
        ));
    }

    #[test]
    fn read_requested_python_version_none_specified() {
        assert_eq!(
            read_requested_python_version(Path::new("tests/fixtures/python_version_unspecified"))
                .unwrap(),
            RequestedPythonVersion {
                major: 3,
                minor: 13,
                patch: None,
                origin: PythonVersionOrigin::BuildpackDefault
            }
        );
    }

    #[test]
    fn resolve_python_version_valid() {
        // Buildpack default version
        assert_eq!(
            resolve_python_version(&DEFAULT_PYTHON_VERSION),
            Ok(DEFAULT_PYTHON_FULL_VERSION)
        );

        for minor in
            OLDEST_SUPPORTED_PYTHON_3_MINOR_VERSION..=NEWEST_SUPPORTED_PYTHON_3_MINOR_VERSION
        {
            // Major-minor version
            let python_version = resolve_python_version(&RequestedPythonVersion {
                major: 3,
                minor,
                patch: None,
                origin: PythonVersionOrigin::PythonVersionFile,
            })
            .unwrap();
            assert_eq!((python_version.major, python_version.minor), (3, minor));

            // Exact version
            assert_eq!(
                resolve_python_version(&RequestedPythonVersion {
                    major: 3,
                    minor,
                    patch: Some(1),
                    origin: PythonVersionOrigin::RuntimeTxt
                }),
                Ok(PythonVersion::new(3, minor, 1))
            );
        }
    }

    #[test]
    fn resolve_python_version_eol() {
        let requested_python_version = RequestedPythonVersion {
            major: 3,
            minor: OLDEST_SUPPORTED_PYTHON_3_MINOR_VERSION - 1,
            patch: None,
            origin: PythonVersionOrigin::PythonVersionFile,
        };
        assert_eq!(
            resolve_python_version(&requested_python_version),
            Err(ResolvePythonVersionError::EolVersion(
                requested_python_version
            ))
        );

        let requested_python_version = RequestedPythonVersion {
            major: 3,
            minor: OLDEST_SUPPORTED_PYTHON_3_MINOR_VERSION - 1,
            patch: Some(0),
            origin: PythonVersionOrigin::PythonVersionFile,
        };
        assert_eq!(
            resolve_python_version(&requested_python_version),
            Err(ResolvePythonVersionError::EolVersion(
                requested_python_version
            ))
        );

        let requested_python_version = RequestedPythonVersion {
            major: 2,
            minor: 7,
            patch: Some(18),
            origin: PythonVersionOrigin::RuntimeTxt,
        };
        assert_eq!(
            resolve_python_version(&requested_python_version),
            Err(ResolvePythonVersionError::EolVersion(
                requested_python_version
            ))
        );
    }

    #[test]
    fn resolve_python_version_unsupported() {
        let requested_python_version = RequestedPythonVersion {
            major: 3,
            minor: NEXT_UNRELEASED_PYTHON_3_MINOR_VERSION,
            patch: None,
            origin: PythonVersionOrigin::PythonVersionFile,
        };
        assert_eq!(
            resolve_python_version(&requested_python_version),
            Err(ResolvePythonVersionError::UnknownVersion(
                requested_python_version
            ))
        );

        let requested_python_version = RequestedPythonVersion {
            major: 3,
            minor: NEXT_UNRELEASED_PYTHON_3_MINOR_VERSION,
            patch: Some(0),
            origin: PythonVersionOrigin::PythonVersionFile,
        };
        assert_eq!(
            resolve_python_version(&requested_python_version),
            Err(ResolvePythonVersionError::UnknownVersion(
                requested_python_version
            ))
        );

        let requested_python_version = RequestedPythonVersion {
            major: 4,
            minor: 0,
            patch: Some(0),
            origin: PythonVersionOrigin::RuntimeTxt,
        };
        assert_eq!(
            resolve_python_version(&requested_python_version),
            Err(ResolvePythonVersionError::UnknownVersion(
                requested_python_version
            ))
        );
    }
}
