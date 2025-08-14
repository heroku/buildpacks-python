use crate::tests::default_build_config;
use indoc::indoc;
use libcnb_test::{PackResult, TestRunner, assert_contains};

#[test]
#[ignore = "integration test"]
fn no_package_manager_detected() {
    TestRunner::default().build(
        default_build_config("tests/fixtures/pyproject_toml_only")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Error: Couldn't find any supported Python package manager files]
                    Your app must have either a 'requirements.txt', 'poetry.lock'
                    or 'uv.lock' package manager file in the root directory of its
                    source code, so its dependencies can be installed.
                    
                    If your app already has one of those files, check that it:
                    
                    1. Is in the top level directory (not a subdirectory).
                    2. Has the correct spelling (the filenames are case-sensitive).
                    3. Isn't excluded by '.gitignore' or 'project.toml'.
                    4. Has been added to the Git repository using 'git add --all'
                       and then committed using 'git commit'.
                    
                    Otherwise, add a package manager file to your app. If your app has
                    no dependencies, then create an empty 'requirements.txt' file.

                    If you aren't sure which package manager to use, we recommend
                    trying uv, since it supports lockfiles, is extremely fast, and
                    is actively maintained by a full-time team:
                    https://docs.astral.sh/uv/

                    For help with using Python on Heroku, see:
                    https://devcenter.heroku.com/articles/getting-started-with-python-fir
                    https://devcenter.heroku.com/articles/python-support

                    ERROR: failed to build: exit status 1
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn multiple_package_managers_detected() {
    TestRunner::default().build(
        default_build_config("tests/fixtures/multiple_package_managers")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Error: Multiple Python package manager files were found]
                    Exactly one package manager file must be present in your app's
                    source code, however, several were found:
                    
                    requirements.txt (pip)
                    poetry.lock (Poetry)
                    uv.lock (uv)
                    
                    Decide which package manager you want to use with your app, and
                    then delete the file(s) and any config from the others.

                    If you aren't sure which package manager to use, we recommend
                    trying uv, since it supports lockfiles, is extremely fast, and
                    is actively maintained by a full-time team:
                    https://docs.astral.sh/uv/

                    ERROR: failed to build: exit status 1
                "}
            );
        },
    );
}
