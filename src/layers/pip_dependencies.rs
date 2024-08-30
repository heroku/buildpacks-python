use crate::utils::{self, StreamedCommandError};
use crate::{BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::UncachedLayerDefinition;
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use libherokubuildpack::log::log_info;
use std::path::PathBuf;
use std::process::Command;

/// Creates a layer containing the application's Python dependencies, installed using pip.
//
// We install into a virtual environment since:
// - We can't install into the system site-packages inside the main Python directory since
//   we need the app dependencies to be in their own layer.
// - Some packages are broken with `--user` installs when using relocated Python, and
//   otherwise require other workarounds. eg: https://github.com/unbit/uwsgi/issues/2525
// - PEP-405 style venvs are very lightweight and are also much more frequently
//   used in the wild compared to `--user`, and therefore the better tested path.
//
// This layer is not cached, since:
// - pip is a package installer rather than a project/environment manager, and so does not
//   deterministically manage installed Python packages. For example, if a package entry in
//   a requirements file is later removed, pip will not uninstall the package. In addition,
//   there is no official lockfile support, so changes in transitive dependencies add yet
//   more opportunity for non-determinism between each install.
// - The pip HTTP/wheel cache is itself cached in a separate layer (exposed via `PIP_CACHE_DIR`),
//   which covers the most time consuming part of performing a pip install: downloading the
//   dependencies and then generating wheels for any packages that don't provide them.
pub(crate) fn install_dependencies(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
) -> Result<PathBuf, libcnb::Error<BuildpackError>> {
    let layer = context.uncached_layer(
        // The name of this layer must be alphabetically after that of the `python` layer so that
        // this layer's `bin/` directory (and thus `python` symlink) is listed first in `PATH`:
        // https://github.com/buildpacks/spec/blob/main/buildpack.md#layer-paths
        layer_name!("venv"),
        UncachedLayerDefinition {
            build: true,
            launch: true,
        },
    )?;
    let layer_path = layer.path();

    log_info("Creating virtual environment");
    utils::run_command_and_stream_output(
        Command::new("python")
            .args(["-m", "venv", "--without-pip", &layer_path.to_string_lossy()])
            .env_clear()
            .envs(&*env),
    )
    .map_err(PipDependenciesLayerError::CreateVenvCommand)?;

    let mut layer_env = LayerEnv::new()
        // Since pip is installed in a different layer (outside of this venv), we have to explicitly
        // tell it to perform operations against this venv instead of the global Python install.
        // https://pip.pypa.io/en/stable/cli/pip/#cmdoption-python
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "PIP_PYTHON",
            &layer_path,
        )
        // For parity with the venv's `bin/activate` script:
        // https://docs.python.org/3/library/venv.html#how-venvs-work
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "VIRTUAL_ENV",
            &layer_path,
        );
    layer.write_env(&layer_env)?;
    // Required to pick up the automatic PATH env var. See: https://github.com/heroku/libcnb.rs/issues/842
    layer_env = layer.read_env()?;
    env.clone_from(&layer_env.apply(Scope::Build, env));

    log_info("Running 'pip install -r requirements.txt'");
    utils::run_command_and_stream_output(
        Command::new("pip")
            .args([
                "install",
                "--no-input",
                "--progress-bar",
                "off",
                "--requirement",
                "requirements.txt",
            ])
            .current_dir(&context.app_dir)
            .env_clear()
            .envs(&*env),
    )
    .map_err(PipDependenciesLayerError::PipInstallCommand)?;

    Ok(layer_path)
}

/// Errors that can occur when installing the project's dependencies into a layer using pip.
#[derive(Debug)]
pub(crate) enum PipDependenciesLayerError {
    CreateVenvCommand(StreamedCommandError),
    PipInstallCommand(StreamedCommandError),
}

impl From<PipDependenciesLayerError> for libcnb::Error<BuildpackError> {
    fn from(error: PipDependenciesLayerError) -> Self {
        Self::BuildpackError(BuildpackError::PipDependenciesLayer(error))
    }
}
