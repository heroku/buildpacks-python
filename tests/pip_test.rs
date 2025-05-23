use crate::packaging_tool_versions::PIP_VERSION;
use crate::python_version::{DEFAULT_PYTHON_FULL_VERSION, DEFAULT_PYTHON_VERSION};
use crate::tests::default_build_config;
use indoc::{formatdoc, indoc};
use libcnb_test::{BuildpackReference, PackResult, TestRunner, assert_contains, assert_empty};

#[test]
#[ignore = "integration test"]
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
                We recommend setting an explicit version. In the root of your app create
                a '.python-version' file, containing a Python version like '{DEFAULT_PYTHON_VERSION}'.
                
                [Installing Python]
                Installing Python {DEFAULT_PYTHON_FULL_VERSION}
                
                [Installing pip]
                Installing pip {PIP_VERSION}
                
                [Installing dependencies using pip]
                Creating virtual environment
                Running 'pip install -r requirements.txt'
                Collecting typing-extensions==4.12.2 (from -r requirements.txt (line 2))
                  Downloading typing_extensions-4.12.2-py3-none-any.whl.metadata (3.0 kB)
                Downloading typing_extensions-4.12.2-py3-none-any.whl (37 kB)
                Installing collected packages: typing-extensions
                Successfully installed typing-extensions-4.12.2
                
                ## Testing buildpack ##
                CPATH=/layers/heroku_python/venv/include:/layers/heroku_python/python/include/python3.13:/layers/heroku_python/python/include
                LD_LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib:/layers/heroku_python/pip/lib
                LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib:/layers/heroku_python/pip/lib
                PATH=/layers/heroku_python/venv/bin:/layers/heroku_python/python/bin:/layers/heroku_python/pip/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PIP_CACHE_DIR=/layers/heroku_python/pip-cache
                PIP_DISABLE_PIP_VERSION_CHECK=1
                PIP_PYTHON=/layers/heroku_python/venv
                PKG_CONFIG_PATH=/layers/heroku_python/python/lib/pkgconfig
                PYTHONUNBUFFERED=1
                PYTHONUSERBASE=/layers/heroku_python/pip
                SOURCE_DATE_EPOCH=315532801
                VIRTUAL_ENV=/layers/heroku_python/venv
                
                ['',
                 '/layers/heroku_python/python/lib/python313.zip',
                 '/layers/heroku_python/python/lib/python3.13',
                 '/layers/heroku_python/python/lib/python3.13/lib-dynload',
                 '/layers/heroku_python/venv/lib/python3.13/site-packages']
                
                pip {PIP_VERSION} from /layers/heroku_python/pip/lib/python3.13/site-packages/pip (python 3.13)
                Package           Version
                ----------------- -------
                typing_extensions 4.12.2
                <module 'typing_extensions' from '/layers/heroku_python/venv/lib/python3.13/site-packages/typing_extensions.py'>
            "}
        );

        // Check that at run-time:
        // - The correct env vars are set.
        // - pip isn't available.
        // - Python can find the typing-extensions package.
        let command_output = context.run_shell_command(
            indoc! {"
                set -euo pipefail
                printenv | sort | grep -vE '^(_|HOME|HOSTNAME|OLDPWD|PWD|SHLVL)='
                ! command -v pip > /dev/null || { echo 'pip unexpectedly found!' && exit 1; }
                python -c 'import typing_extensions'
            "}
        );
        assert_empty!(command_output.stderr);
        assert_eq!(
            command_output.stdout,
            formatdoc! {"
                LD_LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib
                PATH=/layers/heroku_python/venv/bin:/layers/heroku_python/python/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PYTHONUNBUFFERED=1
                VIRTUAL_ENV=/layers/heroku_python/venv
            "}
        );

        context.rebuild(&config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    We recommend setting an explicit version. In the root of your app create
                    a '.python-version' file, containing a Python version like '{DEFAULT_PYTHON_VERSION}'.
                    
                    [Installing Python]
                    Using cached Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    [Installing pip]
                    Using cached pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Using cached pip download/wheel cache
                    Creating virtual environment
                    Running 'pip install -r requirements.txt'
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
fn pip_cache_invalidation_package_manager_changed() {
    let config = default_build_config("tests/fixtures/uv_basic");
    let rebuild_config = default_build_config("tests/fixtures/pip_basic");

    TestRunner::default().build(config, |context| {
        context.rebuild(rebuild_config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    We recommend setting an explicit version. In the root of your app create
                    a '.python-version' file, containing a Python version like '{DEFAULT_PYTHON_VERSION}'.
                    
                    [Installing Python]
                    Using cached Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    [Installing pip]
                    Installing pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Creating virtual environment
                    Running 'pip install -r requirements.txt'
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
        "docker://docker.io/heroku/buildpack-python:0.16.0".to_string(),
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
                    We recommend setting an explicit version. In the root of your app create
                    a '.python-version' file, containing a Python version like '{DEFAULT_PYTHON_VERSION}'.
                    
                    [Installing Python]
                    Discarding cached Python 3.12.5 since:
                     - The Python version has changed from 3.12.5 to {DEFAULT_PYTHON_FULL_VERSION}
                    Installing Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    [Installing pip]
                    Discarding cached pip 24.2
                    Installing pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Discarding cached pip download/wheel cache
                    Creating virtual environment
                    Running 'pip install -r requirements.txt'
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
//  - Building/compiling a source distribution package (as opposed to a pre-built wheel) works.
//  - The Python headers can be found in the `include/pythonX.Y/` directory of the Python layer.
#[test]
#[ignore = "integration test"]
fn pip_editable_git_compiled() {
    let mut config = default_build_config("tests/fixtures/pip_editable_git_compiled");
    config.env("WHEEL_PACKAGE_URL", "https://github.com/pypa/wheel.git");

    TestRunner::default().build(config, |context| {
        // We can't `assert_empty!(context.pack_stderr)` here, since the git clone steps print to stderr.
        assert_contains!(
            context.pack_stdout,
            "Cloning https://github.com/pypa/wheel.git (to revision 0.44.0) to /layers/heroku_python/venv/src/extension-dist"
        );
    });
}

// This checks that the pip bootstrap works even with older bundled pip, and that our chosen
// pip version also supports our oldest supported Python version.
#[test]
#[ignore = "integration test"]
fn pip_oldest_python() {
    let config = default_build_config("tests/fixtures/pip_oldest_python");

    TestRunner::default().build(config, |context| {
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
        assert_contains!(
            context.pack_stdout,
            indoc! {"
                [Determining Python version]
                Using Python version 3.9.0 specified in .python-version
                
                [Installing Python]
                Installing Python 3.9.0
            "}
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
                Creating virtual environment
                Running 'pip install -r requirements.txt'
            "}
        );
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                ERROR: Invalid requirement: 'an-invalid-requirement!': Expected end or semicolon (after name and no valid version specifier)
                    an-invalid-requirement!
                                          ^ (from line 1 of requirements.txt)
                
                [Error: Unable to install dependencies using pip]
                The 'pip install -r requirements.txt' command to install the app's
                dependencies failed (exit status: 1).
                
                See the log output above for more information.
            "}
        );
    });
}
