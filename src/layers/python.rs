use crate::python_version::PythonVersion;
use crate::utils::{self, CommandError, DownloadUnpackArchiveError};
use crate::{BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::{Buildpack, Env};
use libherokubuildpack::log::{log_header, log_info};
use serde::{Deserialize, Serialize};
use std::fs::Permissions;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

const PIP_VERSION: &str = "23.0";
const SETUPTOOLS_VERSION: &str = "67.0.0";
const WHEEL_VERSION: &str = "0.38.4";

pub(crate) struct PythonLayer<'a> {
    pub env: &'a Env,
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

    #[allow(clippy::too_many_lines)]
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
        utils::download_and_unpack_gzipped_archive(&archive_url, layer_path).map_err(|error| {
            match error {
                // TODO: Remove this once the Python version is validated against a manifest (at which
                // point 404s can be treated as an internal error, instead of user error)
                DownloadUnpackArchiveError::Request(ureq::Error::Status(404, _)) => {
                    PythonLayerError::PythonVersionNotFound {
                        stack: context.stack_id.clone(),
                        python_version: self.python_version.clone(),
                    }
                }
                other_error => PythonLayerError::DownloadUnpackArchive(other_error),
            }
        })?;
        log_info("Python installation successful");

        // Remember to force invalidation of the cached layer if this list ever changes.
        let layer_env = LayerEnv::new()
            // We have to set `CPATH` explicitly, since the automatic path set by lifecycle/libcnb is
            // `<layer>/include/` whereas Python's header files are at `<layer>/include/pythonX.Y/`
            // (and compilers don't recursively search).
            .chainable_insert(
                Scope::All,
                ModificationBehavior::Prepend,
                "CPATH",
                layer_path.join(format!(
                    "include/python{}.{}",
                    self.python_version.major, self.python_version.minor
                )),
            )
            .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "CPATH", ":")
            // Ensure Python uses a Unicode locate, to prevent the issues described in:
            // https://github.com/docker-library/python/pull/570
            .chainable_insert(
                Scope::All,
                ModificationBehavior::Override,
                "LANG",
                "C.UTF-8",
            )
            // We have to set `PKG_CONFIG_PATH` explicitly, since the automatic path set by lifecycle/libcnb
            // is `<layer>/pkgconfig/`, whereas Python's pkgconfig files are at `<layer>/lib/pkgconfig/`.
            .chainable_insert(
                Scope::All,
                ModificationBehavior::Prepend,
                "PKG_CONFIG_PATH",
                layer_path.join("lib/pkgconfig"),
            )
            .chainable_insert(
                Scope::All,
                ModificationBehavior::Delimiter,
                "PKG_CONFIG_PATH",
                ":",
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
        let mut env = layer_env.apply(Scope::Build, self.env);

        // The Python binaries are built using `--shared`, and since they're being installed at a
        // different location from their original `--prefix`, they need `LD_LIBRARY_PATH` to be set
        // in order to find `libpython3`. Whilst `LD_LIBRARY_PATH` will be automatically set later by
        // lifecycle/libcnb, it's not set by libcnb until this `Layer` has ended, and so we have to
        // explicitly set it for the Python invocations within this layer.
        env.insert("LD_LIBRARY_PATH", layer_path.join("lib"));

        log_header("Installing Pip");
        log_info(format!("Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}"));

        let python_binary = layer_path.join("bin/python");
        let python_stdlib_dir = layer_path.join(format!(
            "lib/python{}.{}",
            self.python_version.major, self.python_version.minor
        ));
        let site_packages_dir = python_stdlib_dir.join("site-packages");

        // TODO: Explain what's happening here
        let bundled_pip_module =
            bundled_pip_module(&python_stdlib_dir).map_err(PythonLayerError::LocateBundledPipIo)?;
        utils::run_command(
            Command::new(python_binary)
                .args([
                    &bundled_pip_module.to_string_lossy(),
                    "install",
                    "--no-cache-dir",
                    "--no-input",
                    "--quiet",
                    format!("pip=={PIP_VERSION}").as_str(),
                    format!("setuptools=={SETUPTOOLS_VERSION}").as_str(),
                    format!("wheel=={WHEEL_VERSION}").as_str(),
                ])
                .envs(&env)
                // TODO: Explain why we're setting this
                // Using 1980-01-01T00:00:01Z to avoid:
                // ValueError: ZIP does not support timestamps before 1980
                .env("SOURCE_DATE_EPOCH", "315532800"),
        )
        .map_err(PythonLayerError::BootstrapPipCommand)?;

        // By default Pip will install into the system site-packages directory if it is writeable
        // by the current user. Whilst the buildpack's own `pip install` invocations always use
        // `--user` to ensure application dependencies are instead installed into the user
        // site-packages, it's possible other buildpacks or custom scripts may forget to do so.
        // By making the system site-packages directory read-only, Pip will automatically use
        // user installs in such cases:
        // https://github.com/pypa/pip/blob/22.3.1/src/pip/_internal/commands/install.py#L706-L764
        fs::set_permissions(site_packages_dir, Permissions::from_mode(0o555))
            .map_err(PythonLayerError::MakeSitePackagesReadOnlyIo)?;

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
        let old_metadata = &layer_data.content_metadata.metadata;
        let new_metadata = generate_layer_metadata(&context.stack_id, self.python_version);
        if new_metadata == *old_metadata {
            log_header("Installing Python");
            log_info(format!(
                "Re-using cached Python {}",
                old_metadata.python_version
            ));

            log_header("Installing Pip");
            log_info(format!(
                "Re-using cached pip {}, setuptools {} and wheel {}",
                new_metadata.pip_version,
                new_metadata.setuptools_version,
                new_metadata.wheel_version
            ));

            Ok(ExistingLayerStrategy::Keep)
        } else {
            log_info(format!(
                "Discarding cached Python {}",
                old_metadata.python_version
            ));
            log_info(format!(
                "Discarding cached pip {}, setuptools {} and wheel {}",
                old_metadata.pip_version,
                old_metadata.setuptools_version,
                old_metadata.wheel_version
            ));
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

// TODO: Explain what's happening here
// The bundled version of Pip (and thus the wheel filename) varies across Python versions,
// so we have to search the bundled wheels directory for the appropriate file.
// TODO: This returns a module path rather than a wheel path - change?
fn bundled_pip_module(python_stdlib_dir: &Path) -> io::Result<PathBuf> {
    let bundled_wheels_dir = python_stdlib_dir.join("ensurepip/_bundled");
    let pip_wheel_filename_prefix = "pip-";

    for entry in fs::read_dir(bundled_wheels_dir)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(pip_wheel_filename_prefix)
        {
            return Ok(entry.path().join("pip"));
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("No files found matching the filename prefix of '{pip_wheel_filename_prefix}'"),
    ))
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
    BootstrapPipCommand(CommandError),
    DownloadUnpackArchive(DownloadUnpackArchiveError),
    LocateBundledPipIo(io::Error),
    MakeSitePackagesReadOnlyIo(io::Error),
    PythonVersionNotFound {
        python_version: PythonVersion,
        stack: StackId,
    },
}

impl From<PythonLayerError> for BuildpackError {
    fn from(error: PythonLayerError) -> Self {
        Self::PythonLayer(error)
    }
}

// TODO: Unit tests for cache invalidation handling?
