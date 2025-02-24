use crate::packaging_tool_versions::POETRY_VERSION;
use crate::python_version::PythonVersion;
use crate::utils::StreamedCommandError;
use crate::{BuildpackError, PythonBuildpack, utils};
use libcnb::Env;
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction,
};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use std::process::Command;

/// Creates a build-only layer containing Poetry.
pub(crate) fn install_poetry(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
    python_version: &PythonVersion,
    python_layer_path: &Path,
) -> Result<(), libcnb::Error<BuildpackError>> {
    let new_metadata = PoetryLayerMetadata {
        arch: context.target.arch.clone(),
        distro_name: context.target.distro_name.clone(),
        distro_version: context.target.distro_version.clone(),
        python_version: python_version.to_string(),
        poetry_version: POETRY_VERSION.to_string(),
    };

    let layer = context.cached_layer(
        layer_name!("poetry"),
        CachedLayerDefinition {
            build: true,
            launch: false,
            invalid_metadata_action: &|_| InvalidMetadataAction::DeleteLayer,
            restored_layer_action: &|cached_metadata: &PoetryLayerMetadata, _| {
                let cached_poetry_version = cached_metadata.poetry_version.clone();
                if cached_metadata == &new_metadata {
                    (RestoredLayerAction::KeepLayer, cached_poetry_version)
                } else {
                    (RestoredLayerAction::DeleteLayer, cached_poetry_version)
                }
            },
        },
    )?;

    // Move the Python user base directory to this layer instead of under HOME:
    // https://docs.python.org/3/using/cmdline.html#envvar-PYTHONUSERBASE
    let mut layer_env = LayerEnv::new().chainable_insert(
        Scope::Build,
        ModificationBehavior::Override,
        "PYTHONUSERBASE",
        layer.path(),
    );

    match layer.state {
        LayerState::Restored {
            cause: ref cached_poetry_version,
        } => {
            log_info(format!("Using cached Poetry {cached_poetry_version}"));
        }
        LayerState::Empty { ref cause } => {
            match cause {
                EmptyLayerCause::InvalidMetadataAction { .. } => {
                    log_info("Discarding cached Poetry since its layer metadata can't be parsed");
                }
                EmptyLayerCause::RestoredLayerAction {
                    cause: cached_poetry_version,
                } => {
                    log_info(format!("Discarding cached Poetry {cached_poetry_version}"));
                }
                EmptyLayerCause::NewlyCreated => {}
            }

            log_info(format!("Installing Poetry {POETRY_VERSION}"));

            // We use the pip wheel bundled within Python's standard library to install Poetry.
            // Whilst Poetry does still require pip for some tasks (such as package uninstalls),
            // it bundles its own copy for use as a fallback. As such we don't need to install pip
            // into the user site-packages (and in fact, Poetry wouldn't use this install anyway,
            // since it only finds an external pip if it exists in the target venv).
            let bundled_pip_module_path =
                utils::bundled_pip_module_path(python_layer_path, python_version)
                    .map_err(PoetryLayerError::LocateBundledPip)?;

            utils::run_command_and_stream_output(
                Command::new("python")
                    .args([
                        &bundled_pip_module_path.to_string_lossy(),
                        "install",
                        // There is no point using pip's cache here, since the layer itself will be cached.
                        "--no-cache-dir",
                        "--no-input",
                        "--no-warn-script-location",
                        "--quiet",
                        "--user",
                        format!("poetry=={POETRY_VERSION}").as_str(),
                    ])
                    .env_clear()
                    .envs(&layer_env.apply(Scope::Build, env)),
            )
            .map_err(PoetryLayerError::InstallPoetryCommand)?;

            layer.write_metadata(new_metadata)?;
        }
    }

    layer.write_env(&layer_env)?;
    // Required to pick up the automatic PATH env var. See: https://github.com/heroku/libcnb.rs/issues/842
    layer_env = layer.read_env()?;
    env.clone_from(&layer_env.apply(Scope::Build, env));

    Ok(())
}

// Some of Poetry's dependencies contain compiled components so are platform-specific (unlike pure
// Python packages). As such we have to take arch and distro into account for cache invalidation.
#[derive(Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PoetryLayerMetadata {
    arch: String,
    distro_name: String,
    distro_version: String,
    python_version: String,
    poetry_version: String,
}

/// Errors that can occur when installing Poetry into a layer.
#[derive(Debug)]
pub(crate) enum PoetryLayerError {
    InstallPoetryCommand(StreamedCommandError),
    LocateBundledPip(io::Error),
}

impl From<PoetryLayerError> for libcnb::Error<BuildpackError> {
    fn from(error: PoetryLayerError) -> Self {
        Self::BuildpackError(BuildpackError::PoetryLayer(error))
    }
}
