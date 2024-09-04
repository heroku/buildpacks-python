use crate::tests::default_build_config;
use indoc::indoc;
use libcnb_test::{assert_contains, PackResult, TestRunner};

#[test]
#[ignore = "integration test"]
fn no_package_manager_detected() {
    TestRunner::default().build(
        default_build_config("tests/fixtures/pyproject_toml_only")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: Couldn't find any supported Python package manager files]
                    Your app must have either a pip requirements file ('requirements.txt')
                    or Poetry lockfile ('poetry.lock') in the root directory of its source
                    code, so your app's dependencies can be installed.
                    
                    If your app already has one of those files, check that it:
                    
                    1. Is in the top level directory (not a subdirectory).
                    2. Has the correct spelling (the filenames are case-sensitive).
                    3. Isn't excluded by '.gitignore' or 'project.toml'.
                    
                    Otherwise, add a package manager file to your app. If your app has
                    no dependencies, then create an empty 'requirements.txt' file.
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn multiple_package_managers_detected() {
    TestRunner::default().build(
        default_build_config("tests/fixtures/pip_and_poetry")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: Multiple Python package manager files were found]
                    Exactly one package manager file must be present in your app's source code,
                    however, several were found:
                    
                    requirements.txt (pip)
                    poetry.lock (Poetry)
                    
                    Decide which package manager you want to use with your app, and then delete
                    the file(s) and any config from the others.
                "}
            );
        },
    );
}
