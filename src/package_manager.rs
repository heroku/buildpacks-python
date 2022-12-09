use std::io;
use std::path::Path;

pub(crate) enum PackageManager {
    Pip,
}

const PACKAGE_MANAGER_FILE_MAPPING: [(&str, PackageManager); 1] =
    [("requirements.txt", PackageManager::Pip)];

// TODO: Unit test
pub(crate) fn determine_package_manager(
    app_dir: &Path,
) -> Result<PackageManager, DeterminePackageManagerError> {
    // Until `Iterator::try_find` is stabilised, this is cleaner as a for loop.
    for (filename, package_manager) in PACKAGE_MANAGER_FILE_MAPPING {
        if app_dir
            .join(filename)
            .try_exists()
            .map_err(DeterminePackageManagerError::Io)?
        {
            return Ok(package_manager);
        }
    }

    Err(DeterminePackageManagerError::NoneFound)
}

#[derive(Debug)]
pub(crate) enum DeterminePackageManagerError {
    Io(io::Error),
    NoneFound,
}
