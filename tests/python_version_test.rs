use crate::python_version::{
    PythonVersion, DEFAULT_PYTHON_FULL_VERSION, DEFAULT_PYTHON_VERSION, LATEST_PYTHON_3_10,
    LATEST_PYTHON_3_11, LATEST_PYTHON_3_12, LATEST_PYTHON_3_8, LATEST_PYTHON_3_9,
};
use crate::tests::{builder, default_build_config};
use indoc::{formatdoc, indoc};
use libcnb_test::{assert_contains, assert_empty, PackResult, TestRunner};

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
fn python_3_7() {
    let mut config = default_build_config("tests/fixtures/python_3.7");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: Requested Python version has reached end-of-life]
                The requested Python version 3.7 has reached its upstream end-of-life,
                and is therefore no longer receiving security updates:
                https://devguide.python.org/versions/#supported-versions
                
                As such, it is no longer supported by this buildpack.
                
                Please upgrade to a newer Python version by updating the version
                configured via the .python-version file.
                
                If possible, we recommend upgrading all the way to Python {DEFAULT_PYTHON_VERSION},
                since it contains many performance and usability improvements.
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_3_8() {
    // Python 3.8 is only available on Heroku-20 and older.
    let fixture = "tests/fixtures/python_3.8";
    match builder().as_str() {
        "heroku/builder:20" => builds_with_python_version(fixture, &LATEST_PYTHON_3_8),
        _ => rejects_non_existent_python_version(fixture, &LATEST_PYTHON_3_8),
    };
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

fn builds_with_python_version(fixture_path: &str, python_version: &PythonVersion) {
    let PythonVersion {
        major,
        minor,
        patch,
    } = python_version;

    TestRunner::default().build(default_build_config(fixture_path), |context| {
        assert_empty!(context.pack_stderr);
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

fn rejects_non_existent_python_version(fixture_path: &str, python_version: &PythonVersion) {
    let mut config = default_build_config(fixture_path);
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: Requested Python version is not available]
                The requested Python version ({python_version}) is not available for this builder image.
                
                Please switch to a supported Python version, or else don't specify a version
                and the buildpack will use a default version (currently Python {DEFAULT_PYTHON_VERSION}).
                
                For a list of the supported Python versions, see:
                https://devcenter.heroku.com/articles/python-support#supported-runtimes
            "}
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
                The Python version specified in '.python-version' is not in the correct format.
                
                The following version was found:
                an.invalid.version
                
                However, the version must be specified as either:
                1. '<major>.<minor>' (recommended, for automatic security updates)
                2. '<major>.<minor>.<patch>' (to pin to an exact Python version)
                
                Do not include quotes or a 'python-' prefix. To include comments, add them
                on their own line, prefixed with '#'.
                
                For example, to request the latest version of Python {DEFAULT_PYTHON_VERSION},
                update the '.python-version' file so it contains:
                {DEFAULT_PYTHON_VERSION}
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
                Multiple Python versions were found in '.python-version':
                
                // invalid comment
                3.12
                2.7
                
                Update the file so it contains only one Python version.
                
                If the additional versions are actually comments, prefix those lines with '#'.
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
                No Python version was found in the '.python-version' file.
                
                Update the file so that it contain a valid Python version (such as '{DEFAULT_PYTHON_VERSION}'),
                or else delete the file to use the default version (currently Python {DEFAULT_PYTHON_VERSION}).
                
                If the file already contains a version, check the line is not prefixed by
                a '#', since otherwise it will be treated as a comment.
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn python_version_file_unknown_version() {
    let mut config = default_build_config("tests/fixtures/python_version_file_unknown_version");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stderr,
            &formatdoc! {"
                [Error: Requested Python version is not recognised]
                The requested Python version 3.99 is not recognised.
                
                Check that this Python version has been officially released:
                https://devguide.python.org/versions/#supported-versions
                
                If it has, make sure that you are using the latest version of this buildpack.
                
                If it has not, please switch to a supported version (such as Python {DEFAULT_PYTHON_VERSION})
                by updating the version configured via the .python-version file.
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn runtime_txt() {
    let config = default_build_config("tests/fixtures/runtime_txt_and_python_version_file");

    TestRunner::default().build(config, |context| {
        assert_empty!(context.pack_stderr);
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
                The Python version specified in 'runtime.txt' is not in the correct format.
                
                The following file contents were found:
                python-an.invalid.version
                
                However, the file contents must begin with a 'python-' prefix, followed by the
                version specified as '<major>.<minor>.<patch>'. Comments are not supported.
                
                For example, to request Python {DEFAULT_PYTHON_FULL_VERSION}, update the 'runtime.txt' file so it
                contains exactly:
                python-{DEFAULT_PYTHON_FULL_VERSION}
            "}
        );
    });
}
