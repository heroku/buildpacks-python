//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run the tests use: `cargo test -- --ignored`

#![warn(clippy::pedantic)]

use indoc::indoc;
use libcnb_test::{assert_contains, assert_empty, BuildConfig, TestRunner};

#[test]
#[ignore = "integration test"]
fn default_python_version() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "test-fixtures/default"),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Determining Python version]
                    No Python version specified, using default of 3.10.5

                    [Installing Python]
                    Downloading Python 3.10.5
                    Python installation successful

                    [Installing Pip]
                    Installing pip 22.2, setuptools 63.2.0 and wheel 0.37.1
                    Installation completed

                    [Installing dependencies using Pip]
                    Pip cache created
                "}
            );
        },
    );
}
