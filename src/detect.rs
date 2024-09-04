use std::io;
use std::path::Path;

/// Filenames that if found in a project mean it should be treated as a Python project,
/// and so pass this buildpack's detection phase.
///
/// This list is deliberately larger than just the list of supported package manager files,
/// so that Python projects that are missing some of the required files still pass detection,
/// allowing us to show a helpful error message during the build phase.
const KNOWN_PYTHON_PROJECT_FILES: [&str; 14] = [
    ".python-version",
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
    "setup.cfg",
    "setup.py",
    "uv.lock",
];

/// Returns whether the specified project directory is that of a Python project, and so
/// should pass buildpack detection.
pub(crate) fn is_python_project_directory(app_dir: &Path) -> io::Result<bool> {
    // Until `Iterator::try_find` is stabilised, this is cleaner as a for loop.
    for filename in KNOWN_PYTHON_PROJECT_FILES {
        let path = app_dir.join(filename);
        if path.try_exists()? {
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
        assert!(is_python_project_directory(Path::new("tests/fixtures/empty/.gitkeep")).is_err());
    }

    #[test]
    fn known_python_project_files_contains_all_package_manager_files() {
        assert!(SUPPORTED_PACKAGE_MANAGERS.iter().all(|package_manager| {
            KNOWN_PYTHON_PROJECT_FILES.contains(&package_manager.packages_file())
        }));
    }
}
