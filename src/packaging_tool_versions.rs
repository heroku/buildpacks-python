use serde::{Deserialize, Serialize};

/// The versions of various packaging tools used during the build.
/// These are always installed, and are independent of the chosen package manager.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PackagingToolVersions {
    pub pip_version: String,
    pub setuptools_version: String,
    pub wheel_version: String,
}

impl Default for PackagingToolVersions {
    fn default() -> Self {
        Self {
            pip_version: "23.0.1".to_string(),
            setuptools_version: "67.4.0".to_string(),
            wheel_version: "0.38.4".to_string(),
        }
    }
}
