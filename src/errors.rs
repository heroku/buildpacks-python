use crate::BuildpackError;
use crate::checks::ChecksError;
use crate::django::DjangoCollectstaticError;
use crate::layers::pip::PipLayerError;
use crate::layers::pip_dependencies::PipDependenciesLayerError;
use crate::layers::poetry::PoetryLayerError;
use crate::layers::poetry_dependencies::PoetryDependenciesLayerError;
use crate::layers::python::PythonLayerError;
use crate::package_manager::DeterminePackageManagerError;
use crate::python_version::{
    DEFAULT_PYTHON_VERSION, NEWEST_SUPPORTED_PYTHON_3_MINOR_VERSION,
    OLDEST_SUPPORTED_PYTHON_3_MINOR_VERSION, RequestedPythonVersion, RequestedPythonVersionError,
    ResolvePythonVersionError,
};
use crate::python_version_file::ParsePythonVersionFileError;
use crate::utils::{
    CapturedCommandError, CommandIoError, DownloadUnpackArchiveError, FileExistsError,
    FindBundledPipError, ReadOptionalFileError, StreamedCommandError,
};
use indoc::{formatdoc, indoc};
use libherokubuildpack::log::log_error;

/// Handle any non-recoverable buildpack or libcnb errors that occur.
///
/// The buildpack will exit non-zero after this handler has run, so all that needs to be
/// performed here is the logging of an error message - and in the future, emitting metrics.
pub(crate) fn on_error(error: libcnb::Error<BuildpackError>) {
    match error {
        libcnb::Error::BuildpackError(buildpack_error) => on_buildpack_error(buildpack_error),
        libcnb_error => log_error(
            "Internal buildpack error",
            formatdoc! {"
                An error was reported by the framework used by this buildpack.

                Details: {libcnb_error}

                {INTERNAL_ERROR_MESSAGE}
            "},
        ),
    }
}

fn on_buildpack_error(error: BuildpackError) {
    match error {
        BuildpackError::BuildpackDetection(error) | BuildpackError::DjangoDetection(error) => {
            log_file_exists_error(error);
        }
        BuildpackError::Checks(error) => on_buildpack_checks_error(error),
        BuildpackError::DeterminePackageManager(error) => on_determine_package_manager_error(error),
        BuildpackError::DjangoCollectstatic(error) => on_django_collectstatic_error(error),
        BuildpackError::PipDependenciesLayer(error) => on_pip_dependencies_layer_error(error),
        BuildpackError::PipLayer(error) => on_pip_layer_error(error),
        BuildpackError::PoetryDependenciesLayer(error) => on_poetry_dependencies_layer_error(error),
        BuildpackError::PoetryLayer(error) => on_poetry_layer_error(error),
        BuildpackError::PythonLayer(error) => on_python_layer_error(error),
        BuildpackError::RequestedPythonVersion(error) => on_requested_python_version_error(error),
        BuildpackError::ResolvePythonVersion(error) => on_resolve_python_version_error(error),
    }
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
    }
}

fn on_determine_package_manager_error(error: DeterminePackageManagerError) {
    match error {
        DeterminePackageManagerError::CheckFileExists(error) => log_file_exists_error(error),
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
    }
}

fn on_requested_python_version_error(error: RequestedPythonVersionError) {
    match error {
        RequestedPythonVersionError::CheckRuntimeTxtExists(error) => log_file_exists_error(error),
        RequestedPythonVersionError::ReadPythonVersionFile(error) => log_read_file_error(error),
        RequestedPythonVersionError::ParsePythonVersionFile(error) => match error {
            ParsePythonVersionFileError::InvalidVersion(version) => log_error(
                "Invalid Python version in .python-version",
                formatdoc! {"
                    The Python version specified in your .python-version file
                    isn't in the correct format.
                    
                    The following version was found:
                    {version}
                    
                    However, the Python version must be specified as either:
                    1. The major version only, for example: {DEFAULT_PYTHON_VERSION} (recommended)
                    2. An exact patch version, for example: {DEFAULT_PYTHON_VERSION}.999
                    
                    Don't include quotes, a 'python-' prefix or wildcards. Any
                    code comments must be on a separate line prefixed with '#'.
                    
                    For example, to request the latest version of Python {DEFAULT_PYTHON_VERSION},
                    update your .python-version file so it contains exactly:
                    {DEFAULT_PYTHON_VERSION}
                    
                    We strongly recommend that you don't specify the Python patch
                    version number, since it will pin your app to an exact Python
                    version and so stop your app from receiving security updates
                    each time it builds.
                "},
            ),
            ParsePythonVersionFileError::MultipleVersions(versions) => {
                let version_list = versions.join("\n");
                log_error(
                    "Invalid Python version in .python-version",
                    formatdoc! {"
                        Multiple versions were found in your .python-version file:
                        
                        {version_list}
                        
                        Update the file so it contains only one Python version.
                        
                        For example, to request the latest version of Python {DEFAULT_PYTHON_VERSION},
                        update your .python-version file so it contains exactly:
                        {DEFAULT_PYTHON_VERSION}
                        
                        If you have added comments to the file, make sure that those
                        lines begin with a '#', so that they are ignored.
                    "},
                );
            }
            ParsePythonVersionFileError::NoVersion => log_error(
                "Invalid Python version in .python-version",
                formatdoc! {"
                    No Python version was found in your .python-version file.
                    
                    Update the file so that it contains your app's major Python
                    version number. Don't include quotes or a 'python-' prefix.
                    
                    For example, to request the latest version of Python {DEFAULT_PYTHON_VERSION},
                    update your .python-version file so it contains exactly:
                    {DEFAULT_PYTHON_VERSION}
                    
                    If the file already contains a version, check the line doesn't
                    begin with a '#', otherwise it will be treated as a comment.
                "},
            ),
        },
        RequestedPythonVersionError::RuntimeTxtNotSupported => log_error(
            "The runtime.txt file isn't supported",
            formatdoc! {"
                The runtime.txt file can longer be used, since it has been
                replaced by the more widely supported .python-version file.
                
                Please delete your runtime.txt file and create a new file named:
                .python-version
                
                Make sure to include the '.' character at the start of the
                filename. Don't add a file extension such as '.txt'.
                
                In the new file, specify your app's major Python version number
                only. Don't include quotes or a 'python-' prefix.
                
                For example, to request the latest version of Python {DEFAULT_PYTHON_VERSION},
                update your .python-version file so it contains exactly:
                {DEFAULT_PYTHON_VERSION}
                
                We strongly recommend that you don't specify the Python patch
                version number, since it will pin your app to an exact Python
                version and so stop your app from receiving security updates
                each time it builds.
            "},
        ),
    }
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
                "The requested Python version has reached end-of-life",
                formatdoc! {"
                    Python {major}.{minor} has reached its upstream end-of-life, and is
                    therefore no longer receiving security updates:
                    https://devguide.python.org/versions/#supported-versions
                    
                    As such, it's no longer supported by this buildpack:
                    https://devcenter.heroku.com/articles/python-support#supported-python-versions
                    
                    Please upgrade to at least Python 3.{OLDEST_SUPPORTED_PYTHON_3_MINOR_VERSION} by changing the
                    version in your {origin} file.
                    
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
                "The requested Python version isn't recognised",
                formatdoc! {"
                    The requested Python version {major}.{minor} isn't recognised.
                    
                    Check that this Python version has been officially released,
                    and that the Python buildpack has added support for it:
                    https://devguide.python.org/versions/#supported-versions
                    https://devcenter.heroku.com/articles/python-support#supported-python-versions
                    
                    If it has, make sure that you are using the latest version
                    of this buildpack, and haven't pinned to an older release
                    via a custom buildpack configuration in project.toml.
                    
                    Otherwise, switch to a supported version (such as Python 3.{NEWEST_SUPPORTED_PYTHON_3_MINOR_VERSION})
                    by changing the version in your {origin} file.
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
                    An error occurred while downloading the Python runtime archive.
                    
                    In some cases, this happens due to a temporary issue with
                    the network connection or server.
                    
                    First, make sure that you are using the latest version
                    of this buildpack, and haven't pinned to an older release
                    via a custom buildpack configuration in project.toml.
                    
                    Then try building again to see if the error resolves itself.
                    
                    Details: {ureq_error}
                "},
            ),
            DownloadUnpackArchiveError::Unpack(io_error) => log_error(
                "Unable to unpack the Python archive",
                // TODO: Investigate under what circumstances this error can occur, and so whether
                // we should label this as an internal error or else list suggested actions.
                formatdoc! {"
                    An I/O error occurred while unpacking the downloaded Python
                    runtime archive and writing it to disk.
                    
                    Details: I/O Error: {io_error}
                "},
            ),
        },
        // TODO: Remove this once versions are validated against a manifest (at which point all
        // HTTP 403s/404s can be treated as an internal error).
        PythonLayerError::PythonArchiveNotAvailable(requested_python_version) => {
            let RequestedPythonVersion {
                major,
                minor,
                origin,
                ..
            } = &requested_python_version;
            log_error(
                "The requested Python version isn't available",
                formatdoc! {"
                    Your app's {origin} file specifies a Python version
                    of {requested_python_version}, however, we couldn't find that version on S3.
                    
                    Check that this Python version has been released upstream,
                    and that the Python buildpack has added support for it:
                    https://www.python.org/downloads/
                    https://github.com/heroku/buildpacks-python/blob/main/CHANGELOG.md
                    
                    If it has, make sure that you are using the latest version
                    of this buildpack, and haven't pinned to an older release
                    via a custom buildpack configuration in project.toml.
                    
                    We also strongly recommend that you do not pin your app to an
                    exact Python version such as {requested_python_version}, and instead only specify
                    the major Python version of {major}.{minor} in your {origin} file.
                    This will allow your app to receive the latest available Python
                    patch version automatically, and prevent this type of error.
                "},
            );
        }
    }
}

fn on_pip_layer_error(error: PipLayerError) {
    match error {
        PipLayerError::InstallPipCommand(error) => match error {
            StreamedCommandError::Io(error) => log_command_io_error(error),
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
        PipLayerError::LocateBundledPip(error) => log_find_bundled_pip_error(error),
    }
}

fn on_pip_dependencies_layer_error(error: PipDependenciesLayerError) {
    match error {
        PipDependenciesLayerError::CreateVenvCommand(error) => match error {
            StreamedCommandError::Io(error) => log_command_io_error(error),
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
            StreamedCommandError::Io(error) => log_command_io_error(error),
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
    }
}

fn on_poetry_layer_error(error: PoetryLayerError) {
    match error {
        PoetryLayerError::InstallPoetryCommand(error) => match error {
            StreamedCommandError::Io(error) => log_command_io_error(error),
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
        PoetryLayerError::LocateBundledPip(error) => log_find_bundled_pip_error(error),
    }
}

fn on_poetry_dependencies_layer_error(error: PoetryDependenciesLayerError) {
    match error {
        PoetryDependenciesLayerError::CreateVenvCommand(error) => match error {
            StreamedCommandError::Io(error) => log_command_io_error(error),
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
            StreamedCommandError::Io(error) => log_command_io_error(error),
            // TODO: Add more suggestions here as to possible causes (similar to pip)
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to install dependencies using Poetry",
                formatdoc! {"
                    The 'poetry sync --only main' command to install the app's
                    dependencies failed ({exit_status}).
                    
                    See the log output above for more information.
                "},
            ),
        },
    }
}

fn on_django_collectstatic_error(error: DjangoCollectstaticError) {
    match error {
        DjangoCollectstaticError::CheckCollectstaticCommandExists(error) => match error {
            CapturedCommandError::Io(error) => log_command_io_error(error),
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
        DjangoCollectstaticError::CheckManagementScriptExists(error) => {
            log_file_exists_error(error);
        }
        DjangoCollectstaticError::CollectstaticCommand(error) => match error {
            StreamedCommandError::Io(error) => log_command_io_error(error),
            StreamedCommandError::NonZeroExitStatus(exit_status) => log_error(
                "Unable to generate Django static files",
                formatdoc! {"
                    The 'python manage.py collectstatic --noinput' Django management
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
    }
}

fn log_file_exists_error(FileExistsError { path, io_error }: FileExistsError) {
    let filepath = path.to_string_lossy();
    let filename = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();

    log_error(
        format!("Unable to check if {filename} exists"),
        formatdoc! {"
            An I/O error occurred while checking if this file exists:
            {filepath}

            Details: {io_error}

            {INTERNAL_ERROR_MESSAGE}
        "},
    );
}

fn log_read_file_error(ReadOptionalFileError { path, io_error }: ReadOptionalFileError) {
    let filepath = path.to_string_lossy();
    let filename = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();

    log_error(
        format!("Unable to read {filename}"),
        formatdoc! {"
            An I/O error occurred while reading the file:
            {filepath}

            Details: {io_error}

            Check the file's permissions and that it contains valid UTF-8.

            Then try building again.
        "},
    );
}

fn log_find_bundled_pip_error(
    FindBundledPipError {
        bundled_wheels_dir,
        io_error,
    }: FindBundledPipError,
) {
    let bundled_wheels_dir = bundled_wheels_dir.to_string_lossy();

    log_error(
        "Unable to locate the Python stdlib's bundled pip",
        formatdoc! {"
            Couldn't find the pip wheel file bundled inside the Python
            stdlib's `ensurepip` module, at:
            {bundled_wheels_dir}

            Details: {io_error}

            {INTERNAL_ERROR_MESSAGE}
        "
        },
    );
}

fn log_command_io_error(CommandIoError { program, io_error }: CommandIoError) {
    log_error(
        format!("Unable to run {program}"),
        formatdoc! {"
            An I/O error occurred while trying to run:
            `{program}`

            Details: {io_error}

            {INTERNAL_ERROR_MESSAGE}
        "},
    );
}

const INTERNAL_ERROR_MESSAGE: &str = indoc! {"
    This is an unexpected error that could be caused by a bug
    in this buildpack, or an issue with the build environment.

    Try building again to see if the error resolves itself.

    If it doesn't, please file a bug report here:
    https://github.com/heroku/buildpacks-python/issues
"};
