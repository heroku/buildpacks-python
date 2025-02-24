use crate::tests::default_build_config;
use indoc::indoc;
use libcnb_test::{PackResult, TestRunner, assert_contains};

#[test]
#[ignore = "integration test"]
fn detect_rejects_non_python_projects() {
    TestRunner::default().build(
        default_build_config("tests/fixtures/empty").expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stdout,
                indoc! {"========
                    No Python project files found (such as pyproject.toml, requirements.txt or poetry.lock).
                    ======== Results ========
                "}
            );
        },
    );
}
