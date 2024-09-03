use crate::packaging_tool_versions::POETRY_VERSION;
use crate::python_version::PythonVersion;
use crate::utils::StreamedCommandError;
use crate::{utils, BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, InvalidMetadataAction, RestoredLayerAction,
};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Creates a layer containing the application's Python dependencies, installed using Poetry.
//
// We install into a virtual environment since:
// - We can't install into the system site-packages inside the main Python directory since
//   we need the app dependencies to be in their own layer.
// - Some packages are broken with `--user` installs when using relocated Python, and
//   otherwise require other workarounds. eg: https://github.com/unbit/uwsgi/issues/2525
// - Poetry doesn't support `--user`: https://github.com/python-poetry/poetry/issues/1214
// - PEP-405 style venvs are very lightweight and are also much more frequently
//   used in the wild compared to `--user`, and therefore the better tested path.
//
// We cache the virtual environment, since:
// - It results in faster builds than only caching Poetry's download/wheel cache.
// - It's safe to do so, since `poetry install --sync` fully manages the environment
//   (including e.g. uninstalling packages when they are removed from the lockfile).
//
// With the venv cached there is no need to persist Poetry's download/wheel cache in its
// own layer, so we let Poetry write it to the home directory where it will be discarded
// at the end of the build. We don't use `--no-cache` since the cache still offers benefits
// (such as avoiding repeat downloads of PEP-517/518 build requirements).
pub(crate) fn install_dependencies(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
    python_version: &PythonVersion,
) -> Result<PathBuf, libcnb::Error<BuildpackError>> {
    let new_metadata = PoetryDependenciesLayerMetadata {
        arch: context.target.arch.clone(),
        distro_name: context.target.distro_name.clone(),
        distro_version: context.target.distro_version.clone(),
        python_version: python_version.to_string(),
        poetry_version: POETRY_VERSION.to_string(),
    };

    let layer = context.cached_layer(
        // The name of this layer must be alphabetically after that of the `python` layer so that
        // this layer's `bin/` directory (and thus `python` symlink) is listed first in `PATH`:
        // https://github.com/buildpacks/spec/blob/main/buildpack.md#layer-paths
        layer_name!("venv"),
        CachedLayerDefinition {
            build: true,
            launch: true,
            invalid_metadata_action: &|_| InvalidMetadataAction::DeleteLayer,
            restored_layer_action: &|cached_metadata: &PoetryDependenciesLayerMetadata, _| {
                if cached_metadata == &new_metadata {
                    RestoredLayerAction::KeepLayer
                } else {
                    RestoredLayerAction::DeleteLayer
                }
            },
        },
    )?;
    let layer_path = layer.path();

    match layer.state {
        libcnb::layer::LayerState::Restored { .. } => {
            log_info("Using cached virtual environment");
        }
        libcnb::layer::LayerState::Empty { cause } => {
            match cause {
                EmptyLayerCause::InvalidMetadataAction { .. }
                | EmptyLayerCause::RestoredLayerAction { .. } => {
                    log_info("Discarding cached virtual environment");
                }
                EmptyLayerCause::NewlyCreated => {}
            }

            log_info("Creating virtual environment");
            utils::run_command_and_stream_output(
                Command::new("python")
                    .args(["-m", "venv", "--without-pip", &layer_path.to_string_lossy()])
                    .env_clear()
                    .envs(&*env),
            )
            .map_err(PoetryDependenciesLayerError::CreateVenvCommand)?;

            layer.write_metadata(new_metadata)?;
        }
    }

    let mut layer_env = LayerEnv::new()
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

    log_info("Running 'poetry install --sync --only main'");
    utils::run_command_and_stream_output(
        Command::new("poetry")
            .args([
                "install",
                // Compile Python bytecode up front to improve app boot times (pip does this by default).
                "--compile",
                "--only",
                "main",
                "--no-interaction",
                "--sync",
            ])
            .current_dir(&context.app_dir)
            .env_clear()
            .envs(&*env),
    )
    .map_err(PoetryDependenciesLayerError::PoetryInstallCommand)?;

    Ok(layer_path)
}

#[derive(Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PoetryDependenciesLayerMetadata {
    arch: String,
    distro_name: String,
    distro_version: String,
    python_version: String,
    poetry_version: String,
}

/// Errors that can occur when installing the project's dependencies into a layer using Poetry.
#[derive(Debug)]
pub(crate) enum PoetryDependenciesLayerError {
    CreateVenvCommand(StreamedCommandError),
    PoetryInstallCommand(StreamedCommandError),
}

impl From<PoetryDependenciesLayerError> for libcnb::Error<BuildpackError> {
    fn from(error: PoetryDependenciesLayerError) -> Self {
        Self::BuildpackError(BuildpackError::PoetryDependenciesLayer(error))
    }
}
