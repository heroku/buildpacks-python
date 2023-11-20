use serde::{Deserialize, Serialize};

const PIP_REQUIREMENT: &str = include_str!("../requirements/pip.txt");
const SETUPTOOLS_REQUIREMENT: &str = include_str!("../requirements/setuptools.txt");
const WHEEL_REQUIREMENT: &str = include_str!("../requirements/wheel.txt");

/// The versions of various packaging tools used during the build.
/// These are always installed, and are independent of the chosen package manager.
/// Strings are used instead of a semver version, since these packages don't use
/// semver, and we never introspect the version parts anyway.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PackagingToolVersions {
    pub(crate) pip_version: String,
    pub(crate) setuptools_version: String,
    pub(crate) wheel_version: String,
}

impl Default for PackagingToolVersions {
    fn default() -> Self {
        // These versions are effectively buildpack constants, however, we want Dependabot to be able
        // to update them, which requires that they be in requirements files. The requirements files
        // contain contents like `package==1.2.3` (and not just the package version) so we have to
        // extract the version substring from it. Ideally this would be done at compile time, however,
        // using const functions would require use of unsafe and lots of boilerplate, and using proc
        // macros would require the overhead of adding a separate crate. As such, it ends up being
        // simpler to extract the version substring at runtime. Extracting the version is technically
        // fallible, however, we control the buildpack requirements files, so if they are invalid it
        // can only ever be a buildpack bug, and not something a user would ever see given the unit
        // and integration tests. As such, it's safe to use `.expect()` here, and doing so saves us
        // from having to add user-facing error messages that users will never see.
        Self {
            pip_version: extract_requirement_version(PIP_REQUIREMENT)
                .expect("pip requirement file must contain a valid version"),
            setuptools_version: extract_requirement_version(SETUPTOOLS_REQUIREMENT)
                .expect("setuptools requirement file must contain a valid version"),
            wheel_version: extract_requirement_version(WHEEL_REQUIREMENT)
                .expect("wheel requirement file must contain a valid version"),
        }
    }
}

/// Extract the version substring from an exact-version requirement specifier (such as `foo==1.2.3`).
/// This function should only be used to extract the version constants from the buildpack's own
/// requirements files, which are controlled by us and don't require a full PEP 508 version parser.
fn extract_requirement_version(requirement: &str) -> Option<String> {
    match requirement.split("==").collect::<Vec<_>>().as_slice() {
        &[_, version] => Some(version.trim().to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_packaging_tool_versions() {
        // If the versions in the buildpack's `requirements/*.txt` files are invalid, this will panic.
        PackagingToolVersions::default();
    }

    #[test]
    fn extract_requirement_version_valid() {
        assert_eq!(
            extract_requirement_version("some_package==1.2.3"),
            Some("1.2.3".to_string())
        );
        assert_eq!(
            extract_requirement_version("\nsome_package == 1.2.3\n"),
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn extract_requirement_version_invalid() {
        assert_eq!(extract_requirement_version("some_package"), None);
        assert_eq!(extract_requirement_version("some_package=<1.2.3"), None);
        assert_eq!(
            extract_requirement_version("some_package==1.2.3\nanother_package==4.5.6"),
            None
        );
    }
}
