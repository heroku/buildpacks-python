use crate::packaging_tool_versions::PIP_VERSION;
use crate::tests::{default_build_config, DEFAULT_PYTHON_VERSION, LATEST_PYTHON_3_11};
use indoc::{formatdoc, indoc};
use libcnb_test::{assert_contains, assert_empty, BuildpackReference, PackResult, TestRunner};

#[test]
#[ignore = "integration test"]
#[allow(clippy::too_many_lines)]
fn pip_basic_install_and_cache_reuse() {
    let mut config = default_build_config("tests/fixtures/pip_basic");
    config.buildpacks(vec![
        BuildpackReference::CurrentCrate,
        BuildpackReference::Other("file://tests/fixtures/testing_buildpack".to_string()),
    ]);

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
                Collecting typing-extensions==4.12.2 (from -r requirements.txt (line 2))
                  Downloading typing_extensions-4.12.2-py3-none-any.whl.metadata (3.0 kB)
                Downloading typing_extensions-4.12.2-py3-none-any.whl (37 kB)
                Installing collected packages: typing-extensions
                Successfully installed typing-extensions-4.12.2
                
                ## Testing buildpack ##
                CPATH=/layers/heroku_python/python/include/python3.12:/layers/heroku_python/python/include
                LANG=C.UTF-8
                LD_LIBRARY_PATH=/layers/heroku_python/python/lib:/layers/heroku_python/dependencies/lib
                LIBRARY_PATH=/layers/heroku_python/python/lib:/layers/heroku_python/dependencies/lib
                PATH=/layers/heroku_python/python/bin:/layers/heroku_python/dependencies/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PIP_CACHE_DIR=/layers/heroku_python/pip-cache
                PIP_DISABLE_PIP_VERSION_CHECK=1
                PKG_CONFIG_PATH=/layers/heroku_python/python/lib/pkgconfig
                PYTHONHOME=/layers/heroku_python/python
                PYTHONUNBUFFERED=1
                PYTHONUSERBASE=/layers/heroku_python/dependencies
                SOURCE_DATE_EPOCH=315532801
                
                ['',
                 '/layers/heroku_python/python/lib/python312.zip',
                 '/layers/heroku_python/python/lib/python3.12',
                 '/layers/heroku_python/python/lib/python3.12/lib-dynload',
                 '/layers/heroku_python/dependencies/lib/python3.12/site-packages',
                 '/layers/heroku_python/python/lib/python3.12/site-packages']
                
                pip {PIP_VERSION} from /layers/heroku_python/python/lib/python3.12/site-packages/pip (python 3.12)
                Package           Version
                ----------------- -------
                pip               {PIP_VERSION}
                typing_extensions 4.12.2
                Defaulting to user installation because normal site-packages is not writeable
                Requirement already satisfied: typing-extensions in /layers/heroku_python/dependencies/lib/python3.12/site-packages (4.12.2)
                <module 'typing_extensions' from '/layers/heroku_python/dependencies/lib/python3.12/site-packages/typing_extensions.py'>
            "}
        );

        // Check that at run-time:
        // - The correct env vars are set.
        // - pip is available (rather than just during the build).
        // - Both pip and Python can find the typing-extensions package.
        let command_output = context.run_shell_command(
            indoc! {"
                set -euo pipefail
                printenv | sort | grep -vE '^(_|HOME|HOSTNAME|OLDPWD|PWD|SHLVL)='
                echo
                pip list
                python -c 'import typing_extensions'
            "}
        );
        assert_empty!(command_output.stderr);
        assert_eq!(
            command_output.stdout,
            formatdoc! {"
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
                typing_extensions 4.12.2
            "}
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
                    Collecting typing-extensions==4.12.2 (from -r requirements.txt (line 2))
                      Using cached typing_extensions-4.12.2-py3-none-any.whl.metadata (3.0 kB)
                    Using cached typing_extensions-4.12.2-py3-none-any.whl (37 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.12.2
                "}
            );
        });
    });
}

#[test]
#[ignore = "integration test"]
fn pip_cache_invalidation_python_version_changed() {
    let config = default_build_config("tests/fixtures/python_3.11");
    let rebuild_config = default_build_config("tests/fixtures/pip_basic");

    TestRunner::default().build(config, |context| {
        context.rebuild(rebuild_config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python and pip]
                    Discarding cache since:
                     - The Python version has changed from {LATEST_PYTHON_3_11} to {DEFAULT_PYTHON_VERSION}
                    Installing Python {DEFAULT_PYTHON_VERSION}
                    Installing pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Discarding cached pip download/wheel cache
                    Running pip install
                    Collecting typing-extensions==4.12.2 (from -r requirements.txt (line 2))
                      Downloading typing_extensions-4.12.2-py3-none-any.whl.metadata (3.0 kB)
                    Downloading typing_extensions-4.12.2-py3-none-any.whl (37 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.12.2
                "}
            );
        });
    });
}

// This tests that cached layers from a previous buildpack version are compatible, or if we've
// decided to break compatibility recently, that the layers are at least invalidated gracefully.
#[test]
#[ignore = "integration test"]
fn pip_cache_previous_buildpack_version() {
    let mut config = default_build_config("tests/fixtures/pip_basic");
    config.buildpacks([BuildpackReference::Other(
        "docker://docker.io/heroku/buildpack-python:0.14.0".to_string(),
    )]);
    let rebuild_config = default_build_config("tests/fixtures/pip_basic");

    TestRunner::default().build(config, |context| {
        context.rebuild(rebuild_config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python and pip]
                    Discarding cache since:
                     - The Python version has changed from 3.12.4 to {DEFAULT_PYTHON_VERSION}
                     - The pip version has changed from 24.1.2 to {PIP_VERSION}
                    Installing Python {DEFAULT_PYTHON_VERSION}
                    Installing pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Discarding cached pip download/wheel cache
                    Running pip install
                    Collecting typing-extensions==4.12.2 (from -r requirements.txt (line 2))
                      Downloading typing_extensions-4.12.2-py3-none-any.whl.metadata (3.0 kB)
                    Downloading typing_extensions-4.12.2-py3-none-any.whl (37 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.12.2
                "}
            );
        });
    });
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
    let mut config = default_build_config("tests/fixtures/pip_editable_git_compiled");
    config.env("WHEEL_PACKAGE_URL", "https://github.com/pypa/wheel.git");

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stdout,
            "Cloning https://github.com/pypa/wheel.git (to revision 0.44.0) to /layers/heroku_python/dependencies/src/extension-dist"
        );
    });
}

#[test]
#[ignore = "integration test"]
fn pip_install_error() {
    let mut config = default_build_config("tests/fixtures/pip_invalid_requirement");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
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
    });
}
