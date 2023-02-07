use crate::utils::{self, CommandError};
use crate::{BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::{Buildpack, Env};
use libherokubuildpack::log::log_info;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

pub(crate) struct PipDependenciesLayer<'a> {
    pub env: &'a Env,
    pub pip_cache_dir: PathBuf,
}

impl Layer for PipDependenciesLayer<'_> {
    type Buildpack = PythonBuildpack;
    type Metadata = GenericMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            cache: false,
            launch: true,
        }
    }

    // TODO: Explain why we're not caching here.
    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
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
        // TODO: Mention that we're intentionally not using env_clear() otherwise
        // PATH won't be set, and Pip won't be able to find things like Git.
        utils::run_command(
            Command::new("pip")
                .args([
                    "install",
                    "--cache-dir",
                    &self.pip_cache_dir.to_string_lossy(),
                    // We use a curated Pip version, so skip the update check to speed up Pip invocations,
                    // reduce build log spam and prevent users from thinking they need to manually upgrade.
                    "--disable-pip-version-check",
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
                // TODO: Explain why we're setting this
                // Using 1980-01-01T00:00:01Z to avoid:
                // ValueError: ZIP does not support timestamps before 1980
                .env("SOURCE_DATE_EPOCH", "315532800"),
        )
        .map_err(PipDependenciesLayerError::PipInstallCommand)?;

        log_info("Pip install completed");

        LayerResultBuilder::new(GenericMetadata::default())
            .env(layer_env)
            .build()
    }
}

/// Errors that can occur when installing the project's dependencies into a layer using Pip.
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
