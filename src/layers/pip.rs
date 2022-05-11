use crate::python_version::PythonVersion;
use crate::{PythonBuildpack, PythonBuildpackError};
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::{Buildpack, Env};
use libherokubuildpack::log_info;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub(crate) struct PipLayer {
    pub pip_cache_dir: PathBuf,
    pub python_env: Env,
    pub python_version: PythonVersion,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PipLayerMetadata {
    stack: StackId,
    python_version: String,
}

impl Layer for PipLayer {
    type Buildpack = PythonBuildpack;
    type Metadata = PipLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        // TODO: Explain why we're using user install
        // TODO: Consider using `--no-warn-script-location` to suppress warning about bin not being on PATH, or else add to PATH
        // TODO: Refactor this out so it can be shared with `update()`
        Command::new("pip")
            .args([
                "install",
                "--compile",
                "--disable-pip-version-check",
                "--no-input",
                "--progress",
                "off",
                // TODO: Should cache dir be set via env var instead?
                // Do we want other buildpacks using the pip cache?
                "--cache-dir",
                &self.pip_cache_dir.to_string_lossy(),
                "--user",
                "-r",
                "requirements.txt",
            ])
            .env_clear()
            .envs(&self.python_env)
            // TODO: Combine this with setting the LayerEnv
            .env("PYTHONUSERBASE", layer_path)
            // TODO: Decide whether to use this or compileall.
            // If using compileall will need different strategy for `update()`.
            // See also: https://github.com/pypa/pip/blob/3820b0e52c7fed2b2c43ba731b718f316e6816d1/src/pip/_internal/operations/install/wheel.py#L616
            .env("SOURCE_DATE_EPOCH", "1")
            .status()
            .map_err(PipLayerError::PipInstallIOError)
            .and_then(|exit_status| {
                if exit_status.success() {
                    Ok(())
                } else {
                    Err(PipLayerError::PipInstallNonzeroExitCode(exit_status))
                }
            })?;

        let layer_env = LayerEnv::new().chainable_insert(
            Scope::All,
            // TODO: Should this be override?
            ModificationBehavior::Override,
            "PYTHONUSERBASE",
            layer_path,
        );

        LayerResultBuilder::new(self.generate_layer_metadata(context))
            .env(layer_env)
            .build()
    }

    fn update(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_data: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        unimplemented!()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
        // TODO: Also invalidate based on requirements.txt contents
        // TODO: Also invalidate based on time since layer creation
        // TODO: Decide what should be logged
        if layer_data.content_metadata.metadata == self.generate_layer_metadata(context) {
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

impl PipLayer {
    fn generate_layer_metadata(&self, context: &BuildContext<PythonBuildpack>) -> PipLayerMetadata {
        // TODO: Add requirements.txt SHA256 or similar
        // TODO: Add timestamp field or similar
        PipLayerMetadata {
            stack: context.stack_id.clone(),
            python_version: self.python_version.to_string(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum PipLayerError {
    PipInstallIOError(io::Error),
    PipInstallNonzeroExitCode(ExitStatus),
}

impl From<PipLayerError> for PythonBuildpackError {
    fn from(error: PipLayerError) -> Self {
        Self::PipLayer(error)
    }
}
