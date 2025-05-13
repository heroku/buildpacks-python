use crate::packaging_tool_versions::UV_VERSION;
use crate::python_version::PythonVersion;
use crate::utils::StreamedCommandError;
use crate::{BuildpackError, PythonBuildpack, utils};
use libcnb::Env;
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, InvalidMetadataAction, RestoredLayerAction,
};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use std::process::Command;

/// Creates a layer containing the application's Python dependencies, installed using uv.
//
// We install into a virtual environment since:
// - We can't install into the system site-packages inside the main Python directory since
//   we need the app dependencies to be in their own layer.
// - Some packages are broken with `--user` installs when using relocated Python, and
//   otherwise require other workarounds. eg: https://github.com/unbit/uwsgi/issues/2525
// - uv doesn't support `--user`: https://github.com/astral-sh/uv/issues/2077
// - PEP-405 style venvs are very lightweight and are also much more frequently
//   used in the wild compared to `--user`, and therefore the better tested path.
//
// We cache the virtual environment, since:
// - It results in faster builds than only caching uv's download/wheel cache.
// - It's safe to do so, since `uv sync` fully manages the environment (including
//   e.g. uninstalling packages when they are removed from the lockfile).
//
// With the venv cached there is no need to persist uv's download/wheel cache between builds.
// However, we have to ensure uv's cache is written to the same filesystem mount as this layer,
// otherwise uv won't be able to hardlink the downloaded files when installing them, and will
// fall back to slower file copies. By default uv saves its cache into the home directory,
// which for CNB builds will typically be a separate mount from the layers directory (even if
// both of those mounts are backed by the same filesystem on the host). As such, we configure
// uv to write its cache into a temporary layer instead (see `uv_cache.rs`).
pub(crate) fn install_dependencies(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
    python_version: &PythonVersion,
) -> Result<PathBuf, libcnb::Error<BuildpackError>> {
    let new_metadata = UvDependenciesLayerMetadata {
        arch: context.target.arch.clone(),
        distro_name: context.target.distro_name.clone(),
        distro_version: context.target.distro_version.clone(),
        python_version: python_version.to_string(),
        uv_version: UV_VERSION.to_string(),
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
            restored_layer_action: &|cached_metadata: &UvDependenciesLayerMetadata, _| {
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
            // We use Python's `venv` module to create the virtual environment since `uv venv` creates
            // import patching hooks that we don't want: https://github.com/astral-sh/uv/issues/6426
            // TODO: Open a PR upstream to remove those hooks or at least makes them optional.
            utils::run_command_and_stream_output(
                Command::new("python")
                    .args(["-m", "venv", "--without-pip", &layer_path.to_string_lossy()])
                    .env_clear()
                    .envs(&*env),
            )
            .map_err(UvDependenciesLayerError::CreateVenvCommand)?;

            layer.write_metadata(new_metadata)?;
        }
    }

    let mut layer_env = LayerEnv::new()
        // Make uv manage the venv we created above instead of creating its own venv, since we
        // need the venv to be in its own CNB layer instead of inside the app directory.
        // https://docs.astral.sh/uv/concepts/projects/config/#project-environment-path
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Override,
            "UV_PROJECT_ENVIRONMENT",
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

    let visible_args = ["sync", "--locked", "--no-default-groups"];

    // We only display the most relevant command args here, to improve the signal to noise ratio.
    log_info(format!("Running 'uv {}'", visible_args.join(" ")));

    utils::run_command_and_stream_output(
        Command::new("uv")
            .args(visible_args)
            .args([
                "--color",
                "always",
                // Compiling Python bytecode up front improves app boot times (pip does this by default).
                "--compile-bytecode",
                "--no-progress",
            ])
            .current_dir(&context.app_dir)
            .env_clear()
            .envs(&*env)
            // Redirect stderr to stdout, since uv prints to stderr by default:
            // https://github.com/astral-sh/uv/pull/134
            // ...and we need to work around: https://github.com/buildpacks/pack/issues/2330
            // TODO: Decide whether we want to use stdout or stderr for buildpack output in general, and
            // then perform redirection for all commands in `utils::run_command_and_stream_output`.
            .stdout(io::stdout())
            .stderr(io::stdout()),
    )
    .map_err(UvDependenciesLayerError::UvInstallCommand)?;

    Ok(layer_path)
}

#[derive(Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UvDependenciesLayerMetadata {
    arch: String,
    distro_name: String,
    distro_version: String,
    python_version: String,
    uv_version: String,
}

/// Errors that can occur when installing the project's dependencies into a layer using uv.
#[derive(Debug)]
pub(crate) enum UvDependenciesLayerError {
    CreateVenvCommand(StreamedCommandError),
    UvInstallCommand(StreamedCommandError),
}

impl From<UvDependenciesLayerError> for libcnb::Error<BuildpackError> {
    fn from(error: UvDependenciesLayerError) -> Self {
        Self::BuildpackError(BuildpackError::UvDependenciesLayer(error))
    }
}
