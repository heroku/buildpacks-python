use crate::python_version::PythonVersion;
use crate::utils::{self, DownloadUnpackArchiveError};
use crate::{BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction,
};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use libherokubuildpack::log::log_info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Creates a layer containing the Python runtime.
pub(crate) fn install_python(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
    python_version: &PythonVersion,
) -> Result<PathBuf, libcnb::Error<BuildpackError>> {
    let new_metadata = PythonLayerMetadata {
        arch: context.target.arch.clone(),
        distro_name: context.target.distro_name.clone(),
        distro_version: context.target.distro_version.clone(),
        python_version: python_version.to_string(),
    };

    let layer = context.cached_layer(
        layer_name!("python"),
        CachedLayerDefinition {
            build: true,
            launch: true,
            invalid_metadata_action: &|_| InvalidMetadataAction::DeleteLayer,
            restored_layer_action: &|cached_metadata: &PythonLayerMetadata, _| {
                let cached_python_version = cached_metadata.python_version.clone();
                let reasons = cache_invalidation_reasons(cached_metadata, &new_metadata);
                if reasons.is_empty() {
                    Ok((
                        RestoredLayerAction::KeepLayer,
                        (cached_python_version, Vec::new()),
                    ))
                } else {
                    Ok((
                        RestoredLayerAction::DeleteLayer,
                        (cached_python_version, reasons),
                    ))
                }
            },
        },
    )?;
    let layer_path = layer.path();

    match layer.state {
        LayerState::Restored {
            cause: (ref cached_python_version, _),
        } => {
            log_info(format!("Using cached Python {cached_python_version}"));
        }
        LayerState::Empty { ref cause } => {
            match cause {
                EmptyLayerCause::InvalidMetadataAction { .. } => {
                    log_info("Discarding cached Python since its layer metadata can't be parsed");
                }
                EmptyLayerCause::RestoredLayerAction {
                    cause: (ref cached_python_version, reasons),
                } => {
                    // TODO: Move this type of detailed change messaging to a build config summary
                    // at the start of the build. This message could then be simplified to:
                    // "Discarding cached Python X.Y.Z (ubuntu-24.04, arm64)"
                    // ...and the "Installing" message changed similarly.
                    log_info(format!(
                        "Discarding cached Python {cached_python_version} since:\n - {}",
                        reasons.join("\n - ")
                    ));
                }
                EmptyLayerCause::NewlyCreated => {}
            }
            log_info(format!("Installing Python {python_version}"));
            let archive_url = python_version.url(&context.target);
            utils::download_and_unpack_zstd_archive(&archive_url, &layer_path).map_err(
                |error| match error {
                    // TODO: Remove this once the Python version is validated against a manifest (at
                    // which point 404s can be treated as an internal error, instead of user error)
                    DownloadUnpackArchiveError::Request(ureq::Error::Status(404, _)) => {
                        PythonLayerError::PythonArchiveNotFound {
                            python_version: python_version.clone(),
                        }
                    }
                    other_error => PythonLayerError::DownloadUnpackPythonArchive(other_error),
                },
            )?;
            layer.write_metadata(new_metadata)?;
        }
    }

    let mut layer_env = generate_layer_env(&layer_path, python_version);
    layer.write_env(layer_env)?;
    // Required to pick up the automatic env vars such as PATH. See: https://github.com/heroku/libcnb.rs/issues/842
    layer_env = layer.read_env()?;
    env.clone_from(&layer_env.apply(Scope::Build, env));

    Ok(layer_path)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PythonLayerMetadata {
    arch: String,
    distro_name: String,
    distro_version: String,
    python_version: String,
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
    } = cached_metadata;

    let PythonLayerMetadata {
        arch,
        distro_name,
        distro_version,
        python_version,
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

    reasons
}

fn generate_layer_env(layer_path: &Path, python_version: &PythonVersion) -> LayerEnv {
    LayerEnv::new()
        // We have to set `CPATH` explicitly, since:
        // - The automatic path set by lifecycle/libcnb is `<layer>/include/` whereas Python's
        //   headers are at `<layer>/include/pythonX.Y/` (compilers don't recursively search).
        // - Older setuptools cannot find this directory without `CPATH` being set:
        //   https://github.com/pypa/setuptools/issues/3657
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Prepend,
            "CPATH",
            layer_path.join(format!(
                "include/python{}.{}",
                python_version.major, python_version.minor
            )),
        )
        .chainable_insert(Scope::Build, ModificationBehavior::Delimiter, "CPATH", ":")
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
            Scope::Build,
            ModificationBehavior::Prepend,
            "PKG_CONFIG_PATH",
            layer_path.join("lib/pkgconfig"),
        )
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Delimiter,
            "PKG_CONFIG_PATH",
            ":",
        )
        // We relocate Python (install into a different location to which it was compiled), which
        // Python handles fine since it recalculates its actual location at startup. However, the
        // uWSGI package uses the wrong `sysconfig` APIs so tries to reference the old compile
        // location, unless we override that by setting `PYTHONHOME`. See:
        // https://docs.python.org/3/library/sys_path_init.html
        // https://github.com/unbit/uwsgi/issues/2525
        // In addition, some legacy apps have `PYTHONHOME` set to an invalid value, so if we did not
        // set it explicitly here, they would fail to run both during the build and at run-time.
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
        // By default, Python's cached bytecode files (`.pyc` files) embed the last-modified time of
        // their `.py` source file, so Python can determine when they need regenerating. This causes
        // them (and the layer digest) to be non-deterministic in cases where the source file's
        // last-modified time can vary (such as for installed packages). In addition, when lifecycle
        // exports layers it resets the timestamps on all files to a fixed value:
        // https://buildpacks.io/docs/for-app-developers/concepts/reproducibility/#consequences-and-caveats
        //
        // At run-time, this means the `.pyc`'s embedded timestamps no longer match the timestamps
        // of the original `.py` files, causing Python to regenerate the bytecode, and so losing any
        // benefit of having kept the `.pyc` files (at the cost of a larger app image).
        //
        // We could delete the `.pyc` files at the end of this buildpack's build phase, or suppress
        // their creation using `PYTHONDONTWRITEBYTECODE=1`, but this would mean slower app boot.
        //
        // Instead, we use the hash-based cache files mode added in Python 3.7+, which embeds a hash
        // of the original `.py` file in the `.pyc` file instead of the timestamp:
        // https://docs.python.org/3/reference/import.html#pyc-invalidation
        // https://peps.python.org/pep-0552/
        //
        // This mode can be enabled by passing `--invalidation-mode checked-hash` to `compileall`,
        // or via the `SOURCE_DATE_EPOCH` env var:
        // https://docs.python.org/3/library/compileall.html#cmdoption-compileall-invalidation-mode
        //
        // Note: Both the CLI args and the env var only apply to usages of `compileall` or `py_compile`,
        // and not `.pyc` generation as part of Python importing a file during normal operation.
        .chainable_insert(
            Scope::Build,
            ModificationBehavior::Default,
            "SOURCE_DATE_EPOCH",
            // Whilst `compileall` doesn't use the value of `SOURCE_DATE_EPOCH` (only whether it is
            // set or not), the value ends up being used when wheel archives are generated during
            // the pip install. As such, we cannot use a zero value since the ZIP file format doesn't
            // support dates before 1980. Instead, we use a value equivalent to `1980-01-01T00:00:01Z`,
            // for parity with that used by lifecycle:
            // https://github.com/buildpacks/lifecycle/blob/v0.20.1/archive/writer.go#L12
            "315532801",
        )
}

/// Errors that can occur when installing Python into a layer.
#[derive(Debug)]
pub(crate) enum PythonLayerError {
    DownloadUnpackPythonArchive(DownloadUnpackArchiveError),
    PythonArchiveNotFound { python_version: PythonVersion },
}

impl From<PythonLayerError> for libcnb::Error<BuildpackError> {
    fn from(error: PythonLayerError) -> Self {
        Self::BuildpackError(BuildpackError::PythonLayer(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_layer_metadata() -> PythonLayerMetadata {
        PythonLayerMetadata {
            arch: "amd64".to_string(),
            distro_name: "ubuntu".to_string(),
            distro_version: "22.04".to_string(),
            python_version: "3.11.0".to_string(),
        }
    }

    #[test]
    fn cache_invalidation_reasons_unchanged() {
        let cached_metadata = example_layer_metadata();
        let new_metadata = cached_metadata.clone();
        assert_eq!(
            cache_invalidation_reasons(&cached_metadata, &new_metadata),
            Vec::<String>::new()
        );
    }

    #[test]
    fn cache_invalidation_reasons_single_change() {
        let cached_metadata = example_layer_metadata();
        let new_metadata = PythonLayerMetadata {
            distro_version: "24.04".to_string(),
            ..cached_metadata.clone()
        };
        assert_eq!(
            cache_invalidation_reasons(&cached_metadata, &new_metadata),
            ["The OS has changed from ubuntu-22.04 to ubuntu-24.04"]
        );
    }

    #[test]
    fn cache_invalidation_reasons_all_changed() {
        let cached_metadata = example_layer_metadata();
        let new_metadata = PythonLayerMetadata {
            arch: "arm64".to_string(),
            distro_name: "debian".to_string(),
            distro_version: "12".to_string(),
            python_version: "3.11.1".to_string(),
        };
        assert_eq!(
            cache_invalidation_reasons(&cached_metadata, &new_metadata),
            [
                "The CPU architecture has changed from amd64 to arm64",
                "The OS has changed from ubuntu-22.04 to debian-12",
                "The Python version has changed from 3.11.0 to 3.11.1",
            ]
        );
    }

    #[test]
    fn python_layer_env() {
        let mut base_env = Env::new();
        base_env.insert("CPATH", "/base");
        base_env.insert("LANG", "this-should-be-overridden");
        base_env.insert("PKG_CONFIG_PATH", "/base");
        base_env.insert("PYTHONHOME", "this-should-be-overridden");
        base_env.insert("PYTHONUNBUFFERED", "this-should-be-overridden");

        let layer_env = generate_layer_env(
            Path::new("/layer-dir"),
            &PythonVersion {
                major: 3,
                minor: 11,
                patch: 1,
            },
        );

        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Build, &base_env)),
            [
                ("CPATH", "/layer-dir/include/python3.11:/base"),
                ("LANG", "C.UTF-8"),
                ("PKG_CONFIG_PATH", "/layer-dir/lib/pkgconfig:/base"),
                ("PYTHONHOME", "/layer-dir"),
                ("PYTHONUNBUFFERED", "1"),
                ("SOURCE_DATE_EPOCH", "315532801"),
            ]
        );
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Launch, &base_env)),
            [
                ("CPATH", "/base"),
                ("LANG", "C.UTF-8"),
                ("PKG_CONFIG_PATH", "/base"),
                ("PYTHONHOME", "/layer-dir"),
                ("PYTHONUNBUFFERED", "1"),
            ]
        );
    }
}
