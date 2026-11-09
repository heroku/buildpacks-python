use crate::packaging_tool_versions::PIP_VERSION;
use crate::python_version::{DEFAULT_PYTHON_FULL_VERSION, DEFAULT_PYTHON_VERSION};
use crate::tests::default_build_config;
use indoc::{formatdoc, indoc};
use libcnb_test::{BuildpackReference, PackResult, TestRunner, assert_contains, assert_empty};
use std::env::consts::ARCH;

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
                Using Python version {DEFAULT_PYTHON_VERSION} specified in .python-version
                
                [Installing Python]
                Installing Python {DEFAULT_PYTHON_FULL_VERSION}
                
                [Installing pip]
                Installing pip {PIP_VERSION}
                
                [Installing dependencies using pip]
                Creating virtual environment
                Running 'pip install -r requirements.txt'
                Collecting typing-extensions==4.16.0 (from -r requirements.txt (line 2))
                  Downloading typing_extensions-4.16.0-py3-none-any.whl.metadata (3.3 kB)
                Downloading typing_extensions-4.16.0-py3-none-any.whl (45 kB)
                Installing collected packages: typing-extensions
                Successfully installed typing-extensions-4.16.0
                
                ## Testing buildpack ##

                Build-time env vars:
                CPATH=/layers/heroku_python/venv/include:/layers/heroku_python/python/include/python3.15:/layers/heroku_python/python/include
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

                Python sys.path:
                ['',
                 '/layers/heroku_python/python/lib/python315.zip',
                 '/layers/heroku_python/python/lib/python3.15',
                 '/layers/heroku_python/python/lib/python3.15/lib-dynload',
                 '/layers/heroku_python/venv/lib/python3.15/site-packages']

                Package manager version and installed packages:
                pip {PIP_VERSION} from /layers/heroku_python/pip/lib/python3.15/site-packages/pip (python 3.15)
                Package           Version
                ----------------- -------
                typing_extensions 4.16.0

                typing_extensions imported from: /layers/heroku_python/venv/lib/python3.15/site-packages/typing_extensions.py
            "}
        );

        // Check that at launch-time:
        // - The correct env vars are set.
        // - pip isn't available.
        // - Python can find the typing-extensions package.
        let command_output = context.run_shell_command(
            indoc! {r#"
                set -euo pipefail
                echo 'Launch-time env vars:'
                printenv | sort | grep -vE '^(_|HOME|HOSTNAME|OLDPWD|PWD|SHLVL)='
                ! command -v pip > /dev/null || { echo 'pip unexpectedly found!' && exit 1; }
                python -c 'import typing_extensions; print(f"typing_extensions imported from: {typing_extensions.__file__}")'
            "#}
        );
        assert_empty!(command_output.stderr);
        assert_eq!(
            command_output.stdout,
            formatdoc! {"
                Launch-time env vars:
                LD_LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib
                PATH=/layers/heroku_python/venv/bin:/layers/heroku_python/python/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PYTHONUNBUFFERED=1
                VIRTUAL_ENV=/layers/heroku_python/venv
                typing_extensions imported from: /layers/heroku_python/venv/lib/python3.15/site-packages/typing_extensions.py
            "}
        );

        context.rebuild(&config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    Using Python version {DEFAULT_PYTHON_VERSION} specified in .python-version
                    
                    [Installing Python]
                    Using cached Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    [Installing pip]
                    Using cached pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Using cached pip download/wheel cache
                    Creating virtual environment
                    Running 'pip install -r requirements.txt'
                    Collecting typing-extensions==4.16.0 (from -r requirements.txt (line 2))
                      Using cached typing_extensions-4.16.0-py3-none-any.whl.metadata (3.3 kB)
                    Using cached typing_extensions-4.16.0-py3-none-any.whl (45 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.16.0
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
                    Using Python version {DEFAULT_PYTHON_VERSION} specified in .python-version
                    
                    [Installing Python]
                    Using cached Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    [Installing pip]
                    Installing pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Creating virtual environment
                    Running 'pip install -r requirements.txt'
                    Collecting typing-extensions==4.16.0 (from -r requirements.txt (line 2))
                      Downloading typing_extensions-4.16.0-py3-none-any.whl.metadata (3.3 kB)
                    Downloading typing_extensions-4.16.0-py3-none-any.whl (45 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.16.0
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
        "docker://docker.io/heroku/buildpack-python:6.8.0".to_string(),
    )]);
    let rebuild_config = default_build_config("tests/fixtures/pip_basic");

    TestRunner::default().build(config, |context| {
        context.rebuild(rebuild_config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    Using Python version {DEFAULT_PYTHON_VERSION} specified in .python-version
                    
                    [Installing Python]
                    Discarding cached Python 3.15.0 since:
                     - The Python version has changed from 3.15.0 to {DEFAULT_PYTHON_FULL_VERSION}
                    Installing Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    [Installing pip]
                    Discarding cached pip 26.2.1
                    Installing pip {PIP_VERSION}
                    
                    [Installing dependencies using pip]
                    Discarding cached pip download/wheel cache
                    Creating virtual environment
                    Running 'pip install -r requirements.txt'
                    Collecting typing-extensions==4.16.0 (from -r requirements.txt (line 2))
                      Downloading typing_extensions-4.16.0-py3-none-any.whl.metadata (3.3 kB)
                    Downloading typing_extensions-4.16.0-py3-none-any.whl (45 kB)
                    Installing collected packages: typing-extensions
                    Successfully installed typing-extensions-4.16.0
                "}
            );
        });
    });
}

// This tests that:
//  - Installing the current project in editable mode using the setuptools build backend works.
//  - Requirements file env var interpolation works (i.e. user-provided env vars have been propagated to pip).
//  - Git from the base image can be found (i.e. the system PATH has been correctly propagated to pip).
//  - The editable mode repository clone is saved into the dependencies layer not the app dir.
//  - Building/compiling a source distribution package (as opposed to a pre-built wheel) works.
//  - The Python headers can be found in the `include/pythonX.Y/` directory of the Python layer.
//  - At launch-time, the editable project's entrypoint script is on PATH and the editable VCS
//    package can be imported.
#[test]
#[ignore = "integration test"]
fn pip_editable_git_compiled() {
    let mut config = default_build_config("tests/fixtures/pip_editable_git_compiled");
    config.env("WHEEL_PACKAGE_URL", "https://github.com/pypa/wheel.git");

    TestRunner::default().build(config, |context| {
        assert_empty!(context.pack_stderr);
        assert_contains!(
            context.pack_stdout,
            "Cloning https://github.com/pypa/wheel.git (to revision 0.45.1) to /layers/heroku_python/venv/src/extension-dist"
        );

        let command_output = context.run_shell_command(indoc! {r#"
            set -euo pipefail
            project-entrypoint
            python -c 'import extension; print(f"extension imported from: {extension.__file__}")'
        "#});
        assert_empty!(command_output.stderr);
        assert_eq!(
            command_output.stdout,
            formatdoc! {"
                project-entrypoint ran from: /workspace/src/pip_editable_git_compiled/__init__.py
                extension imported from: /layers/heroku_python/venv/src/extension-dist/tests/testdata/extension.dist/extension.cpython-315-{ARCH}-linux-gnu.so
            "}
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
        assert_empty!(context.pack_stderr);
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Determining Python version]
                Using Python version 3.10.0 specified in .python-version
                
                [Warning: Support for Python 3.10 is deprecated]
                Python 3.10 reached its upstream end-of-life on 1st October 2026,
                and so no longer receives security updates:
                https://devguide.python.org/versions/#supported-versions
                
                As such, support for Python 3.10 will be removed from this
                buildpack on 6th January 2027.
                
                Upgrade to a newer Python version as soon as possible, by
                changing the version in your .python-version file.
                
                For more information, see:
                https://devcenter.heroku.com/articles/python-support#supported-python-versions
                

                [Installing Python]
                Installing Python 3.10.0
                
                [Installing pip]
                Installing pip {PIP_VERSION}
                
                [Installing dependencies using pip]
                Creating virtual environment
                Running 'pip install -r requirements.txt'
                Collecting typing-extensions==4.16.0 (from -r requirements.txt (line 1))
                  Downloading typing_extensions-4.16.0-py3-none-any.whl.metadata (3.3 kB)
                Downloading typing_extensions-4.16.0-py3-none-any.whl (45 kB)
                Installing collected packages: typing-extensions
                Successfully installed typing-extensions-4.16.0
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
        assert_contains!(
            context.pack_stdout,
            indoc! {"
                [Installing dependencies using pip]
                Creating virtual environment
                Running 'pip install -r requirements.txt'
                ERROR: Invalid requirement: 'an-invalid-requirement!': Expected semicolon (after name with no version specifier) or end
                    an-invalid-requirement!
                                          ^ (from line 1 of requirements.txt)
                
                [Error: Unable to install dependencies using pip]
                The 'pip install -r requirements.txt' command to install the app's
                dependencies failed (exit status: 1).
                
                See the log output above for more information.

                ERROR: failed to build: exit status 1
            "}
        );
    });
}
