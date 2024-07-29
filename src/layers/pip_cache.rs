use crate::packaging_tool_versions::PackagingToolVersions;
use crate::python_version::PythonVersion;
use crate::{BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction,
};
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Creates a build-only layer for Pip's cache of HTTP requests/downloads and built package wheels.
pub(crate) fn prepare_pip_cache(
    context: &BuildContext<PythonBuildpack>,
    python_version: &PythonVersion,
    packaging_tool_versions: &PackagingToolVersions,
) -> Result<PathBuf, libcnb::Error<BuildpackError>> {
    let new_metadata = PipCacheLayerMetadata {
        arch: context.target.arch.clone(),
        distro_name: context.target.distro_name.clone(),
        distro_version: context.target.distro_version.clone(),
        python_version: python_version.to_string(),
        packaging_tool_versions: packaging_tool_versions.clone(),
    };

    let layer = context.cached_layer(
        layer_name!("pip-cache"),
        CachedLayerDefinition {
            build: false,
            launch: false,
            invalid_metadata_action: &|_| InvalidMetadataAction::DeleteLayer,
            restored_layer_action: &|cached_metadata: &PipCacheLayerMetadata, _| {
                if cached_metadata == &new_metadata {
                    Ok(RestoredLayerAction::KeepLayer)
                } else {
                    Ok(RestoredLayerAction::DeleteLayer)
                }
            },
        },
    )?;

    match layer.state {
        LayerState::Restored { .. } => {
            log_info("Using cached pip download/wheel cache");
        }
        LayerState::Empty { cause } => {
            match cause {
                EmptyLayerCause::InvalidMetadataAction { .. }
                | EmptyLayerCause::RestoredLayerAction { .. } => {
                    // We don't go into more details as to why the cache has been discarded, since
                    // the reasons will be the same as those logged during the earlier Python layer.
                    log_info("Discarding cached pip download/wheel cache");
                }
                EmptyLayerCause::NewlyCreated => {}
            }
            layer.write_metadata(new_metadata)?;
        }
    }

    Ok(layer.path())
}

// Timestamp based cache invalidation isn't used here since the Python/pip/setuptools/wheel
// versions will change often enough that it isn't worth the added complexity. Ideally pip
// would support cleaning up its own cache: https://github.com/pypa/pip/issues/6956
#[derive(Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PipCacheLayerMetadata {
    arch: String,
    distro_name: String,
    distro_version: String,
    python_version: String,
    packaging_tool_versions: PackagingToolVersions,
}
