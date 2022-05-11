use crate::python_version::PythonVersion;
use crate::PythonBuildpack;
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Buildpack;
use libherokubuildpack::log_info;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) struct PipCacheLayer<'a> {
    pub python_version: &'a PythonVersion,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PipCacheLayerMetadata {
    stack: StackId,
    python_version: String,
}

impl Layer for PipCacheLayer<'_> {
    type Buildpack = PythonBuildpack;
    type Metadata = PipCacheLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            // TODO: Decide whether pip cache should be shared with other buildpacks
            build: false,
            launch: false,
            cache: true,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        let layer_env = LayerEnv::new().chainable_insert(
            Scope::All,
            // TODO: Should this be override?
            ModificationBehavior::Default,
            "PIP_CACHE_DIR",
            layer_path,
        );

        log_info("Pip cache configured");

        LayerResultBuilder::new(self.generate_layer_metadata(context))
            .env(layer_env)
            .build()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
        // TODO: Also invalidate based on time since layer creation?
        // TODO: Decide what should be logged
        if layer_data.content_metadata.metadata == self.generate_layer_metadata(context) {
            log_info("Re-using cached pip-cache");
            Ok(ExistingLayerStrategy::Keep)
        } else {
            log_info("Discarding cached pip-cache");
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

impl PipCacheLayer<'_> {
    fn generate_layer_metadata(
        &self,
        context: &BuildContext<PythonBuildpack>,
    ) -> PipCacheLayerMetadata {
        // TODO: Add timestamp field or similar
        PipCacheLayerMetadata {
            stack: context.stack_id.clone(),
            python_version: self.python_version.to_string(),
        }
    }
}
