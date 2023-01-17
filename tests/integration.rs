//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run the tests use: `cargo test -- --ignored`

#![warn(clippy::pedantic)]

use indoc::indoc;
use libcnb_test::{assert_contains, BuildConfig, ContainerConfig, PackResult, TestRunner};
use std::thread;
use std::time::Duration;

const TEST_PORT: u16 = 12345;

// For now, these integration tests only cover functions, since:
// - that's what needs to ship first
// - the buildpack's detect by design rejects anything but a function, so for now
//   all tests here need to actually be a function to get past detect

#[test]
#[ignore = "integration test"]
fn detect_rejects_non_functions() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "test-fixtures/default")
            .expected_pack_result(PackResult::Failure),
        |context| {
            // We can't test the detect failure reason, since by default pack CLI only shows output for
            // non-zero, non-100 exit codes, and `libcnb-test` support enabling pack build's verbose mode:
            // https://github.com/heroku/libcnb.rs/issues/383
            assert_contains!(
                context.pack_stdout,
                "ERROR: No buildpack groups passed detection."
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn function_template() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "test-fixtures/function_template"),
        |context| {
            // Pip outputs git clone output to stderr for some reason, so stderr isn't empty.
            // TODO: Decide whether this is a bug in pip and/or if we should work around it.
            // assert_empty!(context.pack_stderr);

            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of 3.11.1.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python]
                    Downloading Python 3.11.1
                    Python installation successful
                    
                    [Installing Pip]
                    Installing pip 22.3.1, setuptools 65.6.3 and wheel 0.38.3
                    Installation completed
                    
                    [Installing dependencies using Pip]
                    Pip cache created
                    Running pip install
                    Collecting salesforce-functions
                "}
            );

            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    Pip install completed
                    
                    [Validating Salesforce Function]
                    Function passed validation.
                "}
            );

            context.start_container(
                ContainerConfig::new()
                    .env("PORT", TEST_PORT.to_string())
                    .expose_port(TEST_PORT),
                |container| {
                    let address_on_host = container.address_for_port(TEST_PORT).unwrap();
                    let url = format!("http://{}:{}", address_on_host.ip(), address_on_host.port());

                    // Retries needed since the server takes a moment to start up.
                    let mut attempts_remaining = 5;
                    let response = loop {
                        let response = ureq::post(&url).set("x-health-check", "true").call();
                        if response.is_ok() || attempts_remaining == 0 {
                            break response;
                        }
                        attempts_remaining -= 1;
                        thread::sleep(Duration::from_secs(1));
                    };

                    let server_log_output = container.logs_now();
                    assert_contains!(
                        server_log_output.stderr,
                        &format!("Uvicorn running on http://0.0.0.0:{TEST_PORT}")
                    );

                    let body = response.unwrap().into_string().unwrap();
                    assert_eq!(body, r#""OK""#);
                },
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn function_repeat_build() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "test-fixtures/function_template"),
        |context| {
            let config = context.config.clone();
            context.rebuild(config, |rebuild_context| {
                assert_contains!(
                    rebuild_context.pack_stdout,
                    indoc! {"
                        [Determining Python version]
                        No Python version specified, using the current default of 3.11.1.
                        To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                        
                        [Installing Python]
                        Re-using cached Python 3.11.1
                        
                        [Installing Pip]
                        Re-using cached pip 22.3.1, setuptools 65.6.3 and wheel 0.38.3
                        
                        [Installing dependencies using Pip]
                        Re-using cached pip-cache
                        Running pip install
                        Collecting salesforce-functions
                    "}
                );
            });
        },
    );
}

#[test]
#[ignore = "integration test"]
fn function_python_3_10() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "test-fixtures/function_python_3.10"),
        |context| {
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Determining Python version]
                    Using Python version 3.10.9 specified in runtime.txt
                    
                    [Installing Python]
                    Downloading Python 3.10.9
                    Python installation successful
                "}
            );

            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    Pip install completed
                    
                    [Validating Salesforce Function]
                    Function passed validation.
                "}
            );

            context.start_container(
                ContainerConfig::new()
                    .env("PORT", TEST_PORT.to_string())
                    .expose_port(TEST_PORT),
                |container| {
                    let address_on_host = container.address_for_port(TEST_PORT).unwrap();
                    let url = format!("http://{}:{}", address_on_host.ip(), address_on_host.port());

                    // Retries needed since the server takes a moment to start up.
                    let mut attempts_remaining = 5;
                    let response = loop {
                        let response = ureq::post(&url).set("x-health-check", "true").call();
                        if response.is_ok() || attempts_remaining == 0 {
                            break response;
                        }
                        attempts_remaining -= 1;
                        thread::sleep(Duration::from_secs(1));
                    };

                    let server_log_output = container.logs_now();
                    assert_contains!(
                        server_log_output.stderr,
                        &format!("Uvicorn running on http://0.0.0.0:{TEST_PORT}")
                    );

                    let body = response.unwrap().into_string().unwrap();
                    assert_eq!(body, r#""OK""#);
                },
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn function_python_version_too_old() {
    TestRunner::default().build(
        BuildConfig::new(
            "heroku/builder:22",
            "test-fixtures/function_python_version_too_old",
        )
        .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    ERROR: Ignored the following versions that require a different python version: 0.1.0 Requires-Python >=3.10; 0.2.0 Requires-Python >=3.10; 0.3.0 Requires-Python >=3.10
                    ERROR: Could not find a version that satisfies the requirement salesforce-functions (from versions: none)
                    ERROR: No matching distribution found for salesforce-functions
                    
                    [Error: Unable to install dependencies using pip]
                    The 'pip install' command to install the application's dependencies from
                    'requirements.txt' failed (exit status: 1).
                    
                    See the log output above for more information.
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn function_python_version_unavailable() {
    TestRunner::default().build(
        BuildConfig::new(
            "heroku/builder:22",
            "test-fixtures/function_python_version_unavailable",
        )
        .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: Requested Python version is not available]
                    The requested Python version (999.999.999) is not available for this stack (heroku-22).
                    
                    Please update the version in 'runtime.txt' to a supported Python version, or else
                    remove the file to instead use the default version (currently Python 3.11.1).
                    
                    For a list of the supported Python versions, see:
                    https://devcenter.heroku.com/articles/python-support#supported-runtimes
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn function_python_version_invalid() {
    TestRunner::default().build(
        BuildConfig::new(
            "heroku/builder:22",
            "test-fixtures/function_python_version_invalid",
        )
        .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: Invalid Python version in runtime.txt]
                    The Python version specified in 'runtime.txt' is not in the correct format.
                    
                    The following file contents were found:
                    python-an.invalid.version
                    
                    However, the file contents must begin with a 'python-' prefix, followed by the
                    version specified as '<major>.<minor>.<patch>'. Comments are not supported.
                    
                    For example, to request Python 3.11.1, the correct version format is:
                    python-3.11.1
                    
                    Please update 'runtime.txt' to use the correct version format, or else remove
                    the file to instead use the default version (currently Python 3.11.1).
                    
                    For a list of the supported Python versions, see:
                    https://devcenter.heroku.com/articles/python-support#supported-runtimes
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn function_missing_functions_package() {
    TestRunner::default().build(
        BuildConfig::new(
            "heroku/builder:22",
            "test-fixtures/function_missing_functions_package",
        )
        .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {r#"
                    [Error: The Salesforce Functions package is not installed]
                    The 'sf-functions-python' program that is required for Python Salesforce
                    Functions could not be found.
                    
                    Check that the 'salesforce-functions' Python package is listed as a
                    dependency in 'requirements.txt'.
                    
                    If this project is not intended to be a Salesforce Function, remove the
                    'type = "function"' declaration from 'project.toml' to skip this check.
                "#}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn function_fails_self_check() {
    TestRunner::default().build(
        BuildConfig::new(
            "heroku/builder:22",
            "test-fixtures/function_fails_self_check",
        )
        .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: The Salesforce Functions self-check failed]
                    The 'sf-functions-python check' command failed (exit status: 1), indicating
                    there is a problem with the Python Salesforce Function in this project.
                    
                    Details:
                    Function failed validation: 'invalid' is not a valid Salesforce REST API version. Update 'salesforce-api-version' in project.toml to a version of form 'X.Y'.
                "}
            );
        },
    );
}

// TODO:
//
// Detect
// - no Python files
//
// Python versions
// - Default
// - 3.11.<latest>
// - 3.11.<non-latest> (show update warning)
// - 3.10.<latest>
// - 3.9.<latest>
// - 3.8 (unsupported, show reason)
// - 3.7 (unsupported, show reason)
// - 3.6 (unsupported, explain EOL)
// - various invalid version strings
//
// Caching
// - Python version change
// - Stack change
// - Various Pip cache invalidation types (package additions/removals etc)
// - No-op
//
// Other
// - that pip install can find Python headers
