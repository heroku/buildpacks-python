use crate::packaging_tool_versions::UV_VERSION;
use crate::python_version::{DEFAULT_PYTHON_FULL_VERSION, DEFAULT_PYTHON_VERSION};
use crate::tests::default_build_config;
use indoc::{formatdoc, indoc};
use libcnb_test::{
    BuildpackReference, PackResult, TestRunner, assert_contains, assert_contains_match,
    assert_empty,
};

#[test]
#[ignore = "integration test"]
fn uv_basic_install_and_cache_reuse() {
    let mut config = default_build_config("tests/fixtures/uv_basic");
    config.buildpacks(vec![
        BuildpackReference::CurrentCrate,
        BuildpackReference::Other("file://tests/fixtures/testing_buildpack".to_string()),
    ]);

    TestRunner::default().build(&config, |context| {
        assert_empty!(context.pack_stderr);
        assert_contains_match!(
            context.pack_stdout,
            &formatdoc! {"
                \\[Determining Python version\\]
                Using Python version {DEFAULT_PYTHON_VERSION} specified in .python-version
                
                \\[Installing Python\\]
                Installing Python {DEFAULT_PYTHON_FULL_VERSION}
                
                \\[Installing uv\\]
                Installing uv {UV_VERSION}
                
                \\[Installing dependencies using uv\\]
                Creating virtual environment
                Running 'uv sync --locked --no-default-groups'
                Resolved 7 packages in .+s
                Prepared 1 package in .+s
                Installed 1 package in .+s
                Bytecode compiled 1 file in .+s
                 \\+ typing-extensions==4.13.2
                
                ## Testing buildpack ##
                CPATH=/layers/heroku_python/venv/include:/layers/heroku_python/python/include/python3.13:/layers/heroku_python/python/include
                LD_LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib
                LIBRARY_PATH=/layers/heroku_python/venv/lib:/layers/heroku_python/python/lib
                PATH=/layers/heroku_python/venv/bin:/layers/heroku_python/uv/bin:/layers/heroku_python/python/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PKG_CONFIG_PATH=/layers/heroku_python/python/lib/pkgconfig
                PYTHONUNBUFFERED=1
                SOURCE_DATE_EPOCH=315532801
                UV_CACHE_DIR=/layers/heroku_python/uv-cache
                UV_NO_MANAGED_PYTHON=1
                UV_PROJECT_ENVIRONMENT=/layers/heroku_python/venv
                UV_PYTHON_DOWNLOADS=never
                VIRTUAL_ENV=/layers/heroku_python/venv
                
                \\['',
                 '/layers/heroku_python/python/lib/python313.zip',
                 '/layers/heroku_python/python/lib/python3.13',
                 '/layers/heroku_python/python/lib/python3.13/lib-dynload',
                 '/layers/heroku_python/venv/lib/python3.13/site-packages'\\]
                
                uv {UV_VERSION}
                Using Python {DEFAULT_PYTHON_FULL_VERSION} environment at: /layers/heroku_python/venv
                Package           Version
                ----------------- -------
                typing-extensions 4.13.2
                <module 'typing_extensions' from '/layers/heroku_python/venv/lib/python3.13/site-packages/typing_extensions.py'>
            "}
        );

        // Check that at run-time:
        // - The correct env vars are set.
        // - uv isn't available.
        // - Python can find the typing-extensions package.
        let command_output = context.run_shell_command(
            indoc! {"
                set -euo pipefail
                printenv | sort | grep -vE '^(_|HOME|HOSTNAME|OLDPWD|PWD|SHLVL)='
                ! command -v uv > /dev/null || { echo 'uv unexpectedly found!' && exit 1; }
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
            assert_contains_match!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    \\[Determining Python version\\]
                    Using Python version {DEFAULT_PYTHON_VERSION} specified in .python-version
                    
                    \\[Installing Python\\]
                    Using cached Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    \\[Installing uv\\]
                    Using cached uv {UV_VERSION}
                    
                    \\[Installing dependencies using uv\\]
                    Using cached virtual environment
                    Running 'uv sync --locked --no-default-groups'
                    Resolved 7 packages in .+s
                    Bytecode compiled 1 file in .+s
                    
                    ## Testing buildpack ##
                "}
            );
        });
    });
}

#[test]
#[ignore = "integration test"]
fn uv_cache_invalidation_package_manager_changed() {
    let config = default_build_config("tests/fixtures/poetry_basic");
    let rebuild_config = default_build_config("tests/fixtures/uv_basic");

    TestRunner::default().build(config, |context| {
        context.rebuild(rebuild_config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains_match!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    \\[Determining Python version\\]
                    Using Python version {DEFAULT_PYTHON_VERSION} specified in .python-version
                    
                    \\[Installing Python\\]
                    Using cached Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    \\[Installing uv\\]
                    Installing uv {UV_VERSION}
                    
                    \\[Installing dependencies using uv\\]
                    Discarding cached virtual environment
                    Creating virtual environment
                    Running 'uv sync --locked --no-default-groups'
                    Resolved 7 packages in .+s
                    Prepared 1 package in .+s
                    Installed 1 package in .+s
                    Bytecode compiled 1 file in .+s
                     \\+ typing-extensions==4.13.2
                "}
            );
        });
    });
}

// This tests that cached layers from a previous buildpack version are compatible, or if we've
// decided to break compatibility recently, that the layers are at least invalidated gracefully.
#[test]
#[ignore = "integration test"]
fn uv_cache_previous_buildpack_version() {
    let mut config = default_build_config("tests/fixtures/uv_basic");
    config.buildpacks([BuildpackReference::Other(
        "docker://docker.io/heroku/buildpack-python:2.0.0".to_string(),
    )]);
    let rebuild_config = default_build_config("tests/fixtures/uv_basic");

    TestRunner::default().build(config, |context| {
        context.rebuild(rebuild_config, |rebuild_context| {
            assert_empty!(rebuild_context.pack_stderr);
            assert_contains_match!(
                rebuild_context.pack_stdout,
                &formatdoc! {"
                    \\[Determining Python version\\]
                    Using Python version {DEFAULT_PYTHON_VERSION} specified in .python-version
                    
                    \\[Installing Python\\]
                    Discarding cached Python 3.13.3 since:
                     - The Python version has changed from 3.13.3 to {DEFAULT_PYTHON_FULL_VERSION}
                    Installing Python {DEFAULT_PYTHON_FULL_VERSION}
                    
                    \\[Installing uv\\]
                    Discarding cached uv 0.7.3
                    Installing uv {UV_VERSION}
                    
                    \\[Installing dependencies using uv\\]
                    Discarding cached virtual environment
                    Creating virtual environment
                    Running 'uv sync --locked --no-default-groups'
                    Resolved 7 packages in .+s
                    Prepared 1 package in .+s
                    Installed 1 package in .+s
                    Bytecode compiled 1 file in .+s
                     \\+ typing-extensions==4.13.2
                "}
            );
        });
    });
}

// This tests that:
//  - Installing the current project in editable mode using the uv-build buildpack works.
//  - Git from the stack image can be found (ie: the system PATH has been correctly propagated to uv).
//  - Building/compiling a source distribution package (as opposed to a pre-built wheel) works.
//  - The Python headers can be found in the `include/pythonX.Y/` directory of the Python layer.
#[test]
#[ignore = "integration test"]
fn uv_editable_git_compiled() {
    let config = default_build_config("tests/fixtures/uv_editable_git_compiled");

    TestRunner::default().build(config, |context| {
        assert_empty!(context.pack_stderr);
        assert_contains_match!(
            context.pack_stdout,
            indoc! {"
                \\[Installing dependencies using uv\\]
                Creating virtual environment
                Running 'uv sync --locked --no-default-groups'
                Resolved 2 packages in .+ms
                   (?s:.)+
                Prepared 2 packages in .+s
                Installed 2 packages in .+s
                Bytecode compiled 0 files in .+s
                 \\+ extension-dist==0.1 \\(from git\\+https://github.com/pypa/wheel.git@7bb46d7727e6e89fe56b3c78297b3af2672bbbe2#subdirectory=tests/testdata/extension.dist\\)
                 \\+ uv-editable-git-compiled==0.0.0 \\(from file:///workspace\\)
            "}
        );
    });
}

// This checks that our chosen uv version supports our oldest supported Python version.
#[test]
#[ignore = "integration test"]
fn uv_oldest_python() {
    let config = default_build_config("tests/fixtures/uv_oldest_python");

    TestRunner::default().build(config, |context| {
        // We can't `assert_empty!(context.pack_stderr)` here, due to the Python 3.9 deprecation warning.
        assert_contains_match!(
            context.pack_stdout,
            &formatdoc! {"
                \\[Determining Python version\\]
                Using Python version 3.9.0 specified in .python-version
                
                \\[Installing Python\\]
                Installing Python 3.9.0
                
                \\[Installing uv\\]
                Installing uv {UV_VERSION}
                
                \\[Installing dependencies using uv\\]
                Creating virtual environment
                Running 'uv sync --locked --no-default-groups'
                Resolved 2 packages in .+s
                Prepared 1 package in .+s
                Installed 1 package in .+s
                Bytecode compiled 1 file in .+s
                 \\+ typing-extensions==4.13.2
            "}
        );
    });
}

// This tests the error message when there is no .python-version file, and in particular the case where
// the buildpack's default Python version is not compatible with `requires-python` in pyproject.toml.
// (Since we must prevent uv from downloading its own Python or using system Python, and also want a
// clearer error message than using `--python` or `UV_PYTHON` would give us).
#[test]
#[ignore = "integration test"]
fn uv_no_python_version_file() {
    let mut config = default_build_config("tests/fixtures/uv_no_python_version_file");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        // Ideally we could test a combined stdout/stderr, however libcnb-test doesn't support this:
        // https://github.com/heroku/libcnb.rs/issues/536
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Determining Python version]
            "}
        );
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                [Error: No Python version was specified]
                When using the package manager uv on Heroku, you must specify
                your app's Python version with a .python-version file.

                To add a .python-version file:

                1. Make sure you are in the root directory of your app
                   and not a subdirectory.
                2. Run 'uv python pin 3.13'
                   (adjust to match your app's major Python version).
                3. Commit the changes to your Git repository using
                   'git add --all' and then 'git commit'.

                We strongly recommend that you don't specify the Python patch
                version number, since it will pin your app to an exact Python
                version and so stop your app from receiving security updates
                each time it builds.
            "}
        );
    });
}

// This tests the error message when a runtime.txt is present, and in particular the case where
// the runtime.txt version is not compatible with `requires-python` in pyproject.toml. (Since we
// must prevent uv from downloading its own Python or using system Python, and also want a clearer
// error message than using `--python` or `UV_PYTHON` would give us).
#[test]
#[ignore = "integration test"]
fn uv_runtime_txt() {
    let mut config = default_build_config("tests/fixtures/uv_runtime_txt");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Determining Python version]
            "}
        );
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                [Error: The runtime.txt file isn't supported]
                The runtime.txt file can longer be used, since it has been
                replaced by the more widely supported .python-version file.

                Please switch to a .python-version file instead:

                1. Make sure you are in the root directory of your app
                   and not a subdirectory.
                2. Delete your runtime.txt file.
                3. Run 'uv python pin 3.13'
                   (adjust to match your app's major Python version).
                4. Commit the changes to your Git repository using
                   'git add --all' and then 'git commit'.

                We strongly recommend that you don't specify the Python patch
                version number, since it will pin your app to an exact Python
                version and so stop your app from receiving security updates
                each time it builds.
            "}
        );
    });
}

// This tests the error message when `requires-python` in pyproject.toml isn't compatible with
// the version in .python-version. This might seem unnecessary since it's testing something uv
// validates itself, however, the quality of the error message here depends on what uv options
// we use (for example, using `--python` or `UV_PYTHON` results in a worse error message).
#[test]
#[ignore = "integration test"]
fn uv_mismatched_python_version() {
    let mut config = default_build_config("tests/fixtures/uv_mismatched_python_version");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stdout,
            &formatdoc! {"
                [Installing dependencies using uv]
                Creating virtual environment
                Running 'uv sync --locked --no-default-groups'
                Using CPython {DEFAULT_PYTHON_FULL_VERSION} interpreter at: /layers/heroku_python/python/bin/python3.13
                error: The Python request from `.python-version` resolved to Python {DEFAULT_PYTHON_FULL_VERSION}, which is incompatible with the project's Python requirement: `==3.12.*` (from `project.requires-python`)
                Use `uv python pin` to update the `.python-version` file to a compatible version
            "}
        );
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                [Error: Unable to install dependencies using uv]
                The 'uv sync' command to install the app's
                dependencies failed (exit status: 2).

                See the log output above for more information.
            "}
        );
    });
}

#[test]
#[ignore = "integration test"]
fn uv_lockfile_out_of_sync() {
    let mut config = default_build_config("tests/fixtures/uv_lockfile_out_of_sync");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains_match!(
            context.pack_stdout,
            indoc! {"
                \\[Installing dependencies using uv\\]
                Creating virtual environment
                Running 'uv sync --locked --no-default-groups'
                Resolved 2 packages in .+s
                error: The lockfile at `uv.lock` needs to be updated, but `--locked` was provided. To update the lockfile, run `uv lock`.
            "}
        );
        assert_contains!(
            context.pack_stderr,
            indoc! {"
                [Error: Unable to install dependencies using uv]
                The 'uv sync' command to install the app's
                dependencies failed (exit status: 2).

                See the log output above for more information.
            "}
        );
    });
}
