use libcnb::Env;

// We expose all env vars by default to subprocesses to allow for customisation of package manager
// behaviour (such as custom indexes, authentication and requirements file env var interpolation).
// As such, we have to block known problematic env vars that may break the build / the app.
// This list was based on the env vars this buildpack sets, plus an audit of:
// https://docs.python.org/3/using/cmdline.html#environment-variables
// https://pip.pypa.io/en/stable/cli/pip/#general-options
// https://pip.pypa.io/en/stable/cli/pip_install/#options
const FORBIDDEN_ENV_VARS: [&str; 12] = [
    "PIP_CACHE_DIR",
    "PIP_PREFIX",
    "PIP_PYTHON",
    "PIP_ROOT",
    "PIP_TARGET",
    "PIP_USER",
    "PYTHONHOME",
    "PYTHONINSPECT",
    "PYTHONNOUSERSITE",
    "PYTHONPLATLIBDIR",
    "PYTHONUSERBASE",
    "VIRTUAL_ENV",
];

pub(crate) fn check_environment(env: &Env) -> Result<(), ChecksError> {
    if let Some(&name) = FORBIDDEN_ENV_VARS
        .iter()
        .find(|&name| env.contains_key(name))
    {
        return Err(ChecksError::ForbiddenEnvVar(name.to_string()));
    }

    Ok(())
}

/// Errors due to one of the environment checks failing.
#[derive(Debug)]
pub(crate) enum ChecksError {
    ForbiddenEnvVar(String),
}
