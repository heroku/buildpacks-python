mod checks;
mod detect;
mod django;
mod errors;
mod layers;
mod package_manager;
mod packaging_tool_versions;
mod python_version;
mod python_version_file;
mod utils;

use crate::checks::ChecksError;
use crate::django::DjangoCollectstaticError;
use crate::layers::pip::PipLayerError;
use crate::layers::pip_dependencies::PipDependenciesLayerError;
use crate::layers::poetry::PoetryLayerError;
use crate::layers::poetry_dependencies::PoetryDependenciesLayerError;
use crate::layers::python::PythonLayerError;
use crate::layers::{pip, pip_cache, pip_dependencies, poetry, poetry_dependencies, python};
use crate::package_manager::{DeterminePackageManagerError, PackageManager};
use crate::python_version::{
    PythonVersionOrigin, RequestedPythonVersion, RequestedPythonVersionError,
    ResolvePythonVersionError,
};
use crate::utils::FileExistsError;
use indoc::formatdoc;
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::{Buildpack, Env, buildpack_main};
use libherokubuildpack::log::{log_header, log_info, log_warning};

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
            log_info(
                "No Python project files found (such as pyproject.toml, requirements.txt or poetry.lock).",
            );
            DetectResultBuilder::fail().build()
        }
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        // We inherit the current process's env vars, since we want `PATH` and `HOME` from the OS
        // to be set (so that later commands can find tools like Git in the base image), along
        // with previous-buildpack or user-provided env vars (so that features like env vars in
        // in requirements files work). We protect against broken user-provided env vars via the
        // checks feature and making sure that buildpack env vars take precedence in layers envs.
        let mut env = Env::from_current();

        checks::check_environment(&env).map_err(BuildpackError::Checks)?;

        // We perform all project analysis up front, so the build can fail early if the config is invalid.
        // TODO: Add a "Build config" header and list all config in one place?
        let package_manager = package_manager::determine_package_manager(&context.app_dir)
            .map_err(BuildpackError::DeterminePackageManager)?;

        log_header("Determining Python version");

        let requested_python_version =
            python_version::read_requested_python_version(&context.app_dir)
                .map_err(BuildpackError::RequestedPythonVersion)?;
        let python_version = python_version::resolve_python_version(&requested_python_version)
            .map_err(BuildpackError::ResolvePythonVersion)?;

        match requested_python_version.origin {
            PythonVersionOrigin::BuildpackDefault => log_info(formatdoc! {"
                No Python version specified, using the current default of Python {requested_python_version}.
                We recommend setting an explicit version. In the root of your app create
                a '.python-version' file, containing a Python version like '{requested_python_version}'."
            }),
            PythonVersionOrigin::PythonVersionFile => log_info(format!(
                "Using Python version {requested_python_version} specified in .python-version"
            )),
        }

        if let RequestedPythonVersion {
            major: 3,
            minor: 9,
            origin,
            ..
        } = &requested_python_version
        {
            log_warning(
                "Support for Python 3.9 is deprecated",
                formatdoc! {"
                    Python 3.9 will reach its upstream end-of-life in October 2025,
                    at which point it will no longer receive security updates:
                    https://devguide.python.org/versions/#supported-versions

                    As such, support for Python 3.9 will be removed from this
                    buildpack on 7th January 2026.

                    Upgrade to a newer Python version as soon as possible, by
                    changing the version in your {origin} file.

                    For more information, see:
                    https://devcenter.heroku.com/articles/python-support#supported-python-versions
                "},
            );
        }

        log_header("Installing Python");
        let python_layer_path = python::install_python(
            &context,
            &mut env,
            &python_version,
            &requested_python_version,
        )?;

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
    /// I/O errors when performing buildpack detection.
    BuildpackDetection(FileExistsError),
    /// Errors due to one of the environment checks failing.
    Checks(ChecksError),
    /// Errors determining which Python package manager to use for a project.
    DeterminePackageManager(DeterminePackageManagerError),
    /// Errors running the Django collectstatic command.
    DjangoCollectstatic(DjangoCollectstaticError),
    /// I/O errors when detecting whether Django is installed.
    DjangoDetection(FileExistsError),
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
    /// Errors determining which Python version was requested for a project.
    RequestedPythonVersion(RequestedPythonVersionError),
    /// Errors resolving a requested Python version to a specific Python version.
    ResolvePythonVersion(ResolvePythonVersionError),
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
