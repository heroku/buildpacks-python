use crate::python_version::PythonVersion;
use crate::utils::{self, CommandError, DownloadUnpackError};
use crate::{PythonBuildpack, PythonBuildpackError};
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Buildpack;
use libherokubuildpack::{log_header, log_info};
use serde::{Deserialize, Serialize};
use std::fs::Permissions;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

const PIP_VERSION: &str = "22.2";
const SETUPTOOLS_VERSION: &str = "63.2.0";
const WHEEL_VERSION: &str = "0.37.1";

pub(crate) struct PythonLayer<'a> {
    pub python_version: &'a PythonVersion,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct PythonLayerMetadata {
    stack: StackId,
    python_version: String,
    pip_version: String,
    setuptools_version: String,
    wheel_version: String,
}

impl Layer for PythonLayer<'_> {
    type Buildpack = PythonBuildpack;
    type Metadata = PythonLayerMetadata;

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
        log_header("Installing Python");

        // TODO: Move this URL generation somewhere else (ie manifest etc).
        let archive_url = format!(
            "https://heroku-buildpack-python.s3.us-east-1.amazonaws.com/{}/runtimes/python-{}.tar.gz",
            context.stack_id, self.python_version
        );

        log_info(format!("Downloading Python {}", self.python_version));
        utils::download_and_unpack_gzip(&archive_url, layer_path)
            .map_err(PythonLayerError::DownloadUnpack)?;
        log_info("Python installation successful");

        // TODO: Decide whether to move env vars to their own layers, so invalidation can occur separately (check perf of additional layers?)
        let layer_env = LayerEnv::new()
            // Ensure Python uses a Unicode locate, to prevent the issues described in:
            // https://github.com/docker-library/python/pull/570
            .chainable_insert(
                Scope::All,
                ModificationBehavior::Override,
                "LANG",
                "C.UTF-8",
            )
            // We use a curated Pip version, so skip the update check to speed up Pip invocations,
            // reduce build log spam and prevent users from thinking they need to manually upgrade.
            .chainable_insert(
                Scope::All,
                ModificationBehavior::Override,
                "PIP_DISABLE_PIP_VERSION_CHECK",
                "1",
            )
            // Disable Python's output buffering to ensure logs aren't dropped if an app crashes.
            .chainable_insert(
                Scope::All,
                ModificationBehavior::Override,
                "PYTHONUNBUFFERED",
                "1",
            );
        let env = layer_env.apply_to_empty(Scope::Build);

        log_header("Installing Pip");
        log_info(format!("Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}"));

        let python_binary = layer_path.join("bin/python");
        let python_stdlib_dir = layer_path.join(format!(
            "lib/python{}.{}",
            self.python_version.major, self.python_version.minor
        ));
        let site_packages_dir = python_stdlib_dir.join("site-packages");

        // TODO: Explain what's happening here
        let bundled_pip_module = bundled_pip_module(&python_stdlib_dir)
            .map_err(PythonLayerError::CannotLocateBundledPip)?;
        utils::run_command(
            Command::new(&python_binary)
                .args([
                    &bundled_pip_module.to_string_lossy(),
                    "install",
                    "--no-cache-dir",
                    "--no-compile",
                    "--no-input",
                    "--quiet",
                    format!("pip=={PIP_VERSION}").as_str(),
                    format!("setuptools=={SETUPTOOLS_VERSION}").as_str(),
                    format!("wheel=={WHEEL_VERSION}").as_str(),
                ])
                .env_clear()
                .envs(&env),
        )
        .map_err(PythonLayerError::PipBootstrap)?;

        // TODO: Add comment explaining why we're doing this vs pip default compile.
        utils::run_command(
            Command::new(python_binary)
                .args([
                    "-m",
                    "compileall",
                    "-f",
                    "-q",
                    "--invalidation-mode",
                    "unchecked-hash",
                    "--workers",
                    "0",
                    &site_packages_dir.to_string_lossy(),
                ])
                .env_clear()
                .envs(&env),
        )
        .map_err(PythonLayerError::PipCompile)?;

        // By default Pip will install into the system site-packages directory if it is writeable
        // by the current user. Whilst the buildpack's own `pip install` invocations always use
        // `--user` to ensure application dependencies are instead installed into the user
        // site-packages, it's possible other buildpacks or custom scripts may forget to do so.
        // By making the system site-packages directory read-only, Pip will automatically use
        // user installs in such cases:
        // https://github.com/pypa/pip/blob/22.1.1/src/pip/_internal/commands/install.py#L619-L677
        fs::set_permissions(&site_packages_dir, Permissions::from_mode(0o555))
            .map_err(PythonLayerError::CannotMakeSitePackagesReadOnly)?;

        log_info("Installation completed");

        let layer_metadata = generate_layer_metadata(&context.stack_id, self.python_version);
        LayerResultBuilder::new(layer_metadata)
            .env(layer_env)
            .build()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
        // TODO: Decide what should be logged in the cached case (+more granular reason?)
        // Worth including what changed not only for cache invalidation, but also
        // to help debug any issues (eg changed pip version causing issues)
        let new_metadata = generate_layer_metadata(&context.stack_id, self.python_version);
        if layer_data.content_metadata.metadata == new_metadata {
            log_header("Installing Python");
            log_info(format!("Re-using cached Python {}", self.python_version));

            log_header("Installing Pip");
            log_info(format!(
                "Re-using cached pip {}, setuptools {} and wheel {}",
                new_metadata.pip_version,
                new_metadata.setuptools_version,
                new_metadata.wheel_version
            ));

            Ok(ExistingLayerStrategy::Keep)
        } else {
            log_info(format!("Discarding cached Python {}", self.python_version));
            log_info(format!(
                "Discarding cached pip {}, setuptools {} and wheel {}",
                new_metadata.pip_version,
                new_metadata.setuptools_version,
                new_metadata.wheel_version
            ));
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

// TODO: Explain what's happening here
// The bundled version of Pip (and thus the wheel filename) varies across Python versions,
// so we have to search the bundled wheels directory for the appropriate file.
fn bundled_pip_module(python_stdlib_dir: &Path) -> io::Result<PathBuf> {
    let bundled_wheels_dir = python_stdlib_dir.join("ensurepip/_bundled");

    for entry in fs::read_dir(&bundled_wheels_dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("pip-") {
            return Ok(entry.path().join("pip"));
        }
    }

    Err(io::Error::from(io::ErrorKind::NotFound))
}

fn generate_layer_metadata(
    stack_id: &StackId,
    python_version: &PythonVersion,
) -> PythonLayerMetadata {
    PythonLayerMetadata {
        stack: stack_id.clone(),
        python_version: python_version.to_string(),
        pip_version: PIP_VERSION.to_string(),
        setuptools_version: SETUPTOOLS_VERSION.to_string(),
        wheel_version: WHEEL_VERSION.to_string(),
    }
}

#[derive(Debug)]
pub(crate) enum PythonLayerError {
    CannotLocateBundledPip(io::Error),
    CannotMakeSitePackagesReadOnly(io::Error),
    DownloadUnpack(DownloadUnpackError),
    PipBootstrap(CommandError),
    PipCompile(CommandError),
}

impl From<PythonLayerError> for PythonBuildpackError {
    fn from(error: PythonLayerError) -> Self {
        Self::PythonLayer(error)
    }
}

// TODO: Unit and/or integration tests
