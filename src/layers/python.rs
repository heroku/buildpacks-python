use crate::packaging_tool_versions::PackagingToolVersions;
use crate::python_version::PythonVersion;
use crate::utils::{self, DownloadUnpackArchiveError, StreamedCommandError};
use crate::{BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{
    ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder, MetadataMigration,
};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::{Buildpack, Env, Target};
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::fs::Permissions;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

/// Layer containing the Python runtime, and the packages `pip`, `setuptools` and `wheel`.
///
/// We install both Python and the packaging tools into the same layer, since:
///  - We don't want to mix buildpack/packaging dependencies with the app's own dependencies
///    (for a start, we need pip installed to even install the user's own dependencies, plus
///    want to keep caching separate), so cannot install the packaging tools into the user
///    site-packages directory.
///  - We don't want to install the packaging tools into an arbitrary directory added to
///    `PYTHONPATH`, since directories added to `PYTHONPATH` take precedence over the Python
///    stdlib (unlike the system or user site-packages directories), and so can result in hard
///    to debug stdlib shadowing problems that users won't encounter locally.
///  - This leaves just the system site-packages directory, which exists within the Python
///    installation directory and Python does not support moving it elsewhere.
///  - It matches what both local and official Docker image environments do.
pub(crate) struct PythonLayer<'a> {
    /// Environment variables inherited from earlier buildpack steps.
    pub(crate) command_env: &'a Env,
    /// The Python version that this layer should install.
    pub(crate) python_version: &'a PythonVersion,
    /// The pip, setuptools and wheel versions that this layer should install.
    pub(crate) packaging_tool_versions: &'a PackagingToolVersions,
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
        &mut self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        log_info(format!("Installing Python {}", self.python_version));

        let archive_url = self.python_version.url(&context.target);
        utils::download_and_unpack_zstd_archive(&archive_url, layer_path).map_err(|error| {
            match error {
                // TODO: Remove this once the Python version is validated against a manifest (at which
                // point 404s can be treated as an internal error, instead of user error)
                DownloadUnpackArchiveError::Request(ureq::Error::Status(404, _)) => {
                    PythonLayerError::PythonArchiveNotFound {
                        python_version: self.python_version.clone(),
                    }
                }
                other_error => PythonLayerError::DownloadUnpackPythonArchive(other_error),
            }
        })?;

        let layer_env = generate_layer_env(layer_path, self.python_version);
        let mut command_env = layer_env.apply(Scope::Build, self.command_env);

        // The Python binaries are built using `--shared`, and since they're being installed at a
        // different location from their original `--prefix`, they need `LD_LIBRARY_PATH` to be set
        // in order to find `libpython3`. Whilst `LD_LIBRARY_PATH` will be automatically set later by
        // lifecycle/libcnb, it's not set by libcnb until this `Layer` has ended, and so we have to
        // explicitly set it for the Python invocations within this layer.
        command_env.insert("LD_LIBRARY_PATH", layer_path.join("lib"));

        let PackagingToolVersions {
            pip_version,
            setuptools_version,
            wheel_version,
        } = self.packaging_tool_versions;

        log_info(format!(
            "Installing pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}"
        ));

        let python_binary = layer_path.join("bin/python");
        let python_stdlib_dir = layer_path.join(format!(
            "lib/python{}.{}",
            self.python_version.major, self.python_version.minor
        ));
        let site_packages_dir = python_stdlib_dir.join("site-packages");

        // Python bundles Pip within its standard library, which we can use to install our chosen
        // pip version from PyPI, saving us from having to download the usual pip bootstrap script.
        let bundled_pip_module_path = bundled_pip_module_path(&python_stdlib_dir)
            .map_err(PythonLayerError::LocateBundledPip)?;

        utils::run_command_and_stream_output(
            Command::new(python_binary)
                .args([
                    &bundled_pip_module_path.to_string_lossy(),
                    "install",
                    // There is no point using Pip's cache here, since the layer itself will be cached.
                    "--no-cache-dir",
                    "--no-input",
                    "--quiet",
                    format!("pip=={pip_version}").as_str(),
                    format!("setuptools=={setuptools_version}").as_str(),
                    format!("wheel=={wheel_version}").as_str(),
                ])
                .current_dir(&context.app_dir)
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
            .map_err(PythonLayerError::MakeSitePackagesReadOnly)?;

        let layer_metadata = self.generate_layer_metadata(&context.target);
        LayerResultBuilder::new(layer_metadata)
            .env(layer_env)
            .build()
    }

    fn existing_layer_strategy(
        &mut self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
        let cached_metadata = &layer_data.content_metadata.metadata;
        let new_metadata = self.generate_layer_metadata(&context.target);
        let cache_invalidation_reasons = cache_invalidation_reasons(cached_metadata, &new_metadata);

        if cache_invalidation_reasons.is_empty() {
            log_info(format!(
                "Using cached Python {}",
                cached_metadata.python_version
            ));
            let PackagingToolVersions {
                pip_version,
                setuptools_version,
                wheel_version,
            } = &cached_metadata.packaging_tool_versions;
            log_info(format!(
                "Using cached pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}"
            ));
            Ok(ExistingLayerStrategy::Keep)
        } else {
            log_info(format!(
                "Discarding cache since:\n - {}",
                cache_invalidation_reasons.join("\n - ")
            ));
            Ok(ExistingLayerStrategy::Recreate)
        }
    }

    fn migrate_incompatible_metadata(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        _metadata: &GenericMetadata,
    ) -> Result<MetadataMigration<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        // For now we don't migrate old cache metadata formats, since we want to invalidate
        // the cache anyway (to switch to the new runtime archives).
        log_info("Discarding cache since the buildpack cache format has changed");
        Ok(MetadataMigration::RecreateLayer)
    }
}

impl<'a> PythonLayer<'a> {
    fn generate_layer_metadata(&self, target: &Target) -> PythonLayerMetadata {
        PythonLayerMetadata {
            arch: target.arch.clone(),
            distro_name: target.distro_name.clone(),
            distro_version: target.distro_version.clone(),
            python_version: self.python_version.to_string(),
            packaging_tool_versions: self.packaging_tool_versions.clone(),
        }
    }
}

/// Metadata stored in the generated layer that allows future builds to determine whether
/// the cached layer needs to be invalidated or not.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PythonLayerMetadata {
    arch: String,
    distro_name: String,
    distro_version: String,
    python_version: String,
    packaging_tool_versions: PackagingToolVersions,
}

/// Compare cached layer metadata to the new layer metadata to determine if the cache should be
/// invalidated, and if so, for what reason(s). If there is more than one reason then all are
/// returned, to prevent support tickets such as those where build failures are blamed on a stack
/// upgrade but were actually caused by the app's Python version being updated at the same time.
fn cache_invalidation_reasons(
    cached_metadata: &PythonLayerMetadata,
    new_metadata: &PythonLayerMetadata,
) -> Vec<String> {
    // By destructuring here we ensure that if any additional fields are added to the layer
    // metadata in the future, it forces them to be used as part of cache invalidation,
    // otherwise Clippy would report unused variable errors.
    let PythonLayerMetadata {
        arch: cached_arch,
        distro_name: cached_distro_name,
        distro_version: cached_distro_version,
        python_version: cached_python_version,
        packaging_tool_versions:
            PackagingToolVersions {
                pip_version: cached_pip_version,
                setuptools_version: cached_setuptools_version,
                wheel_version: cached_wheel_version,
            },
    } = cached_metadata;

    let PythonLayerMetadata {
        arch,
        distro_name,
        distro_version,
        python_version,
        packaging_tool_versions:
            PackagingToolVersions {
                pip_version,
                setuptools_version,
                wheel_version,
            },
    } = new_metadata;

    let mut reasons = Vec::new();

    if cached_arch != arch {
        reasons.push(format!(
            "The CPU architecture has changed from {cached_arch} to {arch}"
        ));
    }

    if (cached_distro_name, cached_distro_version) != (distro_name, distro_version) {
        reasons.push(format!(
            "The OS has changed from {cached_distro_name}-{cached_distro_version} to {distro_name}-{distro_version}"
        ));
    }

    if cached_python_version != python_version {
        reasons.push(format!(
            "The Python version has changed from {cached_python_version} to {python_version}"
        ));
    }

    if cached_pip_version != pip_version {
        reasons.push(format!(
            "The pip version has changed from {cached_pip_version} to {pip_version}"
        ));
    }

    if cached_setuptools_version != setuptools_version {
        reasons.push(format!(
            "The setuptools version has changed from {cached_setuptools_version} to {setuptools_version}"
        ));
    }

    if cached_wheel_version != wheel_version {
        reasons.push(format!(
            "The wheel version has changed from {cached_wheel_version} to {wheel_version}"
        ));
    }

    reasons
}

/// Environment variables that will be set by this layer.
fn generate_layer_env(layer_path: &Path, python_version: &PythonVersion) -> LayerEnv {
    // Several of the env vars below are technically build-time only vars, however, we use
    // `Scope::All` instead of `Scope::Build` to reduce confusion if pip install commands
    // are used at runtime when debugging.
    //
    // Remember to force invalidation of the cached layer if these env vars ever change.
    LayerEnv::new()
        // We have to set `CPATH` explicitly, since:
        //  - The automatic path set by lifecycle/libcnb is `<layer>/include/` whereas Python's
        //    headers are at `<layer>/include/pythonX.Y/` (compilers don't recursively search).
        //  - Older setuptools cannot find this directory without `CPATH` being set:
        //    https://github.com/pypa/setuptools/issues/3657
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
        // Our Python runtime is relocated (installed into a different location to which is was
        // originally compiled) which Python itself handles well, since it recalculates its actual
        // location at startup:
        // https://docs.python.org/3.11/library/sys_path_init.html
        // However, the uWSGI package uses the wrong `sysconfig` APIs so tries to reference the old
        // compile location, unless we override that by setting `PYTHONHOME`:
        // https://github.com/unbit/uwsgi/issues/2525
        // In addition, some legacy apps have `PYTHONHOME` set to an invalid value, so if we did not
        // set it explicitly here, Python would fail to run both during the build and at runtime.
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "PYTHONHOME",
            layer_path,
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
        //   - This means they need to be regenerated at app boot, slowing boot times.
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

/// Errors that can occur when installing Python and required packaging tools into a layer.
#[derive(Debug)]
pub(crate) enum PythonLayerError {
    BootstrapPipCommand(StreamedCommandError),
    DownloadUnpackPythonArchive(DownloadUnpackArchiveError),
    LocateBundledPip(io::Error),
    MakeSitePackagesReadOnly(io::Error),
    PythonArchiveNotFound { python_version: PythonVersion },
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
    fn cache_invalidation_reasons_unchanged() {
        let metadata = PythonLayerMetadata {
            arch: "amd64".to_string(),
            distro_name: "ubuntu".to_string(),
            distro_version: "22.04".to_string(),
            python_version: "3.11.0".to_string(),
            packaging_tool_versions: PackagingToolVersions {
                pip_version: "A.B.C".to_string(),
                setuptools_version: "D.E.F".to_string(),
                wheel_version: "G.H.I".to_string(),
            },
        };
        assert_eq!(
            cache_invalidation_reasons(&metadata, &metadata),
            Vec::<String>::new()
        );
    }

    #[test]
    fn cache_invalidation_reasons_single_change() {
        let cached_metadata = PythonLayerMetadata {
            arch: "amd64".to_string(),
            distro_name: "ubuntu".to_string(),
            distro_version: "22.04".to_string(),
            python_version: "3.11.0".to_string(),
            packaging_tool_versions: PackagingToolVersions {
                pip_version: "A.B.C".to_string(),
                setuptools_version: "D.E.F".to_string(),
                wheel_version: "G.H.I".to_string(),
            },
        };
        assert_eq!(
            cache_invalidation_reasons(
                &cached_metadata,
                &PythonLayerMetadata {
                    python_version: "3.11.1".to_string(),
                    ..cached_metadata.clone()
                }
            ),
            ["The Python version has changed from 3.11.0 to 3.11.1"]
        );
        assert_eq!(
            cache_invalidation_reasons(
                &cached_metadata,
                &PythonLayerMetadata {
                    distro_version: "24.04".to_string(),
                    ..cached_metadata.clone()
                }
            ),
            ["The OS has changed from ubuntu-22.04 to ubuntu-24.04"]
        );
    }

    #[test]
    fn cache_invalidation_reasons_all_changed() {
        let cached_metadata = PythonLayerMetadata {
            arch: "amd64".to_string(),
            distro_name: "ubuntu".to_string(),
            distro_version: "22.04".to_string(),
            python_version: "3.9.0".to_string(),
            packaging_tool_versions: PackagingToolVersions {
                pip_version: "A.B.C".to_string(),
                setuptools_version: "D.E.F".to_string(),
                wheel_version: "G.H.I".to_string(),
            },
        };
        let new_metadata = PythonLayerMetadata {
            arch: "arm64".to_string(),
            distro_name: "debian".to_string(),
            distro_version: "12".to_string(),
            python_version: "3.11.1".to_string(),
            packaging_tool_versions: PackagingToolVersions {
                pip_version: "A.B.C-new".to_string(),
                setuptools_version: "D.E.F-new".to_string(),
                wheel_version: "G.H.I-new".to_string(),
            },
        };
        assert_eq!(
            cache_invalidation_reasons(&cached_metadata, &new_metadata),
            [
                "The CPU architecture has changed from amd64 to arm64",
                "The OS has changed from ubuntu-22.04 to debian-12",
                "The Python version has changed from 3.9.0 to 3.11.1",
                "The pip version has changed from A.B.C to A.B.C-new",
                "The setuptools version has changed from D.E.F to D.E.F-new",
                "The wheel version has changed from G.H.I to G.H.I-new"
            ]
        );
    }

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
            [
                ("CPATH", "/layers/python/include/python3.9"),
                ("LANG", "C.UTF-8"),
                ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
                ("PKG_CONFIG_PATH", "/layers/python/lib/pkgconfig"),
                ("PYTHONHOME", "/layers/python"),
                ("PYTHONUNBUFFERED", "1"),
                ("SOURCE_DATE_EPOCH", "315532801"),
            ]
        );
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply_to_empty(Scope::Launch)),
            [
                ("CPATH", "/layers/python/include/python3.9"),
                ("LANG", "C.UTF-8"),
                ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
                ("PKG_CONFIG_PATH", "/layers/python/lib/pkgconfig"),
                ("PYTHONHOME", "/layers/python"),
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
        base_env.insert("PYTHONHOME", "this-should-be-overridden");
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
            [
                ("CPATH", "/layers/python/include/python3.11:/base"),
                ("LANG", "C.UTF-8"),
                ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
                ("PKG_CONFIG_PATH", "/layers/python/lib/pkgconfig:/base"),
                ("PYTHONHOME", "/layers/python"),
                ("PYTHONUNBUFFERED", "1"),
                ("SOURCE_DATE_EPOCH", "this-should-be-preserved"),
            ]
        );
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Launch, &base_env)),
            [
                ("CPATH", "/layers/python/include/python3.11:/base"),
                ("LANG", "C.UTF-8"),
                ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
                ("PKG_CONFIG_PATH", "/layers/python/lib/pkgconfig:/base"),
                ("PYTHONHOME", "/layers/python"),
                ("PYTHONUNBUFFERED", "1"),
                ("SOURCE_DATE_EPOCH", "this-should-be-preserved"),
            ]
        );
    }
}
