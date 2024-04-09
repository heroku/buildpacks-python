//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run them use: `cargo test -- --ignored`.
//! These tests are not run via automatic integration test discovery, but instead are
//! imported in main.rs so that they have access to private APIs (see comment in main.rs).

use std::env;

mod detect_test;
mod django_test;
mod package_manager_test;
mod pip_test;
mod python_version_test;

const LATEST_PYTHON_3_7: &str = "3.7.17";
const LATEST_PYTHON_3_8: &str = "3.8.19";
const LATEST_PYTHON_3_9: &str = "3.9.19";
const LATEST_PYTHON_3_10: &str = "3.10.14";
const LATEST_PYTHON_3_11: &str = "3.11.9";
const LATEST_PYTHON_3_12: &str = "3.12.3";
const DEFAULT_PYTHON_VERSION: &str = LATEST_PYTHON_3_12;

const DEFAULT_BUILDER: &str = "heroku/builder:22";

fn builder() -> String {
    env::var("INTEGRATION_TEST_CNB_BUILDER").unwrap_or(DEFAULT_BUILDER.to_string())
}
