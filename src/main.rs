#![warn(clippy::pedantic)]
#![warn(unused_crate_dependencies)]
// Prevent warnings caused by the large size of `ureq::Error` in error enums,
// where it is not worth boxing since the enum size doesn't affect performance.
#![allow(clippy::large_enum_variant)]
#![allow(clippy::result_large_err)]

mod errors;
mod layers;
mod package_manager;
mod project_descriptor;
mod python_version;
mod runtime_txt;
mod salesforce_functions;
mod utils;

use crate::layers::pip_cache::PipCacheLayer;
use crate::layers::pip_dependencies::{PipDependenciesLayer, PipDependenciesLayerError};
use crate::layers::python::{PythonLayer, PythonLayerError};
use crate::package_manager::{DeterminePackageManagerError, PackageManager};
use crate::project_descriptor::ProjectDescriptorError;
use crate::python_version::PythonVersionError;
use crate::salesforce_functions::CheckSalesforceFunctionError;
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::layer_name;
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
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
        if utils::is_python_project(&context.app_dir).map_err(BuildpackError::DetectIo)? {
            DetectResultBuilder::pass().build()
        } else {
            log_info("No Python project files found (such as requirements.txt).");
            DetectResultBuilder::fail().build()
        }
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        // We perform all project analysis up front, so the build can fail early if the config is invalid.
        // TODO: Add a "Build config" header and list all config in one place?
        let is_function = salesforce_functions::is_function_project(&context.app_dir)
            .map_err(BuildpackError::ProjectDescriptor)?;
        let package_manager = package_manager::determine_package_manager(&context.app_dir)
            .map_err(BuildpackError::DeterminePackageManager)?;

        log_header("Determining Python version");
        let python_version = python_version::determine_python_version(&context.app_dir)
            .map_err(BuildpackError::PythonVersion)?;

        // We inherit the current process's env vars, since we want `PATH` and `HOME` to be set
        // so that later commands can find tools like Git in the stack image. Any user-provided
        // env vars will still be excluded, due to the use of `clear-env` in `buildpack.toml`.
        let mut command_env = Env::from_current();

        // Create the layer containing the Python runtime, and the packages `pip`, `setuptools` and `wheel`.
        let python_layer = context.handle_layer(
            layer_name!("python"),
            PythonLayer {
                command_env: &command_env,
                python_version: &python_version,
            },
        )?;
        command_env = python_layer.env.apply(Scope::Build, &command_env);

        // Create the layers for the application dependencies and package manager cache.
        // In the future support will be added for package managers other than pip.
        let dependencies_layer_env = match package_manager {
            PackageManager::Pip => {
                log_header("Installing dependencies using Pip");
                let pip_cache_layer = context.handle_layer(
                    layer_name!("pip-cache"),
                    PipCacheLayer {
                        python_version: &python_version,
                    },
                )?;
                let pip_layer = context.handle_layer(
                    layer_name!("dependencies"),
                    PipDependenciesLayer {
                        command_env: &command_env,
                        pip_cache_dir: pip_cache_layer.path,
                    },
                )?;
                pip_layer.env
            }
        };
        command_env = dependencies_layer_env.apply(Scope::Build, &command_env);

        if is_function {
            log_header("Validating Salesforce Function");
            salesforce_functions::check_function(&command_env)
                .map_err(BuildpackError::CheckSalesforceFunction)?;
            log_info("Function passed validation.");

            BuildResultBuilder::new()
                .launch(salesforce_functions::launch_config())
                .build()
        } else {
            BuildResultBuilder::new().build()
        }
    }

    fn on_error(&self, error: libcnb::Error<Self::Error>) {
        errors::on_error(error);
    }
}

#[derive(Debug)]
pub(crate) enum BuildpackError {
    /// Errors running the `sf-functions-python check` command.
    CheckSalesforceFunction(CheckSalesforceFunctionError),
    /// IO errors when performing buildpack detection.
    DetectIo(io::Error),
    /// Errors determining which Python package manager to use for a project.
    DeterminePackageManager(DeterminePackageManagerError),
    /// Errors installing the project's dependencies into a layer using Pip.
    PipDependenciesLayer(PipDependenciesLayerError),
    /// Errors reading and parsing a `project.toml` file.
    ProjectDescriptor(ProjectDescriptorError),
    /// Errors installing Python and required packaging tools into a layer.
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

#[cfg(test)]
mod tests {
    // Suppress warnings due to the `unused_crate_dependencies` lint not handling integration tests well.
    use libcnb_test as _;
}
