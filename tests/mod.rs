//! All integration tests are skipped by default (using the `ignore` attribute),
//! since performing builds is slow. To run them use: `cargo test -- --ignored`.
//! These tests are not run via automatic integration test discovery, but instead are
//! imported in main.rs so that they have access to private APIs (see comment in main.rs).

mod checks_test;
mod detect_test;
mod django_test;
mod package_manager_test;
mod pip_test;
mod poetry_test;
mod python_version_test;

use libcnb_test::BuildConfig;
use std::env;
use std::path::Path;

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
        ("PIP_DISABLE_PIP_VERSION_CHECK", "0"),
        ("PKG_CONFIG_PATH", "/invalid"),
        ("PYTHONPATH", "/invalid"),
    ]);

    config
}

fn builder() -> String {
    env::var("INTEGRATION_TEST_BUILDER").unwrap_or(DEFAULT_BUILDER.to_string())
}
