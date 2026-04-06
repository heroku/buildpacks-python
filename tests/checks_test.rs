use crate::tests::default_build_config;
use indoc::indoc;
use libcnb_test::{PackResult, TestRunner, assert_contains};

#[test]
#[ignore = "integration test"]
fn checks_reject_forbidden_env_vars() {
    let mut config = default_build_config("tests/fixtures/pyproject_toml_only");
    config.env("PYTHONHOME", "/invalid");
    config.env("VIRTUAL_ENV", "/invalid");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stdout,
            indoc! {"
                [Error: Unsafe environment variable(s) found]
                The following environment variable(s) can cause problems
                with the build so we don't allow using them:

                PYTHONHOME
                VIRTUAL_ENV

                You must unset the above env var(s). If you didn't set them
                yourself, check if they were set by an earlier buildpack.

                ERROR: failed to build: exit status 1
            "}
        );
    });
}
