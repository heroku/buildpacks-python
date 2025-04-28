use crate::{FileExistsError, utils};
use std::path::Path;

pub(crate) const SUPPORTED_PACKAGE_MANAGERS: [PackageManager; 2] =
    [PackageManager::Pip, PackageManager::Poetry];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PackageManager {
    Pip,
    Poetry,
}

impl PackageManager {
    pub(crate) fn name(self) -> &'static str {
        match self {
            PackageManager::Pip => "pip",
            PackageManager::Poetry => "Poetry",
        }
    }

    pub(crate) fn packages_file(self) -> &'static str {
        match self {
            PackageManager::Pip => "requirements.txt",
            PackageManager::Poetry => "poetry.lock",
        }
    }
}

/// Determine the Python package manager to use for a project, or return an error if either
/// multiple supported package manager files are found, or none are.
pub(crate) fn determine_package_manager(
    app_dir: &Path,
) -> Result<PackageManager, DeterminePackageManagerError> {
    let package_managers_found = SUPPORTED_PACKAGE_MANAGERS
        .into_iter()
        .filter_map(|package_manager| {
            utils::file_exists(&app_dir.join(package_manager.packages_file()))
                .map_err(DeterminePackageManagerError::CheckFileExists)
                .map(|exists| exists.then_some(package_manager))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;

    match package_managers_found[..] {
        [package_manager] => Ok(package_manager),
        [] => Err(DeterminePackageManagerError::NoneFound),
        _ => Err(DeterminePackageManagerError::MultipleFound(
            package_managers_found,
        )),
    }
}

/// Errors that can occur when determining which Python package manager to use for a project.
#[derive(Debug)]
pub(crate) enum DeterminePackageManagerError {
    CheckFileExists(FileExistsError),
    MultipleFound(Vec<PackageManager>),
    NoneFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_package_manager_requirements_txt() {
        assert_eq!(
            determine_package_manager(Path::new("tests/fixtures/pip_basic")).unwrap(),
            PackageManager::Pip
        );
    }

    #[test]
    fn determine_package_manager_poetry_lock() {
        assert_eq!(
            determine_package_manager(Path::new("tests/fixtures/poetry_basic")).unwrap(),
            PackageManager::Poetry
        );
    }

    #[test]
    fn determine_package_manager_multiple() {
        assert!(matches!(
            determine_package_manager(Path::new("tests/fixtures/pip_and_poetry")).unwrap_err(),
            DeterminePackageManagerError::MultipleFound(found) if found == [PackageManager::Pip, PackageManager::Poetry]
        ));
    }

    #[test]
    fn determine_package_manager_none() {
        assert!(matches!(
            determine_package_manager(Path::new("tests/fixtures/pyproject_toml_only")).unwrap_err(),
            DeterminePackageManagerError::NoneFound
        ));
    }

    #[test]
    fn determine_package_manager_io_error() {
        // We pass a path containing a NUL byte as an easy way to trigger an I/O error.
        assert!(matches!(
            determine_package_manager(Path::new("\0/invalid")).unwrap_err(),
            DeterminePackageManagerError::CheckFileExists(err) if err.path == Path::new("\0/invalid/requirements.txt")
        ));
    }
}
