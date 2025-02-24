use crate::packaging_tool_versions::PIP_VERSION;
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

/// Creates a layer containing pip.
pub(crate) fn install_pip(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
    python_version: &PythonVersion,
    python_layer_path: &Path,
) -> Result<(), libcnb::Error<BuildpackError>> {
    let new_metadata = PipLayerMetadata {
        python_version: python_version.to_string(),
        pip_version: PIP_VERSION.to_string(),
    };

    let layer = context.cached_layer(
        layer_name!("pip"),
        CachedLayerDefinition {
            build: true,
            launch: false,
            invalid_metadata_action: &|_| InvalidMetadataAction::DeleteLayer,
            restored_layer_action: &|cached_metadata: &PipLayerMetadata, _| {
                let cached_pip_version = cached_metadata.pip_version.clone();
                if cached_metadata == &new_metadata {
                    (RestoredLayerAction::KeepLayer, cached_pip_version)
                } else {
                    (RestoredLayerAction::DeleteLayer, cached_pip_version)
                }
            },
        },
    )?;

    let mut layer_env = LayerEnv::new()
        // We use a curated pip version, so disable the update check to speed up pip invocations,
        // reduce build log spam and prevent users from thinking they need to manually upgrade.
        // https://pip.pypa.io/en/stable/cli/pip/#cmdoption-disable-pip-version-check
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Override,
            "PIP_DISABLE_PIP_VERSION_CHECK",
            "1",
        )
        // Move the Python user base directory to this layer instead of under HOME:
        // https://docs.python.org/3/using/cmdline.html#envvar-PYTHONUSERBASE
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Override,
            "PYTHONUSERBASE",
            layer.path(),
        );

    match layer.state {
        LayerState::Restored {
            cause: ref cached_pip_version,
        } => {
            log_info(format!("Using cached pip {cached_pip_version}"));
        }
        LayerState::Empty { ref cause } => {
            match cause {
                EmptyLayerCause::InvalidMetadataAction { .. } => {
                    log_info("Discarding cached pip since its layer metadata can't be parsed");
                }
                EmptyLayerCause::RestoredLayerAction {
                    cause: cached_pip_version,
                } => {
                    log_info(format!("Discarding cached pip {cached_pip_version}"));
                }
                EmptyLayerCause::NewlyCreated => {}
            }

            log_info(format!("Installing pip {PIP_VERSION}"));

            // We use the pip wheel bundled within Python's standard library to install our chosen
            // pip version, since it's faster than `ensurepip` followed by an upgrade in place.
            let bundled_pip_module_path =
                utils::bundled_pip_module_path(python_layer_path, python_version)
                    .map_err(PipLayerError::LocateBundledPip)?;

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
                        format!("pip=={PIP_VERSION}").as_str(),
                    ])
                    .env_clear()
                    .envs(&layer_env.apply(Scope::Build, env)),
            )
            .map_err(PipLayerError::InstallPipCommand)?;

            layer.write_metadata(new_metadata)?;
        }
    }

    layer.write_env(&layer_env)?;
    // Required to pick up the automatic PATH env var. See: https://github.com/heroku/libcnb.rs/issues/842
    layer_env = layer.read_env()?;
    env.clone_from(&layer_env.apply(Scope::Build, env));

    Ok(())
}

// pip's wheel is a pure Python package with no dependencies, so the layer is not arch or distro
// specific. However, the generated .pyc files vary by Python version.
#[derive(Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PipLayerMetadata {
    python_version: String,
    pip_version: String,
}

/// Errors that can occur when installing pip into a layer.
#[derive(Debug)]
pub(crate) enum PipLayerError {
    InstallPipCommand(StreamedCommandError),
    LocateBundledPip(io::Error),
}

impl From<PipLayerError> for libcnb::Error<BuildpackError> {
    fn from(error: PipLayerError) -> Self {
        Self::BuildpackError(BuildpackError::PipLayer(error))
    }
}
