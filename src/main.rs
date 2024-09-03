mod detect;
mod django;
mod errors;
mod layers;
mod package_manager;
mod packaging_tool_versions;
mod python_version;
mod runtime_txt;
mod utils;

use crate::django::DjangoCollectstaticError;
use crate::layers::pip::PipLayerError;
use crate::layers::pip_dependencies::PipDependenciesLayerError;
use crate::layers::poetry::PoetryLayerError;
use crate::layers::poetry_dependencies::PoetryDependenciesLayerError;
use crate::layers::python::PythonLayerError;
use crate::layers::{pip, pip_cache, pip_dependencies, poetry, poetry_dependencies, python};
use crate::package_manager::{DeterminePackageManagerError, PackageManager};
use crate::python_version::PythonVersionError;
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::{buildpack_main, Buildpack, Env};
use libherokubuildpack::log::{log_header, log_info};
use std::io;

struct PythonBuildpack;

impl Buildpack for PythonBuildpack {
    type Platform = GenericPlatform;
    type Metadata = GenericMetadata;
    type Error = BuildpackError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        // In the future we will add support for requiring this buildpack through the build plan,
        // but we first need a better understanding of real-world use-cases, so that we can work
        // out how best to support them without sacrificing existing error handling UX (such as
        // wanting to show a clear error when requirements.txt is missing).
        if detect::is_python_project_directory(&context.app_dir)
            .map_err(BuildpackError::BuildpackDetection)?
        {
            DetectResultBuilder::pass().build()
        } else {
            log_info("No Python project files found (such as pyproject.toml, requirements.txt or poetry.lock).");
            DetectResultBuilder::fail().build()
        }
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        // We perform all project analysis up front, so the build can fail early if the config is invalid.
        // TODO: Add a "Build config" header and list all config in one place?
        let package_manager = package_manager::determine_package_manager(&context.app_dir)
            .map_err(BuildpackError::DeterminePackageManager)?;

        log_header("Determining Python version");
        let python_version = python_version::determine_python_version(&context.app_dir)
            .map_err(BuildpackError::PythonVersion)?;

        // We inherit the current process's env vars, since we want `PATH` and `HOME` from the OS
        // to be set (so that later commands can find tools like Git in the base image), along
        // with previous-buildpack or user-provided env vars (so that features like env vars in
        // in requirements files work). We protect against broken user-provided env vars by
        // making sure that buildpack env vars take precedence in layers envs and command usage.
        let mut env = Env::from_current();

        log_header("Installing Python");
        let python_layer_path = python::install_python(&context, &mut env, &python_version)?;

        let dependencies_layer_dir = match package_manager {
            PackageManager::Pip => {
                log_header("Installing pip");
                pip::install_pip(&context, &mut env, &python_version, &python_layer_path)?;
                log_header("Installing dependencies using pip");
                pip_cache::prepare_pip_cache(&context, &mut env, &python_version)?;
                pip_dependencies::install_dependencies(&context, &mut env)?
            }
            PackageManager::Poetry => {
                log_header("Installing Poetry");
                poetry::install_poetry(&context, &mut env, &python_version, &python_layer_path)?;
                log_header("Installing dependencies using Poetry");
                poetry_dependencies::install_dependencies(&context, &mut env, &python_version)?
            }
        };

        if django::is_django_installed(&dependencies_layer_dir)
            .map_err(BuildpackError::DjangoDetection)?
        {
            log_header("Generating Django static files");
            django::run_django_collectstatic(&context.app_dir, &env)
                .map_err(BuildpackError::DjangoCollectstatic)?;
        }

        BuildResultBuilder::new().build()
    }

    fn on_error(&self, error: libcnb::Error<Self::Error>) {
        errors::on_error(error);
    }
}

#[derive(Debug)]
pub(crate) enum BuildpackError {
    /// IO errors when performing buildpack detection.
    BuildpackDetection(io::Error),
    /// Errors determining which Python package manager to use for a project.
    DeterminePackageManager(DeterminePackageManagerError),
    /// Errors running the Django collectstatic command.
    DjangoCollectstatic(DjangoCollectstaticError),
    /// IO errors when detecting whether Django is installed.
    DjangoDetection(io::Error),
    /// Errors installing the project's dependencies into a layer using pip.
    PipDependenciesLayer(PipDependenciesLayerError),
    /// Errors installing pip into a layer.
    PipLayer(PipLayerError),
    /// Errors installing the project's dependencies into a layer using Poetry.
    PoetryDependenciesLayer(PoetryDependenciesLayerError),
    /// Errors installing Poetry into a layer.
    PoetryLayer(PoetryLayerError),
    /// Errors installing Python into a layer.
    PythonLayer(PythonLayerError),
    /// Errors determining which Python version to use for a project.
    PythonVersion(PythonVersionError),
}

impl From<BuildpackError> for libcnb::Error<BuildpackError> {
    fn from(error: BuildpackError) -> Self {
        Self::BuildpackError(error)
    }
}

buildpack_main!(PythonBuildpack);

// The integration tests are imported into the crate so that they can have access to private
// APIs and constants, saving having to (a) run a dual binary/library crate, (b) expose APIs
// publicly for things only used for testing. To prevent the tests from being imported twice,
// automatic integration test discovery is disabled using `autotests = false` in Cargo.toml.
#[cfg(test)]
#[path = "../tests/mod.rs"]
mod tests;
