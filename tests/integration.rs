//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run the tests use: `cargo test -- --ignored`

#![warn(clippy::pedantic)]

use indoc::{formatdoc, indoc};
use libcnb::data::buildpack::{BuildpackVersion, SingleBuildpackDescriptor};
use libcnb_test::{
    assert_contains, assert_empty, BuildConfig, ContainerConfig, PackResult, TestRunner,
};
use std::time::Duration;
use std::{env, fs, thread};

// At the moment these can't be imported from the buildpack, since integration
// tests cannot access any interfaces for binary-only crates.
// TODO: Explore moving integration tests into the crate, per:
// https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html
const LATEST_PYTHON_3_7: &str = "3.7.16";
const LATEST_PYTHON_3_8: &str = "3.8.16";
const LATEST_PYTHON_3_9: &str = "3.9.16";
const LATEST_PYTHON_3_10: &str = "3.10.10";
const LATEST_PYTHON_3_11: &str = "3.11.2";
const DEFAULT_PYTHON_VERSION: &str = LATEST_PYTHON_3_11;

const PIP_VERSION: &str = "23.0.1";
const SETUPTOOLS_VERSION: &str = "67.4.0";
const WHEEL_VERSION: &str = "0.38.4";

const DEFAULT_BUILDER: &str = "heroku/builder:22";
const TEST_PORT: u16 = 12345;

fn builder() -> String {
    env::var("INTEGRATION_TEST_CNB_BUILDER").unwrap_or(DEFAULT_BUILDER.to_string())
}

fn buildpack_version() -> BuildpackVersion {
    let buildpack_toml = fs::read_to_string("buildpack.toml").unwrap();
    let buildpack_descriptor =
        toml::from_str::<SingleBuildpackDescriptor<Option<()>>>(&buildpack_toml).unwrap();
    buildpack_descriptor.buildpack.version
}

// Detect

#[test]
#[ignore = "integration test"]
fn detect_rejects_non_python_projects() {
    let buildpack_version = buildpack_version();

    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/empty")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stdout,
                &formatdoc! {"
                    ===> DETECTING
                    ======== Output: heroku/python@{buildpack_version} ========
                    No Python project files found (such as requirements.txt).
                    ======== Results ========
                    fail: heroku/python@{buildpack_version}
                    ERROR: No buildpack groups passed detection.
                "}
            );
        },
    );
}

// Determine package manager

#[test]
#[ignore = "integration test"]
fn no_package_manager_detected() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/pyproject_toml_only")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: No Python package manager files were found]
                    A Pip requirements file was not found in your application's source code.
                    This file is required so that your application's dependencies can be installed.
                    
                    Please add a file named exactly 'requirements.txt' to the root directory of your
                    application, containing a list of the packages required by your application.
                    
                    For more information on what this file should contain, see:
                    https://pip.pypa.io/en/stable/reference/requirements-file-format/
                "}
            );
        },
    );
}

// runtime.txt parsing

#[test]
#[ignore = "integration test"]
fn runtime_txt_invalid_version() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/runtime_txt_invalid_version")
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
        "999.999.999",
    );
}

// Python versions

#[test]
#[ignore = "integration test"]
fn python_version_unspecified() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/python_version_unspecified"),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                &formatdoc! {"
                    ===> BUILDING
                    
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python and packaging tools]
                    Installing Python {DEFAULT_PYTHON_VERSION}
                    Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}
                    
                    [Installing dependencies using Pip]
                    Running pip install
                    Collecting typing-extensions==4.4.0
                      Downloading typing_extensions-4.4.0-py3-none-any.whl (26 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.4.0
                    ===> EXPORTING
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn python_3_7() {
    // Python 3.7 is only available on Heroku-20 and older.
    let fixture = "tests/fixtures/python_3.7";
    match builder().as_str() {
        "heroku/buildpacks:20" => builds_with_python_version(fixture, LATEST_PYTHON_3_7),
        _ => rejects_non_existent_python_version(fixture, LATEST_PYTHON_3_7),
    };
}

#[test]
#[ignore = "integration test"]
fn python_3_8() {
    // Python 3.8 is only available on Heroku-20 and older.
    let fixture = "tests/fixtures/python_3.8";
    match builder().as_str() {
        "heroku/buildpacks:20" => builds_with_python_version(fixture, LATEST_PYTHON_3_8),
        _ => rejects_non_existent_python_version(fixture, LATEST_PYTHON_3_8),
    };
}

#[test]
#[ignore = "integration test"]
fn python_3_9() {
    builds_with_python_version("tests/fixtures/python_3.9", LATEST_PYTHON_3_9);
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

fn builds_with_python_version(fixture_path: &str, python_version: &str) {
    let mut config = BuildConfig::new(builder(), fixture_path);
    // Checks that potentially broken user-provided env vars are not being passed unfiltered to
    // subprocesses we launch (such as `pip install`), thanks to `clear-env` in `buildpack.toml`.
    config.env("PYTHONHOME", "/invalid-path");

    TestRunner::default().build(config, |context| {
        assert_empty!(context.pack_stderr);
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                ===> BUILDING
                
                [Determining Python version]
                Using Python version {python_version} specified in runtime.txt
                
                [Installing Python and packaging tools]
                Installing Python {python_version}
                Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}
                
                [Installing dependencies using Pip]
                Running pip install
                Collecting typing-extensions==4.4.0
                  Downloading typing_extensions-4.4.0-py3-none-any.whl (26 kB)
                Installing collected packages: typing-extensions
                Successfully installed typing-extensions-4.4.0
                ===> EXPORTING
            "}
        );
        // There's no sensible default process type we can set for Python apps.
        assert_contains!(context.pack_stdout, "no default process type");

        // Validate the Python/Pip install works as expected at runtime.
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

                # Check all required dynamically linked libraries can be found in the runtime image.
                if find /layers -name '*.so' -exec ldd '{}' + | grep 'not found'; then
                  echo "The above dynamically linked libraries were not found!"
                  exit 1
                fi

                # Check that:
                #  - Pip is available at runtime too (and not just during the build).
                #  - The correct versions of pip/setuptools/wheel were installed.
                #  - Pip uses (via 'PYTHONUSERBASE') the user site-packages in the dependencies
                #    layer, and so can find the typing-extensions package installed there.
                #  - The "pip update available" warning is not shown (since it should be suppressed).
                #  - The system site-packages directory is protected against running 'pip install'
                #    without having passed '--user'.
                pip list
                pip install --dry-run typing-extensions
            "#}
        );
        assert_empty!(command_output.stderr);
        assert_contains!(
            command_output.stdout,
            &formatdoc! {"
                Python {python_version}
                Package           Version
                ----------------- -------
                pip               {PIP_VERSION}
                setuptools        {SETUPTOOLS_VERSION}
                typing_extensions 4.4.0
                wheel             {WHEEL_VERSION}
                Defaulting to user installation because normal site-packages is not writeable
                Requirement already satisfied: typing-extensions in /layers/heroku_python/dependencies/lib/"
            }
        );
    });
}

fn rejects_non_existent_python_version(fixture_path: &str, python_version: &str) {
    let builder = builder();

    TestRunner::default().build(
        BuildConfig::new(&builder, fixture_path).expected_pack_result(PackResult::Failure),
        |context| {
            let expected_stack = match builder.as_str() {
                "heroku/buildpacks:20" => "heroku-20",
                "heroku/builder:22" => "heroku-22",
                _ => unimplemented!("Unknown builder!"),
            };

            assert_contains!(
                context.pack_stderr,
                &formatdoc! {"
                    [Error: Requested Python version is not available]
                    The requested Python version ({python_version}) is not available for this stack ({expected_stack}).
                    
                    Please update the version in 'runtime.txt' to a supported Python version, or else
                    remove the file to instead use the default version (currently Python {DEFAULT_PYTHON_VERSION}).
                    
                    For a list of the supported Python versions, see:
                    https://devcenter.heroku.com/articles/python-support#supported-runtimes
                "}
            );
        },
    );
}

// Pip

#[test]
#[ignore = "integration test"]
fn pip_editable_git_compiled() {
    // This tests that:
    //  - Git from the stack image can be found (ie: the system PATH has been correctly propagated to pip).
    //  - The editable mode repository clone is saved into the dependencies layer not the app dir.
    //  - Compiling a source distribution package (as opposed to a pre-built wheel) works.
    //  - The Python headers can be found in the `include/pythonX.Y/` directory of the Python layer.
    //  - Headers/libraries from the stack image can be found (in this case, for libpq-dev).
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/pip_editable_git_compiled"),
        |context| {
            assert_contains!(
                context.pack_stdout,
                "Cloning https://github.com/psycopg/psycopg2 (to revision 2_9_5) to /layers/heroku_python/dependencies/src/psycopg2"
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn pip_invalid_requirement() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/pip_invalid_requirement")
            .expected_pack_result(PackResult::Failure),
        |context| {
            // Ideally we could test a combined stdout/stderr, however libcnb-test doesn't support this:
            // https://github.com/heroku/libcnb.rs/issues/536
            assert_contains!(
                context.pack_stdout,
                &formatdoc! {"
                    [Installing dependencies using Pip]
                    Running pip install
                "}
            );
            assert_contains!(
                context.pack_stderr,
                &formatdoc! {"
                    ERROR: Invalid requirement: 'an-invalid-requirement!' (from line 1 of requirements.txt)
                    
                    [Error: Unable to install dependencies using pip]
                    The 'pip install' command to install the application's dependencies from
                    'requirements.txt' failed (exit status: 1).
                    
                    See the log output above for more information.
                "}
            );
        },
    );
}

// Caching

#[test]
#[ignore = "integration test"]
fn cache_used_for_repeat_builds() {
    let config = BuildConfig::new(builder(), "tests/fixtures/python_3.11");

    TestRunner::default().build(&config, |context| {
        context.rebuild(&config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    ===> BUILDING
                    
                    [Determining Python version]
                    Using Python version {LATEST_PYTHON_3_11} specified in runtime.txt
                    
                    [Installing Python and packaging tools]
                    Using cached Python {LATEST_PYTHON_3_11}
                    Using cached pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}
                    
                    [Installing dependencies using Pip]
                    Using cached pip download/wheel cache
                    Running pip install
                    Collecting typing-extensions==4.4.0
                      Using cached typing_extensions-4.4.0-py3-none-any.whl (26 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.4.0
                    ===> EXPORTING
                "}
            );
        });
    });
}

#[test]
#[ignore = "integration test"]
fn cache_discarded_on_python_version_change() {
    let builder = builder();
    let config_before = BuildConfig::new(&builder, "tests/fixtures/python_3.10");
    let config_after = BuildConfig::new(&builder, "tests/fixtures/python_3.11");

    TestRunner::default().build(config_before, |context| {
        context.rebuild(config_after, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    ===> BUILDING
                    
                    [Determining Python version]
                    Using Python version {LATEST_PYTHON_3_11} specified in runtime.txt
                    
                    [Installing Python and packaging tools]
                    Discarding cache since the Python version has changed from {LATEST_PYTHON_3_10} to {LATEST_PYTHON_3_11}
                    Installing Python {LATEST_PYTHON_3_11}
                    Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}
                    
                    [Installing dependencies using Pip]
                    Discarding cached pip download/wheel cache
                    Running pip install
                    Collecting typing-extensions==4.4.0
                      Downloading typing_extensions-4.4.0-py3-none-any.whl (26 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.4.0
                    ===> EXPORTING
                "}
            );
        });
    });
}

#[test]
#[ignore = "integration test"]
fn cache_discarded_on_stack_change() {
    let fixture = "tests/fixtures/python_version_unspecified";
    let config_before = BuildConfig::new("heroku/buildpacks:20", fixture);
    let config_after = BuildConfig::new("heroku/builder:22", fixture);

    TestRunner::default().build(config_before, |context| {
        context.rebuild(config_after, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    ===> BUILDING
                    
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python and packaging tools]
                    Discarding cache since the stack has changed from heroku-20 to heroku-22
                    Installing Python {DEFAULT_PYTHON_VERSION}
                    Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}
                    
                    [Installing dependencies using Pip]
                    Discarding cached pip download/wheel cache
                    Running pip install
                    Collecting typing-extensions==4.4.0
                      Downloading typing_extensions-4.4.0-py3-none-any.whl (26 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.4.0
                    ===> EXPORTING
                "}
            );
        });
    });
}

#[test]
#[ignore = "integration test"]
fn cache_discarded_on_multiple_changes() {
    let config_before = BuildConfig::new("heroku/buildpacks:20", "tests/fixtures/python_3.10");
    let config_after = BuildConfig::new("heroku/builder:22", "tests/fixtures/python_3.11");

    TestRunner::default().build(config_before, |context| {
        context.rebuild(config_after, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    ===> BUILDING
                    
                    [Determining Python version]
                    Using Python version {LATEST_PYTHON_3_11} specified in runtime.txt
                    
                    [Installing Python and packaging tools]
                    Discarding cache since:
                     - the stack has changed from heroku-20 to heroku-22
                     - the Python version has changed from 3.10.10 to 3.11.2
                    Installing Python {LATEST_PYTHON_3_11}
                    Installing pip {PIP_VERSION}, setuptools {SETUPTOOLS_VERSION} and wheel {WHEEL_VERSION}
                    
                    [Installing dependencies using Pip]
                    Discarding cached pip download/wheel cache
                    Running pip install
                    Collecting typing-extensions==4.4.0
                      Downloading typing_extensions-4.4.0-py3-none-any.whl (26 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.4.0
                    ===> EXPORTING
                "}
            );
        });
    });
}

// Salesforce Functions

#[test]
#[ignore = "integration test"]
fn salesforce_function_template() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/salesforce_function_template"),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Validating Salesforce Function]
                    Function passed validation.
                    ===> EXPORTING
                "}
            );
            assert_contains!(context.pack_stdout, "Setting default process type 'web'");

            // Test that the `sf-functions-python` web process the buildpack configures works correctly.
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
fn salesforce_function_missing_package() {
    TestRunner::default().build(
        BuildConfig::new(
            builder(),
            "tests/fixtures/salesforce_function_missing_package",
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
fn salesforce_function_fails_self_check() {
    TestRunner::default().build(
        BuildConfig::new(
            builder(),
            "tests/fixtures/salesforce_function_fails_self_check",
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
                    Function failed validation: 'invalid' isn't a valid Salesforce REST API version."
                }
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn project_toml_invalid() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/project_toml_invalid")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {r#"
                    [Error: Invalid project.toml]
                    A parsing/validation error error occurred whilst loading the project.toml file.
                    
                    Details: TOML parse error at line 4, column 1
                      |
                    4 | [com.salesforce]
                      | ^^^^^^^^^^^^^^^^
                    missing field `type`
                "#}
            );
        },
    );
}
