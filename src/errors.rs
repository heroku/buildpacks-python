use crate::django::DjangoCollectstaticError;
use crate::layers::pip_dependencies::PipDependenciesLayerError;
use crate::layers::python::PythonLayerError;
use crate::package_manager::DeterminePackageManagerError;
use crate::python_version::{PythonVersion, PythonVersionError, DEFAULT_PYTHON_VERSION};
use crate::runtime_txt::{ParseRuntimeTxtError, RuntimeTxtError};
use crate::utils::{CapturedCommandError, DownloadUnpackArchiveError, StreamedCommandError};
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
        BuildpackError::BuildpackDetection(error) => on_buildpack_detection_error(&error),
        BuildpackError::DeterminePackageManager(error) => on_determine_package_manager_error(error),
        BuildpackError::DjangoCollectstatic(error) => on_django_collectstatic_error(error),
        BuildpackError::DjangoDetection(error) => on_django_detection_error(&error),
        BuildpackError::PipDependenciesLayer(error) => on_pip_dependencies_layer_error(error),
        BuildpackError::PythonLayer(error) => on_python_layer_error(error),
        BuildpackError::PythonVersion(error) => on_python_version_error(error),
    };
}

fn on_buildpack_detection_error(error: &io::Error) {
    log_io_error(
        "Unable to complete buildpack detection",
        "determining if the Python buildpack should be run for this application",
        error,
    );
}

fn on_determine_package_manager_error(error: DeterminePackageManagerError) {
    match error {
        DeterminePackageManagerError::CheckFileExists(io_error) => log_io_error(
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
            // TODO: (W-12613425) Write the supported Python versions inline, instead of linking out to Dev Center.
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
            RuntimeTxtError::Read(io_error) => log_io_error(
                "Unable to read runtime.txt",
                "reading the (optional) runtime.txt file",
                &io_error,
            ),
        },
    };
}

fn on_python_layer_error(error: PythonLayerError) {
    match error {
        PythonLayerError::BootstrapPipCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to bootstrap pip",
                "running the command to install pip, setuptools and wheel",
                &io_error,
            ),
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
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
            DownloadUnpackArchiveError::Request(ureq_error) => log_error(
                "Unable to download Python",
                formatdoc! {"
                    An error occurred whilst downloading the Python runtime archive.
                    
                    In some cases, this happens due to an unstable network connection.
                    Please try again and to see if the error resolves itself.
                    
                    Details: {ureq_error}
                "},
            ),
            DownloadUnpackArchiveError::Unpack(io_error) => log_io_error(
                "Unable to unpack the Python archive",
                "unpacking the downloaded Python runtime archive and writing it to disk",
                &io_error,
            ),
        },
        PythonLayerError::LocateBundledPip(io_error) => log_io_error(
            "Unable to locate the bundled copy of pip",
            "locating the pip wheel file bundled inside the Python 'ensurepip' module",
            &io_error,
        ),
        PythonLayerError::MakeSitePackagesReadOnly(io_error) => log_io_error(
            "Unable to make site-packages directory read-only",
            "modifying the permissions on Python's 'site-packages' directory",
            &io_error,
        ),
        // This error will change once the Python version is validated against a manifest.
        // TODO: (W-12613425) Write the supported Python versions inline, instead of linking out to Dev Center.
        // TODO: Decide how to explain to users how stacks, base images and builder images versions relate to each other.
        PythonLayerError::PythonArchiveNotFound { python_version } => log_error(
            "Requested Python version is not available",
            formatdoc! {"
                The requested Python version ({python_version}) is not available for this builder image.
                
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
        PipDependenciesLayerError::CreateSrcDir(io_error) => log_io_error(
            "Unable to create 'src' directory required for pip install",
            "creating the 'src' directory in the pip layer, prior to running pip install",
            &io_error,
        ),
        PipDependenciesLayerError::PipInstallCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to install dependencies using pip",
                "running the 'pip install' command to install the application's dependencies",
                &io_error,
            ),
            // TODO: Add more suggestions here as to causes (eg network, invalid requirements.txt,
            // package broken or not compatible with version of Python, missing system dependencies etc)
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
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

fn on_django_detection_error(error: &io::Error) {
    log_io_error(
        "Unable to determine if this is a Django-based app",
        "checking if the 'django-admin' command exists",
        error,
    );
}

fn on_django_collectstatic_error(error: DjangoCollectstaticError) {
    match error {
        DjangoCollectstaticError::CheckCollectstaticCommandExists(error) => match error {
            CapturedCommandError::Io(io_error) => log_io_error(
                "Unable to inspect Django configuration",
                "running 'python manage.py help collectstatic' to inspect the Django configuration",
                &io_error,
            ),
            CapturedCommandError::NonZeroExitStatus(output) => log_error(
                "Unable to inspect Django configuration",
                formatdoc! {"
                    The 'python manage.py help collectstatic' Django management command
                    (used to check whether Django's static files feature is enabled)
                    failed ({exit_status}).
                    
                    Details:
                    
                    {stderr}
                    
                    This indicates there is a problem with your application code or Django
                    configuration. Try running the 'manage.py' script locally to see if the
                    same error occurs.
                    ",
                    exit_status = &output.status,
                    stderr = String::from_utf8_lossy(&output.stderr)
                },
            ),
        },
        DjangoCollectstaticError::CheckManagementScriptExists(io_error) => log_io_error(
            "Unable to inspect Django configuration",
            "checking if the 'manage.py' script exists",
            &io_error,
        ),
        DjangoCollectstaticError::CollectstaticCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to generate Django static files",
                "running 'python manage.py collectstatic' to generate Django static files",
                &io_error,
            ),
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to generate Django static files",
                formatdoc! {"
                    The 'python manage.py collectstatic --link --noinput' Django management
                    command to generate static files failed ({exit_status}).
                    
                    This is most likely due an issue in your application code or Django
                    configuration. See the log output above for more information.
                    
                    If you are using the WhiteNoise package to optimize the serving of static
                    files with Django (recommended), check that your app is using the Django
                    config options shown here:
                    https://whitenoise.readthedocs.io/en/stable/django.html
                    
                    Or, if you do not need to use static files in your app, disable the
                    Django static files feature by removing 'django.contrib.staticfiles'
                    from 'INSTALLED_APPS' in your app's Django configuration.
                "},
            ),
        },
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
