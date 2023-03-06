use crate::integration_tests::builder;
use indoc::indoc;
use libcnb_test::{
    assert_contains, assert_empty, BuildConfig, ContainerConfig, PackResult, TestRunner,
};
use std::thread;
use std::time::Duration;

const TEST_PORT: u16 = 12345;

#[test]
#[ignore = "integration test"]
fn salesforce_function_template() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/salesforce_function_template"),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Validating Salesforce Function]
                    Function passed validation.
                    ===> EXPORTING
                "}
            );
            assert_contains!(context.pack_stdout, "Setting default process type 'web'");

            // Test that the `sf-functions-python` web process the buildpack configures works correctly.
            context.start_container(
                ContainerConfig::new()
                    .env("PORT", TEST_PORT.to_string())
                    .expose_port(TEST_PORT),
                |container| {
                    let address_on_host = container.address_for_port(TEST_PORT).unwrap();
                    let url = format!("http://{}:{}", address_on_host.ip(), address_on_host.port());

                    // Retries needed since the server takes a moment to start up.
                    let mut attempts_remaining = 5;
                    let response = loop {
                        let response = ureq::post(&url).set("x-health-check", "true").call();
                        if response.is_ok() || attempts_remaining == 0 {
                            break response;
                        }
                        attempts_remaining -= 1;
                        thread::sleep(Duration::from_secs(1));
                    };

                    let server_log_output = container.logs_now();
                    assert_contains!(
                        server_log_output.stderr,
                        &format!("Uvicorn running on http://0.0.0.0:{TEST_PORT}")
                    );

                    let body = response.unwrap().into_string().unwrap();
                    assert_eq!(body, r#""OK""#);
                },
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn salesforce_function_missing_package() {
    TestRunner::default().build(
        BuildConfig::new(
            builder(),
            "tests/fixtures/salesforce_function_missing_package",
        )
        .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {r#"
                    [Error: The Salesforce Functions package is not installed]
                    The 'sf-functions-python' program that is required for Python Salesforce
                    Functions could not be found.
                    
                    Check that the 'salesforce-functions' Python package is listed as a
                    dependency in 'requirements.txt'.
                    
                    If this project is not intended to be a Salesforce Function, remove the
                    'type = "function"' declaration from 'project.toml' to skip this check.
                "#}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn salesforce_function_fails_self_check() {
    TestRunner::default().build(
        BuildConfig::new(
            builder(),
            "tests/fixtures/salesforce_function_fails_self_check",
        )
        .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: The Salesforce Functions self-check failed]
                    The 'sf-functions-python check' command failed (exit status: 1), indicating
                    there is a problem with the Python Salesforce Function in this project.
                    
                    Details:
                    Function failed validation: 'invalid' isn't a valid Salesforce REST API version."
                }
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn project_toml_invalid() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/project_toml_invalid")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stderr,
                indoc! {r#"
                    [Error: Invalid project.toml]
                    A parsing/validation error error occurred whilst loading the project.toml file.
                    
                    Details: TOML parse error at line 4, column 1
                      |
                    4 | [com.salesforce]
                      | ^^^^^^^^^^^^^^^^
                    missing field `type`
                "#}
            );
        },
    );
}
