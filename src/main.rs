#![warn(clippy::pedantic)]
#![warn(unused_crate_dependencies)]

mod errors;
mod layers;
mod python_version;
mod runtime_txt;
mod utils;

use crate::errors::PythonBuildpackError;
use crate::layers::pip::PipLayer;
use crate::layers::pip_cache::PipCacheLayer;
use crate::layers::python::PythonLayer;
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::build_plan::{BuildPlan, BuildPlanBuilder};
use libcnb::data::layer_name;
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
use libcnb::{buildpack_main, Buildpack};
use libherokubuildpack::log_header;
use std::path::Path;

pub(crate) struct PythonBuildpack;

impl Buildpack for PythonBuildpack {
    type Platform = GenericPlatform;
    type Metadata = GenericMetadata;
    type Error = PythonBuildpackError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        // Detect always passes, and instead the build plan is used to determine whether this
        // buildpack should run during the build. This allows other buildpacks to require the
        // use of this buildpack even if the app source doesn't contain a recognised Python file.
        let build_plan = generate_build_plan(&context.app_dir);
        DetectResultBuilder::pass().build_plan(build_plan).build()
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        log_header("Determining Python version");
        let python_version = python_version::determine_python_version(&context.app_dir)
            .map_err(PythonBuildpackError::PythonVersion)?;

        let python_layer = context.handle_layer(
            layer_name!("python"),
            PythonLayer {
                python_version: &python_version,
            },
        )?;

        // TODO: Move these inside a conditional for whether using pip or another package manager to install packages
        log_header("Installing dependencies using Pip");
        let pip_cache_layer = context.handle_layer(
            layer_name!("pip-cache"),
            PipCacheLayer {
                python_version: &python_version,
            },
        )?;
        context.handle_layer(
            // TODO: Should this layer be called `site-packages` instead?
            layer_name!("pip"),
            PipLayer {
                pip_cache_dir: pip_cache_layer.path,
                python_env: python_layer.env.apply_to_empty(Scope::Build),
                python_version: &python_version,
            },
        )?;

        // Temporary hack: Fail the build early to speed up iteration time.
        // println!("{}", "\n".repeat(10));
        // unimplemented!();
        // #[allow(unreachable_code)]
        BuildResultBuilder::new().build()
    }

    // fn on_error(&self, error: libcnb::Error<Self::Error>) -> i32 {
    //     libherokubuildpack::on_error_heroku(error::on_python_buildpack_error, error)
    // }
}

// TODO: Add a warning if Python not detected?
// TODO: Unit test or save for integration tests?
fn generate_build_plan(app_dir: &Path) -> BuildPlan {
    let mut build_plan = BuildPlanBuilder::new().provides("python");

    if dir_is_python_app(app_dir) {
        // Have to reassign due to: https://github.com/Malax/libcnb.rs/issues/295
        build_plan = build_plan.requires("python");
    }

    build_plan.build()
}

fn dir_is_python_app(app_dir: &Path) -> bool {
    ["pyproject.toml", "requirements.txt", "setup.py"]
        .iter()
        .any(|filename| app_dir.join(filename).exists())
}

buildpack_main!(PythonBuildpack);

// Suppress warnings due to the `unused_crate_dependencies` lint not handling integration tests well.
#[cfg(test)]
use libcnb_test as _;
