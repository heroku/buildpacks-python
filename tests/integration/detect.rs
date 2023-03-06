use crate::integration_tests::builder;
use indoc::formatdoc;
use libcnb::data::buildpack::{BuildpackVersion, SingleBuildpackDescriptor};
use libcnb_test::{assert_contains, BuildConfig, PackResult, TestRunner};
use std::fs;

#[test]
#[ignore = "integration test"]
fn detect_rejects_non_python_projects() {
    let buildpack_version = buildpack_version();

    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/empty")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stdout,
                &formatdoc! {"
                    ===> DETECTING
                    ======== Output: heroku/python@{buildpack_version} ========
                    No Python project files found (such as requirements.txt).
                    ======== Results ========
                    fail: heroku/python@{buildpack_version}
                    ERROR: No buildpack groups passed detection.
                "}
            );
        },
    );
}

fn buildpack_version() -> BuildpackVersion {
    let buildpack_toml = fs::read_to_string("buildpack.toml").unwrap();
    let buildpack_descriptor =
        toml::from_str::<SingleBuildpackDescriptor<Option<()>>>(&buildpack_toml).unwrap();
    buildpack_descriptor.buildpack.version
}
