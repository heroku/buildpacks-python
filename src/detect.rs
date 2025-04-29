use crate::utils::{self, FileExistsError};
use std::path::Path;

/// Filenames that if found in a project mean it should be treated as a Python project,
/// and so pass this buildpack's detection phase.
///
/// This list is deliberately larger than just the list of supported package manager files,
/// so that Python projects that are missing some of the required files still pass detection,
/// allowing us to show a helpful error message during the build phase.
const KNOWN_PYTHON_PROJECT_FILES: [&str; 23] = [
    ".python-version",
    "__init__.py",
    "app.py",
    "main.py",
    "manage.py",
    "pdm.lock",
    "Pipfile",
    "Pipfile.lock",
    "poetry.lock",
    "pyproject.toml",
    "requirements.txt",
    "runtime.txt",
    "server.py",
    "setup.cfg",
    "setup.py",
    "uv.lock",
    // Commonly seen misspellings of requirements.txt. (Which occur since pip doesn't
    // create/manage requirements files itself, so the filenames are manually typed.)
    "requirement.txt",
    "Requirements.txt",
    "requirements.text",
    "requirements.txt.txt",
    "requirments.txt",
    // Whilst virtual environments shouldn't be committed to Git (and so shouldn't
    // normally be present during the build), they are often present for beginner
    // Python apps that are missing all of the other Python related files above.
    ".venv/",
    "venv/",
];

/// Returns whether the specified project directory is that of a Python project, and so
/// should pass buildpack detection.
pub(crate) fn is_python_project_directory(app_dir: &Path) -> Result<bool, FileExistsError> {
    // Until `Iterator::try_find` is stabilised, this is cleaner as a for loop.
    for filename in KNOWN_PYTHON_PROJECT_FILES {
        if utils::file_exists(&app_dir.join(filename))? {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_manager::SUPPORTED_PACKAGE_MANAGERS;

    #[test]
    fn is_python_project_directory_valid_project() {
        assert!(
            is_python_project_directory(Path::new("tests/fixtures/pyproject_toml_only")).unwrap()
        );
    }

    #[test]
    fn is_python_project_directory_empty() {
        assert!(!is_python_project_directory(Path::new("tests/fixtures/empty")).unwrap());
    }

    #[test]
    fn is_python_project_directory_io_error() {
        // We pass a path containing a NUL byte as an easy way to trigger an I/O error.
        let err = is_python_project_directory(Path::new("\0/invalid")).unwrap_err();
        assert_eq!(err.path, Path::new("\0/invalid/.python-version"));
    }

    #[test]
    fn known_python_project_files_contains_all_package_manager_files() {
        assert!(SUPPORTED_PACKAGE_MANAGERS.iter().all(|package_manager| {
            KNOWN_PYTHON_PROJECT_FILES.contains(&package_manager.packages_file())
        }));
    }
}
