use crate::layers::pip_dependencies::PipDependenciesLayerError;
use crate::layers::python::PythonLayerError;
use crate::package_manager::DeterminePackageManagerError;
use crate::project_descriptor::ProjectDescriptorError;
use crate::python_version::{PythonVersion, PythonVersionError, DEFAULT_PYTHON_VERSION};
use crate::runtime_txt::{ParseRuntimeTxtError, RuntimeTxtError};
use crate::salesforce_functions::{CheckSalesforceFunctionError, FUNCTION_RUNTIME_PROGRAM_NAME};
use crate::utils::{CommandError, DownloadUnpackArchiveError};
use crate::BuildpackError;
use indoc::{formatdoc, indoc};
use libherokubuildpack::log::log_error;
use std::io;

/// Handle any non-recoverable buildpack or libcnb errors that occur.
///
/// The buildpack will exit non-zero after this handler has run, so all that needs to be
/// performed here is the logging of an error message - and in the future, emitting metrics.
///
/// We're intentionally not using `libherokubuildpack::error::on_error` since:
/// - It doesn't currently do anything other than logging an internal error for the libcnb
///   error case, and by inlining that here it's easier to keep the output consistent with
///   the messages emitted for buildpack-specific errors.
/// - Using it causes trait mismatch errors when Dependabot PRs incrementally update crates.
/// - When we want to add metrics to our buildpacks, it's going to need a rewrite of
///   `Buildpack::on_error` anyway (we'll need to write out metrics not log them, so will need
///   access to the `BuildContext`), at which point we can re-evaluate.
pub(crate) fn on_error(error: libcnb::Error<BuildpackError>) {
    match error {
        libcnb::Error::BuildpackError(buildpack_error) => on_buildpack_error(buildpack_error),
        libcnb_error => log_error(
            "Internal buildpack error",
            formatdoc! {"
                An unexpected internal error was reported by the framework used by this buildpack.
                
                Please open a support ticket and include the full log output of this build.
                
                Details: {libcnb_error}
            "},
        ),
    };
}

fn on_buildpack_error(error: BuildpackError) {
    match error {
        BuildpackError::CheckSalesforceFunction(error) => on_check_salesforce_function_error(error),
        BuildpackError::DetectIo(io_error) => log_io_error(
            "Unable to complete buildpack detection",
            "determining if the Python buildpack should be run for this application",
            &io_error,
        ),
        BuildpackError::DeterminePackageManager(error) => on_determine_package_manager_error(error),
        BuildpackError::PipDependenciesLayer(error) => on_pip_dependencies_layer_error(error),
        BuildpackError::ProjectDescriptor(error) => on_project_descriptor_error(error),
        BuildpackError::PythonLayer(error) => on_python_layer_error(error),
        BuildpackError::PythonVersion(error) => on_python_version_error(error),
    };
}

fn on_project_descriptor_error(error: ProjectDescriptorError) {
    match error {
        ProjectDescriptorError::Io(io_error) => log_io_error(
            "Unable to read project.toml",
            "reading the (optional) project.toml file",
            &io_error,
        ),
        ProjectDescriptorError::Parse(toml_error) => log_error(
            "Invalid project.toml",
            formatdoc! {"
                A parsing/validation error error occurred whilst loading the project.toml file.
                
                Details: {toml_error}
            "},
        ),
    };
}

fn on_determine_package_manager_error(error: DeterminePackageManagerError) {
    match error {
        DeterminePackageManagerError::Io(io_error) => log_io_error(
            "Unable to determine the package manager",
            "determining which Python package manager to use for this project",
            &io_error,
        ),
        // TODO: Should this mention the setup.py / pyproject.toml case?
        DeterminePackageManagerError::NoneFound => log_error(
            "No Python package manager files were found",
            indoc! {"
                A Pip requirements file was not found in your application's source code.
                This file is required so that your application's dependencies can be installed.
                
                Please add a file named exactly 'requirements.txt' to the root directory of your
                application, containing a list of the packages required by your application.
                
                For more information on what this file should contain, see:
                https://pip.pypa.io/en/stable/reference/requirements-file-format/
            "},
        ),
    };
}

fn on_python_version_error(error: PythonVersionError) {
    match error {
        PythonVersionError::RuntimeTxt(error) => match error {
            RuntimeTxtError::Io(io_error) => log_io_error(
                "Unable to read runtime.txt",
                "reading the (optional) runtime.txt file",
                &io_error,
            ),
            // TODO: Write the supported Python versions inline, instead of linking out to Dev Center.
            RuntimeTxtError::Parse(ParseRuntimeTxtError { cleaned_contents }) => {
                let PythonVersion {
                    major,
                    minor,
                    patch,
                } = DEFAULT_PYTHON_VERSION;
                log_error(
                    "Invalid Python version in runtime.txt",
                    formatdoc! {"
                        The Python version specified in 'runtime.txt' is not in the correct format.
                        
                        The following file contents were found:
                        {cleaned_contents}
                        
                        However, the file contents must begin with a 'python-' prefix, followed by the
                        version specified as '<major>.<minor>.<patch>'. Comments are not supported.
                        
                        For example, to request Python {DEFAULT_PYTHON_VERSION}, the correct version format is:
                        python-{major}.{minor}.{patch}
                        
                        Please update 'runtime.txt' to use the correct version format, or else remove
                        the file to instead use the default version (currently Python {DEFAULT_PYTHON_VERSION}).
                        
                        For a list of the supported Python versions, see:
                        https://devcenter.heroku.com/articles/python-support#supported-runtimes
                    "},
                );
            }
        },
    };
}

fn on_python_layer_error(error: PythonLayerError) {
    match error {
        PythonLayerError::BootstrapPipCommand(error) => match error {
            CommandError::Io(io_error) => log_io_error(
                "Unable to bootstrap pip",
                "running the command to install pip, setuptools and wheel",
                &io_error,
            ),
            CommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to bootstrap pip",
                formatdoc! {"
                    The command to install pip, setuptools and wheel did not exit successfully ({exit_status}).
                    
                    See the log output above for more information.
                    
                    In some cases, this happens due to an unstable network connection.
                    Please try again to see if the error resolves itself.
                    
                    If that does not help, check the status of PyPI (the upstream Python
                    package repository service), here:
                    https://status.python.org
                "},
            ),
        },
        PythonLayerError::DownloadUnpackPythonArchive(error) => match error {
            DownloadUnpackArchiveError::Io(io_error) => log_io_error(
                "Unable to unpack the Python archive",
                "unpacking the downloaded Python runtime archive and writing it to disk",
                &io_error,
            ),
            DownloadUnpackArchiveError::Request(ureq_error) => log_error(
                "Unable to download Python",
                formatdoc! {"
                    An error occurred whilst downloading the Python runtime archive.
                    
                    In some cases, this happens due to an unstable network connection.
                    Please try again and to see if the error resolves itself.
                    
                    Details: {ureq_error}
                "},
            ),
        },
        PythonLayerError::LocateBundledPipIo(io_error) => log_io_error(
            "Unable to locate the bundled copy of pip",
            "locating the pip wheel file bundled inside the Python 'ensurepip' module",
            &io_error,
        ),
        PythonLayerError::MakeSitePackagesReadOnlyIo(io_error) => log_io_error(
            "Unable to make site-packages directory read-only",
            "modifying the permissions on Python's 'site-packages' directory",
            &io_error,
        ),
        // This error will change once the Python version is validated against a manifest.
        // TODO: Write the supported Python versions inline, instead of linking out to Dev Center.
        PythonLayerError::PythonArchiveNotFound {
            python_version,
            stack,
        } => log_error(
            "Requested Python version is not available",
            formatdoc! {"
                The requested Python version ({python_version}) is not available for this stack ({stack}).
                
                Please update the version in 'runtime.txt' to a supported Python version, or else
                remove the file to instead use the default version (currently Python {DEFAULT_PYTHON_VERSION}).
                
                For a list of the supported Python versions, see:
                https://devcenter.heroku.com/articles/python-support#supported-runtimes
            "},
        ),
    };
}

fn on_pip_dependencies_layer_error(error: PipDependenciesLayerError) {
    match error {
        PipDependenciesLayerError::CreateSrcDirIo(io_error) => log_io_error(
            "Unable to create 'src' directory required for pip install",
            "creating the 'src' directory in the pip layer, prior to running pip install",
            &io_error,
        ),
        PipDependenciesLayerError::PipInstallCommand(error) => match error {
            CommandError::Io(io_error) => log_io_error(
                "Unable to install dependencies using pip",
                "running the 'pip install' command to install the application's dependencies",
                &io_error,
            ),
            // TODO: Add more suggestions here as to causes (eg network, invalid requirements.txt,
            // package broken or not compatible with version of Python, missing system dependencies etc)
            CommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to install dependencies using pip",
                formatdoc! {"
                    The 'pip install' command to install the application's dependencies from
                    'requirements.txt' failed ({exit_status}).
                    
                    See the log output above for more information.
                "},
            ),
        },
    };
}

fn on_check_salesforce_function_error(error: CheckSalesforceFunctionError) {
    match error {
        CheckSalesforceFunctionError::Io(io_error) => log_io_error(
            "Unable to run the Salesforce Functions self-check command",
            &format!("running the '{FUNCTION_RUNTIME_PROGRAM_NAME} check' command"),
            &io_error,
        ),
        CheckSalesforceFunctionError::NonZeroExitStatus(output) => log_error(
            "The Salesforce Functions self-check failed",
            formatdoc! {"
                The '{FUNCTION_RUNTIME_PROGRAM_NAME} check' command failed ({exit_status}), indicating
                there is a problem with the Python Salesforce Function in this project.
                
                Details:
                {stderr}
                ",
                exit_status = output.status,
                stderr = String::from_utf8_lossy(&output.stderr),
            },
        ),
        CheckSalesforceFunctionError::ProgramNotFound => log_error(
            "The Salesforce Functions package is not installed",
            formatdoc! {"
                The '{FUNCTION_RUNTIME_PROGRAM_NAME}' program that is required for Python Salesforce
                Functions could not be found.

                Check that the 'salesforce-functions' Python package is listed as a
                dependency in 'requirements.txt'.
                
                If this project is not intended to be a Salesforce Function, remove the
                'type = \"function\"' declaration from 'project.toml' to skip this check.
            "},
        ),
    };
}

fn log_io_error(header: &str, occurred_whilst: &str, io_error: &io::Error) {
    // We don't suggest opening a support ticket, since a subset of I/O errors can be caused
    // by issues in the application. In the future, perhaps we should try and split these out?
    log_error(
        header,
        formatdoc! {"
            An unexpected error occurred whilst {occurred_whilst}.
            
            Details: I/O Error: {io_error}
        "},
    );
}
