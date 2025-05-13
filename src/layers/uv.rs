use crate::packaging_tool_versions::UV_VERSION;
use crate::utils::{self, DownloadUnpackArchiveError};
use crate::{BuildpackError, PythonBuildpack};
use libcnb::Env;
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction,
};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::env::consts::ARCH;

/// Creates a build-only layer containing uv.
pub(crate) fn install_uv(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
) -> Result<(), libcnb::Error<BuildpackError>> {
    let new_metadata = UvLayerMetadata {
        arch: context.target.arch.clone(),
        uv_version: UV_VERSION.to_string(),
    };

    let layer = context.cached_layer(
        layer_name!("uv"),
        CachedLayerDefinition {
            build: true,
            launch: false,
            invalid_metadata_action: &|_| InvalidMetadataAction::DeleteLayer,
            restored_layer_action: &|cached_metadata: &UvLayerMetadata, _| {
                let cached_uv_version = cached_metadata.uv_version.clone();
                if cached_metadata == &new_metadata {
                    (RestoredLayerAction::KeepLayer, cached_uv_version)
                } else {
                    (RestoredLayerAction::DeleteLayer, cached_uv_version)
                }
            },
        },
    )?;

    // Prevent uv from downloading/using its own Python installation:
    // https://docs.astral.sh/uv/concepts/python-versions/#disabling-automatic-python-downloads
    let mut layer_env = LayerEnv::new()
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Override,
            "UV_NO_MANAGED_PYTHON",
            "1",
        )
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Override,
            "UV_PYTHON_DOWNLOADS",
            "never",
        );

    match layer.state {
        LayerState::Restored {
            cause: ref cached_uv_version,
        } => {
            log_info(format!("Using cached uv {cached_uv_version}"));
        }
        LayerState::Empty { ref cause } => {
            match cause {
                EmptyLayerCause::InvalidMetadataAction { .. } => {
                    log_info("Discarding cached uv since its layer metadata can't be parsed");
                }
                EmptyLayerCause::RestoredLayerAction {
                    cause: cached_uv_version,
                } => {
                    log_info(format!("Discarding cached uv {cached_uv_version}"));
                }
                EmptyLayerCause::NewlyCreated => {}
            }

            log_info(format!("Installing uv {UV_VERSION}"));
            // There's also a statically compiled musl uv binary archive, but:
            // 1. It's currently slower than the glibc variant: https://github.com/astral-sh/uv/issues/10610
            // 2. At the moment this buildpack only supports Ubuntu anyway (we only compile Python runtimes for Ubuntu).
            let archive_url = format!(
                "https://github.com/astral-sh/uv/releases/download/{UV_VERSION}/uv-{ARCH}-unknown-linux-gnu.tar.gz"
            );
            let layer_bin_dir = layer.path().join("bin");
            utils::download_and_unpack_nested_gzip_archive(&archive_url, &layer_bin_dir, 1)
                .map_err(UvLayerError::DownloadUnpackUvArchive)?;

            layer.write_metadata(new_metadata)?;
        }
    }

    layer.write_env(&layer_env)?;
    // Required to pick up the automatic PATH env var. See: https://github.com/heroku/libcnb.rs/issues/842
    layer_env = layer.read_env()?;
    env.clone_from(&layer_env.apply(Scope::Build, env));

    Ok(())
}

#[derive(Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UvLayerMetadata {
    arch: String,
    uv_version: String,
}

/// Errors that can occur when installing uv into a layer.
#[derive(Debug)]
pub(crate) enum UvLayerError {
    DownloadUnpackUvArchive(DownloadUnpackArchiveError),
}

impl From<UvLayerError> for libcnb::Error<BuildpackError> {
    fn from(error: UvLayerError) -> Self {
        Self::BuildpackError(BuildpackError::UvLayer(error))
    }
}
