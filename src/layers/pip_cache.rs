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

pub(crate) struct PipCacheLayer<'a> {
    pub python_version: &'a PythonVersion,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PipCacheLayerMetadata {
    python_version: String,
    stack: StackId,
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
        log_info("Pip cache created");
        let layer_metadata = generate_layer_metadata(&context.stack_id, self.python_version);
        LayerResultBuilder::new(layer_metadata).build()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
        // TODO: Also invalidate based on time since layer creation?
        // TODO: Decide what should be logged
        if layer_data.content_metadata.metadata
            == generate_layer_metadata(&context.stack_id, self.python_version)
        {
            log_info("Re-using cached pip-cache");
            Ok(ExistingLayerStrategy::Keep)
        } else {
            log_info("Discarding cached pip-cache");
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

fn generate_layer_metadata(
    stack_id: &StackId,
    python_version: &PythonVersion,
) -> PipCacheLayerMetadata {
    // TODO: Add timestamp field or similar (maybe not necessary if invalidating on pip/python change?)
    // TODO: Invalidate on pip version change?
    PipCacheLayerMetadata {
        python_version: python_version.to_string(),
        stack: stack_id.clone(),
    }
}

// TODO: Unit tests for cache invalidation handling?
