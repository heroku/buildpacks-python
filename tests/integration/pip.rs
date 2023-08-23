use crate::integration_tests::{builder, DEFAULT_PYTHON_VERSION};
use crate::packaging_tool_versions::PackagingToolVersions;
use indoc::{formatdoc, indoc};
use libcnb_test::{
    assert_contains, assert_empty, BuildConfig, BuildpackReference, PackResult, TestRunner,
};

#[test]
#[ignore = "integration test"]
fn pip_basic_install_and_cache_reuse() {
    let PackagingToolVersions {
        pip_version,
        setuptools_version,
        wheel_version,
    } = PackagingToolVersions::default();

    let config = BuildConfig::new(builder(), "tests/fixtures/pip_basic");

    TestRunner::default().build(&config, |context| {
        assert_empty!(context.pack_stderr);
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Determining Python version]
                No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                
                [Installing Python and packaging tools]
                Installing Python {DEFAULT_PYTHON_VERSION}
                Installing pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                
                [Installing dependencies using Pip]
                Running pip install
                Collecting typing-extensions==4.7.1 (from -r requirements.txt (line 2))
                  Obtaining dependency information for typing-extensions==4.7.1 from https://files.pythonhosted.org/packages/ec/6b/63cc3df74987c36fe26157ee12e09e8f9db4de771e0f3404263117e75b95/typing_extensions-4.7.1-py3-none-any.whl.metadata
                  Downloading typing_extensions-4.7.1-py3-none-any.whl.metadata (3.1 kB)
                Downloading typing_extensions-4.7.1-py3-none-any.whl (33 kB)
                Installing collected packages: typing-extensions
                Successfully installed typing-extensions-4.7.1
            "}
        );

        // Check that:
        // - Pip is available at runtime too (and not just during the build).
        // - The correct versions of pip/setuptools/wheel were installed.
        // - Pip uses (via 'PYTHONUSERBASE') the user site-packages in the dependencies
        //   layer, and so can find the typing-extensions package installed there.
        // - The "pip update available" warning is not shown (since it should be suppressed).
        // - The system site-packages directory is protected against running 'pip install'
        //   without having passed '--user'.
        let command_output =
            context.run_shell_command("pip list && pip install --dry-run typing-extensions");
        assert_empty!(command_output.stderr);
        assert_contains!(
            command_output.stdout,
            &formatdoc! {"
                Package           Version
                ----------------- -------
                pip               {pip_version}
                setuptools        {setuptools_version}
                typing_extensions 4.7.1
                wheel             {wheel_version}
                Defaulting to user installation because normal site-packages is not writeable
                Requirement already satisfied: typing-extensions in /layers/heroku_python/dependencies/lib/"
            }
        );

        context.rebuild(&config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python and packaging tools]
                    Using cached Python {DEFAULT_PYTHON_VERSION}
                    Using cached pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                    
                    [Installing dependencies using Pip]
                    Using cached pip download/wheel cache
                    Running pip install
                    Collecting typing-extensions==4.7.1 (from -r requirements.txt (line 2))
                      Obtaining dependency information for typing-extensions==4.7.1 from https://files.pythonhosted.org/packages/ec/6b/63cc3df74987c36fe26157ee12e09e8f9db4de771e0f3404263117e75b95/typing_extensions-4.7.1-py3-none-any.whl.metadata
                      Using cached typing_extensions-4.7.1-py3-none-any.whl.metadata (3.1 kB)
                    Using cached typing_extensions-4.7.1-py3-none-any.whl (33 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.7.1
                "}
            );
        });
    });
}

// This tests that:
// - The cached layers are correctly invalidated when Python/other versions change.
// - The layer metadata written by older versions of the buildpack are still compatible.
#[test]
#[ignore = "integration test"]
fn pip_cache_invalidation_and_metadata_compatibility() {
    let PackagingToolVersions {
        pip_version,
        setuptools_version,
        wheel_version,
    } = PackagingToolVersions::default();

    let config = BuildConfig::new(builder(), "tests/fixtures/pip_basic");

    TestRunner::default().build(
        config.clone().buildpacks(vec![BuildpackReference::Other(
            "docker://docker.io/heroku/buildpack-python:0.1.0".to_string(),
        )]),
        |context| {
            context.rebuild(config, |rebuild_context| {
                assert_empty!(rebuild_context.pack_stderr);
                assert_contains!(
                    rebuild_context.pack_stdout,
                    &formatdoc! {"
                        [Determining Python version]
                        No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                        To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                        
                        [Installing Python and packaging tools]
                        Discarding cache since:
                         - The Python version has changed from 3.11.2 to {DEFAULT_PYTHON_VERSION}
                         - The pip version has changed from 23.0.1 to {pip_version}
                         - The setuptools version has changed from 67.5.0 to {setuptools_version}
                         - The wheel version has changed from 0.38.4 to {wheel_version}
                        Installing Python {DEFAULT_PYTHON_VERSION}
                        Installing pip {pip_version}, setuptools {setuptools_version} and wheel {wheel_version}
                        
                        [Installing dependencies using Pip]
                        Discarding cached pip download/wheel cache
                        Running pip install
                        Collecting typing-extensions==4.7.1 (from -r requirements.txt (line 2))
                          Obtaining dependency information for typing-extensions==4.7.1 from https://files.pythonhosted.org/packages/ec/6b/63cc3df74987c36fe26157ee12e09e8f9db4de771e0f3404263117e75b95/typing_extensions-4.7.1-py3-none-any.whl.metadata
                          Downloading typing_extensions-4.7.1-py3-none-any.whl.metadata (3.1 kB)
                        Downloading typing_extensions-4.7.1-py3-none-any.whl (33 kB)
                        Installing collected packages: typing-extensions
                        Successfully installed typing-extensions-4.7.1
                    "}
                );
            });
        },
    );
}

// This tests that:
//  - Requirements file env var interpolation works (ie: user-provided env vars have been propagated to pip).
//  - Git from the stack image can be found (ie: the system PATH has been correctly propagated to pip).
//  - The editable mode repository clone is saved into the dependencies layer not the app dir.
//  - Compiling a source distribution package (as opposed to a pre-built wheel) works.
//  - The Python headers can be found in the `include/pythonX.Y/` directory of the Python layer.
#[test]
#[ignore = "integration test"]
fn pip_editable_git_compiled() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/pip_editable_git_compiled")
            .env("WHEEL_PACKAGE_URL", "https://github.com/pypa/wheel"),
        |context| {
            assert_contains!(
                context.pack_stdout,
                "Cloning https://github.com/pypa/wheel (to revision 0.40.0) to /layers/heroku_python/dependencies/src/extension-dist"
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
                indoc! {"
                    [Installing dependencies using Pip]
                    Running pip install
                "}
            );
            assert_contains!(
                context.pack_stderr,
                indoc! {"
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
