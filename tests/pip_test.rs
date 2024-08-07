use crate::packaging_tool_versions::PIP_VERSION;
use crate::tests::{default_build_config, DEFAULT_PYTHON_VERSION};
use indoc::{formatdoc, indoc};
use libcnb_test::{assert_contains, assert_empty, BuildpackReference, PackResult, TestRunner};

#[test]
#[ignore = "integration test"]
fn pip_basic_install_and_cache_reuse() {
    let config = default_build_config("tests/fixtures/pip_basic");

    TestRunner::default().build(&config, |context| {
        assert_empty!(context.pack_stderr);
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Determining Python version]
                No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                
                [Installing Python and pip]
                Installing Python {DEFAULT_PYTHON_VERSION}
                Installing pip {PIP_VERSION}
                
                [Installing dependencies using pip]
                Running pip install
                Collecting typing-extensions==4.7.1 (from -r requirements.txt (line 2))
                  Downloading typing_extensions-4.7.1-py3-none-any.whl.metadata (3.1 kB)
                Downloading typing_extensions-4.7.1-py3-none-any.whl (33 kB)
                Installing collected packages: typing-extensions
                Successfully installed typing-extensions-4.7.1
            "}
        );

        // Check that:
        // - The correct env vars are set at run-time.
        // - pip is available at run-time too (and not just during the build).
        // - The correct version of pip was installed.
        // - pip uses (via 'PYTHONUSERBASE') the user site-packages in the dependencies
        //   layer, and so can find the typing-extensions package installed there.
        // - The "pip update available" warning is not shown (since it should be suppressed).
        // - The system site-packages directory is protected against running 'pip install'
        //   without having passed '--user'.
        let command_output = context.run_shell_command(
            indoc! {"
                set -euo pipefail
                printenv | sort | grep -vE '^(_|HOME|HOSTNAME|OLDPWD|PWD|SHLVL)='
                echo
                pip list
                pip install --dry-run typing-extensions
            "}
        );
        assert_empty!(command_output.stderr);
        assert_contains!(
            command_output.stdout,
            &formatdoc! {"
                LANG=C.UTF-8
                LD_LIBRARY_PATH=/layers/heroku_python/python/lib:/layers/heroku_python/dependencies/lib
                PATH=/layers/heroku_python/dependencies/bin:/layers/heroku_python/python/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PIP_DISABLE_PIP_VERSION_CHECK=1
                PYTHONHOME=/layers/heroku_python/python
                PYTHONUNBUFFERED=1
                PYTHONUSERBASE=/layers/heroku_python/dependencies
                
                Package           Version
                ----------------- -------
                pip               {PIP_VERSION}
                typing_extensions 4.7.1
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
                    
                    [Installing Python and pip]
                    Using cached Python {DEFAULT_PYTHON_VERSION} and pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Using cached pip download/wheel cache
                    Running pip install
                    Collecting typing-extensions==4.7.1 (from -r requirements.txt (line 2))
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
fn pip_cache_invalidation_with_compatible_metadata() {
    // TODO: Re-enable this test the next time the default-Python/pip versions change, at which point
    // there will be a historic buildpack version with compatible metadata that triggers invalidation.
    #![allow(unreachable_code)]
    return;

    let config = default_build_config("tests/fixtures/pip_basic");

    TestRunner::default().build(
        config.clone().buildpacks([BuildpackReference::Other(
            "docker://docker.io/heroku/buildpack-python:TODO".to_string(),
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
                        
                        [Installing Python and pip]
                        Discarding cache since:
                         - The Python version has changed from 3.12.3 to {DEFAULT_PYTHON_VERSION}
                         - The pip version has changed from 24.0 to {PIP_VERSION}
                        Installing Python {DEFAULT_PYTHON_VERSION}
                        Installing pip {PIP_VERSION}
                        
                        [Installing dependencies using pip]
                        Discarding cached pip download/wheel cache
                        Running pip install
                        Collecting typing-extensions==4.7.1 (from -r requirements.txt (line 2))
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
// - The cached layers are correctly invalidated when the layer metadata was incompatible.
// - That a suitable message was output explaining why.
#[test]
#[ignore = "integration test"]
fn pip_cache_invalidation_with_incompatible_metadata() {
    let config = default_build_config("tests/fixtures/pip_basic");

    TestRunner::default().build(
        config.clone().buildpacks([BuildpackReference::Other(
            "docker://docker.io/heroku/buildpack-python:0.13.0".to_string(),
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
                        
                        [Installing Python and pip]
                        Discarding cache since the buildpack cache format has changed
                        Installing Python {DEFAULT_PYTHON_VERSION}
                        Installing pip {PIP_VERSION}
                        
                        [Installing dependencies using pip]
                        Discarding cached pip download/wheel cache
                        Running pip install
                        Collecting typing-extensions==4.7.1 (from -r requirements.txt (line 2))
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
        default_build_config( "tests/fixtures/pip_editable_git_compiled")
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
        default_build_config( "tests/fixtures/pip_invalid_requirement")
            .expected_pack_result(PackResult::Failure),
        |context| {
            // Ideally we could test a combined stdout/stderr, however libcnb-test doesn't support this:
            // https://github.com/heroku/libcnb.rs/issues/536
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Installing dependencies using pip]
                    Running pip install
                "}
            );
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    ERROR: Invalid requirement: 'an-invalid-requirement!': Expected end or semicolon (after name and no valid version specifier)
                        an-invalid-requirement!
                                              ^ (from line 1 of requirements.txt)
                    
                    [Error: Unable to install dependencies using pip]
                    The 'pip install' command to install the application's dependencies from
                    'requirements.txt' failed (exit status: 1).
                    
                    See the log output above for more information.
                "}
            );
        },
    );
}
