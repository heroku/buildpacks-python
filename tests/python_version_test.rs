use crate::python_version::{
    DEFAULT_PYTHON_FULL_VERSION, DEFAULT_PYTHON_VERSION, LATEST_PYTHON_3_9, LATEST_PYTHON_3_10,
    LATEST_PYTHON_3_11, LATEST_PYTHON_3_12, LATEST_PYTHON_3_13,
    NEWEST_SUPPORTED_PYTHON_3_MINOR_VERSION, PythonVersion,
};
use crate::tests::default_build_config;
use indoc::{formatdoc, indoc};
use libcnb_test::{PackResult, TestRunner, assert_contains, assert_empty};

#[test]
#[ignore = "integration test"]
fn python_version_unspecified() {
    let config = default_build_config("tests/fixtures/python_version_unspecified");

    TestRunner::default().build(config, |context| {
        assert_empty!(context.pack_stderr);
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Determining Python version]
                No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                We recommend setting an explicit version. In the root of your app create
                a '.python-version' file, containing a Python version like '{DEFAULT_PYTHON_VERSION}'.
                
                [Installing Python]
                Installing Python {DEFAULT_PYTHON_FULL_VERSION}
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_3_9() {
    builds_with_python_version("tests/fixtures/python_3.9", &LATEST_PYTHON_3_9);
}

#[test]
#[ignore = "integration test"]
fn python_3_10() {
    builds_with_python_version("tests/fixtures/python_3.10", &LATEST_PYTHON_3_10);
}

#[test]
#[ignore = "integration test"]
fn python_3_11() {
    builds_with_python_version("tests/fixtures/python_3.11", &LATEST_PYTHON_3_11);
}

#[test]
#[ignore = "integration test"]
fn python_3_12() {
    builds_with_python_version("tests/fixtures/python_3.12", &LATEST_PYTHON_3_12);
}

#[test]
#[ignore = "integration test"]
fn python_3_13() {
    builds_with_python_version("tests/fixtures/python_3.13", &LATEST_PYTHON_3_13);
}

fn builds_with_python_version(fixture_path: &str, python_version: &PythonVersion) {
    let &PythonVersion {
        major,
        minor,
        patch,
    } = python_version;

    TestRunner::default().build(default_build_config(fixture_path), |context| {
        if major == 3 && minor == 9 {
            assert_eq!(
                context.pack_stderr,
                indoc! {"
                    
                    [Warning: Support for Python 3.9 is deprecated]
                    Python 3.9 will reach its upstream end-of-life in October 2025,
                    at which point it will no longer receive security updates:
                    https://devguide.python.org/versions/#supported-versions
                    
                    As such, support for Python 3.9 will be removed from this
                    buildpack on 7th January 2026.
                    
                    Upgrade to a newer Python version as soon as possible, by
                    changing the version in your .python-version file.
                    
                    For more information, see:
                    https://devcenter.heroku.com/articles/python-support#supported-python-versions
                    
                "}
            );
        } else {
            assert_empty!(context.pack_stderr);
        }

        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Determining Python version]
                Using Python version {major}.{minor} specified in .python-version
                
                [Installing Python]
                Installing Python {major}.{minor}.{patch}
            "}
        );

        // There's no sensible default process type we can set for Python apps.
        assert_contains!(context.pack_stdout, "no default process type");

        // Validate that the Python install works as expected at run-time.
        let command_output = context.run_shell_command(
            indoc! {r#"
                set -euo pipefail
                
                # Check that we installed the correct Python version, and that the command
                # 'python' works (since it's a symlink to the actual 'python3' binary).
                python --version
                
                # Check that the Python binary is using its own 'libpython' and not the system one:
                # https://github.com/docker-library/python/issues/784
                # Note: This has to handle Python 3.9 and older not being built in shared library mode.
                libpython_path=$(ldd /layers/heroku_python/python/bin/python | grep libpython || true)
                if [[ -n "${libpython_path}" && "${libpython_path}" != *"=> /layers/"* ]]; then
                  echo "The Python binary is not using the correct libpython!"
                  echo "${libpython_path}"
                  exit 1
                fi
                
                # Check all required dynamically linked libraries can be found in the run image.
                ldd_output=$(find /layers -type f,l \( -name 'python3' -o -name '*.so*' \) -exec ldd '{}' +)
                if grep 'not found' <<<"${ldd_output}" | sort --unique; then
                  echo "The above dynamically linked libraries were not found!"
                  exit 1
                fi
            "#}
        );
        assert_empty!(command_output.stderr);
        assert_eq!(
            command_output.stdout,
            format!("Python {major}.{minor}.{patch}\n")
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_version_file_io_error() {
    let mut config = default_build_config("tests/fixtures/python_version_file_invalid_unicode");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                [Error: Unable to read .python-version]
                An unexpected error occurred whilst reading the .python-version file.
                
                Details: I/O Error: stream did not contain valid UTF-8
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_version_file_invalid_version() {
    let mut config = default_build_config("tests/fixtures/python_version_file_invalid_version");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: Invalid Python version in .python-version]
                The Python version specified in your .python-version file
                isn't in the correct format.
                
                The following version was found:
                3.12.0invalid
                
                However, the Python version must be specified as either:
                1. The major version only: 3.X  (recommended)
                2. An exact patch version: 3.X.Y
                
                Don't include quotes or a 'python-' prefix. To include
                comments, add them on their own line, prefixed with '#'.
                
                For example, to request the latest version of Python {DEFAULT_PYTHON_VERSION},
                update your .python-version file so it contains:
                {DEFAULT_PYTHON_VERSION}

                We strongly recommend that you use the major version form
                instead of pinning to an exact version, since it will allow
                your app to receive Python security updates.
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_version_file_multiple_versions() {
    let mut config = default_build_config("tests/fixtures/python_version_file_multiple_versions");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                [Error: Invalid Python version in .python-version]
                Multiple versions were found in your .python-version file:
                
                // invalid comment
                3.12
                2.7
                
                Update the file so it contains only one Python version.
                
                If you have added comments to the file, make sure that those
                lines begin with a '#', so that they are ignored.
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_version_file_no_version() {
    let mut config = default_build_config("tests/fixtures/python_version_file_no_version");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: Invalid Python version in .python-version]
                No Python version was found in your .python-version file.
                
                Update the file so that it contains a valid Python version.
                
                For example, to request the latest version of Python {DEFAULT_PYTHON_VERSION},
                update your .python-version file so it contains:
                {DEFAULT_PYTHON_VERSION}
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_version_eol() {
    let mut config = default_build_config("tests/fixtures/python_version_eol");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: The requested Python version has reached end-of-life]
                Python 3.8 has reached its upstream end-of-life, and is
                therefore no longer receiving security updates:
                https://devguide.python.org/versions/#supported-versions

                As such, it's no longer supported by this buildpack:
                https://devcenter.heroku.com/articles/python-support#supported-python-versions

                Please upgrade to at least Python 3.9 by changing the
                version in your .python-version file.

                If possible, we recommend upgrading all the way to Python {DEFAULT_PYTHON_VERSION},
                since it contains many performance and usability improvements.
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_version_non_existent_major() {
    let mut config = default_build_config("tests/fixtures/python_version_non_existent_major");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: The requested Python version isn't recognised]
                The requested Python version 3.99 isn't recognised.

                Check that this Python version has been officially released,
                and that the Python buildpack has added support for it:
                https://devguide.python.org/versions/#supported-versions
                https://devcenter.heroku.com/articles/python-support#supported-python-versions

                If it has, make sure that you are using the latest version
                of this buildpack, and haven't pinned to an older release
                via a custom buildpack configuration in project.toml.

                Otherwise, switch to a supported version (such as Python 3.{NEWEST_SUPPORTED_PYTHON_3_MINOR_VERSION})
                by changing the version in your .python-version file.
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_version_non_existent_minor() {
    let mut config = default_build_config("tests/fixtures/python_version_non_existent_minor");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: The requested Python version wasn't found]
                The requested Python version (3.12.999) wasn't found.
                
                Please switch to a supported Python version, or else don't specify a version
                and the buildpack will use a default version (currently Python {DEFAULT_PYTHON_VERSION}).
                
                For a list of the supported Python versions, see:
                https://devcenter.heroku.com/articles/python-support#supported-runtimes
            "}
        );
    });
}

// This tests that:
// - The Python version can be specified using runtime.txt.
// - A runtime.txt deprecation warning is shown.
// - pip works with the oldest currently supported Python version (3.9.0).
// - The Python 3.9 deprecation warning correctly lists the origin as runtime.txt.
#[test]
#[ignore = "integration test"]
fn runtime_txt() {
    let config = default_build_config("tests/fixtures/runtime_txt_and_python_version_file");

    TestRunner::default().build(config, |context| {
        assert_eq!(
            context.pack_stderr,
            indoc! {"

                [Warning: The runtime.txt file is deprecated]
                The runtime.txt file is deprecated since it has been replaced
                by the more widely supported .python-version file.
                
                Please delete your runtime.txt file and create a new file named:
                .python-version
                
                Make sure to include the '.' at the start of the filename.
                
                In the new file, specify your app's Python version without
                quotes or a 'python-' prefix. For example:
                3.9
                
                We strongly recommend that you use the major version form
                instead of pinning to an exact version, since it will allow
                your app to receive Python security updates.
                
                In the near future support for runtime.txt will be removed
                and this warning will be made an error.
                
                
                [Warning: Support for Python 3.9 is deprecated]
                Python 3.9 will reach its upstream end-of-life in October 2025,
                at which point it will no longer receive security updates:
                https://devguide.python.org/versions/#supported-versions
                
                As such, support for Python 3.9 will be removed from this
                buildpack on 7th January 2026.
                
                Upgrade to a newer Python version as soon as possible, by
                changing the version in your runtime.txt file.
                
                For more information, see:
                https://devcenter.heroku.com/articles/python-support#supported-python-versions
                
            "}
        );
        assert_contains!(
            context.pack_stdout,
            indoc! {"
                [Determining Python version]
                Using Python version 3.9.0 specified in runtime.txt
                
                [Installing Python]
                Installing Python 3.9.0
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn runtime_txt_io_error() {
    let mut config = default_build_config("tests/fixtures/runtime_txt_invalid_unicode");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                [Error: Unable to read runtime.txt]
                An unexpected error occurred whilst reading the runtime.txt file.
                
                Details: I/O Error: stream did not contain valid UTF-8
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn runtime_txt_invalid_version() {
    let mut config = default_build_config("tests/fixtures/runtime_txt_invalid_version");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: Invalid Python version in runtime.txt]
                The Python version specified in your runtime.txt file isn't
                in the correct format.
                
                The following file contents were found, which aren't valid:
                python-an.invalid.version
                
                However, the runtime.txt file is deprecated since it has
                been replaced by the .python-version file. As such, we
                recommend that you switch to using a .python-version file
                instead of fixing your runtime.txt file.
                
                Please delete your runtime.txt file and create a new file named:
                .python-version
                
                Make sure to include the '.' at the start of the filename.
                
                In the new file, specify your app's Python version without
                quotes or a 'python-' prefix. For example:
                {DEFAULT_PYTHON_VERSION}
                
                We strongly recommend that you use the major version form
                instead of pinning to an exact version, since it will allow
                your app to receive Python security updates.
            "}
        );
    });
}
