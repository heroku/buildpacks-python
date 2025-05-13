use crate::{BuildpackError, PythonBuildpack};
use libcnb::Env;
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::UncachedLayerDefinition;
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};

/// Creates an uncached build-only layer for uv's cache.
//
// We don't need to persist uv's cache between builds, since we cache site-packages instead.
// However, we must make uv write its cache into the same filesystem mount as the venv layer,
// otherwise uv can't use hardlinks when installing and will fall back to slower file copies.
// The easiest way to do this is via a temporary `cache=false`, `launch=false` layer.
// See comments in `uv_dependencies.rs` for more details.
pub(crate) fn prepare_uv_cache(
    context: &BuildContext<PythonBuildpack>,
    env: &mut Env,
) -> Result<(), libcnb::Error<BuildpackError>> {
    let layer = context.uncached_layer(
        layer_name!("uv-cache"),
        UncachedLayerDefinition {
            build: true,
            launch: false,
        },
    )?;

    // https://docs.astral.sh/uv/configuration/environment/#uv_cache_dir
    let layer_env = LayerEnv::new().chainable_insert(
        Scope::Build,
        ModificationBehavior::Override,
        "UV_CACHE_DIR",
        layer.path(),
    );
    layer.write_env(&layer_env)?;
    env.clone_from(&layer_env.apply(Scope::Build, env));

    Ok(())
}
