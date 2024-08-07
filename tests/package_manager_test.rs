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
                    [Error: No Python package manager files were found]
                    A pip requirements file was not found in your application's source code.
                    This file is required so that your application's dependencies can be installed.
                    
                    Please add a file named exactly 'requirements.txt' to the root directory of your
                    application, containing a list of the packages required by your application.
                    
                    For more information on what this file should contain, see:
                    https://pip.pypa.io/en/stable/reference/requirements-file-format/
                "}
            );
        },
    );
}
