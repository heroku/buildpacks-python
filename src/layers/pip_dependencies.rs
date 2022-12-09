use crate::python_version::PythonVersion;
use crate::utils::{self, CommandError};
use crate::{BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::{Buildpack, Env};
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

pub(crate) struct PipDependenciesLayer<'a> {
    pub env: &'a Env,
    pub pip_cache_dir: PathBuf,
    pub python_version: &'a PythonVersion,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PipDependenciesLayerMetadata {
    python_version: String,
    stack: StackId,
}

impl Layer for PipDependenciesLayer<'_> {
    type Buildpack = PythonBuildpack;
    type Metadata = PipDependenciesLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            // TODO: Re-enabling caching once remaining invalidation logic finished.
            cache: false,
            launch: true,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        // TODO: Explain PYTHONUSERBASE and that it will contain bin/, lib/.../site-packages/
        // etc and so does not need to be nested due to the env/ directory.
        let layer_env = LayerEnv::new().chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "PYTHONUSERBASE",
            layer_path,
        );
        let env = layer_env.apply(Scope::Build, self.env);

        let src_dir = layer_path.join("src");
        fs::create_dir(&src_dir).map_err(PipDependenciesLayerError::CreateSrcDirIo)?;

        log_info("Running pip install");

        // TODO: Explain why we're using user install
        // TODO: Refactor this out so it can be shared with `update()`
        // TODO: Mention that we're intentionally not using env_clear() otherwise
        // PATH won't be set, and Pip won't be able to find things like Git.
        utils::run_command(
            Command::new("pip")
                .args([
                    "install",
                    "--cache-dir",
                    &self.pip_cache_dir.to_string_lossy(),
                    "--no-input",
                    // Prevent warning about the `bin/` directory not being on `PATH`, since it
                    // will be added automatically by libcnb/lifecycle later.
                    "--no-warn-script-location",
                    "--progress",
                    "off",
                    "--user",
                    "--requirement",
                    "requirements.txt",
                    // Make pip clone any VCS repositories installed in editable mode into a directory in this layer,
                    // rather than the default of the current working directory (the app dir).
                    "--src",
                    &src_dir.to_string_lossy(),
                ])
                .envs(&env)
                // TODO: Decide whether to use this or `--no-compile` + `compileall`.
                // If using compileall will need different strategy for `update()`.
                // See also: https://github.com/pypa/pip/blob/3820b0e52c7fed2b2c43ba731b718f316e6816d1/src/pip/_internal/operations/install/wheel.py#L616
                // Using 1980-01-01T00:00:01Z to avoid:
                // ValueError: ZIP does not support timestamps before 1980
                .env("SOURCE_DATE_EPOCH", "315532800"),
        )
        .map_err(PipDependenciesLayerError::PipInstallCommand)?;

        log_info("Pip install completed");

        let layer_metadata = generate_layer_metadata(&context.stack_id, self.python_version);
        LayerResultBuilder::new(layer_metadata)
            .env(layer_env)
            .build()
    }

    // TODO: Re-enabling caching once remaining invalidation logic finished.
    // fn update(
    //     &self,
    //     _context: &BuildContext<Self::Buildpack>,
    //     _layer_data: &LayerData<Self::Metadata>,
    // ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
    //     // TODO
    //     unimplemented!()
    // }
    //
    // fn existing_layer_strategy(
    //     &self,
    //     context: &BuildContext<Self::Buildpack>,
    //     layer_data: &LayerData<Self::Metadata>,
    // ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
    //     // TODO: Also invalidate based on requirements.txt contents
    //     // TODO: Decide whether sub-requirements files should also invalidate? If not, should we warn?
    //     // TODO: Also invalidate based on time since layer creation
    //     // TODO: Decide what should be logged
    //     // TODO: Re-test the performance of caching site-modules vs only caching Pip's cache.
    //     #[allow(unreachable_code)]
    //     if layer_data.content_metadata.metadata
    //         == generate_layer_metadata(&context.stack_id, self.python_version)
    //     {
    //         log_info("Re-using cached dependencies");
    //         Ok(ExistingLayerStrategy::Update)
    //     } else {
    //         log_info("Discarding cached dependencies");
    //         Ok(ExistingLayerStrategy::Recreate)
    //     }
    // }
}

fn generate_layer_metadata(
    stack_id: &StackId,
    python_version: &PythonVersion,
) -> PipDependenciesLayerMetadata {
    // TODO: Add requirements.txt SHA256 or similar
    // TODO: Add timestamp field or similar
    PipDependenciesLayerMetadata {
        python_version: python_version.to_string(),
        stack: stack_id.clone(),
    }
}

#[derive(Debug)]
pub(crate) enum PipDependenciesLayerError {
    CreateSrcDirIo(io::Error),
    PipInstallCommand(CommandError),
}

impl From<PipDependenciesLayerError> for BuildpackError {
    fn from(error: PipDependenciesLayerError) -> Self {
        Self::PipLayer(error)
    }
}

// TODO: Unit tests for cache invalidation handling?
