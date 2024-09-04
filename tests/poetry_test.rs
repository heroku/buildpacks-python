use crate::packaging_tool_versions::POETRY_VERSION;
use crate::tests::{default_build_config, DEFAULT_PYTHON_VERSION};
use indoc::{formatdoc, indoc};
use libcnb_test::{assert_contains, assert_empty, BuildpackReference, PackResult, TestRunner};

#[test]
#[ignore = "integration test"]
fn poetry_basic_install_and_cache_reuse() {
    let mut config = default_build_config("tests/fixtures/poetry_basic");
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
                
                [Installing Python]
                Installing Python {DEFAULT_PYTHON_VERSION}
                
                [Installing Poetry]
                Installing Poetry {POETRY_VERSION}
                
                [Installing dependencies using Poetry]
                Creating virtual environment
                Running 'poetry install --sync --only main'
                Installing dependencies from lock file
                
                Package operations: 1 install, 0 updates, 0 removals
                
                  - Installing typing-extensions (4.12.2)
                
                ## Testing buildpack ##
                CPATH=/layers/heroku_python/venv/include:/layers/heroku_python/python/include/python3.12:/layers/heroku_python/python/include
                LANG=C.UTF-8
                LD_LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib:/layers/heroku_python/poetry/lib
                LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib:/layers/heroku_python/poetry/lib
                PATH=/layers/heroku_python/venv/bin:/layers/heroku_python/python/bin:/layers/heroku_python/poetry/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PKG_CONFIG_PATH=/layers/heroku_python/python/lib/pkgconfig
                PYTHONHOME=/layers/heroku_python/python
                PYTHONUNBUFFERED=1
                PYTHONUSERBASE=/layers/heroku_python/poetry
                SOURCE_DATE_EPOCH=315532801
                VIRTUAL_ENV=/layers/heroku_python/venv
                
                ['',
                 '/layers/heroku_python/python/lib/python312.zip',
                 '/layers/heroku_python/python/lib/python3.12',
                 '/layers/heroku_python/python/lib/python3.12/lib-dynload',
                 '/layers/heroku_python/venv/lib/python3.12/site-packages']
                
                Poetry (version {POETRY_VERSION})
                typing-extensions 4.12.2 Backported and Experimental Type Hints for Python ...
                <module 'typing_extensions' from '/layers/heroku_python/venv/lib/python3.12/site-packages/typing_extensions.py'>
            "}
        );

        // Check that at run-time:
        // - The correct env vars are set.
        // - Poetry isn't available.
        // - Python can find the typing-extensions package.
        let command_output = context.run_shell_command(
            indoc! {"
                set -euo pipefail
                printenv | sort | grep -vE '^(_|HOME|HOSTNAME|OLDPWD|PWD|SHLVL)='
                ! command -v poetry > /dev/null || { echo 'Poetry unexpectedly found!' && exit 1; }
                python -c 'import typing_extensions'
            "}
        );
        assert_empty!(command_output.stderr);
        assert_eq!(
            command_output.stdout,
            formatdoc! {"
                LANG=C.UTF-8
                LD_LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib
                PATH=/layers/heroku_python/venv/bin:/layers/heroku_python/python/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PYTHONHOME=/layers/heroku_python/python
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
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python]
                    Using cached Python {DEFAULT_PYTHON_VERSION}
                    
                    [Installing Poetry]
                    Using cached Poetry {POETRY_VERSION}
                    
                    [Installing dependencies using Poetry]
                    Using cached virtual environment
                    Running 'poetry install --sync --only main'
                    Installing dependencies from lock file
                    
                    No dependencies to install or update
                "}
            );
        });
    });
}

#[test]
#[ignore = "integration test"]
fn poetry_cache_invalidation_package_manager_changed() {
    let config = default_build_config("tests/fixtures/pip_basic");
    let rebuild_config = default_build_config("tests/fixtures/poetry_basic");

    TestRunner::default().build(config, |context| {
        context.rebuild(rebuild_config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python]
                    Using cached Python {DEFAULT_PYTHON_VERSION}
                    
                    [Installing Poetry]
                    Installing Poetry {POETRY_VERSION}
                    
                    [Installing dependencies using Poetry]
                    Creating virtual environment
                    Running 'poetry install --sync --only main'
                    Installing dependencies from lock file
                    
                    Package operations: 1 install, 0 updates, 0 removals
                    
                      - Installing typing-extensions (4.12.2)
                "}
            );
        });
    });
}

// This tests that cached layers from a previous buildpack version are compatible, or if we've
// decided to break compatibility recently, that the layers are at least invalidated gracefully.
#[test]
#[ignore = "integration test"]
fn poetry_cache_previous_buildpack_version() {
    let mut config = default_build_config("tests/fixtures/poetry_basic");
    config.buildpacks([BuildpackReference::Other(
        "docker://docker.io/heroku/buildpack-python:0.17.0".to_string(),
    )]);
    let rebuild_config = default_build_config("tests/fixtures/poetry_basic");

    TestRunner::default().build(config, |context| {
        context.rebuild(rebuild_config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    [Determining Python version]
                    No Python version specified, using the current default of Python {DEFAULT_PYTHON_VERSION}.
                    To use a different version, see: https://devcenter.heroku.com/articles/python-runtimes
                    
                    [Installing Python]
                    Using cached Python {DEFAULT_PYTHON_VERSION}
                    
                    [Installing Poetry]
                    Using cached Poetry {POETRY_VERSION}
                    
                    [Installing dependencies using Poetry]
                    Using cached virtual environment
                    Running 'poetry install --sync --only main'
                    Installing dependencies from lock file
                    
                    No dependencies to install or update
                "}
            );
        });
    });
}

// This tests that:
//  - Git from the stack image can be found (ie: the system PATH has been correctly propagated to Poetry).
//  - The editable mode repository clone is saved into the dependencies layer not the app dir.
//  - Compiling a source distribution package (as opposed to a pre-built wheel) works.
//  - The Python headers can be found in the `include/pythonX.Y/` directory of the Python layer.
#[test]
#[ignore = "integration test"]
fn poetry_editable_git_compiled() {
    let config = default_build_config("tests/fixtures/poetry_editable_git_compiled");

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stdout,
            indoc! {"
                [Installing dependencies using Poetry]
                Creating virtual environment
                Running 'poetry install --sync --only main'
                Installing dependencies from lock file
                
                Package operations: 1 install, 0 updates, 0 removals
                
                  - Installing extension-dist (0.1 7bb46d7)
            "}
        );

        let command_output =
            context.run_shell_command("python -c 'import extension; print(extension)'");
        assert_empty!(command_output.stderr);
        assert_contains!(
            command_output.stdout,
            "<module 'extension' from '/layers/heroku_python/venv/src/wheel/"
        );
    });
}

#[test]
#[ignore = "integration test"]
fn poetry_install_error() {
    let mut config = default_build_config("tests/fixtures/poetry_outdated_lockfile");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        // Ideally we could test a combined stdout/stderr, however libcnb-test doesn't support this:
        // https://github.com/heroku/libcnb.rs/issues/536
        assert_contains!(
            context.pack_stdout,
            indoc! {"
                    [Installing dependencies using Poetry]
                    Creating virtual environment
                    Running 'poetry install --sync --only main'
                    Installing dependencies from lock file
                "}
        );
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                pyproject.toml changed significantly since poetry.lock was last generated. Run `poetry lock [--no-update]` to fix the lock file.

                [Error: Unable to install dependencies using Poetry]
                The 'poetry install --sync --only main' command to install the app's
                dependencies failed (exit status: 1).

                See the log output above for more information.
            "}
        );
    });
}
