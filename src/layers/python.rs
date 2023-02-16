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
const SETUPTOOLS_VERSION: &str = "67.1.0";
const WHEEL_VERSION: &str = "0.38.4";

/// Layer containing the Python runtime, and the packages `pip`, `setuptools` and `wheel`.
pub(crate) struct PythonLayer<'a> {
    /// Environment variables inherited from earlier buildpack steps.
    pub command_env: &'a Env,
    /// The Python version that will be installed.
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
        utils::download_and_unpack_gzipped_archive(&archive_url, layer_path).map_err(|error| {
            match error {
                // TODO: Remove this once the Python version is validated against a manifest (at which
                // point 404s can be treated as an internal error, instead of user error)
                DownloadUnpackArchiveError::Request(ureq::Error::Status(404, _)) => {
                    PythonLayerError::PythonArchiveNotFound {
                        stack: context.stack_id.clone(),
                        python_version: self.python_version.clone(),
                    }
                }
                other_error => PythonLayerError::DownloadUnpackPythonArchive(other_error),
            }
        })?;
        log_info("Python installation successful");

        let layer_env = generate_layer_env(layer_path, self.python_version);
        let mut command_env = layer_env.apply(Scope::Build, self.command_env);

        // The Python binaries are built using `--shared`, and since they're being installed at a
        // different location from their original `--prefix`, they need `LD_LIBRARY_PATH` to be set
        // in order to find `libpython3`. Whilst `LD_LIBRARY_PATH` will be automatically set later by
        // lifecycle/libcnb, it's not set by libcnb until this `Layer` has ended, and so we have to
        // explicitly set it for the Python invocations within this layer.
        command_env.insert("LD_LIBRARY_PATH", layer_path.join("lib"));

        log_header("Installing Pip");
        log_info(format!("Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}"));

        let python_binary = layer_path.join("bin/python");
        let python_stdlib_dir = layer_path.join(format!(
            "lib/python{}.{}",
            self.python_version.major, self.python_version.minor
        ));
        let site_packages_dir = python_stdlib_dir.join("site-packages");

        // Python bundles Pip within its standard library, which we can use to install our chosen
        // pip version from PyPI, saving us from having to download the usual pip bootstrap script.
        let bundled_pip_module_path = bundled_pip_module_path(&python_stdlib_dir)
            .map_err(PythonLayerError::LocateBundledPipIo)?;

        utils::run_command(
            Command::new(python_binary)
                .args([
                    &bundled_pip_module_path.to_string_lossy(),
                    "install",
                    // There is no point using Pip's cache here, since the layer itself will be cached.
                    "--no-cache-dir",
                    "--no-input",
                    "--quiet",
                    format!("pip=={PIP_VERSION}").as_str(),
                    format!("setuptools=={SETUPTOOLS_VERSION}").as_str(),
                    format!("wheel=={WHEEL_VERSION}").as_str(),
                ])
                .env_clear()
                .envs(&command_env),
        )
        .map_err(PythonLayerError::BootstrapPipCommand)?;

        // By default Pip will install into the system site-packages directory if it is writeable
        // by the current user. Whilst the buildpack's own `pip install` invocations always use
        // `--user` to ensure application dependencies are instead installed into the user
        // site-packages, it's possible other buildpacks or custom scripts may forget to do so.
        // By making the system site-packages directory read-only, Pip will automatically use
        // user installs in such cases:
        // https://github.com/pypa/pip/blob/23.0/src/pip/_internal/commands/install.py#L715-L773
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

/// Environment variables that will be set by this layer.
fn generate_layer_env(layer_path: &Path, python_version: &PythonVersion) -> LayerEnv {
    // Several of the env vars below are technically build-time only vars, however, we use
    // `Scope::All` instead of `Scope::Build` to reduce confusion if pip install commands
    // are used at runtime when debugging.
    //
    // Remember to force invalidation of the cached layer if these env vars ever change.
    LayerEnv::new()
        // We have to set `CPATH` explicitly, since the automatic path set by lifecycle/libcnb is
        // `<layer>/include/` whereas Python's header files are at `<layer>/include/pythonX.Y/`
        // (and compilers don't recursively search).
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Prepend,
            "CPATH",
            layer_path.join(format!(
                "include/python{}.{}",
                python_version.major, python_version.minor
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
        // We use a curated Pip version, so disable the update check to speed up Pip invocations,
        // reduce build log spam and prevent users from thinking they need to manually upgrade.
        // This uses an env var (rather than the `--disable-pip-version-check` arg) so that it also
        // takes effect for any pip invocations in later buildpacks or when debugging at runtime.
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "PIP_DISABLE_PIP_VERSION_CHECK",
            "1",
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
        // Disable Python's output buffering to ensure logs aren't dropped if an app crashes.
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "PYTHONUNBUFFERED",
            "1",
        )
        // By default, when Python creates cached bytecode files (`.pyc` files) it embeds the
        // `.py` source file's last-modified time in the `.pyc` file, so it can later be used
        // to determine whether the cached bytecode file needs regenerating.
        //
        // This causes the `.pyc` file contents (and thus layer SHA256) to be non-deterministic in
        // cases where the `.py` file's last-modified time can vary (such as files installed by Pip,
        // since it doesn't preserve the last modified time of the original downloaded package).
        //
        // In addition, as part of generating the OCI image, lifecycle resets the timestamps on all
        // files to a fixed value in order to improve the determinism of builds:
        // https://buildpacks.io/docs/features/reproducibility/#consequences-and-caveats
        //
        // At runtime, this then means the timestamps embedded in the `.pyc` files no longer match
        // the timestamps of the original `.py` files, causing Python to have to regenerate the
        // bytecode, and so losing any benefit of having kept the `.pyc` files in the image.
        //
        // One option to solve all of the above, would be to delete the `.pyc` files from the image
        // at the end of the buildpack's build phase, however:
        //   - This means they need to be regenerated at app start boot, slowing boot times.
        //     (For a simple Django project on a Perf-M, boot time increases from ~0.5s to ~1.5s.)
        //   - If any other later buildpack runs any of the Python files added by this buildpack, then
        //     the timestamp based `.pyc` files will be created again, re-introducing non-determinism.
        //
        // Instead, we use the hash-based cache files mode added in Python 3.7+, which embeds a hash
        // of the original `.py` file in the `.pyc` file instead of the timestamp:
        // https://docs.python.org/3.11/reference/import.html#pyc-invalidation
        // https://peps.python.org/pep-0552/
        //
        // This mode can be enabled by passing `--invalidation-mode checked-hash` to `compileall`,
        // or via the `SOURCE_DATE_EPOCH` env var:
        // https://docs.python.org/3.11/library/compileall.html#cmdoption-compileall-invalidation-mode
        //
        // Note: Both the CLI args and the env var only apply to usages of `compileall` or `py_compile`,
        // and not `.pyc` generation as part of Python importing a file during normal operation.
        //
        // We use the env var, since:
        //   - Pip calls `compileall` itself after installing packages, and doesn't allow us to
        //     customise the options passed to it, which would mean we'd have to pass `--no-compile`
        //     to Pip followed by running `compileall` manually ourselves, meaning more complexity
        //     every time we (or a later buildpack) use `pip install`.
        //   - When we add support for Poetry, we'll have to use an env var regardless, since Poetry
        //     doesn't allow customising the options passed to its internal Pip invocations, so we'd
        //     have no way of passing `--no-compile` to Pip.
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Default,
            "SOURCE_DATE_EPOCH",
            // Whilst `compileall` doesn't use the value of `SOURCE_DATE_EPOCH` (only whether it is
            // set or not), the value ends up being used when wheel archives are generated during
            // the pip install. As such, we cannot use a zero value since the ZIP file format doesn't
            // support dates before 1980. Instead, we use a value equivalent to `1980-01-01T00:00:01Z`,
            // for parity with that used by lifecycle:
            // https://github.com/buildpacks/lifecycle/blob/v0.15.3/archive/writer.go#L12
            "315532801",
        )
}

/// The path to the Pip module bundled in Python's standard library.
fn bundled_pip_module_path(python_stdlib_dir: &Path) -> io::Result<PathBuf> {
    let bundled_wheels_dir = python_stdlib_dir.join("ensurepip/_bundled");

    // The wheel filename includes the Pip version (for example `pip-XX.Y-py3-none-any.whl`),
    // which varies from one Python release to the next (including between patch releases).
    // As such, we have to find the wheel based on the known filename prefix of `pip-`.
    for entry in fs::read_dir(bundled_wheels_dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("pip-") {
            let pip_wheel_path = entry.path();
            // The Pip module exists inside the pip wheel (which is a zip file), however,
            // Python can load it directly by appending the module name to the zip filename,
            // as though it were a path. For example: `pip-XX.Y-py3-none-any.whl/pip`
            let pip_module_path = pip_wheel_path.join("pip");
            return Ok(pip_module_path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "No files found matching the pip wheel filename prefix",
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

/// Errors that can occur when installing Python and required packaging tools into a layer.
#[derive(Debug)]
pub(crate) enum PythonLayerError {
    BootstrapPipCommand(CommandError),
    DownloadUnpackPythonArchive(DownloadUnpackArchiveError),
    LocateBundledPipIo(io::Error),
    MakeSitePackagesReadOnlyIo(io::Error),
    PythonArchiveNotFound {
        python_version: PythonVersion,
        stack: StackId,
    },
}

impl From<PythonLayerError> for BuildpackError {
    fn from(error: PythonLayerError) -> Self {
        Self::PythonLayer(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_layer_env() {
        let layer_env = generate_layer_env(
            Path::new("/layers/python"),
            &PythonVersion {
                major: 3,
                minor: 9,
                patch: 0,
            },
        );

        // Remember to force invalidation of the cached layer if these env vars ever change.
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply_to_empty(Scope::Build)),
            vec![
                ("CPATH", "/layers/python/include/python3.9"),
                ("LANG", "C.UTF-8"),
                ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
                ("PKG_CONFIG_PATH", "/layers/python/lib/pkgconfig"),
                ("PYTHONUNBUFFERED", "1"),
                ("SOURCE_DATE_EPOCH", "315532801"),
            ]
        );
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply_to_empty(Scope::Launch)),
            vec![
                ("CPATH", "/layers/python/include/python3.9"),
                ("LANG", "C.UTF-8"),
                ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
                ("PKG_CONFIG_PATH", "/layers/python/lib/pkgconfig"),
                ("PYTHONUNBUFFERED", "1"),
            ]
        );
    }

    #[test]
    fn python_layer_env_with_existing_env() {
        let mut base_env = Env::new();
        base_env.insert("CPATH", "/base");
        base_env.insert("LANG", "this-should-be-overridden");
        base_env.insert("PIP_DISABLE_PIP_VERSION_CHECK", "this-should-be-overridden");
        base_env.insert("PKG_CONFIG_PATH", "/base");
        base_env.insert("PYTHONUNBUFFERED", "this-should-be-overridden");
        base_env.insert("SOURCE_DATE_EPOCH", "this-should-be-preserved");

        let layer_env = generate_layer_env(
            Path::new("/layers/python"),
            &PythonVersion {
                major: 3,
                minor: 11,
                patch: 1,
            },
        );

        // Remember to force invalidation of the cached layer if these env vars ever change.
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Build, &base_env)),
            vec![
                ("CPATH", "/layers/python/include/python3.11:/base"),
                ("LANG", "C.UTF-8"),
                ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
                ("PKG_CONFIG_PATH", "/layers/python/lib/pkgconfig:/base"),
                ("PYTHONUNBUFFERED", "1"),
                ("SOURCE_DATE_EPOCH", "this-should-be-preserved"),
            ]
        );
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Launch, &base_env)),
            vec![
                ("CPATH", "/layers/python/include/python3.11:/base"),
                ("LANG", "C.UTF-8"),
                ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
                ("PKG_CONFIG_PATH", "/layers/python/lib/pkgconfig:/base"),
                ("PYTHONUNBUFFERED", "1"),
                ("SOURCE_DATE_EPOCH", "this-should-be-preserved"),
            ]
        );
    }
}
