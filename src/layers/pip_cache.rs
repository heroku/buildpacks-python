use crate::packaging_tool_versions::PackagingToolVersions;
use crate::python_version::PythonVersion;
use crate::PythonBuildpack;
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::Buildpack;
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Layer containing Pip's cache of HTTP requests/downloads and built package wheels.
pub(crate) struct PipCacheLayer<'a> {
    /// The Python version used for this build.
    pub(crate) python_version: &'a PythonVersion,
    /// The pip, setuptools and wheel versions used for this build.
    pub(crate) packaging_tool_versions: &'a PackagingToolVersions,
}

impl Layer for PipCacheLayer<'_> {
    type Buildpack = PythonBuildpack;
    type Metadata = PipCacheLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: false,
            cache: true,
            launch: false,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        let layer_metadata = self.generate_layer_metadata(&context.stack_id);
        LayerResultBuilder::new(layer_metadata).build()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
        let cached_metadata = &layer_data.content_metadata.metadata;
        let new_metadata = &self.generate_layer_metadata(&context.stack_id);

        if cached_metadata == new_metadata {
            log_info("Using cached pip download/wheel cache");
            Ok(ExistingLayerStrategy::Keep)
        } else {
            log_info("Discarding cached pip download/wheel cache");
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

impl<'a> PipCacheLayer<'a> {
    fn generate_layer_metadata(&self, stack_id: &StackId) -> PipCacheLayerMetadata {
        PipCacheLayerMetadata {
            stack: stack_id.clone(),
            python_version: self.python_version.to_string(),
            packaging_tool_versions: self.packaging_tool_versions.clone(),
        }
    }
}

/// Metadata stored in the generated layer that allows future builds to determine whether
/// the cached layer needs to be invalidated or not.
// Timestamp based cache invalidation isn't used here since the Python/pip/setuptools/wheel
// versions will change often enough that it isn't worth the added complexity. Ideally pip
// would support cleaning up its own cache: https://github.com/pypa/pip/issues/6956
#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PipCacheLayerMetadata {
    stack: StackId,
    python_version: String,
    packaging_tool_versions: PackagingToolVersions,
}
