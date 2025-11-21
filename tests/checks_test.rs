use crate::tests::default_build_config;
use indoc::indoc;
use libcnb_test::{PackResult, TestRunner, assert_contains};

#[test]
#[ignore = "integration test"]
fn checks_reject_pythonhome_env_var() {
    let mut config = default_build_config("tests/fixtures/pyproject_toml_only");
    config.env("PYTHONHOME", "/invalid");
    config.expected_pack_result(PackResult::Failure);

    TestRunner::default().build(config, |context| {
        assert_contains!(
            context.pack_stdout,
            indoc! {"
                [Error: Unsafe environment variable found]
                The environment variable `PYTHONHOME` is set, however, it can
                cause problems with the build so we don't allow using it.

                You must unset that environment variable. If you didn't set it
                yourself, check that it wasn't set by an earlier buildpack.

                ERROR: failed to build: exit status 1
            "}
        );
    });
}
