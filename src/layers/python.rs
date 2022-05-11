use crate::python_version::PythonVersion;
use crate::utils::{self, DownloadUnpackError};
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
use std::process::{Command, ExitStatus};
use std::{fs, io};

const PIP_VERSION: &str = "22.0.4";
const SETUPTOOLS_VERSION: &str = "62.2.0";
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
            launch: true,
            cache: true,
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
            "https://heroku-buildpack-python.s3.amazonaws.com/{}/runtimes/python-{}.tar.gz",
            context.stack_id, self.python_version
        );

        log_info(format!("Downloading Python {}", self.python_version));

        utils::download_and_unpack_gzip(&archive_url, layer_path)
            .map_err(PythonLayerError::DownloadUnpack)?;

        log_info("Python installation successful");

        log_header("Installing Pip");

        // TODO: Explain why we're using the bundled pip and not ensurepip, mention pip creates pyc on install
        // TODO: Test whether pip's built-in retry handling is sufficient.
        // TODO: Refactor command out to utils once decision made on whether retries are necessary.
        // TODO: Decide whether to move requirement specifiers to requirements file.
        log_info(format!("Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}"));
        Command::new(layer_path.join("bin/python"))
            .args([
                &self
                    .bundled_pip_wheel_path(layer_path)
                    .unwrap()
                    .to_string_lossy(),
                "install",
                "--disable-pip-version-check",
                "--no-cache-dir",
                "--no-compile",
                "--no-input",
                "--quiet",
                format!("pip=={PIP_VERSION}").as_str(),
                format!("setuptools=={SETUPTOOLS_VERSION}").as_str(),
                format!("wheel=={WHEEL_VERSION}").as_str(),
            ])
            .env_clear()
            .status()
            .map_err(PythonLayerError::PipBootstrapIOError)
            .and_then(|exit_status| {
                if exit_status.success() {
                    Ok(())
                } else {
                    Err(PythonLayerError::PipBootstrapNonzeroExitCode(exit_status))
                }
            })?;
        log_info("Installation completed");

        log_info("Compiling pycs for pip");
        // TODO: Add comment explaining why we're doing this vs pip default compile.
        Command::new(layer_path.join("bin/python"))
            .args([
                "-m",
                "compileall",
                "-f",
                "-q",
                "--invalidation-mode",
                "unchecked-hash",
                "--workers",
                "0",
                &self.site_packages_path(layer_path).to_string_lossy(),
            ])
            .env_clear()
            .status()
            .map_err(PythonLayerError::PipCompileIOError)
            .and_then(|exit_status| {
                if exit_status.success() {
                    Ok(())
                } else {
                    Err(PythonLayerError::PipCompileNonzeroExitCode(exit_status))
                }
            })?;
        log_info("Completed compiling pycs");

        // By default Pip will install into the system site-packages directory if it is writeable
        // by the current user. Whilst the buildpack's own `pip install` invocations always use
        // `--user` to ensure application dependencies are instead installed into the user
        // site-packages, it's possible other buildpacks or custom scripts may forget to do so.
        // By making the system site-packages directory read-only, Pip will automatically use
        // user installs in such cases:
        // https://github.com/pypa/pip/blob/22.0.4/src/pip/_internal/commands/install.py#L617-L675
        log_info("Marking system site-packages directory as read-only");
        fs::set_permissions(
            &self.site_packages_path(layer_path),
            Permissions::from_mode(0o555),
        )
        .map_err(PythonLayerError::CannotMakeSitePackagesReadOnly)?;

        log_info("System site-packages directory marked as read-only");

        // TODO: Decide whether to pass these to other commands etc, or just leave in the layer.
        // TODO: For PIP_DISABLE_PIP_VERSION_CHECK should we:
        // - only pass `--disable-pip-version-check` to our pip commands
        // - pass `--disable-pip-version-check` and set PIP_DISABLE_PIP_VERSION_CHECK in the layer
        // - only set PIP_DISABLE_PIP_VERSION_CHECK
        // TODO: Set LANG per https://github.com/docker-library/python/pull/570
        let layer_env = LayerEnv::new()
            .chainable_insert(
                Scope::All,
                // TODO: Should this be Default or Override?
                ModificationBehavior::Default,
                "PYTHONUNBUFFERED",
                "1",
            )
            .chainable_insert(
                Scope::All,
                // TODO: Should this be Default or Override?
                ModificationBehavior::Default,
                "PIP_DISABLE_PIP_VERSION_CHECK",
                "1",
            );

        LayerResultBuilder::new(self.generate_layer_metadata(context))
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
        let new_metadata = self.generate_layer_metadata(context);
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

impl PythonLayer<'_> {
    fn python_stdlib_path(&self, layer_path: &Path) -> PathBuf {
        layer_path.join(format!(
            "lib/python{}.{}",
            self.python_version.major, self.python_version.minor
        ))
    }

    // TODO: Should this return a string given it's not really a path? Or the `/pip` append be elsewhere?
    fn bundled_pip_wheel_path(&self, layer_path: &Path) -> io::Result<PathBuf> {
        let bundled_wheels_dir = self
            .python_stdlib_path(layer_path)
            .join("ensurepip/_bundled");

        for entry in fs::read_dir(&bundled_wheels_dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("pip-") {
                return Ok(entry.path().join("pip"));
            }
        }

        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    fn site_packages_path(&self, layer_path: &Path) -> PathBuf {
        self.python_stdlib_path(layer_path).join("site-packages")
    }

    fn generate_layer_metadata(
        &self,
        context: &BuildContext<PythonBuildpack>,
    ) -> PythonLayerMetadata {
        PythonLayerMetadata {
            stack: context.stack_id.clone(),
            python_version: self.python_version.to_string(),
            pip_version: PIP_VERSION.to_string(),
            setuptools_version: SETUPTOOLS_VERSION.to_string(),
            wheel_version: WHEEL_VERSION.to_string(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum PythonLayerError {
    CannotMakeSitePackagesReadOnly(io::Error),
    DownloadUnpack(DownloadUnpackError),
    PipBootstrapIOError(io::Error),
    PipBootstrapNonzeroExitCode(ExitStatus),
    PipCompileIOError(io::Error),
    PipCompileNonzeroExitCode(ExitStatus),
}

impl From<PythonLayerError> for PythonBuildpackError {
    fn from(error: PythonLayerError) -> Self {
        Self::PythonLayer(error)
    }
}
