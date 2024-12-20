use crate::checks::ChecksError;
use crate::django::DjangoCollectstaticError;
use crate::layers::pip::PipLayerError;
use crate::layers::pip_dependencies::PipDependenciesLayerError;
use crate::layers::poetry::PoetryLayerError;
use crate::layers::poetry_dependencies::PoetryDependenciesLayerError;
use crate::layers::python::PythonLayerError;
use crate::package_manager::DeterminePackageManagerError;
use crate::python_version::{
    RequestedPythonVersion, RequestedPythonVersionError, ResolvePythonVersionError,
    DEFAULT_PYTHON_FULL_VERSION, DEFAULT_PYTHON_VERSION,
};
use crate::python_version_file::ParsePythonVersionFileError;
use crate::runtime_txt::ParseRuntimeTxtError;
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
        BuildpackError::Checks(error) => on_buildpack_checks_error(error),
        BuildpackError::DeterminePackageManager(error) => on_determine_package_manager_error(error),
        BuildpackError::DjangoCollectstatic(error) => on_django_collectstatic_error(error),
        BuildpackError::DjangoDetection(error) => on_django_detection_error(&error),
        BuildpackError::PipDependenciesLayer(error) => on_pip_dependencies_layer_error(error),
        BuildpackError::PipLayer(error) => on_pip_layer_error(error),
        BuildpackError::PoetryDependenciesLayer(error) => on_poetry_dependencies_layer_error(error),
        BuildpackError::PoetryLayer(error) => on_poetry_layer_error(error),
        BuildpackError::PythonLayer(error) => on_python_layer_error(error),
        BuildpackError::RequestedPythonVersion(error) => on_requested_python_version_error(error),
        BuildpackError::ResolvePythonVersion(error) => on_resolve_python_version_error(error),
    };
}

fn on_buildpack_detection_error(error: &io::Error) {
    log_io_error(
        "Unable to complete buildpack detection",
        "determining if the Python buildpack should be run for this application",
        error,
    );
}

fn on_buildpack_checks_error(error: ChecksError) {
    match error {
        ChecksError::ForbiddenEnvVar(name) => log_error(
            "Unsafe environment variable found",
            formatdoc! {"
                The environment variable '{name}' is set, however, it can
                cause problems with the build so we do not allow using it.

                You must unset that environment variable. If you didn't set it
                yourself, check that it wasn't set by an earlier buildpack.
            "},
        ),
    };
}

fn on_determine_package_manager_error(error: DeterminePackageManagerError) {
    match error {
        DeterminePackageManagerError::CheckFileExists(io_error) => log_io_error(
            "Unable to determine the package manager",
            "determining which Python package manager to use for this project",
            &io_error,
        ),
        DeterminePackageManagerError::MultipleFound(package_managers) => {
            let files_found = package_managers
                .into_iter()
                .map(|package_manager| {
                    format!(
                        "{} ({})",
                        package_manager.packages_file(),
                        package_manager.name()
                    )
                })
                .collect::<Vec<String>>()
                .join("\n");
            log_error(
                "Multiple Python package manager files were found",
                formatdoc! {"
                    Exactly one package manager file must be present in your app's source code,
                    however, several were found:
                    
                    {files_found}
                    
                    Decide which package manager you want to use with your app, and then delete
                    the file(s) and any config from the others.
                "},
            );
        }
        DeterminePackageManagerError::NoneFound => log_error(
            "Couldn't find any supported Python package manager files",
            indoc! {"
                Your app must have either a pip requirements file ('requirements.txt')
                or Poetry lockfile ('poetry.lock') in the root directory of its source
                code, so your app's dependencies can be installed.
                
                If your app already has one of those files, check that it:
                
                1. Is in the top level directory (not a subdirectory).
                2. Has the correct spelling (the filenames are case-sensitive).
                3. Isn't excluded by '.gitignore' or 'project.toml'.
                
                Otherwise, add a package manager file to your app. If your app has
                no dependencies, then create an empty 'requirements.txt' file.
            "},
        ),
    };
}

fn on_requested_python_version_error(error: RequestedPythonVersionError) {
    match error {
        RequestedPythonVersionError::ReadPythonVersionFile(io_error) => log_io_error(
            "Unable to read .python-version",
            "reading the .python-version file",
            &io_error,
        ),
        RequestedPythonVersionError::ReadRuntimeTxt(io_error) => log_io_error(
            "Unable to read runtime.txt",
            "reading the runtime.txt file",
            &io_error,
        ),
        RequestedPythonVersionError::ParsePythonVersionFile(error) => match error {
            ParsePythonVersionFileError::InvalidVersion(version) => log_error(
                "Invalid Python version in .python-version",
                formatdoc! {"
                    The Python version specified in '.python-version' is not in the correct format.
                    
                    The following version was found:
                    {version}
                    
                    However, the version must be specified as either:
                    1. '<major>.<minor>' (recommended, for automatic security updates)
                    2. '<major>.<minor>.<patch>' (to pin to an exact Python version)
                    
                    Do not include quotes or a 'python-' prefix. To include comments, add them
                    on their own line, prefixed with '#'.
                    
                    For example, to request the latest version of Python {DEFAULT_PYTHON_VERSION},
                    update the '.python-version' file so it contains:
                    {DEFAULT_PYTHON_VERSION}
                "},
            ),
            ParsePythonVersionFileError::MultipleVersions(versions) => {
                let version_list = versions.join("\n");
                log_error(
                    "Invalid Python version in .python-version",
                    formatdoc! {"
                        Multiple Python versions were found in '.python-version':
                        
                        {version_list}
                        
                        Update the file so it contains only one Python version.
                        
                        If the additional versions are actually comments, prefix those lines with '#'.
                    "},
                );
            }
            ParsePythonVersionFileError::NoVersion => log_error(
                "Invalid Python version in .python-version",
                formatdoc! {"
                    No Python version was found in the '.python-version' file.
                    
                    Update the file so that it contain a valid Python version (such as '{DEFAULT_PYTHON_VERSION}'),
                    or else delete the file to use the default version (currently Python {DEFAULT_PYTHON_VERSION}).

                    If the file already contains a version, check the line is not prefixed by
                    a '#', since otherwise it will be treated as a comment.
                "},
            ),
        },
        RequestedPythonVersionError::ParseRuntimeTxt(ParseRuntimeTxtError { cleaned_contents }) => {
            log_error(
                "Invalid Python version in runtime.txt",
                formatdoc! {"
                    The Python version specified in 'runtime.txt' is not in the correct format.
                    
                    The following file contents were found:
                    {cleaned_contents}
                    
                    However, the file contents must begin with a 'python-' prefix, followed by the
                    version specified as '<major>.<minor>.<patch>'. Comments are not supported.
                    
                    For example, to request Python {DEFAULT_PYTHON_FULL_VERSION}, update the 'runtime.txt' file so it
                    contains exactly:
                    python-{DEFAULT_PYTHON_FULL_VERSION}
                "},
            );
        }
    };
}

fn on_resolve_python_version_error(error: ResolvePythonVersionError) {
    match error {
        ResolvePythonVersionError::EolVersion(requested_python_version) => {
            let RequestedPythonVersion {
                major,
                minor,
                origin,
                ..
            } = requested_python_version;
            log_error(
                "Requested Python version has reached end-of-life",
                formatdoc! {"
                    The requested Python version {major}.{minor} has reached its upstream end-of-life,
                    and is therefore no longer receiving security updates:
                    https://devguide.python.org/versions/#supported-versions
                    
                    As such, it is no longer supported by this buildpack.
                    
                    Please upgrade to a newer Python version by updating the version
                    configured via the {origin} file.
                    
                    If possible, we recommend upgrading all the way to Python {DEFAULT_PYTHON_VERSION},
                    since it contains many performance and usability improvements.
                "},
            );
        }
        ResolvePythonVersionError::UnknownVersion(requested_python_version) => {
            let RequestedPythonVersion {
                major,
                minor,
                origin,
                ..
            } = requested_python_version;
            log_error(
                "Requested Python version is not recognised",
                formatdoc! {"
                    The requested Python version {major}.{minor} is not recognised.
                    
                    Check that this Python version has been officially released:
                    https://devguide.python.org/versions/#supported-versions
                    
                    If it has, make sure that you are using the latest version of this buildpack.
                    
                    If it has not, please switch to a supported version (such as Python {DEFAULT_PYTHON_VERSION})
                    by updating the version configured via the {origin} file.
                "},
            );
        }
    }
}

fn on_python_layer_error(error: PythonLayerError) {
    match error {
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
        // This error will change once the Python version is validated against a manifest.
        // TODO: (W-12613425) Write the supported Python versions inline, instead of linking out to Dev Center.
        // TODO: Decide how to explain to users how stacks, base images and builder images versions relate to each other.
        PythonLayerError::PythonArchiveNotFound { python_version } => log_error(
            "Requested Python version is not available",
            formatdoc! {"
                The requested Python version ({python_version}) is not available for this builder image.
                
                Please switch to a supported Python version, or else don't specify a version
                and the buildpack will use a default version (currently Python {DEFAULT_PYTHON_VERSION}).
                
                For a list of the supported Python versions, see:
                https://devcenter.heroku.com/articles/python-support#supported-runtimes
            "},
        ),
    };
}

fn on_pip_layer_error(error: PipLayerError) {
    match error {
        PipLayerError::InstallPipCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to install pip",
                "running 'python' to install pip",
                &io_error,
            ),
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to install pip",
                formatdoc! {"
                    The command to install pip did not exit successfully ({exit_status}).
                    
                    See the log output above for more information.
                    
                    In some cases, this happens due to an unstable network connection.
                    Please try again to see if the error resolves itself.
                    
                    If that does not help, check the status of PyPI (the upstream Python
                    package repository service), here:
                    https://status.python.org
                "},
            ),
        },
        PipLayerError::LocateBundledPip(io_error) => log_io_error(
            "Unable to locate the bundled copy of pip",
            "locating the pip wheel file bundled inside the Python 'ensurepip' module",
            &io_error,
        ),
    };
}

fn on_pip_dependencies_layer_error(error: PipDependenciesLayerError) {
    match error {
        PipDependenciesLayerError::CreateVenvCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to create virtual environment",
                "running 'python -m venv' to create a virtual environment",
                &io_error,
            ),
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to create virtual environment",
                formatdoc! {"
                    The 'python -m venv' command to create a virtual environment did
                    not exit successfully ({exit_status}).
                    
                    See the log output above for more information.
                "},
            ),
        },
        PipDependenciesLayerError::PipInstallCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to install dependencies using pip",
                "running 'pip install' to install the app's dependencies",
                &io_error,
            ),
            // TODO: Add more suggestions here as to causes (eg network, invalid requirements.txt,
            // package broken or not compatible with version of Python, missing system dependencies etc)
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to install dependencies using pip",
                formatdoc! {"
                    The 'pip install -r requirements.txt' command to install the app's
                    dependencies failed ({exit_status}).
                    
                    See the log output above for more information.
                "},
            ),
        },
    };
}

fn on_poetry_layer_error(error: PoetryLayerError) {
    match error {
        PoetryLayerError::InstallPoetryCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to install Poetry",
                "running 'python' to install Poetry",
                &io_error,
            ),
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to install Poetry",
                formatdoc! {"
                    The command to install Poetry did not exit successfully ({exit_status}).
                    
                    See the log output above for more information.
                    
                    In some cases, this happens due to an unstable network connection.
                    Please try again to see if the error resolves itself.
                    
                    If that does not help, check the status of PyPI (the upstream Python
                    package repository service), here:
                    https://status.python.org
                "},
            ),
        },
        PoetryLayerError::LocateBundledPip(io_error) => log_io_error(
            "Unable to locate the bundled copy of pip",
            "locating the pip wheel file bundled inside the Python 'ensurepip' module",
            &io_error,
        ),
    };
}

fn on_poetry_dependencies_layer_error(error: PoetryDependenciesLayerError) {
    match error {
        PoetryDependenciesLayerError::CreateVenvCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to create virtual environment",
                "running 'python -m venv' to create a virtual environment",
                &io_error,
            ),
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to create virtual environment",
                formatdoc! {"
                    The 'python -m venv' command to create a virtual environment did
                    not exit successfully ({exit_status}).
                    
                    See the log output above for more information.
                "},
            ),
        },
        PoetryDependenciesLayerError::PoetryInstallCommand(error) => match error {
            StreamedCommandError::Io(io_error) => log_io_error(
                "Unable to install dependencies using Poetry",
                "running 'poetry install' to install the app's dependencies",
                &io_error,
            ),
            // TODO: Add more suggestions here as to possible causes (similar to pip)
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to install dependencies using Poetry",
                formatdoc! {"
                    The 'poetry install --sync --only main' command to install the app's
                    dependencies failed ({exit_status}).
                    
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
