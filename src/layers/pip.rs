use crate::python_version::PythonVersion;
use crate::utils::{self, CommandError};
use crate::{PythonBuildpack, PythonBuildpackError};
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::{Buildpack, Env};
use libherokubuildpack::log_info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) struct PipLayer<'a> {
    pub pip_cache_dir: PathBuf,
    pub python_env: Env,
    pub python_version: &'a PythonVersion,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PipLayerMetadata {
    python_version: String,
    stack: StackId,
}

impl Layer for PipLayer<'_> {
    type Buildpack = PythonBuildpack;
    type Metadata = PipLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            cache: true,
            launch: true,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        let layer_env = LayerEnv::new().chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "PYTHONUSERBASE",
            layer_path,
        );
        let env = layer_env.apply(Scope::Build, &self.python_env);

        // TODO: Explain why we're using user install
        // TODO: Refactor this out so it can be shared with `update()`
        utils::run_command(
            Command::new("pip")
                .args([
                    "install",
                    "--cache-dir",
                    &self.pip_cache_dir.to_string_lossy(),
                    // TODO: Remove this if not using compileall
                    // "--no-compile",
                    "--no-input",
                    // Prevent warning about the `bin/` directory not being on `PATH`, since it
                    // will be added automatically by libcnb/lifecycle later.
                    "--no-warn-script-location",
                    "--progress",
                    "off",
                    "--user",
                    "-r",
                    "requirements.txt",
                ])
                .env_clear()
                .envs(&env)
                // TODO: Decide whether to use this or compileall.
                // If using compileall will need different strategy for `update()`.
                // See also: https://github.com/pypa/pip/blob/3820b0e52c7fed2b2c43ba731b718f316e6816d1/src/pip/_internal/operations/install/wheel.py#L616
                .env("SOURCE_DATE_EPOCH", "1"),
        )
        .map_err(PipLayerError::PipInstall)?;

        let layer_metadata = generate_layer_metadata(&context.stack_id, self.python_version);
        LayerResultBuilder::new(layer_metadata)
            .env(layer_env)
            .build()
    }

    fn update(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_data: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        // TODO
        unimplemented!()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
        // TODO: Also invalidate based on requirements.txt contents
        // TODO: Decide whether sub-requirements files should also invalidate? If not, should we warn?
        // TODO: Also invalidate based on time since layer creation
        // TODO: Decide what should be logged
        if layer_data.content_metadata.metadata
            == generate_layer_metadata(&context.stack_id, self.python_version)
        {
            // log_info("Re-using cached dependencies");
            // TODO: Switch to ExistingLayerStrategy::Update once `update()` implemented
            Ok(ExistingLayerStrategy::Keep)
            // Ok(ExistingLayerStrategy::Recreate)
        } else {
            log_info("Discarding cached dependencies");
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

fn generate_layer_metadata(stack_id: &StackId, python_version: &PythonVersion) -> PipLayerMetadata {
    // TODO: Add requirements.txt SHA256 or similar
    // TODO: Add timestamp field or similar
    PipLayerMetadata {
        python_version: python_version.to_string(),
        stack: stack_id.clone(),
    }
}

#[derive(Debug)]
pub(crate) enum PipLayerError {
    PipInstall(CommandError),
}

impl From<PipLayerError> for PythonBuildpackError {
    fn from(error: PipLayerError) -> Self {
        Self::PipLayer(error)
    }
}

// TODO: Unit and/or integration tests
