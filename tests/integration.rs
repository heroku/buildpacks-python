//! Integration tests using libcnb-test.
//!
//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run the tests use: `cargo test -- --ignored`

#![warn(clippy::pedantic)]

use libcnb_test::IntegrationTest;

#[test]
#[ignore = "integration test"]
#[should_panic(expected = "pack command failed with exit-code 1!
pack stdout:
pack stderr:
ERROR: failed to build: failed to fetch builder image 'index.docker.io/libcnb/void-builder:doesntexist': image 'index.docker.io/libcnb/void-builder:doesntexist' does not exist on the daemon: not found
")]
fn panic_on_unsuccessful_pack_run() {
    IntegrationTest::new("heroku/buildpacks:20", "test-fixtures/empty").run_test(|_context| {
        // context
        //     .prepare_container()
        //     .start_with_shell_command("env", |container| {
        //         let env_stdout = container.logs_wait().stdout;

        //         assert_contains!(env_stdout, "ROLL_1D6=");
        //         assert_contains!(env_stdout, "ROLL_4D6=");
        //         assert_contains!(env_stdout, "ROLL_1D20=");
        //     });
    });
}
