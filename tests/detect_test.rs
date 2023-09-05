use crate::tests::builder;
use indoc::indoc;
use libcnb_test::{assert_contains, BuildConfig, PackResult, TestRunner};

#[test]
#[ignore = "integration test"]
fn detect_rejects_non_python_projects() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/empty")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stdout,
                indoc! {"========
                    No Python project files found (such as requirements.txt).
                    ======== Results ========
                "}
            );
        },
    );
}
