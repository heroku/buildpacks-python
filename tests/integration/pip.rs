use crate::integration_tests::{
    builder, DEFAULT_PYTHON_VERSION, LATEST_PYTHON_3_10, LATEST_PYTHON_3_11,
};
use crate::packaging_tool_versions::PackagingToolVersions;
use indoc::formatdoc;
use libcnb_test::{assert_contains, assert_empty, BuildConfig, PackResult, TestRunner};

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
fn pip_install_error() {
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

#[test]
#[ignore = "integration test"]
fn cache_used_for_repeat_builds() {
    let PackagingToolVersions {
        pip_version,
        setuptools_version,
        wheel_version,
    } = PackagingToolVersions::default();

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
                    Using cached pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                    
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
    let PackagingToolVersions {
        pip_version,
        setuptools_version,
        wheel_version,
    } = PackagingToolVersions::default();

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
                    Installing pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                    
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
    let PackagingToolVersions {
        pip_version,
        setuptools_version,
        wheel_version,
    } = PackagingToolVersions::default();

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
                    Installing pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                    
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
    let PackagingToolVersions {
        pip_version,
        setuptools_version,
        wheel_version,
    } = PackagingToolVersions::default();

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
                    Installing pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                    
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
