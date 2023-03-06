//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run the tests use: `cargo test -- --ignored`

use std::env;

mod detect;
mod package_manager;
mod pip;
mod python_version;
mod salesforce_functions;

const LATEST_PYTHON_3_7: &str = "3.7.16";
const LATEST_PYTHON_3_8: &str = "3.8.16";
const LATEST_PYTHON_3_9: &str = "3.9.16";
const LATEST_PYTHON_3_10: &str = "3.10.10";
const LATEST_PYTHON_3_11: &str = "3.11.2";
const DEFAULT_PYTHON_VERSION: &str = LATEST_PYTHON_3_11;

const DEFAULT_BUILDER: &str = "heroku/builder:22";

fn builder() -> String {
    env::var("INTEGRATION_TEST_CNB_BUILDER").unwrap_or(DEFAULT_BUILDER.to_string())
}
