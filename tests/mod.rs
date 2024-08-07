//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run them use: `cargo test -- --ignored`.
//! These tests are not run via automatic integration test discovery, but instead are
//! imported in main.rs so that they have access to private APIs (see comment in main.rs).

use libcnb_test::BuildConfig;
use std::env;
use std::path::Path;

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
const LATEST_PYTHON_3_12: &str = "3.12.4";
const DEFAULT_PYTHON_VERSION: &str = LATEST_PYTHON_3_12;

const DEFAULT_BUILDER: &str = "heroku/builder:24";

fn default_build_config(fixture_path: impl AsRef<Path>) -> BuildConfig {
    let builder = builder();
    let mut config = BuildConfig::new(&builder, fixture_path);

    // TODO: Once Pack build supports `--platform` and libcnb-test adjusted accordingly, change this
    // to allow configuring the target arch independently of the builder name (eg via env var).
    let target_triple = match builder.as_str() {
        // Compile the buildpack for ARM64 iff the builder supports multi-arch and the host is ARM64.
        "heroku/builder:24" if cfg!(target_arch = "aarch64") => "aarch64-unknown-linux-musl",
        _ => "x86_64-unknown-linux-musl",
    };
    config.target_triple(target_triple);

    // Ensure that potentially broken user-provided env vars don't take precedence over those set
    // by this buildpack and break running Python/pip. Some of these are based on the env vars that
    // used to be set by `bin/release` by very old versions of the classic Python buildpack:
    // https://github.com/heroku/heroku-buildpack-python/blob/27abdfe7d7ad104dabceb45641415251e965671c/bin/release#L11-L18
    config.envs([
        ("CPATH", "/invalid"),
        ("LD_LIBRARY_PATH", "/invalid"),
        ("LIBRARY_PATH", "/invalid"),
        ("PATH", "/invalid"),
        ("PIP_CACHE_DIR", "/invalid"),
        ("PIP_DISABLE_PIP_VERSION_CHECK", "0"),
        ("PKG_CONFIG_PATH", "/invalid"),
        ("PYTHONHOME", "/invalid"),
        ("PYTHONPATH", "/invalid"),
        ("PYTHONUSERBASE", "/invalid"),
    ]);

    config
}

fn builder() -> String {
    env::var("INTEGRATION_TEST_BUILDER").unwrap_or(DEFAULT_BUILDER.to_string())
}
