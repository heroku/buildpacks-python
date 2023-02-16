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

/// Layer containing the application's Python dependencies, installed using Pip.
pub(crate) struct PipDependenciesLayer<'a> {
    /// Environment variables inherited from earlier buildpack steps.
    pub command_env: &'a Env,
    /// The path to the Pip cache directory, which is stored in another layer since it isn't needed at runtime.
    pub pip_cache_dir: PathBuf,
}

impl Layer for PipDependenciesLayer<'_> {
    type Buildpack = PythonBuildpack;
    type Metadata = GenericMetadata;

    fn types(&self) -> LayerTypes {
        // This layer is not cached, since:
        // - Pip is a package installer rather than a project/environment manager, and so does
        //   not deterministically manage installed Python packages. For example, if a package
        //   entry in a requirements file is later removed, Pip will not uninstall the package.
        //   In addition, there is no official lockfile support (only partial support via
        //   third-party requirements file tools), so changes in transitive dependencies add yet
        //   more opportunity for non-determinism between each install.
        // - The Pip HTTP/wheel cache is itself cached in a separate layer, which covers the most
        //   time consuming part of performing a pip install: downloading the dependencies and then
        //   generating wheels (for any packages that use compiled components but don't distribute
        //   pre-built wheels matching the current Python version).
        // - The only case where the Pip wheel cache doesn't help, is for projects that use
        //   hash-checking mode and so are affected by this Pip issue:
        //   https://github.com/pypa/pip/issues/5037
        //   ...however, the limitation should really be fixed upstream, and this mode is rarely
        //   used in practice.
        //
        // Longer term, the best option for projects that want no-op deterministic installs will
        // be to use Poetry instead of Pip (once the buildpack supports Poetry).
        LayerTypes {
            build: true,
            cache: false,
            launch: true,
        }
    }

    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        let layer_env = generate_layer_env(layer_path);
        let command_env = layer_env.apply(Scope::Build, self.command_env);

        // When Pip installs dependencies from a VCS URL it has to clone the repository in order
        // to install it. In standard installation mode the clone is made to a temporary directory
        // and then deleted, however, when packages are installed in editable mode Pip must keep
        // the repository around, since the directory is added to the Python path directly (via
        // the `.pth` file created in `site-packages`). By default Pip will store the repository
        // in the current working directory (the app dir), however, we would prefer it to be stored
        // in the dependencies layer instead for consistency. (Plus if this layer were ever cached,
        // storing the repository in the app dir would break on repeat-builds).
        let src_dir = layer_path.join("src");
        fs::create_dir(&src_dir).map_err(PipDependenciesLayerError::CreateSrcDirIo)?;

        log_info("Running pip install");

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
                    // Install dependencies into the user `site-packages` directory (set by `PYTHONUSERBASE`),
                    // rather than the system `site-packages` directory (since we want to keep dependencies in
                    // a separate layer to the Python runtime).
                    //
                    // Another option is to install into an arbitrary directory using Pip's `--target` option
                    // combined with adding that directory to `PYTHONPATH`, however:
                    //   - Using `--target` causes a number of issues with Pip, eg:
                    //     https://github.com/pypa/pip/issues/8799
                    //   - Directories added to `PYTHONPATH` take precedence over the Python stdlib (unlike
                    //     the system or user site-packages directories), and so can result in hard to debug
                    //     stdlib shadowing problems that users won't encounter locally (for example if one
                    //     of the app's transitive dependencies is an outdated stdlib backport package).
                    "--user",
                    "--requirement",
                    "requirements.txt",
                    // Clone any VCS repositories installed in editable mode into the directory created
                    // above, rather than the default of the current working directory (the app dir).
                    "--src",
                    &src_dir.to_string_lossy(),
                ])
                .env_clear()
                .envs(&command_env),
        )
        .map_err(PipDependenciesLayerError::PipInstallCommand)?;

        log_info("Pip install completed");

        LayerResultBuilder::new(GenericMetadata::default())
            .env(layer_env)
            .build()
    }
}

/// Environment variables that will be set by this layer.
fn generate_layer_env(layer_path: &Path) -> LayerEnv {
    LayerEnv::new()
        // `PYTHONUSERBASE` overrides the default user base directory, which is used by Python to
        // compute the path of the user `site-packages` directory:
        // https://docs.python.org/3/using/cmdline.html#envvar-PYTHONUSERBASE
        //
        // Setting this:
        //   - Makes `pip install --user` install the dependencies into the current layer rather
        //     than the user's home directory (which would be discarded at the end of the build).
        //   - Allows Python to find the installed packages at import time.
        //
        // It's fine for this directory to be set to the root of the layer, since all of the files
        // created by Pip will be nested inside subdirectories (such as `bin/` or `lib/`), and so
        // won't conflict with the CNB layer metadata related files generated by libcnb.rs.
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "PYTHONUSERBASE",
            layer_path,
        )
}

/// Errors that can occur when installing the project's dependencies into a layer using Pip.
#[derive(Debug)]
pub(crate) enum PipDependenciesLayerError {
    CreateSrcDirIo(io::Error),
    PipInstallCommand(CommandError),
}

impl From<PipDependenciesLayerError> for BuildpackError {
    fn from(error: PipDependenciesLayerError) -> Self {
        Self::PipDependenciesLayer(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pip_dependencies_layer_env() {
        let mut base_env = Env::new();
        base_env.insert("PYTHONUSERBASE", "this-should-be-overridden");

        let layer_env = generate_layer_env(Path::new("/layers/dependencies"));

        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Build, &base_env)),
            vec![("PYTHONUSERBASE", "/layers/dependencies")]
        );
        assert_eq!(
            utils::environment_as_sorted_vector(&layer_env.apply(Scope::Launch, &base_env)),
            vec![("PYTHONUSERBASE", "/layers/dependencies")]
        );
    }
}
