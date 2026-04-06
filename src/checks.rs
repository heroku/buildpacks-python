use libcnb::Env;

// We expose all env vars by default to subprocesses to allow for customisation of package manager
// behaviour (such as custom indexes, authentication and requirements file env var interpolation).
// As such, we have to block known problematic env vars that may break the build / the app.
// This list was based on the env vars this buildpack sets, plus an audit of:
// https://docs.python.org/3/using/cmdline.html#environment-variables
// https://pip.pypa.io/en/stable/cli/pip/#general-options
// https://pip.pypa.io/en/stable/cli/pip_install/#options
// https://python-poetry.org/docs/configuration/
// https://docs.astral.sh/uv/reference/environment/
const FORBIDDEN_ENV_VARS: &[&str] = &[
    "PIP_CACHE_DIR",
    "PIP_PREFIX",
    "PIP_PYTHON",
    "PIP_ROOT",
    "PIP_TARGET",
    "PIP_USER",
    "POETRY_CACHE_DIR",
    "POETRY_DATA_DIR",
    "POETRY_HOME",
    "POETRY_VIRTUALENVS_CREATE",
    "POETRY_VIRTUALENVS_USE_POETRY_PYTHON",
    "PYTHONHOME",
    "PYTHONINSPECT",
    "PYTHONNOUSERSITE",
    "PYTHONPLATLIBDIR",
    "PYTHONUSERBASE",
    "UV_CACHE_DIR",
    "UV_LINK_MODE",
    "UV_MANAGED_PYTHON",
    "UV_NO_CACHE",
    "UV_NO_MANAGED_PYTHON",
    "UV_PROJECT",
    "UV_PROJECT_ENVIRONMENT",
    "UV_PYTHON",
    "UV_PYTHON_DOWNLOADS",
    "UV_PYTHON_PREFERENCE",
    "VIRTUAL_ENV",
];

pub(crate) fn check_environment(env: &Env) -> Result<(), ChecksError> {
    let forbidden_vars_found: Vec<&'static str> = FORBIDDEN_ENV_VARS
        .iter()
        .copied()
        .filter(|&name| env.contains_key(name))
        .collect();

    if !forbidden_vars_found.is_empty() {
        return Err(ChecksError::ForbiddenEnvVars(forbidden_vars_found));
    }

    Ok(())
}

/// Errors due to one of the environment checks failing.
#[derive(Debug, PartialEq)]
pub(crate) enum ChecksError {
    ForbiddenEnvVars(Vec<&'static str>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_environment_valid() {
        assert_eq!(check_environment(&Env::new()), Ok(()));
        let mut env = Env::new();
        env.insert("PIP_EXTRA_INDEX_URL", "https://example.tld/simple");
        env.insert("POETRY_FOO", "example");
        env.insert("PYTHONPATH", "/example");
        env.insert("UV_FOO", "example");
        assert_eq!(check_environment(&env), Ok(()));
    }

    #[test]
    fn check_environment_invalid() {
        let mut env = Env::new();
        env.insert("PYTHONHOME", "/example");
        assert_eq!(
            check_environment(&env),
            Err(ChecksError::ForbiddenEnvVars(vec!["PYTHONHOME"]))
        );
        env.insert("PIP_PYTHON", "/example");
        env.insert("POETRY_HOME", "/example");
        env.insert("VIRTUAL_ENV", "/example");
        env.insert("UV_PYTHON", "/example");
        assert_eq!(
            check_environment(&env),
            Err(ChecksError::ForbiddenEnvVars(vec![
                "PIP_PYTHON",
                "POETRY_HOME",
                "PYTHONHOME",
                "UV_PYTHON",
                "VIRTUAL_ENV",
            ]))
        );
    }
}
