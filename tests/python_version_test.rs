use crate::tests::{
    builder, default_build_config, DEFAULT_PYTHON_VERSION, LATEST_PYTHON_3_10, LATEST_PYTHON_3_11,
    LATEST_PYTHON_3_12, LATEST_PYTHON_3_7, LATEST_PYTHON_3_8, LATEST_PYTHON_3_9,
};
use indoc::{formatdoc, indoc};
use libcnb_test::{assert_contains, assert_empty, PackResult, TestRunner};

#[test]
#[ignore = "integration test"]
fn python_version_unspecified() {
    TestRunner::default().build(
        default_build_config( "tests/fixtures/python_version_unspecified"),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python]
                    Installing Python {DEFAULT_PYTHON_VERSION}
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn python_3_7() {
    // Python 3.7 is EOL and so archives for it don't exist at the new S3 filenames.
    rejects_non_existent_python_version("tests/fixtures/python_3.7", LATEST_PYTHON_3_7);
}

#[test]
#[ignore = "integration test"]
fn python_3_8() {
    // Python 3.8 is only available on Heroku-20 and older.
    let fixture = "tests/fixtures/python_3.8";
    match builder().as_str() {
        "heroku/builder:20" => builds_with_python_version(fixture, LATEST_PYTHON_3_8),
        _ => rejects_non_existent_python_version(fixture, LATEST_PYTHON_3_8),
    };
}

#[test]
#[ignore = "integration test"]
fn python_3_9() {
    // Python 3.9 is only available on Heroku-22 and older.
    let fixture = "tests/fixtures/python_3.9";
    match builder().as_str() {
        "heroku/builder:20" | "heroku/builder:22" => {
            builds_with_python_version(fixture, LATEST_PYTHON_3_9);
        }
        _ => rejects_non_existent_python_version(fixture, LATEST_PYTHON_3_9),
    };
}

#[test]
#[ignore = "integration test"]
fn python_3_10() {
    builds_with_python_version("tests/fixtures/python_3.10", LATEST_PYTHON_3_10);
}

#[test]
#[ignore = "integration test"]
fn python_3_11() {
    builds_with_python_version("tests/fixtures/python_3.11", LATEST_PYTHON_3_11);
}

#[test]
#[ignore = "integration test"]
fn python_3_12() {
    builds_with_python_version("tests/fixtures/python_3.12", LATEST_PYTHON_3_12);
}

fn builds_with_python_version(fixture_path: &str, python_version: &str) {
    TestRunner::default().build(default_build_config(fixture_path), |context| {
        assert_empty!(context.pack_stderr);
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Determining Python version]
                Using Python version {python_version} specified in runtime.txt
                
                [Installing Python]
                Installing Python {python_version}
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
            format!("Python {python_version}\n")
        );
    });
}

#[test]
#[ignore = "integration test"]
fn runtime_txt_io_error() {
    TestRunner::default().build(
        default_build_config("tests/fixtures/runtime_txt_invalid_unicode")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                &formatdoc! {"
                    [Error: Unable to read runtime.txt]
                    An unexpected error occurred whilst reading the (optional) runtime.txt file.
                    
                    Details: I/O Error: stream did not contain valid UTF-8
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn runtime_txt_invalid_version() {
    TestRunner::default().build(
        default_build_config( "tests/fixtures/runtime_txt_invalid_version")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                &formatdoc! {"
                    [Error: Invalid Python version in runtime.txt]
                    The Python version specified in 'runtime.txt' is not in the correct format.
                    
                    The following file contents were found:
                    python-an.invalid.version
                    
                    However, the file contents must begin with a 'python-' prefix, followed by the
                    version specified as '<major>.<minor>.<patch>'. Comments are not supported.
                    
                    For example, to request Python {DEFAULT_PYTHON_VERSION}, the correct version format is:
                    python-{DEFAULT_PYTHON_VERSION}
                    
                    Please update 'runtime.txt' to use the correct version format, or else remove
                    the file to instead use the default version (currently Python {DEFAULT_PYTHON_VERSION}).
                    
                    For a list of the supported Python versions, see:
                    https://devcenter.heroku.com/articles/python-support#supported-runtimes
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn runtime_txt_non_existent_version() {
    rejects_non_existent_python_version(
        "tests/fixtures/runtime_txt_non_existent_version",
        "999.888.777",
    );
}

fn rejects_non_existent_python_version(fixture_path: &str, python_version: &str) {
    TestRunner::default().build(
        default_build_config(fixture_path).expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                &formatdoc! {"
                    [Error: Requested Python version is not available]
                    The requested Python version ({python_version}) is not available for this builder image.
                    
                    Please update the version in 'runtime.txt' to a supported Python version, or else
                    remove the file to instead use the default version (currently Python {DEFAULT_PYTHON_VERSION}).
                    
                    For a list of the supported Python versions, see:
                    https://devcenter.heroku.com/articles/python-support#supported-runtimes
                "}
            );
        },
    );
}
