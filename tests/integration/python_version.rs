use crate::integration_tests::{
    builder, DEFAULT_PYTHON_VERSION, LATEST_PYTHON_3_10, LATEST_PYTHON_3_11, LATEST_PYTHON_3_7,
    LATEST_PYTHON_3_8, LATEST_PYTHON_3_9,
};
use crate::packaging_tool_versions::PackagingToolVersions;
use indoc::{formatdoc, indoc};
use libcnb_test::{assert_contains, assert_empty, BuildConfig, PackResult, TestRunner};

#[test]
#[ignore = "integration test"]
fn python_version_unspecified() {
    let PackagingToolVersions {
        pip_version,
        setuptools_version,
        wheel_version,
    } = PackagingToolVersions::default();

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
                    Installing pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                    
                    [Installing dependencies using Pip]
                    Running pip install
                    Collecting typing-extensions==4.4.0 (from -r requirements.txt (line 2))
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
        "999.888.777",
    );
}

fn builds_with_python_version(fixture_path: &str, python_version: &str) {
    let PackagingToolVersions {
        pip_version,
        setuptools_version,
        wheel_version,
    } = PackagingToolVersions::default();

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
                Installing pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                
                [Installing dependencies using Pip]
                Running pip install
                Collecting typing-extensions==4.4.0 (from -r requirements.txt (line 2))
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
                pip               {pip_version}
                setuptools        {setuptools_version}
                typing_extensions 4.4.0
                wheel             {wheel_version}
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
