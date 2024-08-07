use crate::utils::{self, StreamedCommandError};
use crate::{BuildpackError, PythonBuildpack};
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::UncachedLayerDefinition;
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use libherokubuildpack::log::log_info;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Creates a layer containing the application's Python dependencies, installed using pip.
//
// To do this we use `pip install --user` so that the dependencies are installed into the user
// `site-packages` directory in this layer (set by `PYTHONUSERBASE`), rather than the system
// `site-packages` subdirectory of the Python installation layer.
//
// Note: We can't instead use pip's `--target` option along with `PYTHONPATH`, since:
// - Directories on `PYTHONPATH` take precedence over the Python stdlib (unlike the system or
//   user site-packages directories), which can cause hard to debug stdlib shadowing issues
//   if one of the app's transitive dependencies is an outdated stdlib backport package.
// - `--target` has bugs, eg: <https://github.com/pypa/pip/issues/8799>
//
// This layer is not cached, since:
// - pip is a package installer rather than a project/environment manager, and so does not
//   deterministically manage installed Python packages. For example, if a package entry in
//   a requirements file is later removed, pip will not uninstall the package. In addition,
//   there is no official lockfile support, so changes in transitive dependencies add yet
//   more opportunity for non-determinism between each install.
// - The pip HTTP/wheel cache is itself cached in a separate layer (exposed via `PIP_CACHE_DIR`),
//   which covers the most time consuming part of performing a pip install: downloading the
//   dependencies and then generating wheels for any packages that don't provide them.
pub(crate) fn install_dependencies(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
) -> Result<PathBuf, libcnb::Error<BuildpackError>> {
    let layer = context.uncached_layer(
        layer_name!("dependencies"),
        UncachedLayerDefinition {
            build: true,
            launch: true,
        },
    )?;

    let layer_path = layer.path();
    let layer_env = generate_layer_env(&layer_path);
    layer.write_env(&layer_env)?;
    env.clone_from(&layer_env.apply(Scope::Build, env));

    log_info("Running pip install");

    utils::run_command_and_stream_output(
        Command::new("pip")
            .args([
                "install",
                "--no-input",
                "--progress-bar",
                "off",
                // Using `--user` rather than `PIP_USER` since the latter affects `pip list` too.
                "--user",
                "--requirement",
                "requirements.txt",
                // For VCS dependencies installed in editable mode, the repository clones must be
                // kept after installation, since their directories are added to the Python path
                // directly (via `.pth` files in `site-packages`). By default pip will store the
                // repositories in the current working directory (the app dir), but we want them
                // in the dependencies layer instead.
                "--src",
                &layer_path.join("src").to_string_lossy(),
            ])
            .current_dir(&context.app_dir)
            .env_clear()
            .envs(&*env),
    )
    .map_err(PipDependenciesLayerError::PipInstallCommand)?;

    Ok(layer_path)
}

fn generate_layer_env(layer_path: &Path) -> LayerEnv {
    LayerEnv::new()
        // We set `PATH` explicitly, since lifecycle will only add the bin directory to `PATH` if it
        // exists - and we want to support the scenario of installing a debugging package with CLI at
        // run-time, when none of the dependencies installed at build-time had an entrypoint script.
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Prepend,
            "PATH",
            layer_path.join("bin"),
        )
        .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "PATH", ":")
        // Overrides the default user base directory, used by Python to compute the path of the user
        // `site-packages` directory. Setting this:
        //   - Makes `pip install --user` install the dependencies into the current layer rather
        //     than the user's home directory (which would be discarded at the end of the build).
        //   - Allows Python to find the installed packages at import time.
        // See: https://docs.python.org/3/using/cmdline.html#envvar-PYTHONUSERBASE
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "PYTHONUSERBASE",
            layer_path,
        )
}

/// Errors that can occur when installing the project's dependencies into a layer using pip.
#[derive(Debug)]
pub(crate) enum PipDependenciesLayerError {
    PipInstallCommand(StreamedCommandError),
}

impl From<PipDependenciesLayerError> for libcnb::Error<BuildpackError> {
    fn from(error: PipDependenciesLayerError) -> Self {
        Self::BuildpackError(BuildpackError::PipDependenciesLayer(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pip_dependencies_layer_env() {
        let mut base_env = Env::new();
        base_env.insert("PATH", "/base");
        base_env.insert("PYTHONUSERBASE", "this-should-be-overridden");

        let layer_env = generate_layer_env(Path::new("/layer-dir"));

        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Build, &base_env)),
            [
                ("PATH", "/layer-dir/bin:/base"),
                ("PYTHONUSERBASE", "/layer-dir"),
            ]
        );
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Launch, &base_env)),
            [
                ("PATH", "/layer-dir/bin:/base"),
                ("PYTHONUSERBASE", "/layer-dir"),
            ]
        );
    }
}
