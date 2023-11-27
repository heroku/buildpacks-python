use std::io;
use std::path::Path;

/// Filenames that if found in a project mean it should be treated as a Python project,
/// and so pass this buildpack's detection phase.
///
/// This list is deliberately larger than just the list of supported package manager files,
/// so that Python projects that are missing some of the required files still pass detection,
/// allowing us to show a helpful error message during the build phase.
const KNOWN_PYTHON_PROJECT_FILES: [&str; 9] = [
    ".python-version",
    "main.py",
    "manage.py",
    "Pipfile",
    "poetry.lock",
    "pyproject.toml",
    "requirements.txt",
    "runtime.txt",
    "setup.py",
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
    use crate::package_manager::PACKAGE_MANAGER_FILE_MAPPING;

    #[test]
    fn is_python_project_valid_project() {
        assert!(
            is_python_project_directory(Path::new("tests/fixtures/pyproject_toml_only")).unwrap()
        );
    }

    #[test]
    fn is_python_project_empty() {
        assert!(!is_python_project_directory(Path::new("tests/fixtures/empty")).unwrap());
    }

    #[test]
    fn is_python_project_io_error() {
        assert!(is_python_project_directory(Path::new("tests/fixtures/empty/.gitkeep")).is_err());
    }

    #[test]
    fn known_python_project_files_contains_all_package_manager_files() {
        assert!(PACKAGE_MANAGER_FILE_MAPPING
            .iter()
            .all(|(filename, _)| { KNOWN_PYTHON_PROJECT_FILES.contains(filename) }));
    }
}
