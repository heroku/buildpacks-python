use std::io;
use std::path::Path;

/// An ordered mapping of project filenames to their associated package manager.
/// Earlier entries will take precedence if a project matches multiple package managers.
pub(crate) const PACKAGE_MANAGER_FILE_MAPPING: [(&str, PackageManager); 1] =
    [("requirements.txt", PackageManager::Pip)];

/// Python package managers supported by the buildpack.
#[derive(Debug)]
pub(crate) enum PackageManager {
    Pip,
}

/// Determine the Python package manager to use for a project, or return an error if no supported
/// package manager files are found. If a project contains the files for multiple package managers,
/// then the earliest entry in `PACKAGE_MANAGER_FILE_MAPPING` takes precedence.
pub(crate) fn determine_package_manager(
    app_dir: &Path,
) -> Result<PackageManager, DeterminePackageManagerError> {
    // Until `Iterator::try_find` is stabilised, this is cleaner as a for loop.
    for (filename, package_manager) in PACKAGE_MANAGER_FILE_MAPPING {
        if app_dir
            .join(filename)
            .try_exists()
            .map_err(DeterminePackageManagerError::CheckFileExists)?
        {
            return Ok(package_manager);
        }
    }

    Err(DeterminePackageManagerError::NoneFound)
}

/// Errors that can occur when determining which Python package manager to use for a project.
#[derive(Debug)]
pub(crate) enum DeterminePackageManagerError {
    CheckFileExists(io::Error),
    NoneFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_package_manager_requirements_txt() {
        assert!(matches!(
            determine_package_manager(Path::new("tests/fixtures/pip_editable_git_compiled"))
                .unwrap(),
            PackageManager::Pip
        ));
    }

    #[test]
    fn determine_package_manager_none() {
        assert!(matches!(
            determine_package_manager(Path::new("tests/fixtures/empty")).unwrap_err(),
            DeterminePackageManagerError::NoneFound
        ));
    }
}
